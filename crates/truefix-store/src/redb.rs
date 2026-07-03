//! Embedded transactional message store, behind the `redb` feature (US5, feature 004).
//!
//! A modern, license-clean replacement for QuickFIX/J's obsolete `SleepycatStore` (Berkeley DB
//! JE), not a port of it — `redb` has no QuickFIX/J/Go precedent (Constitution Principle III: this
//! is a from-scratch design against the FIX-agnostic `redb` crate, not derived from any reference
//! implementation's storage layer). Behaves equivalently to [`crate::SqlStore`] — same
//! sequence-number/sent-message schema shape, differing only in the connection/query layer.
//!
//! `redb`'s own API is fully synchronous (no async variant) — every [`MessageStore`] method here
//! wraps its `redb` work in `tokio::task::spawn_blocking`, so it never blocks the async executor
//! (research.md §5, Constitution Principle I: production-readiness bars blocking the executor).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

use crate::{MessageStore, StoreError};

const SENDER_SEQ: TableDefinition<&str, u64> = TableDefinition::new("sender_seq");
const TARGET_SEQ: TableDefinition<&str, u64> = TableDefinition::new("target_seq");
const MESSAGES: TableDefinition<(&str, u64), &[u8]> = TableDefinition::new("messages");
/// GAP-38/FR-017 (feature 005): Unix-seconds creation time, keyed by `session_id`.
const CREATION_TIME: TableDefinition<&str, i64> = TableDefinition::new("creation_time");

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn backend<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Backend(e.to_string())
}

fn join_err(e: tokio::task::JoinError) -> StoreError {
    StoreError::Backend(format!("redb task panicked: {e}"))
}

/// Configuration for [`RedbStore::connect_with_config`] — mirrors [`crate::SqlStoreConfig`]'s
/// shape (minus pool settings; `redb::Database` handles its own internal concurrency, no separate
/// pool needed).
#[derive(Debug, Clone)]
pub struct RedbStoreConfig {
    /// Path to the `redb` database file. Created if it doesn't exist yet.
    pub path: PathBuf,
    /// The composite session key discriminating rows when several sessions share one file.
    pub session_id: String,
}

impl RedbStoreConfig {
    /// A config using `path` with a default `session_id` of `"default"` (single-session use).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            session_id: "default".to_owned(),
        }
    }
}

/// An embedded, transactional store using `redb` (US5, FR-006). Maps the same sequence-number/
/// sent-message schema shape as [`crate::SqlStore`]: two tables for sequence numbers
/// (`sender_seq`/`target_seq`, keyed by `session_id`), one for message bodies (`messages`, keyed by
/// `(session_id, seq)`).
pub struct RedbStore {
    db: Arc<Database>,
    session_id: String,
}

impl RedbStore {
    /// Connect to `path` with `session_id` `"default"` (backward-compatible shorthand for
    /// [`Self::connect_with_config`]).
    pub async fn connect(path: &Path) -> Result<Self, StoreError> {
        Self::connect_with_config(RedbStoreConfig::new(path)).await
    }

    /// Connect using a full [`RedbStoreConfig`]. Creates the file (and its tables) if it doesn't
    /// exist yet; opens it as-is otherwise (`redb::Database::create`'s own documented behavior).
    pub async fn connect_with_config(config: RedbStoreConfig) -> Result<Self, StoreError> {
        let path = config.path;
        let session_id = config.session_id;
        let session_id_for_init = session_id.clone();
        let db = tokio::task::spawn_blocking(move || -> Result<Database, StoreError> {
            let db = Database::create(&path).map_err(backend)?;
            // Ensure every table exists up front, so later reads never hit a "table not found"
            // error on a fresh database. Also records the creation time once, on first connect
            // for this session_id (GAP-38/FR-017, feature 005) — never overwritten by later
            // opens.
            let txn = db.begin_write().map_err(backend)?;
            txn.open_table(SENDER_SEQ).map_err(backend)?;
            txn.open_table(TARGET_SEQ).map_err(backend)?;
            txn.open_table(MESSAGES).map_err(backend)?;
            {
                let mut creation = txn.open_table(CREATION_TIME).map_err(backend)?;
                if creation
                    .get(session_id_for_init.as_str())
                    .map_err(backend)?
                    .is_none()
                {
                    creation
                        .insert(session_id_for_init.as_str(), now_unix())
                        .map_err(backend)?;
                }
            }
            txn.commit().map_err(backend)?;
            Ok(db)
        })
        .await
        .map_err(join_err)??;
        Ok(Self {
            db: Arc::new(db),
            session_id,
        })
    }

    /// A second handle onto the *same already-open* database file, for a different `session_id`.
    ///
    /// Unlike a networked SQL server, `redb::Database::create`/`open` takes an exclusive file lock
    /// per call — two independent `connect`/`connect_with_config` calls against the *same path*
    /// fail with a lock-contention error (confirmed empirically: this is `redb`'s real, documented
    /// single-writer-process model, not a bug). Multiple sessions sharing one `redb` file (the same
    /// pattern `SqlStore`/`MssqlStore` support via independent connections/pool entries) must
    /// therefore share one already-open `Database` handle instead — this method clones the cheap
    /// `Arc<Database>` and swaps only `session_id`.
    pub fn with_session_id(&self, session_id: impl Into<String>) -> Self {
        Self {
            db: self.db.clone(),
            session_id: session_id.into(),
        }
    }

