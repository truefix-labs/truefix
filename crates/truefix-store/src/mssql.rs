//! MSSQL-backed message store, behind the `mssql` feature (FR-020).
//!
//! Independent of the `sql` feature (PostgreSQL/MySQL/SQLite via `sqlx`, which has no official
//! MSSQL support): MSSQL is reached via `tiberius`, a separate TDS driver, so this module can be
//! enabled on its own without pulling in `sqlx` at all. Behaves equivalently to [`crate::SqlStore`]
//! — same sequence-number/sent-message schema shape, differing only in the connection/query layer
//! (Constitution Principle IV-adjacent: one behavioral contract, multiple backends).
//!
//! A single `tiberius::Client` connection, serialized behind a mutex, rather than a real
//! connection pool (unlike the `sqlx`-backed [`crate::SqlStore`], which uses `sqlx`'s own pooling)
//! — `tiberius` has no built-in pool, and pulling in a separate pooling crate (e.g. `bb8-tiberius`)
//! for this one backend wasn't judged worth the added dependency surface. Fine for the
//! low-concurrency, one-write-at-a-time access pattern a FIX session's own store use draws (each
//! session's own store calls are already inherently sequential).

use std::net::SocketAddr;

use async_trait::async_trait;
use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::{MessageStore, StoreError};

fn backend<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Backend(e.to_string())
}

/// Parse an `mssql://user:password@host[:port]/database` (or `sqlserver://...`) URL into a
/// `tiberius::Config`. Port defaults to `1433` (the standard SQL Server port) when omitted.
/// Server-certificate validation is not performed (`trust_cert()`) — MSSQL's own TDS-level
/// encryption still protects the connection in transit; this only skips CA-chain validation,
/// matching how most containerized/dev SQL Server instances (no real CA-issued cert) are reached.
fn parse_url(url: &str) -> Result<Config, StoreError> {
    let rest = url
        .strip_prefix("mssql://")
        .or_else(|| url.strip_prefix("sqlserver://"))
        .ok_or_else(|| {
            backend(format!(
                "expected an mssql:// or sqlserver:// URL, got {url:?}"
            ))
        })?;
    let (userinfo, hostpart) = rest
        .split_once('@')
        .ok_or_else(|| backend("mssql URL must be user:password@host[:port]/database"))?;
    let (user, password) = userinfo
        .split_once(':')
        .ok_or_else(|| backend("mssql URL must include user:password"))?;
    let (hostport, database) = hostpart
        .split_once('/')
        .ok_or_else(|| backend("mssql URL must include /database"))?;
    let (host, port) = match hostport.split_once(':') {
        Some((h, p)) => (
            h,
            p.parse::<u16>()
                .map_err(|_| backend(format!("invalid port {p:?}")))?,
        ),
        None => (hostport, 1433),
    };

    let mut config = Config::new();
    config.host(host);
    config.port(port);
    config.database(database);
    config.authentication(AuthMethod::sql_server(user, password));
    config.trust_cert();
    Ok(config)
}

async fn connect_client(config: Config) -> Result<Client<Compat<TcpStream>>, StoreError> {
    let addr: SocketAddr = tokio::net::lookup_host(config.get_addr())
        .await
        .map_err(backend)?
        .next()
        .ok_or_else(|| backend(format!("could not resolve {}", config.get_addr())))?;
    let tcp = TcpStream::connect(addr).await.map_err(backend)?;
    tcp.set_nodelay(true).map_err(backend)?;
    Client::connect(config, tcp.compat_write())
        .await
        .map_err(backend)
}

/// Configuration for [`MssqlStore::connect_with_config`] — mirrors [`crate::SqlStoreConfig`]'s
/// shape (minus pool settings, since MSSQL access here is a single serialized connection, not a
/// pool; see this module's doc).
#[derive(Debug, Clone)]
pub struct MssqlStoreConfig {
    /// The database URL (`mssql://user:password@host[:port]/database`).
    pub url: String,
    /// Table holding the sender/target sequence numbers.
    pub sessions_table: String,
    /// Table holding sent message bodies.
    pub messages_table: String,
    /// The composite session key discriminating rows when several sessions share one table pair.
    pub session_id: String,
}

impl MssqlStoreConfig {
    /// A config using `url` with default table names (`seqnums`/`messages`) and a default
    /// `session_id` of `"default"` (single-session use).
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            sessions_table: "seqnums".to_owned(),
            messages_table: "messages".to_owned(),
            session_id: "default".to_owned(),
        }
    }
}

/// An MSSQL-backed store using `tiberius` (FR-020). Maps the same sequence-number/sent-message
/// schema shape as [`crate::SqlStore`], via T-SQL.
pub struct MssqlStore {
    client: Mutex<Client<Compat<TcpStream>>>,
    sessions_table: String,
    messages_table: String,
    session_id: String,
}

