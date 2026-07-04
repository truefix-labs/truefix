//! SQL-backed message store (JDBC-equivalent), behind the `sql` feature.
//!
//! Supports PostgreSQL, MySQL, and SQLite (FR-024), selected by the connect URL's scheme
//! (`postgres://`/`postgresql://`, `mysql://`, everything else treated as SQLite). Table names
//! and connection-pool settings are configurable via [`SqlStoreConfig`], matching the registered
//! `JdbcStoreSessionsTableName`/`JdbcStoreMessagesTableName`/`JdbcMaxActiveConnection`/etc. keys.
//! A `session_id` discriminator column lets multiple FIX sessions share one pair of tables, as in
//! QuickFIX/J's JDBC store schema.

use std::str::FromStr;
use std::time::Duration;

use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::pool::PoolOptions;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Database, MySqlPool, PgPool, Row, SqlitePool};

use async_trait::async_trait;

use crate::{MessageStore, StoreError};

fn backend<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Backend(e.to_string())
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

/// Connection-pool settings (FR-024), matching the registered `Jdbc*Connection*` keys.
#[derive(Debug, Clone, Copy)]
pub struct SqlPoolOptions {
    /// `JdbcMaxActiveConnection`.
    pub max_connections: u32,
    /// `JdbcMinIdleConnection`.
    pub min_connections: u32,
    /// `JdbcConnectionTimeout`: how long to wait for a pooled connection.
    pub acquire_timeout: Duration,
    /// `JdbcConnectionIdleTimeout`: close a connection idle longer than this.
    pub idle_timeout: Option<Duration>,
    /// `JdbcMaxConnectionLifeTime`: recycle a connection older than this regardless of use.
    pub max_lifetime: Option<Duration>,
    /// `JdbcConnectionKeepaliveTime` (US8, feature 005, FR-021): parsed and stored for `.cfg`
    /// drop-in compatibility, but intentionally not wired into `sqlx`'s pool — `sqlx` has no
    /// keepalive-probe concept distinct from `idle_timeout`/`max_lifetime` (both already exposed
    /// above), matching this project's existing precedent for QuickFIX/J config keys with no
    /// underlying Rust mechanism to attach to (e.g. `SessionConfig::closed_resend_interval`).
    pub keepalive: Option<Duration>,
}

impl Default for SqlPoolOptions {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: None,
            max_lifetime: None,
            keepalive: None,
        }
    }
}

/// Configuration for [`SqlStore::connect_with_config`] (FR-024).
#[derive(Debug, Clone)]
pub struct SqlStoreConfig {
    /// The database URL (`postgres://...`, `mysql://...`, or `sqlite:...`/`sqlite::memory:`).
    pub url: String,
    /// `JdbcStoreSessionsTableName`: table holding the sender/target sequence numbers.
    pub sessions_table: String,
    /// `JdbcStoreMessagesTableName`: table holding sent message bodies.
    pub messages_table: String,
    /// The composite session key discriminating rows when several sessions share one table pair.
    pub session_id: String,
    /// Connection-pool settings.
    pub pool: SqlPoolOptions,
}

impl SqlStoreConfig {
    /// A config using `url` with default table names (`seqnums`/`messages`), a default
    /// `session_id` of `"default"` (single-session use), and default pool settings.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            sessions_table: "seqnums".to_owned(),
            messages_table: "messages".to_owned(),
            session_id: "default".to_owned(),
            pool: SqlPoolOptions::default(),
        }
    }
}

fn valid_identifier(s: &str) -> Result<(), StoreError> {
    let ok = !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(StoreError::Backend(format!(
            "{s:?} is not a valid SQL table identifier"
        )))
    }
}

enum Pool {
    Sqlite(SqlitePool),
    Postgres(PgPool),
    MySql(MySqlPool),
}

fn apply_pool_options<DB: Database>(
    opts: PoolOptions<DB>,
    cfg: &SqlPoolOptions,
) -> PoolOptions<DB> {
    opts.max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(cfg.acquire_timeout)
        .idle_timeout(cfg.idle_timeout)
        .max_lifetime(cfg.max_lifetime)
}