    async fn get_seq(&self, table: TableDefinition<'static, &str, u64>) -> Result<u64, StoreError> {
        let db = self.db.clone();
        let session_id = self.session_id.clone();
        tokio::task::spawn_blocking(move || -> Result<u64, StoreError> {
            let txn = db.begin_read().map_err(backend)?;
            let t = txn.open_table(table).map_err(backend)?;
            Ok(t.get(session_id.as_str())
                .map_err(backend)?
                .map(|guard| guard.value())
                .unwrap_or(1))
        })
        .await
        .map_err(join_err)?
    }

    async fn set_seq(
        &self,
        table: TableDefinition<'static, &str, u64>,
        seq: u64,
    ) -> Result<(), StoreError> {
        let db = self.db.clone();
        let session_id = self.session_id.clone();
        tokio::task::spawn_blocking(move || -> Result<(), StoreError> {
            let txn = db.begin_write().map_err(backend)?;
            {
                let mut t = txn.open_table(table).map_err(backend)?;
                t.insert(session_id.as_str(), seq).map_err(backend)?;
            }
            txn.commit().map_err(backend)?;
            Ok(())
        })
        .await
        .map_err(join_err)?
    }
}

#[async_trait]
impl MessageStore for RedbStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        self.get_seq(SENDER_SEQ).await
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        self.get_seq(TARGET_SEQ).await
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.set_seq(SENDER_SEQ, seq).await
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.set_seq(TARGET_SEQ, seq).await
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        let db = self.db.clone();
        let session_id = self.session_id.clone();
        let message = message.to_vec();
        tokio::task::spawn_blocking(move || -> Result<(), StoreError> {
            let txn = db.begin_write().map_err(backend)?;
            {
                let mut t = txn.open_table(MESSAGES).map_err(backend)?;
                t.insert((session_id.as_str(), seq), message.as_slice())
                    .map_err(backend)?;
            }
            txn.commit().map_err(backend)?;
            Ok(())
        })
        .await
        .map_err(join_err)?
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        let db = self.db.clone();
        let session_id = self.session_id.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
            let txn = db.begin_read().map_err(backend)?;
            let t = txn.open_table(MESSAGES).map_err(backend)?;
            let mut out = Vec::new();
            let range = t
                .range((session_id.as_str(), begin)..=(session_id.as_str(), end))
                .map_err(backend)?;
            for entry in range {
                let (key, value) = entry.map_err(backend)?;
                let (_, seq) = key.value();
                out.push((seq, value.value().to_vec()));
            }
            Ok(out)
        })
        .await
        .map_err(join_err)?
    }
    async fn reset(&self) -> Result<(), StoreError> {
        let db = self.db.clone();
        let session_id = self.session_id.clone();
        tokio::task::spawn_blocking(move || -> Result<(), StoreError> {
            let txn = db.begin_write().map_err(backend)?;
            {
                let mut messages = txn.open_table(MESSAGES).map_err(backend)?;
                let keys: Vec<(String, u64)> = messages
                    .range((session_id.as_str(), 0)..=(session_id.as_str(), u64::MAX))
                    .map_err(backend)?
                    .filter_map(|entry| entry.ok())
                    .map(|(key, _)| {
                        let (sid, seq) = key.value();
                        (sid.to_owned(), seq)
                    })
                    .collect();
                for (sid, seq) in keys {
                    messages.remove((sid.as_str(), seq)).map_err(backend)?;
                }
                let mut sender = txn.open_table(SENDER_SEQ).map_err(backend)?;
                sender.insert(session_id.as_str(), 1u64).map_err(backend)?;
                let mut target = txn.open_table(TARGET_SEQ).map_err(backend)?;
                target.insert(session_id.as_str(), 1u64).map_err(backend)?;
                let mut creation = txn.open_table(CREATION_TIME).map_err(backend)?;
                creation
                    .insert(session_id.as_str(), now_unix())
                    .map_err(backend)?;
            }
            txn.commit().map_err(backend)?;
            Ok(())
        })
        .await
        .map_err(join_err)?
    }
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        let db = self.db.clone();
        let session_id = self.session_id.clone();
        tokio::task::spawn_blocking(
            move || -> Result<Option<time::OffsetDateTime>, StoreError> {
                let txn = db.begin_read().map_err(backend)?;
                let t = txn.open_table(CREATION_TIME).map_err(backend)?;
                let secs = t
                    .get(session_id.as_str())
                    .map_err(backend)?
                    .map(|guard| guard.value());
                Ok(secs.and_then(|s| time::OffsetDateTime::from_unix_timestamp(s).ok()))
            },
        )
        .await
        .map_err(join_err)?
    }
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        // GAP-39/FR-018 (feature 005): a single `redb` write transaction, mirroring `reset()`'s
        // existing atomicity.
        let db = self.db.clone();
        let session_id = self.session_id.clone();
        let message = message.to_vec();
        tokio::task::spawn_blocking(move || -> Result<(), StoreError> {
            let txn = db.begin_write().map_err(backend)?;
            {
                let mut messages = txn.open_table(MESSAGES).map_err(backend)?;
                messages
                    .insert((session_id.as_str(), seq), message.as_slice())
                    .map_err(backend)?;
                let mut sender = txn.open_table(SENDER_SEQ).map_err(backend)?;
                sender
                    .insert(session_id.as_str(), seq + 1)
                    .map_err(backend)?;
            }
            txn.commit().map_err(backend)?;
            Ok(())
        })
        .await
        .map_err(join_err)?
    }
}
