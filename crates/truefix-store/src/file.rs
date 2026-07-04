//! File-backed message stores.
//!
//! Layout in the store directory:
//! - `senderseqnums`/`targetseqnums` — one text value each (BUG-30/BUG-99, feature 007), each
//!   written atomically (write-temp-then-rename). A legacy combined `seqnums` file (one sender
//!   line, one target line — the pre-007 layout) is auto-migrated into this pair on first open if
//!   found and left in place afterward (FR-001a).
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
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

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
        // T088/BUG-20 (feature 006): the offset-determining `fs::metadata` read must happen
        // inside the same critical section as the write and index-insert below, not before it —
        // otherwise two concurrent `append` callers could both read the file's length before
        // either has written, racing to record the *same* (now-wrong-for-one-of-them) offset in
        // the index. Holding `self.lock()` across the whole body (all synchronous std calls, no
        // `.await` inside) serializes appends entirely, closing that window.
        let mut guard = self.lock()?;
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
        guard.insert(seq, (offset, len));
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

    fn reset(&self) -> Result<(), StoreError> {
        // BUG-15/FR-016 (feature 006): sync when `FileStoreSync=Y`, matching `append()`'s
        // existing discipline — previously this never synced regardless of the option, so a
        // crash immediately after `fs::write` truncated the body file, but before the OS
        // actually flushed that truncation to disk, could leave a stale pre-reset body on disk
        // after restart even though the in-memory index (and, if this ran second per the
        // ordering below, the seqnums file) already reflect the reset.
        let f = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.path)
            .map_err(io_err)?;
        if self.sync {
            f.sync_data().map_err(io_err)?;
        }
        self.lock()?.clear();
        Ok(())
    }
}

/// The sequence-number half of a file-backed store: `sender`/`target` next-seq counters,
/// persisted to a small text file.
/// Sender/target sequence-number persistence (BUG-30/BUG-31/BUG-99, FR-001/FR-001a, feature 007):
/// two independent files (`senderseqnums`/`targetseqnums`, matching QuickFIX/J's layout), each
/// written atomically (write-to-temp-then-rename, never truncate-in-place) so a crash mid-write
/// always leaves either the prior or the new value recoverable, never neither. A present-but-
/// unparseable file is a typed `StoreError`, distinct from a legitimately-absent one (which
/// defaults to sequence 1). `reset()` deletes and recreates both files wholesale, matching
/// QuickFIX/J's `closeAndDeleteFiles()`-then-reinitialize semantics.
struct SeqFile {
    sender_path: PathBuf,
    target_path: PathBuf,
    state: Mutex<(u64, u64)>,
    sync: bool,
}

