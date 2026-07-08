//! `truefix-store` — pluggable persistence of sequence numbers and sent messages.
//!
//! The [`MessageStore`] trait abstracts the storage needed for resend (FR-G2): the next inbound
//! and outbound sequence numbers, and the bytes of each sent message keyed by sequence number.
//! Implementations: [`MemoryStore`], [`FileStore`], [`CachedFileStore`], [`NoopStore`], and a
//! SQL-backed store behind the `sql` feature.
//!
//! Audit 006 additions: [`MessageStore::contains`] exposes duplicate-sequence detection without
//! changing `save()`'s existing overwrite-on-conflict contract (NEW-137);
//! [`FileStoreOptions::max_body_records`] bounds a file-backed store's body-log index while
//! preserving resend/recovery semantics (NEW-108); Mongo/SQL/MSSQL `reset()` is atomic across
//! messages and sequence numbers (NEW-116); and `NoopStore` tracks sequence numbers in memory for
//! the session lifetime (NEW-118).
//!
//! Feature 012 (audit 007) additions / **migration note**: [`StoreConfig`] gained a `Custom(Arc<dyn
//! MessageStore>)` variant (NEW-158), letting a caller supply a backend not on the built-in list
//! through the same config surface `Engine::start` resolves (see `truefix::Engine::start_with_overrides`).
//! This is additive to the variant set, but `StoreConfig` can no longer derive `Debug` (`dyn
//! MessageStore` isn't `Debug`) -- it now has a hand-written `Debug` impl with the same output for
//! every pre-existing variant, so this only breaks code that exhaustively `match`es `StoreConfig`
//! without a `_ =>` arm.
//!
//! Design: `specs/001-fix-engine-parity/`, `specs/012-audit-007-remediation/`.
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
#[cfg(feature = "mongodb")]
mod mongo;
#[cfg(feature = "mssql")]
mod mssql;
mod noop;
#[cfg(feature = "redb")]
mod redb;
#[cfg(feature = "sql")]
mod sql;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

pub use file::{CachedFileStore, FileStore, FileStoreOptions};
pub use memory::MemoryStore;
#[cfg(feature = "mongodb")]
pub use mongo::{MongoStore, MongoStoreConfig};
#[cfg(feature = "mssql")]
pub use mssql::{MssqlStore, MssqlStoreConfig};
pub use noop::NoopStore;
#[cfg(feature = "redb")]
pub use redb::{RedbStore, RedbStoreConfig};
#[cfg(feature = "sql")]
pub use sql::{SqlPoolOptions, SqlStore, SqlStoreConfig};

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
    /// Whether this store detected and recovered from corruption when it was opened
    /// (`ForceResendWhenCorruptedStore`). Backends with no corruption-detection concept (e.g.
    /// `MemoryStore`, `NoopStore`, SQL backends relying on the database's own durability) default
    /// to `false`.
    fn was_corrupted(&self) -> bool {
        false
    }
    /// The wall-clock time this store was created (or last reset), if the backend tracks one
    /// (GAP-38/FR-017, feature 005). `None` for backends with no such concept (e.g.
    /// `MemoryStore`, `NoopStore`) or when not yet recorded.
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        Ok(None)
    }
    /// Atomically persist an outbound message and advance the sender sequence past it
    /// (GAP-39/FR-018, feature 005): closes the crash window where `save()` succeeds but a
    /// subsequent `set_next_sender_seq()` doesn't (or vice versa), which could otherwise
    /// double-send or skip a sequence number on restart. The default implementation preserves
    /// today's behavior (two independent calls) for any backend that doesn't override it.
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        self.save(seq, message).await?;
        self.set_next_sender_seq(seq + 1).await
    }
    /// Whether a message is already saved under `seq` (NEW-137, audit 006): a way for callers to
    /// observe duplicate sequence usage without changing `save()`'s existing overwrite-on-conflict
    /// contract (deliberately kept separate rather than having `save()` itself return an
    /// overwrote-something flag, to avoid a breaking signature change across every backend).
    /// Default `Ok(false)` for any backend that doesn't override it.
    async fn contains(&self, seq: u64) -> Result<bool, StoreError> {
        let _ = seq;
        Ok(false)
    }
}

