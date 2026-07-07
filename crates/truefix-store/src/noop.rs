//! A store that persists nothing.

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;

use crate::{MessageStore, StoreError};

/// A no-op store: sequence numbers are tracked in memory for the session's lifetime (matching
/// QFJ's own `NoopStore`, NEW-118/audit 006 -- previously always reported `1` regardless of
/// `set_next_*_seq` calls) but nothing else is persisted; `save`/`get` are no-ops.
pub struct NoopStore {
    sender_seq: AtomicU64,
    target_seq: AtomicU64,
}

impl Default for NoopStore {
    fn default() -> Self {
        Self {
            sender_seq: AtomicU64::new(1),
            target_seq: AtomicU64::new(1),
        }
    }
}

impl NoopStore {
    /// Create a new in-memory-only sequence tracker, starting both sequence numbers at 1.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MessageStore for NoopStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        Ok(self.sender_seq.load(Ordering::SeqCst))
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        Ok(self.target_seq.load(Ordering::SeqCst))
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.sender_seq.store(seq, Ordering::SeqCst);
        Ok(())
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.target_seq.store(seq, Ordering::SeqCst);
        Ok(())
    }
    async fn save(&self, _seq: u64, _message: &[u8]) -> Result<(), StoreError> {
        Ok(())
    }
    async fn get(&self, _begin: u64, _end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        Ok(Vec::new())
    }
    async fn reset(&self) -> Result<(), StoreError> {
        self.sender_seq.store(1, Ordering::SeqCst);
        self.target_seq.store(1, Ordering::SeqCst);
        Ok(())
    }
}
