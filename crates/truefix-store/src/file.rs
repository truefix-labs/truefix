//! File-backed message stores.
//!
//! Layout in the store directory:
//! - `seqnums` — text: sender sequence on line 1, target sequence on line 2.
//! - `body` — append log of records: `seq(8 LE) | len(4 LE) | bytes`.
//!
//! On open, the body log's offset index is replayed into memory (not the message bytes
//! themselves); a truncated/corrupt trailing record is tolerated (recovered up to the last
//! complete record) and flagged, supporting ForceResendWhenCorruptedStore (T060).
//!
//! [`FileStore`] reads message bodies from disk on every `get()` (no in-memory message cache,
//! matching QuickFIX's plain file store — only the offset index is kept in memory).
//! [`CachedFileStore`] additionally keeps a bounded in-memory cache of message bodies
//! (`FileStoreMaxCachedMsgs`; `0` means unbounded) to avoid the disk read for warm resends,
//! falling back to disk for cache misses. Both honor a `FileStoreSync` fsync toggle via
//! [`FileStoreOptions`].

use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;

use crate::{MessageStore, StoreError};

/// GAP-38/FR-017 (feature 005): read the creation-time sibling file (a single line, Unix
/// seconds), writing it with `now` if it doesn't exist yet — the store's creation time is
/// recorded once, on first `open()`, and never overwritten by later opens.
fn creation_time_file(path: &Path) -> Result<time::OffsetDateTime, StoreError> {
    match fs::read_to_string(path) {
        Ok(s) => {
            let secs: i64 = s.trim().parse().unwrap_or(0);
            Ok(time::OffsetDateTime::from_unix_timestamp(secs)
                .unwrap_or(time::OffsetDateTime::UNIX_EPOCH))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => reset_creation_time_file(path),
        Err(e) => Err(io_err(e)),
    }
}

/// Overwrite the creation-time sibling file with the current time (used at first `open()` and on
/// every `reset()`, matching QFJ/QFGo's "creation time updates on reset" semantics).
fn reset_creation_time_file(path: &Path) -> Result<time::OffsetDateTime, StoreError> {
    let now = time::OffsetDateTime::now_utc();
    fs::write(path, now.unix_timestamp().to_string()).map_err(io_err)?;
    Ok(now)
}

fn io_err(e: std::io::Error) -> StoreError {
    StoreError::Io(e.to_string())
}

fn poisoned() -> StoreError {
    StoreError::Backend("poisoned lock".into())
}

/// Options shared by [`FileStore`] and [`CachedFileStore`] (FR-025).
#[derive(Debug, Clone, Copy)]
pub struct FileStoreOptions {
    /// `FileStoreSync`: fsync the body/seqnum files after every write.
    pub sync: bool,
    /// `FileStoreMaxCachedMsgs` (honored by [`CachedFileStore`] only): maximum number of message
    /// bodies held in memory; `0` means unbounded (cache every message saved this session, the
    /// original pre-FR-025 behavior).
    pub max_cached_msgs: usize,
}

impl Default for FileStoreOptions {
    fn default() -> Self {
        Self {
            sync: true,
            max_cached_msgs: 0,
        }
    }
}

/// The on-disk body log shared by both file-backed stores: an append-only record log, plus an
/// in-memory offset index (seq → (offset, len)) so `get()` can seek directly rather than
/// rescanning the file.
/// seq → (offset of record start, message length).
type BodyIndex = BTreeMap<u64, (u64, u32)>;

struct BodyLog {
    path: PathBuf,
    index: Mutex<BodyIndex>,
    sync: bool,
}

impl BodyLog {
    fn open(path: PathBuf, sync: bool) -> Result<(Self, bool), StoreError> {
        let (index, corrupted) = load_index(&path)?;
        Ok((
            Self {
                path,
                index: Mutex::new(index),
                sync,
            },
            corrupted,
        ))
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, BodyIndex>, StoreError> {
        self.index.lock().map_err(|_| poisoned())
    }

    fn append(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        let offset = fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(io_err)?;
        f.write_all(&seq.to_le_bytes()).map_err(io_err)?;
        let len = u32::try_from(message.len())
            .map_err(|_| StoreError::Backend("message too large".into()))?;
        f.write_all(&len.to_le_bytes()).map_err(io_err)?;
        f.write_all(message).map_err(io_err)?;
        if self.sync {
            f.sync_data().map_err(io_err)?;
        }
        self.lock()?.insert(seq, (offset, len));
        Ok(())
    }

    fn read(&self, seq: u64) -> Result<Option<Vec<u8>>, StoreError> {
        let entry = self.lock()?.get(&seq).copied();
        let Some((offset, len)) = entry else {
            return Ok(None);
        };
        let mut f = File::open(&self.path).map_err(io_err)?;
        f.seek(SeekFrom::Start(offset + 12)).map_err(io_err)?;
        let mut buf = vec![0u8; len as usize];
        f.read_exact(&mut buf).map_err(io_err)?;
        Ok(Some(buf))
    }

    /// The sequence numbers indexed within `[begin, end]`, in order (no disk read).
    fn seqs_in_range(&self, begin: u64, end: u64) -> Result<Vec<u64>, StoreError> {
        Ok(self.lock()?.range(begin..=end).map(|(s, _)| *s).collect())
    }

    fn range(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        let seqs = self.seqs_in_range(begin, end)?;
        let mut out = Vec::with_capacity(seqs.len());
        for seq in seqs {
            if let Some(bytes) = self.read(seq)? {
                out.push((seq, bytes));
            }
        }
        Ok(out)
    }

    /// All indexed `(seq, message)` pairs in ascending order (used to warm a cache on open).
    fn all(&self) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        self.range(0, u64::MAX)
    }

    fn reset(&self) -> Result<(), StoreError> {
        fs::write(&self.path, []).map_err(io_err)?;
        self.lock()?.clear();
        Ok(())
    }
}

/// The sequence-number half of a file-backed store: `sender`/`target` next-seq counters,
/// persisted to a small text file.
struct SeqFile {
    path: PathBuf,
    state: Mutex<(u64, u64)>,
    sync: bool,
}

impl SeqFile {
    fn open(path: PathBuf, sync: bool) -> Result<Self, StoreError> {
        let (sender, target) = load_seqnums(&path)?;
        Ok(Self {
            path,
            state: Mutex::new((sender, target)),
            sync,
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, (u64, u64)>, StoreError> {
        self.state.lock().map_err(|_| poisoned())
    }

    fn get(&self) -> Result<(u64, u64), StoreError> {
        Ok(*self.lock()?)
    }

    fn write(&self, sender: u64, target: u64) -> Result<(), StoreError> {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
            .map_err(io_err)?;
        write!(f, "{sender}\n{target}\n").map_err(io_err)?;
        if self.sync {
            f.sync_data().map_err(io_err)?;
        }
        Ok(())
    }

    fn set_sender(&self, seq: u64) -> Result<(), StoreError> {
        let target = {
            let mut s = self.lock()?;
            s.0 = seq;
            s.1
        };
        self.write(seq, target)
    }

    fn set_target(&self, seq: u64) -> Result<(), StoreError> {
        let sender = {
            let mut s = self.lock()?;
            s.1 = seq;
            s.0
        };
        self.write(sender, seq)
    }

    fn reset(&self) -> Result<(), StoreError> {
        *self.lock()? = (1, 1);
        self.write(1, 1)
    }
}

/// A durable, file-backed message store with no in-memory message-body cache (bodies are read
/// from disk on every `get()`).
pub struct FileStore {
    seq: SeqFile,
    body: BodyLog,
    corrupted: AtomicBool,
    creation_time_path: PathBuf,
    creation_time: Mutex<time::OffsetDateTime>,
}

impl FileStore {
    /// Open (creating if needed) a file store in `dir` with default options (`FileStoreSync=Y`).
    pub fn open(dir: &Path) -> Result<Self, StoreError> {
        Self::open_with_options(dir, FileStoreOptions::default())
    }

    /// Open (creating if needed) a file store in `dir` with explicit options (FR-025).
    pub fn open_with_options(dir: &Path, options: FileStoreOptions) -> Result<Self, StoreError> {
        fs::create_dir_all(dir).map_err(io_err)?;
        let seq = SeqFile::open(dir.join("seqnums"), options.sync)?;
        let (body, corrupted) = BodyLog::open(dir.join("body"), options.sync)?;
        let creation_time_path = dir.join("session");
        let creation_time = creation_time_file(&creation_time_path)?;
        Ok(Self {
            seq,
            body,
            corrupted: AtomicBool::new(corrupted),
            creation_time_path,
            creation_time: Mutex::new(creation_time),
        })
    }

    /// Whether a corrupt trailing record was detected and recovered on open.
    pub fn was_corrupted(&self) -> bool {
        self.corrupted.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl MessageStore for FileStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        Ok(self.seq.get()?.0)
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        Ok(self.seq.get()?.1)
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.seq.set_sender(seq)
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.seq.set_target(seq)
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        self.body.append(seq, message)
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        self.body.range(begin, end)
    }
    async fn reset(&self) -> Result<(), StoreError> {
        self.body.reset()?;
        self.corrupted.store(false, Ordering::SeqCst);
        let now = reset_creation_time_file(&self.creation_time_path)?;
        *self.creation_time.lock().map_err(|_| poisoned())? = now;
        self.seq.reset()
    }
    fn was_corrupted(&self) -> bool {
        self.was_corrupted()
    }
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        Ok(Some(*self.creation_time.lock().map_err(|_| poisoned())?))
    }
}

struct CacheState {
    map: BTreeMap<u64, Vec<u8>>,
    order: VecDeque<u64>,
    max: usize,
}

impl CacheState {
    fn new(max: usize) -> Self {
        Self {
            map: BTreeMap::new(),
            order: VecDeque::new(),
            max,
        }
    }

    fn insert(&mut self, seq: u64, bytes: Vec<u8>) {
        if self.map.insert(seq, bytes).is_none() {
            self.order.push_back(seq);
        }
        if self.max > 0 {
            while self.map.len() > self.max {
                match self.order.pop_front() {
                    Some(oldest) => {
                        self.map.remove(&oldest);
                    }
                    None => break,
                }
            }
        }
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

/// A file-backed store that also maintains a bounded in-memory cache of message bodies
/// (`FileStoreMaxCachedMsgs`; `0` = unbounded), avoiding a disk read for cached resends.
pub struct CachedFileStore {
    seq: SeqFile,
    body: BodyLog,
    corrupted: AtomicBool,
    cache: Mutex<CacheState>,
    creation_time_path: PathBuf,
    creation_time: Mutex<time::OffsetDateTime>,
}

impl CachedFileStore {
    /// Open (creating if needed) a cached file store in `dir` with default options
    /// (`FileStoreSync=Y`, unbounded cache — matching the original pre-FR-025 behavior).
    pub fn open(dir: &Path) -> Result<Self, StoreError> {
        Self::open_with_options(dir, FileStoreOptions::default())
    }

    /// Open (creating if needed) a cached file store in `dir` with explicit options (FR-025). On
    /// open, the cache is warmed from the existing body log (most-recent messages first, up to
    /// `max_cached_msgs`).
    pub fn open_with_options(dir: &Path, options: FileStoreOptions) -> Result<Self, StoreError> {
        fs::create_dir_all(dir).map_err(io_err)?;
        let seq = SeqFile::open(dir.join("seqnums"), options.sync)?;
        let (body, corrupted) = BodyLog::open(dir.join("body"), options.sync)?;
        let mut cache = CacheState::new(options.max_cached_msgs);
        for (seq_no, bytes) in body.all()? {
            cache.insert(seq_no, bytes);
        }
        let creation_time_path = dir.join("session");
        let creation_time = creation_time_file(&creation_time_path)?;
        Ok(Self {
            seq,
            body,
            corrupted: AtomicBool::new(corrupted),
            cache: Mutex::new(cache),
            creation_time_path,
            creation_time: Mutex::new(creation_time),
        })
    }

    /// Whether a corrupt trailing record was detected and recovered on open.
    pub fn was_corrupted(&self) -> bool {
        self.corrupted.load(Ordering::SeqCst)
    }

    /// The number of message bodies currently held in the in-memory cache.
    pub fn cached_len(&self) -> Result<usize, StoreError> {
        Ok(self.cache.lock().map_err(|_| poisoned())?.map.len())
    }
}

#[async_trait]
impl MessageStore for CachedFileStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        Ok(self.seq.get()?.0)
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        Ok(self.seq.get()?.1)
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.seq.set_sender(seq)
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.seq.set_target(seq)
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        self.body.append(seq, message)?;
        self.cache
            .lock()
            .map_err(|_| poisoned())?
            .insert(seq, message.to_vec());
        Ok(())
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        let seqs = self.body.seqs_in_range(begin, end)?;
        let mut out = Vec::with_capacity(seqs.len());
        for seq_no in seqs {
            let cached = self
                .cache
                .lock()
                .map_err(|_| poisoned())?
                .map
                .get(&seq_no)
                .cloned();
            match cached {
                Some(bytes) => out.push((seq_no, bytes)),
                None => {
                    if let Some(bytes) = self.body.read(seq_no)? {
                        out.push((seq_no, bytes));
                    }
                }
            }
        }
        Ok(out)
    }
    async fn reset(&self) -> Result<(), StoreError> {
        self.body.reset()?;
        self.corrupted.store(false, Ordering::SeqCst);
        self.cache.lock().map_err(|_| poisoned())?.clear();
        let now = reset_creation_time_file(&self.creation_time_path)?;
        *self.creation_time.lock().map_err(|_| poisoned())? = now;
        self.seq.reset()
    }
    fn was_corrupted(&self) -> bool {
        self.was_corrupted()
    }
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        Ok(Some(*self.creation_time.lock().map_err(|_| poisoned())?))
    }
}

fn load_seqnums(path: &Path) -> Result<(u64, u64), StoreError> {
    match fs::read_to_string(path) {
        Ok(s) => {
            let mut lines = s.lines();
            let sender = lines
                .next()
                .and_then(|l| l.trim().parse().ok())
                .unwrap_or(1);
            let target = lines
                .next()
                .and_then(|l| l.trim().parse().ok())
                .unwrap_or(1);
            Ok((sender, target))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok((1, 1)),
        Err(e) => Err(io_err(e)),
    }
}

fn load_index(path: &Path) -> Result<(BodyIndex, bool), StoreError> {
    match fs::read(path) {
        Ok(data) => Ok(parse_records(&data)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok((BTreeMap::new(), false)),
        Err(e) => Err(io_err(e)),
    }
}

/// Replay record headers (not bodies) into an offset index; returns the index and whether a
/// corrupt trailing record was found.
fn parse_records(data: &[u8]) -> (BodyIndex, bool) {
    let mut index = BTreeMap::new();
    let mut pos = 0usize;
    loop {
        let record_start = pos;
        let Some(seq) = read_u64(data, pos) else {
            return (index, pos != data.len());
        };
        let Some(len) = read_u32(data, pos + 8) else {
            return (index, true);
        };
        let start = pos + 12;
        let Some(end) = start.checked_add(len as usize) else {
            return (index, true);
        };
        if data.get(start..end).is_none() {
            return (index, true);
        }
        index.insert(seq, (record_start as u64, len));
        pos = end;
    }
}

fn read_u64(data: &[u8], pos: usize) -> Option<u64> {
    let end = pos.checked_add(8)?;
    let bytes: [u8; 8] = data.get(pos..end)?.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn read_u32(data: &[u8], pos: usize) -> Option<u32> {
    let end = pos.checked_add(4)?;
    let bytes: [u8; 4] = data.get(pos..end)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}