impl SeqFile {
    /// `dir` is the store directory. Auto-migrates from a legacy combined `seqnums` file (one
    /// sender/target pair) if present and either new-format file is still missing — migration is
    /// itself per-file idempotent/crash-safe: each of `senderseqnums`/`targetseqnums` is only
    /// written from the legacy source if it doesn't already exist, so an interrupted migration
    /// simply resumes correctly on the next open. The legacy file is never deleted.
    fn open(dir: &Path, sync: bool) -> Result<Self, StoreError> {
        let sender_path = dir.join("senderseqnums");
        let target_path = dir.join("targetseqnums");
        let legacy_path = dir.join("seqnums");
        if legacy_path.exists() && (!sender_path.exists() || !target_path.exists()) {
            let (legacy_sender, legacy_target) = load_legacy_seqnums(&legacy_path)?;
            if !sender_path.exists() {
                write_seq_value(&sender_path, legacy_sender, sync)?;
            }
            if !target_path.exists() {
                write_seq_value(&target_path, legacy_target, sync)?;
            }
        }
        let sender = load_seq_value(&sender_path)?;
        let target = load_seq_value(&target_path)?;
        Ok(Self {
            sender_path,
            target_path,
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

    fn set_sender(&self, seq: u64) -> Result<(), StoreError> {
        self.lock()?.0 = seq;
        write_seq_value(&self.sender_path, seq, self.sync)
    }

    fn set_target(&self, seq: u64) -> Result<(), StoreError> {
        self.lock()?.1 = seq;
        write_seq_value(&self.target_path, seq, self.sync)
    }

    fn reset(&self) -> Result<(), StoreError> {
        *self.lock()? = (1, 1);
        let _ = fs::remove_file(&self.sender_path);
        let _ = fs::remove_file(&self.target_path);
        write_seq_value(&self.sender_path, 1, self.sync)?;
        write_seq_value(&self.target_path, 1, self.sync)?;
        Ok(())
    }
}

/// Atomically write a single sequence value to `path`: write to a sibling `.tmp` file, fsync it
/// (if `sync`), then rename over `path` — the rename is the only step that can be observed
/// mid-flight, and a rename is atomic on the same filesystem, so a crash never leaves `path`
/// truncated/empty (BUG-30).
fn write_seq_value(path: &Path, value: u64, sync: bool) -> Result<(), StoreError> {
    let tmp_path = path.with_file_name(format!(
        "{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("seq")
    ));
    {
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(io_err)?;
        writeln!(f, "{value}").map_err(io_err)?;
        if sync {
            f.sync_data().map_err(io_err)?;
        }
    }
    fs::rename(&tmp_path, path).map_err(io_err)?;
    Ok(())
}

/// Read a single sequence-number file: absent means a fresh store (sequence 1, not an error);
/// present-but-unparseable is a typed error (BUG-31) — the two cases were previously
/// indistinguishable (`unwrap_or(1)` collapsed both to 1).
fn load_seq_value(path: &Path) -> Result<u64, StoreError> {
    match fs::read_to_string(path) {
        Ok(s) => s
            .trim()
            .parse()
            .map_err(|_| StoreError::Backend(format!("corrupted sequence file {path:?}: {s:?}"))),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(1),
        Err(e) => Err(io_err(e)),
    }
}

/// Parse the legacy combined `seqnums` file (sender on line 1, target on line 2) for migration
/// (FR-001a). Unlike the pre-007 `load_seqnums` this replaced, a present-but-unparseable legacy
/// file is a typed error rather than silently defaulting — the same BUG-31 fix applies to the
/// migration path.
fn load_legacy_seqnums(path: &Path) -> Result<(u64, u64), StoreError> {
    let corrupt =
        |s: &str| StoreError::Backend(format!("corrupted legacy seqnums file {path:?}: {s:?}"));
    let s = fs::read_to_string(path).map_err(io_err)?;
    let mut lines = s.lines();
    let sender: u64 = lines
        .next()
        .ok_or_else(|| corrupt(&s))?
        .trim()
        .parse()
        .map_err(|_| corrupt(&s))?;
    let target: u64 = lines
        .next()
        .ok_or_else(|| corrupt(&s))?
        .trim()
        .parse()
        .map_err(|_| corrupt(&s))?;
    Ok((sender, target))
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
        let seq = SeqFile::open(dir, options.sync)?;
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
        // B17/FR-016 (feature 006): reset `seq` (the durability anchor, per
        // `save_and_advance_sender`'s ordering rationale above) *before* `body` — a crash
        // mid-reset then leaves `seqnums` already at the post-reset `(1,1)` with `body` still
        // holding harmless, unindexed pre-reset data, never the reverse (which was previously
        // possible and unrecoverable: `body` empty but `seqnums` still reporting the old,
        // advanced values, with no way to serve a resend for messages the old sequence numbers
        // claimed existed).
        self.seq.reset()?;
        self.body.reset()?;
        self.corrupted.store(false, Ordering::SeqCst);
        let now = reset_creation_time_file(&self.creation_time_path)?;
        *self.creation_time.lock().map_err(|_| poisoned())? = now;
        Ok(())
    }
    fn was_corrupted(&self) -> bool {
        self.was_corrupted()
    }
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        Ok(Some(*self.creation_time.lock().map_err(|_| poisoned())?))
    }
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        // GAP-49/FR-015 (feature 006): unlike the SQL/MSSQL/Redb overrides (feature 005), a
        // two-separate-file backend has no cross-file transaction to make this fully atomic.
        // The safest achievable ordering advances the durable sequence-number record FIRST: if a
        // crash occurs before the body write completes, `next_out_seq()` on restart already
        // reflects this message as sent (matching what the counterparty may already have
        // received over the wire), and the missing body is safely recoverable via the existing
        // gap-fill path (`build_resend` already treats a missing stored message as a gap to
        // fill) — the reverse order would instead risk generating a brand-new, differently-
        // content message under an already-transmitted sequence number on restart.
        self.seq.set_sender(seq + 1)?;
        self.body.append(seq, message)
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
        let seq = SeqFile::open(dir, options.sync)?;
        let (body, corrupted) = BodyLog::open(dir.join("body"), options.sync)?;
        let mut cache = CacheState::new(options.max_cached_msgs);
        // BUG-81/FR-031 (feature 007): warm the cache by reading only the (at most)
        // `max_cached_msgs` most-recent bodies, not every body ever stored -- previously
        // `body.all()` read every message body into memory up front, then relied on `cache.insert`
        // evicting the extras one by one, so peak memory during open was the *entire* message
        // history rather than the bound this cache is supposed to enforce. `seqs_in_range` is
        // index-only (no body reads), so slicing to the tail before reading anything is free.
        let all_seqs = body.seqs_in_range(0, u64::MAX)?;
        let warm_from = if options.max_cached_msgs > 0 {
            all_seqs.len().saturating_sub(options.max_cached_msgs)
        } else {
            0 // unbounded cache: warm the whole history, matching the original semantics
        };
        for seq_no in all_seqs.get(warm_from..).unwrap_or_default() {
            if let Some(bytes) = body.read(*seq_no)? {
                cache.insert(*seq_no, bytes);
            }
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
        // B17/FR-016 (feature 006): see FileStore's identical override for the ordering
        // rationale (sequence-number-first, matching `save_and_advance_sender`'s durability
        // anchor).
        self.seq.reset()?;
        self.body.reset()?;
        self.corrupted.store(false, Ordering::SeqCst);
        self.cache.lock().map_err(|_| poisoned())?.clear();
        let now = reset_creation_time_file(&self.creation_time_path)?;
        *self.creation_time.lock().map_err(|_| poisoned())? = now;
        Ok(())
    }
    fn was_corrupted(&self) -> bool {
        self.was_corrupted()
    }
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        Ok(Some(*self.creation_time.lock().map_err(|_| poisoned())?))
    }
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        // GAP-49/FR-015 (feature 006): see FileStore's identical override for the ordering
        // rationale (sequence-number-first, safest achievable ordering for a non-transactional
        // two-file backend).
        self.seq.set_sender(seq + 1)?;
        self.body.append(seq, message)?;
        self.cache
            .lock()
            .map_err(|_| poisoned())?
            .insert(seq, message.to_vec());
        Ok(())
    }
}