/// A SQL-backed store using `sqlx` (PostgreSQL, MySQL, or SQLite). Maps the QuickFIX/J JDBC store
/// tables (sequence numbers + sent messages) to a relational schema (FR-024).
pub struct SqlStore {
    pool: Pool,
    sessions_table: String,
    messages_table: String,
    session_id: String,
}

impl SqlStore {
    /// Connect to `url` with default table names/pool settings (backward-compatible shorthand for
    /// [`Self::connect_with_config`]).
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        Self::connect_with_config(SqlStoreConfig::new(url)).await
    }

    /// Connect using a full [`SqlStoreConfig`] (FR-024): selects PostgreSQL/MySQL/SQLite by the
    /// URL's scheme, applies pool settings, validates table names, and ensures the schema.
    pub async fn connect_with_config(config: SqlStoreConfig) -> Result<Self, StoreError> {
        valid_identifier(&config.sessions_table)?;
        valid_identifier(&config.messages_table)?;
        let pool = connect_pool(&config.url, &config.pool).await?;
        let store = Self {
            pool,
            sessions_table: config.sessions_table,
            messages_table: config.messages_table,
            session_id: config.session_id,
        };
        store.ensure_schema().await?;
        Ok(store)
    }

    async fn ensure_schema(&self) -> Result<(), StoreError> {
        let sessions = &self.sessions_table;
        let messages = &self.messages_table;
        match &self.pool {
            Pool::Sqlite(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "CREATE TABLE IF NOT EXISTS {sessions} (\
                     session_id TEXT NOT NULL PRIMARY KEY, sender INTEGER NOT NULL, target INTEGER NOT NULL, \
                     creation_time BIGINT)"
                )))
                .execute(pool)
                .await
                .map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "CREATE TABLE IF NOT EXISTS {messages} (\
                     session_id TEXT NOT NULL, seq INTEGER NOT NULL, data BLOB NOT NULL, \
                     PRIMARY KEY (session_id, seq))"
                )))
                .execute(pool)
                .await
                .map_err(backend)?;
                // BUG-26 (feature 007): the `creation_time`-column migration MUST run before any
                // statement referencing it -- a table created before this column existed (a
                // pre-005 deployment) leaves `CREATE TABLE IF NOT EXISTS` a no-op, so the INSERT
                // below would otherwise reference a column that doesn't exist yet.
                self.add_creation_time_column_if_missing().await.ok();
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT OR IGNORE INTO {sessions} (session_id, sender, target, creation_time) \
                     VALUES (?, 1, 1, ?)"
                )))
                .bind(&self.session_id)
                .bind(now_unix())
                .execute(pool)
                .await
                .map_err(backend)?;
            }
            Pool::Postgres(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "CREATE TABLE IF NOT EXISTS {sessions} (\
                     session_id TEXT PRIMARY KEY, sender BIGINT NOT NULL, target BIGINT NOT NULL, \
                     creation_time BIGINT)"
                )))
                .execute(pool)
                .await
                .map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "CREATE TABLE IF NOT EXISTS {messages} (\
                     session_id TEXT NOT NULL, seq BIGINT NOT NULL, data BYTEA NOT NULL, \
                     PRIMARY KEY (session_id, seq))"
                )))
                .execute(pool)
                .await
                .map_err(backend)?;
                // BUG-26 (feature 007): see the Sqlite branch's comment above -- same ordering fix.
                self.add_creation_time_column_if_missing().await.ok();
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT INTO {sessions} (session_id, sender, target, creation_time) \
                     VALUES ($1, 1, 1, $2) ON CONFLICT (session_id) DO NOTHING"
                )))
                .bind(&self.session_id)
                .bind(now_unix())
                .execute(pool)
                .await
                .map_err(backend)?;
            }
            Pool::MySql(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "CREATE TABLE IF NOT EXISTS {sessions} (\
                     session_id VARCHAR(255) PRIMARY KEY, sender BIGINT NOT NULL, target BIGINT NOT NULL, \
                     creation_time BIGINT)"
                )))
                .execute(pool)
                .await
                .map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "CREATE TABLE IF NOT EXISTS {messages} (\
                     session_id VARCHAR(255) NOT NULL, seq BIGINT NOT NULL, data BLOB NOT NULL, \
                     PRIMARY KEY (session_id, seq))"
                )))
                .execute(pool)
                .await
                .map_err(backend)?;
                // BUG-26 (feature 007): see the Sqlite branch's comment above -- same ordering fix.
                self.add_creation_time_column_if_missing().await.ok();
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT IGNORE INTO {sessions} (session_id, sender, target, creation_time) \
                     VALUES (?, 1, 1, ?)"
                )))
                .bind(&self.session_id)
                .bind(now_unix())
                .execute(pool)
                .await
                .map_err(backend)?;
            }
        }
        Ok(())
    }

    /// Best-effort `ALTER TABLE ... ADD COLUMN creation_time` for a sessions table that predates
    /// this column (GAP-38/FR-017, feature 005). Errors (most commonly "column already exists")
    /// are intentionally swallowed by the caller — this is a compatibility nicety, not a
    /// correctness requirement (a freshly created table already has the column from
    /// `CREATE TABLE`, above).
    async fn add_creation_time_column_if_missing(&self) -> Result<(), StoreError> {
        let sessions = &self.sessions_table;
        match &self.pool {
            Pool::Sqlite(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "ALTER TABLE {sessions} ADD COLUMN creation_time BIGINT"
                )))
                .execute(pool)
                .await
                .map_err(backend)?;
            }
            Pool::Postgres(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "ALTER TABLE {sessions} ADD COLUMN IF NOT EXISTS creation_time BIGINT"
                )))
                .execute(pool)
                .await
                .map_err(backend)?;
            }
            Pool::MySql(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "ALTER TABLE {sessions} ADD COLUMN creation_time BIGINT"
                )))
                .execute(pool)
                .await
                .map_err(backend)?;
            }
        }
        Ok(())
    }

    async fn get_seq(&self, column: &str) -> Result<u64, StoreError> {
        let sessions = &self.sessions_table;
        let value: i64 = match &self.pool {
            Pool::Sqlite(pool) => {
                let sql = format!("SELECT {column} FROM {sessions} WHERE session_id = ?");
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .fetch_one(pool)
                    .await
                    .map_err(backend)?
                    .try_get(column)
                    .map_err(backend)?
            }
            Pool::Postgres(pool) => {
                let sql = format!("SELECT {column} FROM {sessions} WHERE session_id = $1");
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .fetch_one(pool)
                    .await
                    .map_err(backend)?
                    .try_get(column)
                    .map_err(backend)?
            }
            Pool::MySql(pool) => {
                let sql = format!("SELECT {column} FROM {sessions} WHERE session_id = ?");
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .fetch_one(pool)
                    .await
                    .map_err(backend)?
                    .try_get(column)
                    .map_err(backend)?
            }
        };
        Ok(value as u64)
    }

    async fn set_seq(&self, column: &str, seq: u64) -> Result<(), StoreError> {
        let sessions = &self.sessions_table;
        let seq = seq as i64;
        match &self.pool {
            Pool::Sqlite(pool) => {
                let sql = format!("UPDATE {sessions} SET {column} = ? WHERE session_id = ?");
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(seq)
                    .bind(&self.session_id)
                    .execute(pool)
                    .await
                    .map_err(backend)?;
            }
            Pool::Postgres(pool) => {
                let sql = format!("UPDATE {sessions} SET {column} = $1 WHERE session_id = $2");
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(seq)
                    .bind(&self.session_id)
                    .execute(pool)
                    .await
                    .map_err(backend)?;
            }
            Pool::MySql(pool) => {
                let sql = format!("UPDATE {sessions} SET {column} = ? WHERE session_id = ?");
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(seq)
                    .bind(&self.session_id)
                    .execute(pool)
                    .await
                    .map_err(backend)?;
            }
        }
        Ok(())
    }
}

