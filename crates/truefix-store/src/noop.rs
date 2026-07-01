//! A store that persists nothing.

use async_trait::async_trait;

use crate::{MessageStore, StoreError};

/// A no-op store: sequence numbers always report `1`, saves are discarded, gets are empty.
pub struct NoopStore;

#[async_trait]
impl MessageStore for NoopStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        Ok(1)
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        Ok(1)
    }
    async fn set_next_sender_seq(&self, _seq: u64) -> Result<(), StoreError> {
        Ok(())
    }
    async fn set_next_target_seq(&self, _seq: u64) -> Result<(), StoreError> {
        Ok(())
    }
    async fn save(&self, _seq: u64, _message: &[u8]) -> Result<(), StoreError> {
        Ok(())
    }
    async fn get(&self, _begin: u64, _end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        Ok(Vec::new())
    }
    async fn reset(&self) -> Result<(), StoreError> {
        Ok(())
    }
}