/// Replays record headers (not bodies) into an offset index, seeking past each record's body
/// bytes without ever reading them into memory (BUG-45/FR-031, feature 007) -- previously
/// `fs::read(path)` read the *entire* body-log file (every message ever stored) into one `Vec`
/// just to walk its 12-byte record headers, allocating memory proportional to the whole message
/// history rather than to the (tiny, fixed-size-per-record) index being rebuilt. Returns the
/// index and whether a corrupt trailing record was found.
fn load_index(path: &Path) -> Result<(BodyIndex, bool), StoreError> {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok((BTreeMap::new(), false)),
        Err(e) => return Err(io_err(e)),
    };
    let file_len = f.metadata().map_err(io_err)?.len();
    let mut index = BTreeMap::new();
    let mut pos: u64 = 0;
    loop {
        if pos == file_len {
            return Ok((index, false));
        }
        if file_len - pos < 12 {
            return Ok((index, true)); // trailing bytes too short for a full header
        }
        // Read the two header fields into their own exactly-sized buffers, rather than one
        // 12-byte buffer sliced afterward — this crate forbids `unwrap`/`expect`/indexing
        // (Constitution Principle I), and a fixed-size array read via `read_exact` needs no
        // fallible slice-to-array conversion at all.
        let mut seq_bytes = [0u8; 8];
        let mut len_bytes = [0u8; 4];
        f.read_exact(&mut seq_bytes).map_err(io_err)?;
        f.read_exact(&mut len_bytes).map_err(io_err)?;
        let seq = u64::from_le_bytes(seq_bytes);
        let len = u32::from_le_bytes(len_bytes);
        let body_start = pos + 12;
        let Some(body_end) = body_start.checked_add(u64::from(len)) else {
            return Ok((index, true));
        };
        if body_end > file_len {
            return Ok((index, true)); // declared body length runs past actual EOF
        }
        index.insert(seq, (pos, len));
        f.seek(SeekFrom::Start(body_end)).map_err(io_err)?;
        pos = body_end;
    }
}
