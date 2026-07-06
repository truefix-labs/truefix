//! SQL-backed log (JDBC-equivalent), behind the `sql` feature.
//!
//! Supports PostgreSQL, MySQL, and SQLite (FR-024), selected by the connect URL's scheme, mirroring
//! `truefix_store::SqlStore`. The [`Log`] trait is synchronous and infallible, while `sqlx` is
//! async: each entry is pushed onto a bounded channel and written to the database by a background
//! task. When the database cannot keep up, excess entries are dropped in accordance with [`Log`]'s
//! best-effort contract instead of allowing the queue to consume unbounded memory. Table names
//! (`JdbcLogIncomingTable`/
//! `JdbcLogOutgoingTable`/`JdbcLogEventTable`) and pool settings are configurable via
//! [`SqlLogConfig`]; `JdbcLogHeartBeats` filters Heartbeat messages before they're queued.

use std::str::FromStr;
use std::time::Duration;

use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::pool::PoolOptions;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Database, MySqlPool, PgPool, SqlitePool};
use tokio::sync::mpsc;

use crate::{Log, LogError, is_heartbeat};

const ASYNC_LOG_CHANNEL_CAPACITY: usize = 1024;

fn io_err<E: std::fmt::Display>(e: E) -> LogError {
    LogError::Io(e.to_string())
}

/// Connection-pool settings (FR-024), matching the registered `Jdbc*Connection*` keys. A
/// crate-local duplicate of `truefix_store::SqlPoolOptions`'s shape, since `truefix-log` doesn't
/// depend on `truefix-store` (siblings in the layering).
#[derive(Debug, Clone, Copy)]
pub struct SqlLogPoolOptions {
    /// `JdbcMaxActiveConnection`.
    pub max_connections: u32,
    /// `JdbcMinIdleConnection`.
    pub min_connections: u32,
    /// `JdbcConnectionTimeout`.
    pub acquire_timeout: Duration,
    /// `JdbcConnectionIdleTimeout`.
    pub idle_timeout: Option<Duration>,
    /// `JdbcMaxConnectionLifeTime`.
    pub max_lifetime: Option<Duration>,
}

impl Default for SqlLogPoolOptions {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: None,
            max_lifetime: None,
        }
    }
}

/// Configuration for [`SqlLog::connect_with_config`] (FR-024).
#[derive(Debug, Clone)]
pub struct SqlLogConfig {
    /// The database URL (`postgres://...`, `mysql://...`, or `sqlite:...`/`sqlite::memory:`).
    pub url: String,
    /// `JdbcLogIncomingTable`.
    pub incoming_table: String,
    /// `JdbcLogOutgoingTable`.
    pub outgoing_table: String,
    /// `JdbcLogEventTable`.
    pub event_table: String,
    /// `JdbcLogHeartBeats`: whether Heartbeat (`35=0`) messages are persisted.
    pub include_heartbeats: bool,
    /// Connection-pool settings.
    pub pool: SqlLogPoolOptions,
    /// The session identity stamped onto every row's `session_id` column (GAP-41/FR-019,
    /// feature 005), letting a row be audited/replayed on its own without an external join —
    /// even though table *selection* already segregates sessions, this makes that fact explicit
    /// per-row (and supports intentionally sharing one table across sessions in the future).
    pub session_id: String,
}

impl SqlLogConfig {
    /// A config using `url` with default table names, pool settings, and `session_id`
    /// `"default"`.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            incoming_table: "log_incoming".to_owned(),
            outgoing_table: "log_outgoing".to_owned(),
            event_table: "log_event".to_owned(),
            include_heartbeats: true,
            pool: SqlLogPoolOptions::default(),
            session_id: "default".to_owned(),
        }
    }
}

fn valid_identifier(s: &str) -> Result<(), LogError> {
    let ok = !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(LogError::Io(format!(
            "{s:?} is not a valid SQL table identifier"
        )))
    }
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

enum Pool {
    Sqlite(SqlitePool),
    Postgres(PgPool),
    MySql(MySqlPool),
}

