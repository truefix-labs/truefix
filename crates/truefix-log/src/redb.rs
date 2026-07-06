//! Embedded transactional log, behind the `redb` feature (US5, feature 004).
//!
//! Mirrors `truefix_store::RedbStore`'s design: a modern, license-clean replacement for
//! QuickFIX/J's obsolete `SleepycatStore`/`SleepycatLog`, not a port of it (Constitution Principle
//! III). Like [`crate::SqlLog`]/[`crate::MssqlLog`], the [`Log`] trait is synchronous while `redb`'s
//! API is also synchronous but blocking — each entry is pushed onto a bounded channel and written
//! by a background task holding the single `redb::Database` handle, routing each write through
//! `tokio::task::spawn_blocking` (matching `RedbStore`'s own discipline of never blocking the
//! executor with `redb`'s blocking API). A saturated channel drops excess entries under [`Log`]'s
//! best-effort contract rather than growing without limit.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use tokio::sync::mpsc;

use crate::{Log, LogError, is_heartbeat};

const ASYNC_LOG_CHANNEL_CAPACITY: usize = 1024;

/// GAP-41/FR-019 (feature 005): the value type widened from bare `&str` to
/// `(logged_at: Unix seconds, session_id, text)`, so a row can be audited/replayed on its own
/// without an external join. A schema-widening, deliberately-breaking change to the on-disk
/// format for pre-existing `.redb` log files (no migration path — matches research.md's own
/// framing of this as a widening, not an additive change).
const INCOMING: TableDefinition<u64, (i64, &str, &str)> = TableDefinition::new("log_incoming");
const OUTGOING: TableDefinition<u64, (i64, &str, &str)> = TableDefinition::new("log_outgoing");
const EVENT: TableDefinition<u64, (i64, &str, &str)> = TableDefinition::new("log_event");

fn backend<E: std::fmt::Display>(e: E) -> LogError {
    LogError::Io(e.to_string())
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

enum Entry {
    Message {
        direction: &'static str,
        text: String,
    },
    Event {
        text: String,
    },
}

/// Configuration for [`RedbLog::connect_with_config`] (US5, feature 004).
#[derive(Debug, Clone)]
pub struct RedbLogConfig {
    /// Path to the `redb` database file. Created if it doesn't exist yet.
    pub path: PathBuf,
    /// Whether Heartbeat (`35=0`) messages are persisted.
    pub include_heartbeats: bool,
    /// The session identity stamped onto every row (GAP-41/FR-019, feature 005). See
    /// `truefix_log::SqlLogConfig::session_id`'s doc for the rationale.
    pub session_id: String,
}

impl RedbLogConfig {
    /// A config using `path` with `include_heartbeats` defaulting to `true` and `session_id`
    /// `"default"`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            include_heartbeats: true,
            session_id: "default".to_owned(),
        }
    }
}

/// An embedded, transactional log using `redb` (US5). Writes to three append-only tables
/// (incoming/outgoing/event), each keyed by an auto-incrementing counter maintained in the
/// background writer task (redb has no built-in identity/autoincrement column).
pub struct RedbLog {
    tx: mpsc::Sender<Entry>,
    include_heartbeats: bool,
}

/// The next free key for `table` — one past its current max key, or `0` if empty.
fn next_key(
    db: &Database,
    table: TableDefinition<u64, (i64, &str, &str)>,
) -> Result<u64, LogError> {
    let txn = db.begin_read().map_err(backend)?;
    let t = txn.open_table(table).map_err(backend)?;
    let last = t.last().map_err(backend)?;
    let next = last.map(|(k, _)| k.value() + 1).unwrap_or(0);
    Ok(next)
}

fn insert_blocking(
    db: &Database,
    table: TableDefinition<u64, (i64, &str, &str)>,
    key: u64,
    session_id: &str,
    text: &str,
) -> Result<(), LogError> {
    let txn = db.begin_write().map_err(backend)?;
    {
        let mut t = txn.open_table(table).map_err(backend)?;
        t.insert(key, (now_unix(), session_id, text))
            .map_err(backend)?;
    }
    txn.commit().map_err(backend)?;
    Ok(())
}

impl RedbLog {
    /// Connect to `path` with `include_heartbeats` defaulting to `true` (backward-compatible
    /// shorthand for [`Self::connect_with_config`]).
    pub async fn connect(path: &Path) -> Result<Self, LogError> {
        Self::connect_with_config(RedbLogConfig::new(path)).await
    }

    /// Connect using a full [`RedbLogConfig`]: ensures the tables exist and spawns the background
    /// writer holding the single `redb::Database` handle.
    pub async fn connect_with_config(config: RedbLogConfig) -> Result<Self, LogError> {
        let path = config.path;
        let db = tokio::task::spawn_blocking(move || -> Result<Database, LogError> {
            let db = Database::create(&path).map_err(backend)?;
            let txn = db.begin_write().map_err(backend)?;
            txn.open_table(INCOMING).map_err(backend)?;
            txn.open_table(OUTGOING).map_err(backend)?;
            txn.open_table(EVENT).map_err(backend)?;
            txn.commit().map_err(backend)?;
            Ok(db)
        })
        .await
        .map_err(backend)??;
        let db = Arc::new(db);

        let mut next_incoming = next_key(&db, INCOMING)?;
        let mut next_outgoing = next_key(&db, OUTGOING)?;
        let mut next_event = next_key(&db, EVENT)?;

        let (tx, mut rx) = mpsc::channel::<Entry>(ASYNC_LOG_CHANNEL_CAPACITY);
        let session_id = config.session_id;
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                let db = db.clone();
                let session_id = session_id.clone();
                let result = match entry {
                    Entry::Message { direction, text } => {
                        let (table, key) = if direction == "I" {
                            let key = next_incoming;
                            next_incoming += 1;
                            (INCOMING, key)
                        } else {
                            let key = next_outgoing;
                            next_outgoing += 1;
                            (OUTGOING, key)
                        };
                        tokio::task::spawn_blocking(move || {
                            insert_blocking(&db, table, key, &session_id, &text)
                        })
                        .await
                    }
                    Entry::Event { text } => {
                        let key = next_event;
                        next_event += 1;
                        tokio::task::spawn_blocking(move || {
                            insert_blocking(&db, EVENT, key, &session_id, &text)
                        })
                        .await
                    }
                };
                // Best-effort: a write failure (join or backend error) drops the entry rather than
                // failing the session (matches the synchronous, infallible `Log` trait contract,
                // same as `SqlLog`/`MssqlLog`'s `insert_text`).
                let _ = result;
            }
        });

        Ok(Self {
            tx,
            include_heartbeats: config.include_heartbeats,
        })
    }
}

impl Log for RedbLog {
    fn on_incoming(&self, message: &str) {
        if is_heartbeat(message) && !self.include_heartbeats {
            return;
        }
        let _ = self.tx.try_send(Entry::Message {
            direction: "I",
            text: message.to_owned(),
        });
    }
    fn on_outgoing(&self, message: &str) {
        if is_heartbeat(message) && !self.include_heartbeats {
            return;
        }
        let _ = self.tx.try_send(Entry::Message {
            direction: "O",
            text: message.to_owned(),
        });
    }
    fn on_event(&self, text: &str) {
        let _ = self.tx.try_send(Entry::Event {
            text: text.to_owned(),
        });
    }
}
