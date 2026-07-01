//! `truefix-store` — pluggable persistence of sequence numbers and sent messages.
//!
//! The [`MessageStore`] trait abstracts the storage needed for resend (FR-G2): the next inbound
//! and outbound sequence numbers, and the bytes of each sent message keyed by sequence number.
//! Implementations: [`MemoryStore`], [`FileStore`], [`CachedFileStore`], [`NoopStore`], and a
//! SQL-backed store behind the `sql` feature.
//!
//! Design: `specs/001-fix-engine-parity/`.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

mod file;
mod memory;
mod noop;
#[cfg(feature = "sql")]
mod sql;

use std::path::PathBuf;

use async_trait::async_trait;
use thiserror::Error;

pub use file::{CachedFileStore, FileStore};
pub use memory::MemoryStore;
pub use noop::NoopStore;
#[cfg(feature = "sql")]
pub use sql::SqlStore;

/// An error from a message store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// An underlying I/O error.
    #[error("store I/O error: {0}")]
    Io(String),
    /// A backend (e.g. SQL) error.
    #[error("store backend error: {0}")]
    Backend(String),
}

/// Persistence for sequence numbers and outbound messages, sufficient to satisfy resend
/// across restarts (FR-G2, SC-006).
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// The next outbound (sender) sequence number.
    async fn next_sender_seq(&self) -> Result<u64, StoreError>;
    /// The next inbound (target) sequence number expected.
    async fn next_target_seq(&self) -> Result<u64, StoreError>;
    /// Set the next outbound sequence number.
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError>;
    /// Set the next inbound sequence number.
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError>;
    /// Persist an outbound message's bytes under its sequence number.
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError>;
    /// Retrieve stored messages whose sequence numbers fall in `[begin, end]`, in order.
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError>;
    /// Reset: clear stored messages and set both sequence numbers back to 1.
    async fn reset(&self) -> Result<(), StoreError>;
}

/// Which store backend to construct.
#[derive(Debug, Clone)]
pub enum StoreConfig {
    /// Volatile in-memory store.
    Memory,
    /// File-backed store in a directory.
    File {
        /// Directory holding the store files.
        dir: PathBuf,
    },
    /// File-backed store that also caches messages in memory.
    CachedFile {
        /// Directory holding the store files.
        dir: PathBuf,
    },
    /// No-op store (never persists; sequence numbers stay at 1).
    Noop,
    /// SQL-backed store (requires the `sql` feature).
    #[cfg(feature = "sql")]
    Sql {
        /// Database URL.
        url: String,
    },
}

/// Build a boxed [`MessageStore`] from a [`StoreConfig`].
pub async fn build_store(config: &StoreConfig) -> Result<Box<dyn MessageStore>, StoreError> {
    Ok(match config {
        StoreConfig::Memory => Box::new(MemoryStore::new()),
        StoreConfig::File { dir } => Box::new(FileStore::open(dir)?),
        StoreConfig::CachedFile { dir } => Box::new(CachedFileStore::open(dir)?),
        StoreConfig::Noop => Box::new(NoopStore),
        #[cfg(feature = "sql")]
        StoreConfig::Sql { url } => Box::new(SqlStore::connect(url).await?),
    })
}
