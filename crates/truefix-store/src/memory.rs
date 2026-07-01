//! Volatile in-memory message store.

use std::collections::BTreeMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::{MessageStore, StoreError};

pub(crate) struct State {
    pub(crate) sender: u64,
    pub(crate) target: u64,
    pub(crate) messages: BTreeMap<u64, Vec<u8>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sender: 1,
            target: 1,
            messages: BTreeMap::new(),
        }
    }
}

impl State {
    pub(crate) fn range(&self, begin: u64, end: u64) -> Vec<(u64, Vec<u8>)> {
        self.messages
            .range(begin..=end)
            .map(|(seq, bytes)| (*seq, bytes.clone()))
            .collect()
    }
}

/// An in-memory store (does not survive process restart).
#[derive(Default)]
pub struct MemoryStore {
    state: Mutex<State>,
}

impl MemoryStore {
    /// Create an empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

fn lock<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, StoreError> {
    m.lock()
        .map_err(|_| StoreError::Backend("poisoned lock".into()))
}

#[async_trait]
impl MessageStore for MemoryStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        Ok(lock(&self.state)?.sender)
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        Ok(lock(&self.state)?.target)
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        lock(&self.state)?.sender = seq;
        Ok(())
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        lock(&self.state)?.target = seq;
        Ok(())
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        lock(&self.state)?.messages.insert(seq, message.to_vec());
        Ok(())
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        Ok(lock(&self.state)?.range(begin, end))
    }
    async fn reset(&self) -> Result<(), StoreError> {
        let mut s = lock(&self.state)?;
        *s = State::default();
        Ok(())
    }
}
