//! File-backed message stores.
//!
//! Layout in the store directory:
//! - `seqnums` — text: sender sequence on line 1, target sequence on line 2.
//! - `body` — append log of records: `seq(8 LE) | len(4 LE) | bytes`.
//!
//! On open, the body log is replayed into memory; a truncated/corrupt trailing record is
//! tolerated (recovered up to the last complete record) and flagged, supporting
//! ForceResendWhenCorruptedStore (T060).

use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;

use crate::memory::State;
use crate::{MessageStore, StoreError};

fn io_err(e: std::io::Error) -> StoreError {
    StoreError::Io(e.to_string())
}

/// A durable, file-backed message store.
pub struct FileStore {
    seq_path: PathBuf,
    body_path: PathBuf,
    state: Mutex<State>,
    corrupted: AtomicBool,
}

impl FileStore {
    /// Open (creating if needed) a file store in `dir`.
    pub fn open(dir: &Path) -> Result<Self, StoreError> {
        fs::create_dir_all(dir).map_err(io_err)?;
        let seq_path = dir.join("seqnums");
        let body_path = dir.join("body");
        let (sender, target) = load_seqnums(&seq_path)?;
        let (messages, corrupted) = load_body(&body_path)?;
        Ok(Self {
            seq_path,
            body_path,
            state: Mutex::new(State {
                sender,
                target,
                messages,
            }),
            corrupted: AtomicBool::new(corrupted),
        })
    }

    /// Whether a corrupt trailing record was detected and recovered on open.
    pub fn was_corrupted(&self) -> bool {
        self.corrupted.load(Ordering::SeqCst)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, StoreError> {
        self.state
            .lock()
            .map_err(|_| StoreError::Backend("poisoned lock".into()))
    }

    fn write_seqnums(&self, sender: u64, target: u64) -> Result<(), StoreError> {
        fs::write(&self.seq_path, format!("{sender}\n{target}\n")).map_err(io_err)
    }
}

#[async_trait]
impl MessageStore for FileStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        Ok(self.lock()?.sender)
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        Ok(self.lock()?.target)
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        let target = {
            let mut s = self.lock()?;
            s.sender = seq;
            s.target
        };
        self.write_seqnums(seq, target)
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        let sender = {
            let mut s = self.lock()?;
            s.target = seq;
            s.sender
        };
        self.write_seqnums(sender, seq)
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        append_record(&self.body_path, seq, message)?;
        self.lock()?.messages.insert(seq, message.to_vec());
        Ok(())
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        Ok(self.lock()?.range(begin, end))
    }
    async fn reset(&self) -> Result<(), StoreError> {
        {
            let mut s = self.lock()?;
            *s = State::default();
        }
        fs::write(&self.body_path, []).map_err(io_err)?;
        self.corrupted.store(false, Ordering::SeqCst);
        self.write_seqnums(1, 1)
    }
}

/// A file-backed store that also caches messages in memory for fast resend.
///
/// In Stage S5 it shares [`FileStore`]'s durable implementation; the cached variant is where
/// write-batching optimizations will live.
pub struct CachedFileStore(FileStore);

impl CachedFileStore {
    /// Open (creating if needed) a cached file store in `dir`.
    pub fn open(dir: &Path) -> Result<Self, StoreError> {
        Ok(Self(FileStore::open(dir)?))
    }

    /// Whether a corrupt trailing record was detected and recovered on open.
    pub fn was_corrupted(&self) -> bool {
        self.0.was_corrupted()
    }
}

#[async_trait]
impl MessageStore for CachedFileStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        self.0.next_sender_seq().await
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        self.0.next_target_seq().await
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.0.set_next_sender_seq(seq).await
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.0.set_next_target_seq(seq).await
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        self.0.save(seq, message).await
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        self.0.get(begin, end).await
    }
    async fn reset(&self) -> Result<(), StoreError> {
        self.0.reset().await
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

fn load_body(path: &Path) -> Result<(BTreeMap<u64, Vec<u8>>, bool), StoreError> {
    match fs::read(path) {
        Ok(data) => Ok(parse_records(&data)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok((BTreeMap::new(), false)),
        Err(e) => Err(io_err(e)),
    }
}

/// Replay records; returns the message map and whether a corrupt trailing record was found.
fn parse_records(data: &[u8]) -> (BTreeMap<u64, Vec<u8>>, bool) {
    let mut map = BTreeMap::new();
    let mut pos = 0usize;
    loop {
        let Some(seq) = read_u64(data, pos) else {
            return (map, pos != data.len());
        };
        let Some(len) = read_u32(data, pos + 8) else {
            return (map, true);
        };
        let start = pos + 12;
        let Some(end) = start.checked_add(len as usize) else {
            return (map, true);
        };
        let Some(bytes) = data.get(start..end) else {
            return (map, true);
        };
        map.insert(seq, bytes.to_vec());
        pos = end;
    }
}

fn append_record(path: &Path, seq: u64, message: &[u8]) -> Result<(), StoreError> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_err)?;
    f.write_all(&seq.to_le_bytes()).map_err(io_err)?;
    let len = u32::try_from(message.len())
        .map_err(|_| StoreError::Backend("message too large".into()))?;
    f.write_all(&len.to_le_bytes()).map_err(io_err)?;
    f.write_all(message).map_err(io_err)?;
    Ok(())
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