fn apply_pool_options<DB: Database>(
    opts: PoolOptions<DB>,
    cfg: &SqlLogPoolOptions,
) -> PoolOptions<DB> {
    opts.max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(cfg.acquire_timeout)
        .idle_timeout(cfg.idle_timeout)
        .max_lifetime(cfg.max_lifetime)
}

async fn connect_pool(url: &str, pool_cfg: &SqlLogPoolOptions) -> Result<Pool, LogError> {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        let opts = PgConnectOptions::from_str(url).map_err(io_err)?;
        let pool = apply_pool_options(PgPoolOptions::new(), pool_cfg)
            .connect_with(opts)
            .await
            .map_err(io_err)?;
        Ok(Pool::Postgres(pool))
    } else if url.starts_with("mysql://") {
        let opts = MySqlConnectOptions::from_str(url).map_err(io_err)?;
        let pool = apply_pool_options(MySqlPoolOptions::new(), pool_cfg)
            .connect_with(opts)
            .await
            .map_err(io_err)?;
        Ok(Pool::MySql(pool))
    } else {
        let opts = SqliteConnectOptions::from_str(url)
            .map_err(io_err)?
            .create_if_missing(true);
        let pool = apply_pool_options(SqlitePoolOptions::new(), pool_cfg)
            .connect_with(opts)
            .await
            .map_err(io_err)?;
        Ok(Pool::Sqlite(pool))
    }
}

async fn ensure_table(pool: &Pool, table: &str) -> Result<(), LogError> {
    // GAP-41/FR-019 (feature 005): `logged_at` (Unix seconds) + `session_id` widen every row so
    // it can be audited/replayed on its own, without an external join.
    match pool {
        Pool::Sqlite(p) => {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE TABLE IF NOT EXISTS {table} (\
                 id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL, \
                 logged_at BIGINT, session_id TEXT)"
            )))
            .execute(p)
            .await
            .map_err(io_err)?;
        }
        Pool::Postgres(p) => {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE TABLE IF NOT EXISTS {table} (id BIGSERIAL PRIMARY KEY, text TEXT NOT NULL, \
                 logged_at BIGINT, session_id TEXT)"
            )))
            .execute(p)
            .await
            .map_err(io_err)?;
        }
        Pool::MySql(p) => {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE TABLE IF NOT EXISTS {table} (\
                 id BIGINT AUTO_INCREMENT PRIMARY KEY, text TEXT NOT NULL, \
                 logged_at BIGINT, session_id VARCHAR(255))"
            )))
            .execute(p)
            .await
            .map_err(io_err)?;
        }
    }
    // Best-effort widening of a table created before these columns existed; errors (most
    // commonly "column already exists") are intentionally swallowed.
    match pool {
        Pool::Sqlite(p) => {
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "ALTER TABLE {table} ADD COLUMN logged_at BIGINT"
            )))
            .execute(p)
            .await;
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "ALTER TABLE {table} ADD COLUMN session_id TEXT"
            )))
            .execute(p)
            .await;
        }
        Pool::Postgres(p) => {
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS logged_at BIGINT"
            )))
            .execute(p)
            .await;
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS session_id TEXT"
            )))
            .execute(p)
            .await;
        }
        Pool::MySql(p) => {
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "ALTER TABLE {table} ADD COLUMN logged_at BIGINT"
            )))
            .execute(p)
            .await;
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "ALTER TABLE {table} ADD COLUMN session_id VARCHAR(255)"
            )))
            .execute(p)
            .await;
        }
    }
    Ok(())
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

