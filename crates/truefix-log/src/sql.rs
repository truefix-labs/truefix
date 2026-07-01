//! SQL-backed log (JDBC-equivalent), behind the `sql` feature.
//!
//! Supports PostgreSQL, MySQL, and SQLite (FR-024), selected by the connect URL's scheme, mirroring
//! `truefix_store::SqlStore`. The [`Log`] trait is synchronous and infallible, while `sqlx` is
//! async: each entry is pushed onto an unbounded channel (a non-blocking, sync send) and written
//! to the database by a background task. Table names (`JdbcLogIncomingTable`/
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

use crate::{is_heartbeat, Log, LogError};

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
}

impl SqlLogConfig {
    /// A config using `url` with default table names and pool settings.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            incoming_table: "log_incoming".to_owned(),
            outgoing_table: "log_outgoing".to_owned(),
            event_table: "log_event".to_owned(),
            include_heartbeats: true,
            pool: SqlLogPoolOptions::default(),
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
    match pool {
        Pool::Sqlite(p) => {
            sqlx::query(&format!(
                "CREATE TABLE IF NOT EXISTS {table} (\
                 id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL)"
            ))
            .execute(p)
            .await
            .map_err(io_err)?;
        }
        Pool::Postgres(p) => {
            sqlx::query(&format!(
                "CREATE TABLE IF NOT EXISTS {table} (id BIGSERIAL PRIMARY KEY, text TEXT NOT NULL)"
            ))
            .execute(p)
            .await
            .map_err(io_err)?;
        }
        Pool::MySql(p) => {
            sqlx::query(&format!(
                "CREATE TABLE IF NOT EXISTS {table} (\
                 id BIGINT AUTO_INCREMENT PRIMARY KEY, text TEXT NOT NULL)"
            ))
            .execute(p)
            .await
            .map_err(io_err)?;
        }
    }
    Ok(())
}

async fn insert_text(pool: &Pool, table: &str, text: &str) {
    // Best-effort: a write failure drops the entry rather than failing the session (matches the
    // synchronous, infallible `Log` trait contract). Each backend's `execute` yields a distinct
    // concrete `QueryResult` type, so each arm discards its own result rather than unifying.
    match pool {
        Pool::Sqlite(p) => {
            let _ = sqlx::query(&format!("INSERT INTO {table} (text) VALUES (?)"))
                .bind(text)
                .execute(p)
                .await;
        }
        Pool::Postgres(p) => {
            let _ = sqlx::query(&format!("INSERT INTO {table} (text) VALUES ($1)"))
                .bind(text)
                .execute(p)
                .await;
        }
        Pool::MySql(p) => {
            let _ = sqlx::query(&format!("INSERT INTO {table} (text) VALUES (?)"))
                .bind(text)
                .execute(p)
                .await;
        }
    }
}

/// A SQL-backed log writing to configurable incoming/outgoing/event tables via a background task.
pub struct SqlLog {
    tx: mpsc::UnboundedSender<Entry>,
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

        let (tx, mut rx) = mpsc::unbounded_channel::<Entry>();
        let incoming_table = config.incoming_table;
        let outgoing_table = config.outgoing_table;
        let event_table = config.event_table;
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                match entry {
                    Entry::Message { direction, text } => {
                        let table = if direction == "I" {
                            &incoming_table
                        } else {
                            &outgoing_table
                        };
                        insert_text(&pool, table, &text).await;
                    }
                    Entry::Event { text } => {
                        insert_text(&pool, &event_table, &text).await;
                    }
                }
            }
        });

        Ok(Self {
            tx,
            include_heartbeats: config.include_heartbeats,
        })
    }
}

impl Log for SqlLog {
    fn on_incoming(&self, message: &str) {
        if is_heartbeat(message) && !self.include_heartbeats {
            return;
        }
        let _ = self.tx.send(Entry::Message {
            direction: "I",
            text: message.to_owned(),
        });
    }
    fn on_outgoing(&self, message: &str) {
        if is_heartbeat(message) && !self.include_heartbeats {
            return;
        }
        let _ = self.tx.send(Entry::Message {
            direction: "O",
            text: message.to_owned(),
        });
    }
    fn on_event(&self, text: &str) {
        let _ = self.tx.send(Entry::Event {
            text: text.to_owned(),
        });
    }
}