/// NEW-158 (feature 012): lets a `StoreConfig::Custom(Arc<dyn MessageStore>)` be handed to
/// `build_store` and used exactly like any built-in backend, mirroring `truefix_log`'s
/// `impl Log for Box<dyn Log>`.
#[async_trait]
impl MessageStore for Arc<dyn MessageStore> {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        (**self).next_sender_seq().await
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        (**self).next_target_seq().await
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        (**self).set_next_sender_seq(seq).await
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        (**self).set_next_target_seq(seq).await
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        (**self).save(seq, message).await
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        (**self).get(begin, end).await
    }
    async fn reset(&self) -> Result<(), StoreError> {
        (**self).reset().await
    }
    fn was_corrupted(&self) -> bool {
        (**self).was_corrupted()
    }
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        (**self).creation_time().await
    }
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        (**self).save_and_advance_sender(seq, message).await
    }
    async fn contains(&self, seq: u64) -> Result<bool, StoreError> {
        (**self).contains(seq).await
    }
}

/// Which store backend to construct.
#[derive(Clone)]
pub enum StoreConfig {
    /// Volatile in-memory store.
    Memory,
    /// File-backed store in a directory.
    File {
        /// Directory holding the store files.
        dir: PathBuf,
        /// `FileStoreSync` (FR-025).
        options: FileStoreOptions,
    },
    /// File-backed store that also caches messages in memory.
    CachedFile {
        /// Directory holding the store files.
        dir: PathBuf,
        /// `FileStoreSync`/`FileStoreMaxCachedMsgs` (FR-025).
        options: FileStoreOptions,
    },
    /// No-op store (never persists; sequence numbers stay at 1).
    Noop,
    /// SQL-backed store (PostgreSQL/MySQL/SQLite; requires the `sql` feature).
    #[cfg(feature = "sql")]
    Sql {
        /// Database URL.
        url: String,
        /// `JdbcStoreSessionsTableName` (US8, feature 005, FR-021); `None` uses
        /// `SqlStoreConfig::new`'s default (`"seqnums"`).
        sessions_table: Option<String>,
        /// `JdbcStoreMessagesTableName` (US8, feature 005, FR-021); `None` uses
        /// `SqlStoreConfig::new`'s default (`"messages"`).
        messages_table: Option<String>,
        /// `JdbcSessionIdDefaultPropertyValue` (US8, feature 005, FR-021); `None` uses
        /// `SqlStoreConfig::new`'s default (`"default"`).
        session_id: Option<String>,
        /// `Jdbc*Connection*` pool-tuning keys (US8, feature 005, FR-021); `None` uses
        /// `SqlPoolOptions::default()`.
        pool: Option<SqlPoolOptions>,
    },
    /// MSSQL-backed store (requires the `mssql` feature; FR-020).
    #[cfg(feature = "mssql")]
    Mssql {
        /// Database URL (`mssql://user:password@host[:port]/database`).
        url: String,
        /// `JdbcStoreSessionsTableName` (US8, feature 005, FR-021); `None` uses
        /// `MssqlStoreConfig::new`'s default (`"seqnums"`).
        sessions_table: Option<String>,
        /// `JdbcStoreMessagesTableName` (US8, feature 005, FR-021); `None` uses
        /// `MssqlStoreConfig::new`'s default (`"messages"`).
        messages_table: Option<String>,
        /// `JdbcSessionIdDefaultPropertyValue` (US8, feature 005, FR-021); `None` uses
        /// `MssqlStoreConfig::new`'s default (`"default"`).
        session_id: Option<String>,
    },
    /// Embedded transactional store via `redb` (requires the `redb` feature; US5, feature 004,
    /// FR-006), a modern replacement for QuickFIX/J's obsolete `SleepycatStore`.
    #[cfg(feature = "redb")]
    Redb {
        /// Path to the `redb` database file.
        path: PathBuf,
    },
    /// MongoDB-backed store (requires the `mongodb` feature; US6, feature 004), matching
    /// QuickFIX/Go's MongoDB option.
    #[cfg(feature = "mongodb")]
    Mongo {
        /// MongoDB connection URI (`mongodb://...`).
        uri: String,
    },
    /// NEW-158 (feature 012): a caller-supplied `MessageStore` implementation, for backends not
    /// on the built-in list (or a proprietary audit-log sink) -- previously only reachable via
    /// the lower-level `truefix::transport::Services::store` escape hatch; this widens the
    /// `.cfg`-adjacent config surface itself to express the same thing (FR-006). `Arc` (not
    /// `Box`) so `StoreConfig` keeps deriving `Clone`.
    Custom(Arc<dyn MessageStore>),
}