async fn insert_text(pool: &Pool, table: &str, session_id: &str, text: &str) {
    // Best-effort: a write failure drops the entry rather than failing the session (matches the
    // synchronous, infallible `Log` trait contract). Each backend's `execute` yields a distinct
    // concrete `QueryResult` type, so each arm discards its own result rather than unifying.
    match pool {
        Pool::Sqlite(p) => {
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {table} (text, logged_at, session_id) VALUES (?, ?, ?)"
            )))
            .bind(text)
            .bind(now_unix())
            .bind(session_id)
            .execute(p)
            .await;
        }
        Pool::Postgres(p) => {
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {table} (text, logged_at, session_id) VALUES ($1, $2, $3)"
            )))
            .bind(text)
            .bind(now_unix())
            .bind(session_id)
            .execute(p)
            .await;
        }
        Pool::MySql(p) => {
            let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {table} (text, logged_at, session_id) VALUES (?, ?, ?)"
            )))
            .bind(text)
            .bind(now_unix())
            .bind(session_id)
            .execute(p)
            .await;
        }
    }
}

/// A SQL-backed log writing to configurable incoming/outgoing/event tables via a background task.
pub struct SqlLog {
    // NEW-91 (feature 009): see `RedbLog`'s identical field for the rationale.
    tx: std::sync::Mutex<Option<mpsc::Sender<Entry>>>,
    task: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    include_heartbeats: bool,
}

impl SqlLog {
    /// Connect to `url` with default table names/pool settings (backward-compatible shorthand for
    /// [`Self::connect_with_config`]).
    pub async fn connect(url: &str) -> Result<Self, LogError> {
        Self::connect_with_config(SqlLogConfig::new(url)).await
    }

    /// Connect using a full [`SqlLogConfig`] (FR-024): selects PostgreSQL/MySQL/SQLite by the
    /// URL's scheme, ensures the schema, and spawns the background writer.
    pub async fn connect_with_config(config: SqlLogConfig) -> Result<Self, LogError> {
        valid_identifier(&config.incoming_table)?;
        valid_identifier(&config.outgoing_table)?;
        valid_identifier(&config.event_table)?;
        let pool = connect_pool(&config.url, &config.pool).await?;
        ensure_table(&pool, &config.incoming_table).await?;
        ensure_table(&pool, &config.outgoing_table).await?;
        ensure_table(&pool, &config.event_table).await?;

        let (tx, mut rx) = mpsc::channel::<Entry>(ASYNC_LOG_CHANNEL_CAPACITY);
        let incoming_table = config.incoming_table;
        let outgoing_table = config.outgoing_table;
        let event_table = config.event_table;
        let session_id = config.session_id;
        let task = tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                match entry {
                    Entry::Message { direction, text } => {
                        let table = if direction == "I" {
                            &incoming_table
                        } else {
                            &outgoing_table
                        };
                        insert_text(&pool, table, &session_id, &text).await;
                    }
                    Entry::Event { text } => {
                        insert_text(&pool, &event_table, &session_id, &text).await;
                    }
                }
            }
        });

        Ok(Self {
            tx: std::sync::Mutex::new(Some(tx)),
            task: std::sync::Mutex::new(Some(task)),
            include_heartbeats: config.include_heartbeats,
        })
    }

    fn send(&self, entry: Entry) {
        if let Ok(guard) = self.tx.lock()
            && let Some(tx) = guard.as_ref()
        {
            let _ = tx.try_send(entry);
        }
    }
}

#[async_trait::async_trait]
impl Log for SqlLog {
    fn on_incoming(&self, message: &str) {
        if is_heartbeat(message) && !self.include_heartbeats {
            return;
        }
        self.send(Entry::Message {
            direction: "I",
            text: message.to_owned(),
        });
    }
    fn on_outgoing(&self, message: &str) {
        if is_heartbeat(message) && !self.include_heartbeats {
            return;
        }
        self.send(Entry::Message {
            direction: "O",
            text: message.to_owned(),
        });
    }
    fn on_event(&self, text: &str) {
        self.send(Entry::Event {
            text: text.to_owned(),
        });
    }
    async fn shutdown(&self) {
        let tx = self.tx.lock().ok().and_then(|mut guard| guard.take());
        drop(tx);
        let task = self.task.lock().ok().and_then(|mut guard| guard.take());
        if let Some(task) = task {
            let _ = task.await;
        }
    }
}