impl MssqlStore {
    /// Connect to `url` (`mssql://user:password@host[:port]/database`) with default table names
    /// (`seqnums`/`messages`) and `session_id` `"default"` (backward-compatible shorthand for
    /// [`Self::connect_with_config`]).
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        Self::connect_with_config(MssqlStoreConfig::new(url)).await
    }

    /// Connect using a full [`MssqlStoreConfig`].
    pub async fn connect_with_config(config: MssqlStoreConfig) -> Result<Self, StoreError> {
        let tiberius_config = parse_url(&config.url)?;
        let client = connect_client(tiberius_config).await?;
        let store = Self {
            client: Mutex::new(client),
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
        let mut client = self.client.lock().await;
        client
            .execute(
                format!(
                    "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='{sessions}' AND xtype='U') \
                     CREATE TABLE {sessions} (\
                     session_id NVARCHAR(255) NOT NULL PRIMARY KEY, \
                     sender BIGINT NOT NULL, target BIGINT NOT NULL)"
                ),
                &[],
            )
            .await
            .map_err(backend)?;
        client
            .execute(
                format!(
                    "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='{messages}' AND xtype='U') \
                     CREATE TABLE {messages} (\
                     session_id NVARCHAR(255) NOT NULL, seq BIGINT NOT NULL, data VARBINARY(MAX) NOT NULL, \
                     PRIMARY KEY (session_id, seq))"
                ),
                &[],
            )
            .await
            .map_err(backend)?;
        client
            .execute(
                format!(
                    "IF NOT EXISTS (SELECT 1 FROM {sessions} WHERE session_id = @P1) \
                     INSERT INTO {sessions} (session_id, sender, target) VALUES (@P1, 1, 1)"
                ),
                &[&self.session_id],
            )
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn get_seq(&self, column: &str) -> Result<u64, StoreError> {
        let sessions = &self.sessions_table;
        let mut client = self.client.lock().await;
        let row = client
            .query(
                format!("SELECT {column} FROM {sessions} WHERE session_id = @P1"),
                &[&self.session_id],
            )
            .await
            .map_err(backend)?
            .into_row()
            .await
            .map_err(backend)?
            .ok_or_else(|| backend(format!("no row for session_id {:?}", self.session_id)))?;
        let value: i64 = row
            .get(0)
            .ok_or_else(|| backend(format!("{column} was NULL")))?;
        Ok(value as u64)
    }

    async fn set_seq(&self, column: &str, seq: u64) -> Result<(), StoreError> {
        let sessions = &self.sessions_table;
        let seq_i = seq as i64;
        let mut client = self.client.lock().await;
        client
            .execute(
                format!("UPDATE {sessions} SET {column} = @P1 WHERE session_id = @P2"),
                &[&seq_i, &self.session_id],
            )
            .await
            .map_err(backend)?;
        Ok(())
    }
}

#[async_trait]
impl MessageStore for MssqlStore {
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
        let mut client = self.client.lock().await;
        client
            .execute(
                format!(
                    "IF EXISTS (SELECT 1 FROM {messages} WHERE session_id = @P1 AND seq = @P2) \
                     UPDATE {messages} SET data = @P3 WHERE session_id = @P1 AND seq = @P2 \
                     ELSE INSERT INTO {messages} (session_id, seq, data) VALUES (@P1, @P2, @P3)"
                ),
                &[&self.session_id, &seq_i, &message],
            )
            .await
            .map_err(backend)?;
        Ok(())
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        let messages = &self.messages_table;
        let (begin_i, end_i) = (begin as i64, end as i64);
        let mut client = self.client.lock().await;
        let rows = client
            .query(
                format!(
                    "SELECT seq, data FROM {messages} WHERE session_id = @P1 AND seq BETWEEN @P2 AND @P3 ORDER BY seq"
                ),
                &[&self.session_id, &begin_i, &end_i],
            )
            .await
            .map_err(backend)?
            .into_first_result()
            .await
            .map_err(backend)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let seq: i64 = row.get(0).ok_or_else(|| backend("seq was NULL"))?;
            let data: &[u8] = row.get(1).ok_or_else(|| backend("data was NULL"))?;
            out.push((seq as u64, data.to_vec()));
        }
        Ok(out)
    }
    async fn reset(&self) -> Result<(), StoreError> {
        let sessions = &self.sessions_table;
        let messages = &self.messages_table;
        let mut client = self.client.lock().await;
        client
            .execute(
                format!("DELETE FROM {messages} WHERE session_id = @P1"),
                &[&self.session_id],
            )
            .await
            .map_err(backend)?;
        client
            .execute(
                format!("UPDATE {sessions} SET sender = 1, target = 1 WHERE session_id = @P1"),
                &[&self.session_id],
            )
            .await
            .map_err(backend)?;
        Ok(())
    }
}