async fn connect_pool(url: &str, pool_cfg: &SqlPoolOptions) -> Result<Pool, StoreError> {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        let opts = PgConnectOptions::from_str(url).map_err(backend)?;
        let pool = apply_pool_options(PgPoolOptions::new(), pool_cfg)
            .connect_with(opts)
            .await
            .map_err(backend)?;
        Ok(Pool::Postgres(pool))
    } else if url.starts_with("mysql://") {
        let opts = MySqlConnectOptions::from_str(url).map_err(backend)?;
        let pool = apply_pool_options(MySqlPoolOptions::new(), pool_cfg)
            .connect_with(opts)
            .await
            .map_err(backend)?;
        Ok(Pool::MySql(pool))
    } else {
        let opts = SqliteConnectOptions::from_str(url)
            .map_err(backend)?
            .create_if_missing(true);
        let pool = apply_pool_options(SqlitePoolOptions::new(), pool_cfg)
            .connect_with(opts)
            .await
            .map_err(backend)?;
        Ok(Pool::Sqlite(pool))
    }
}

#[async_trait]
impl MessageStore for SqlStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        self.get_seq("sender").await
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        self.get_seq("target").await
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.set_seq("sender", seq).await
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.set_seq("target", seq).await
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        let messages = &self.messages_table;
        let seq_i = seq as i64;
        match &self.pool {
            Pool::Sqlite(pool) => {
                let sql = format!(
                    "INSERT OR REPLACE INTO {messages} (session_id, seq, data) VALUES (?, ?, ?)"
                );
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .bind(seq_i)
                    .bind(message)
                    .execute(pool)
                    .await
                    .map_err(backend)?;
            }
            Pool::Postgres(pool) => {
                let sql = format!(
                    "INSERT INTO {messages} (session_id, seq, data) VALUES ($1, $2, $3) \
                     ON CONFLICT (session_id, seq) DO UPDATE SET data = EXCLUDED.data"
                );
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .bind(seq_i)
                    .bind(message)
                    .execute(pool)
                    .await
                    .map_err(backend)?;
            }
            Pool::MySql(pool) => {
                let sql = format!(
                    "INSERT INTO {messages} (session_id, seq, data) VALUES (?, ?, ?) \
                     ON DUPLICATE KEY UPDATE data = VALUES(data)"
                );
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .bind(seq_i)
                    .bind(message)
                    .execute(pool)
                    .await
                    .map_err(backend)?;
            }
        }
        Ok(())
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        let messages = &self.messages_table;
        let (begin_i, end_i) = (begin as i64, end as i64);
        // Each backend's `fetch_all` yields a distinct concrete `Row` type (SqliteRow/PgRow/
        // MySqlRow), so each arm decodes its own rows directly rather than unifying into one
        // shared post-processing helper.
        match &self.pool {
            Pool::Sqlite(pool) => {
                let sql = format!(
                    "SELECT seq, data FROM {messages} WHERE session_id = ? AND seq BETWEEN ? AND ? ORDER BY seq"
                );
                let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .bind(begin_i)
                    .bind(end_i)
                    .fetch_all(pool)
                    .await
                    .map_err(backend)?;
                let mut out = Vec::with_capacity(rows.len());
                for row in rows {
                    let seq: i64 = row.try_get("seq").map_err(backend)?;
                    let data: Vec<u8> = row.try_get("data").map_err(backend)?;
                    out.push((seq as u64, data));
                }
                Ok(out)
            }
            Pool::Postgres(pool) => {
                let sql = format!(
                    "SELECT seq, data FROM {messages} WHERE session_id = $1 AND seq BETWEEN $2 AND $3 ORDER BY seq"
                );
                let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .bind(begin_i)
                    .bind(end_i)
                    .fetch_all(pool)
                    .await
                    .map_err(backend)?;
                let mut out = Vec::with_capacity(rows.len());
                for row in rows {
                    let seq: i64 = row.try_get("seq").map_err(backend)?;
                    let data: Vec<u8> = row.try_get("data").map_err(backend)?;
                    out.push((seq as u64, data));
                }
                Ok(out)
            }
            Pool::MySql(pool) => {
                let sql = format!(
                    "SELECT seq, data FROM {messages} WHERE session_id = ? AND seq BETWEEN ? AND ? ORDER BY seq"
                );
                let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .bind(begin_i)
                    .bind(end_i)
                    .fetch_all(pool)
                    .await
                    .map_err(backend)?;
                let mut out = Vec::with_capacity(rows.len());
                for row in rows {
                    let seq: i64 = row.try_get("seq").map_err(backend)?;
                    let data: Vec<u8> = row.try_get("data").map_err(backend)?;
                    out.push((seq as u64, data));
                }
                Ok(out)
            }
        }
    }
    async fn reset(&self) -> Result<(), StoreError> {
        let sessions = &self.sessions_table;
        let messages = &self.messages_table;
        match &self.pool {
            Pool::Sqlite(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "DELETE FROM {messages} WHERE session_id = ?"
                )))
                .bind(&self.session_id)
                .execute(pool)
                .await
                .map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE {sessions} SET sender = 1, target = 1, creation_time = ? WHERE session_id = ?"
                )))
                .bind(now_unix())
                .bind(&self.session_id)
                .execute(pool)
                .await
                .map_err(backend)?;
            }
            Pool::Postgres(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "DELETE FROM {messages} WHERE session_id = $1"
                )))
                .bind(&self.session_id)
                .execute(pool)
                .await
                .map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE {sessions} SET sender = 1, target = 1, creation_time = $1 WHERE session_id = $2"
                )))
                .bind(now_unix())
                .bind(&self.session_id)
                .execute(pool)
                .await
                .map_err(backend)?;
            }
            Pool::MySql(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "DELETE FROM {messages} WHERE session_id = ?"
                )))
                .bind(&self.session_id)
                .execute(pool)
                .await
                .map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE {sessions} SET sender = 1, target = 1, creation_time = ? WHERE session_id = ?"
                )))
                .bind(now_unix())
                .bind(&self.session_id)
                .execute(pool)
                .await
                .map_err(backend)?;
            }
        }
        Ok(())
    }
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        let sessions = &self.sessions_table;
        let secs: Option<i64> = match &self.pool {
            Pool::Sqlite(pool) => {
                let sql = format!("SELECT creation_time FROM {sessions} WHERE session_id = ?");
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .fetch_one(pool)
                    .await
                    .map_err(backend)?
                    .try_get("creation_time")
                    .map_err(backend)?
            }
            Pool::Postgres(pool) => {
                let sql = format!("SELECT creation_time FROM {sessions} WHERE session_id = $1");
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .fetch_one(pool)
                    .await
                    .map_err(backend)?
                    .try_get("creation_time")
                    .map_err(backend)?
            }
            Pool::MySql(pool) => {
                let sql = format!("SELECT creation_time FROM {sessions} WHERE session_id = ?");
                sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(&self.session_id)
                    .fetch_one(pool)
                    .await
                    .map_err(backend)?
                    .try_get("creation_time")
                    .map_err(backend)?
            }
        };
        Ok(secs.and_then(|s| time::OffsetDateTime::from_unix_timestamp(s).ok()))
    }
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        // GAP-39/FR-018 (feature 005): a single transaction per dialect, closing the crash window
        // between persisting the message and advancing the sender sequence.
        let messages = &self.messages_table;
        let sessions = &self.sessions_table;
        let seq_i = seq as i64;
        let next = seq_i + 1;
        match &self.pool {
            Pool::Sqlite(pool) => {
                let mut tx = pool.begin().await.map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT OR REPLACE INTO {messages} (session_id, seq, data) VALUES (?, ?, ?)"
                )))
                .bind(&self.session_id)
                .bind(seq_i)
                .bind(message)
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE {sessions} SET sender = ? WHERE session_id = ?"
                )))
                .bind(next)
                .bind(&self.session_id)
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
                tx.commit().await.map_err(backend)?;
            }
            Pool::Postgres(pool) => {
                let mut tx = pool.begin().await.map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT INTO {messages} (session_id, seq, data) VALUES ($1, $2, $3) \
                     ON CONFLICT (session_id, seq) DO UPDATE SET data = EXCLUDED.data"
                )))
                .bind(&self.session_id)
                .bind(seq_i)
                .bind(message)
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE {sessions} SET sender = $1 WHERE session_id = $2"
                )))
                .bind(next)
                .bind(&self.session_id)
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
                tx.commit().await.map_err(backend)?;
            }
            Pool::MySql(pool) => {
                let mut tx = pool.begin().await.map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT INTO {messages} (session_id, seq, data) VALUES (?, ?, ?) \
                     ON DUPLICATE KEY UPDATE data = VALUES(data)"
                )))
                .bind(&self.session_id)
                .bind(seq_i)
                .bind(message)
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE {sessions} SET sender = ? WHERE session_id = ?"
                )))
                .bind(next)
                .bind(&self.session_id)
                .execute(&mut *tx)
                .await
                .map_err(backend)?;
                tx.commit().await.map_err(backend)?;
            }
        }
        Ok(())
    }
}