impl std::fmt::Debug for StoreConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Memory => write!(f, "Memory"),
            Self::File { dir, options } => f
                .debug_struct("File")
                .field("dir", dir)
                .field("options", options)
                .finish(),
            Self::CachedFile { dir, options } => f
                .debug_struct("CachedFile")
                .field("dir", dir)
                .field("options", options)
                .finish(),
            Self::Noop => write!(f, "Noop"),
            #[cfg(feature = "sql")]
            Self::Sql { url, .. } => f.debug_struct("Sql").field("url", url).finish(),
            #[cfg(feature = "mssql")]
            Self::Mssql { url, .. } => f.debug_struct("Mssql").field("url", url).finish(),
            #[cfg(feature = "redb")]
            Self::Redb { path } => f.debug_struct("Redb").field("path", path).finish(),
            #[cfg(feature = "mongodb")]
            Self::Mongo { uri } => f.debug_struct("Mongo").field("uri", uri).finish(),
            // `dyn MessageStore` isn't `Debug`; this variant only ever comes from programmatic
            // construction, so a placeholder is sufficient for diagnostics.
            Self::Custom(_) => f
                .debug_tuple("Custom")
                .field(&"<dyn MessageStore>")
                .finish(),
        }
    }
}

/// Build a boxed [`MessageStore`] from a [`StoreConfig`].
pub async fn build_store(config: &StoreConfig) -> Result<Box<dyn MessageStore>, StoreError> {
    Ok(match config {
        StoreConfig::Memory => Box::new(MemoryStore::new()),
        StoreConfig::File { dir, options } => {
            Box::new(FileStore::open_with_options(dir, *options)?)
        }
        StoreConfig::CachedFile { dir, options } => {
            Box::new(CachedFileStore::open_with_options(dir, *options)?)
        }
        StoreConfig::Noop => Box::new(NoopStore::new()),
        #[cfg(feature = "sql")]
        StoreConfig::Sql {
            url,
            sessions_table,
            messages_table,
            session_id,
            pool,
        } => {
            let defaults = SqlStoreConfig::new(url);
            Box::new(
                SqlStore::connect_with_config(SqlStoreConfig {
                    sessions_table: sessions_table.clone().unwrap_or(defaults.sessions_table),
                    messages_table: messages_table.clone().unwrap_or(defaults.messages_table),
                    session_id: session_id.clone().unwrap_or(defaults.session_id),
                    pool: pool.unwrap_or(defaults.pool),
                    ..defaults
                })
                .await?,
            )
        }
        #[cfg(feature = "mssql")]
        StoreConfig::Mssql {
            url,
            sessions_table,
            messages_table,
            session_id,
        } => {
            let defaults = MssqlStoreConfig::new(url);
            Box::new(
                MssqlStore::connect_with_config(MssqlStoreConfig {
                    sessions_table: sessions_table.clone().unwrap_or(defaults.sessions_table),
                    messages_table: messages_table.clone().unwrap_or(defaults.messages_table),
                    session_id: session_id.clone().unwrap_or(defaults.session_id),
                    ..defaults
                })
                .await?,
            )
        }
        #[cfg(feature = "redb")]
        StoreConfig::Redb { path } => Box::new(RedbStore::connect(path).await?),
        #[cfg(feature = "mongodb")]
        StoreConfig::Mongo { uri } => Box::new(MongoStore::connect(uri).await?),
        StoreConfig::Custom(store) => Box::new(store.clone()),
    })
}
