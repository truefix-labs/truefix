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

/// BUG-14/FR-017 (feature 006): ported from `sql.rs`'s identical function (duplicated, not
/// shared — `sql`/`mssql` are independently feature-gated modules, so `mssql` can compile
/// without the `sql` feature enabled). A conservative allowlist (ASCII alphanumeric/underscore,
/// starting with a letter or underscore, max 64 chars) for a table identifier that will be
/// interpolated directly into T-SQL via `format!`.
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

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
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

    // BUG-10/FR-019 (feature 006): the real QuickFIX/J MSSQL JDBC grammar is semicolon-delimited
    // properties with no path segment (`host[:port][;databaseName=X;user=Y;password=Z;...]`),
    // detected here by the presence of `;` — additive to, not a replacement for, the existing
    // TrueFix-native `user:password@host[:port]/database` shorthand parsed below.
    if rest.contains(';') {
        return parse_semicolon_form(rest);
    }

    // BUG-70/FR-030 (feature 007): split at the *last* '@', not the first -- a password
    // containing its own '@' character (e.g. `user:p@ssword@host/db`) would otherwise be
    // truncated, with the remainder wrongly folded into the host. The host portion of a valid URL
    // never itself contains '@', so the last occurrence unambiguously separates credentials from
    // host even when the password does.
    let (userinfo, hostpart) = rest
        .rsplit_once('@')
        .ok_or_else(|| backend("mssql URL must be user:password@host[:port]/database"))?;
    let (user, password) = userinfo
        .split_once(':')
        .ok_or_else(|| backend("mssql URL must include user:password"))?;
    let (hostport, database) = hostpart
        .split_once('/')
        .ok_or_else(|| backend("mssql URL must include /database"))?;
    let (host, port) = parse_host_port(hostport)?;

    let mut config = Config::new();
    config.host(host);
    config.port(port);
    config.database(database);
    config.authentication(AuthMethod::sql_server(
        percent_decode(user),
        percent_decode(password),
    ));
    Ok(config)
}

/// GAP-55/FR-020 (feature 006): decode the minimal set of reserved-character percent-escapes
/// `splice_credentials` (`truefix-config`) may have applied to `JdbcUser`/`JdbcPassword` before
/// splicing them into this URL — matching that function's encoder exactly (`%25`/`%40`/`%3A`/
/// `%2F`), plus any other well-formed `%XX` escape, so a credential round-trips correctly rather
/// than being passed to authentication as the literal escaped text.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        let escape = (b == b'%')
            .then(|| bytes.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escape {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_host_port(hostport: &str) -> Result<(&str, u16), StoreError> {
    match hostport.split_once(':') {
        Some((h, p)) => Ok((
            h,
            p.parse::<u16>()
                .map_err(|_| backend(format!("invalid port {p:?}")))?,
        )),
        None => Ok((hostport, 1433)),
    }
}

/// The real QuickFIX/J MSSQL JDBC grammar: `host[:port][;databaseName=X;user=Y;password=Z;...]`,
/// optionally still carrying `user:password@` ahead of the host (as `splice_credentials` would
/// produce when `JdbcUser`/`JdbcPassword` are configured alongside a bare semicolon-form
/// `JdbcURL`). Unrecognized properties (e.g. `encrypt=true`) are accepted and ignored.
fn parse_semicolon_form(rest: &str) -> Result<Config, StoreError> {
    let (authority, props_str) = rest
        .split_once(';')
        .ok_or_else(|| backend("expected `;`-delimited properties"))?;
    let (userinfo, hostport) = match authority.split_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, authority),
    };
    let (host, port) = parse_host_port(hostport)?;

    let mut database = None;
    let mut prop_user = None;
    let mut prop_password = None;
    for prop in props_str.split(';') {
        if prop.is_empty() {
            continue;
        }
        let (k, v) = prop
            .split_once('=')
            .ok_or_else(|| backend(format!("malformed property {prop:?}")))?;
        match k.trim().to_ascii_lowercase().as_str() {
            "databasename" => database = Some(v.to_owned()),
            "user" => prop_user = Some(v.to_owned()),
            "password" => prop_password = Some(v.to_owned()),
            _ => {} // ignore unrecognized properties
        }
    }
    let database =
        database.ok_or_else(|| backend("mssql URL must include a databaseName property"))?;
    let (user, password) = match userinfo {
        Some(ui) => {
            let (u, p) = ui
                .split_once(':')
                .ok_or_else(|| backend("mssql URL must include user:password"))?;
            (u.to_owned(), p.to_owned())
        }
        None => {
            let u = prop_user.ok_or_else(|| {
                backend("mssql URL must include a user property or user:password@")
            })?;
            let p = prop_password.ok_or_else(|| {
                backend("mssql URL must include a password property or user:password@")
            })?;
            (u, p)
        }
    };

    let mut config = Config::new();
    config.host(host);
    config.port(port);
    config.database(database);
    config.authentication(AuthMethod::sql_server(
        percent_decode(&user),
        percent_decode(&password),
    ));
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
    /// NEW-90 (feature 009): whether to skip TLS server-certificate-chain validation
    /// (`tiberius::Config::trust_cert()`). Defaults to `true` (today's unconditional behavior,
    /// preserved for backward compatibility) — set to `false` to enable real validation.
    pub trust_server_certificate: bool,
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
            trust_server_certificate: true,
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
        // BUG-14/FR-017 (feature 006): validate configuration-sourced table identifiers before
        // any query interpolates them, matching the identifier validation `SqlStore`/`SqlLog`/
        // `MssqlLog` already perform — `MssqlStore` was the one backend inconsistent with its
        // siblings, producing a raw `tiberius` SQL-syntax error on a typo'd name instead of a
        // clean, typed configuration error.
        valid_identifier(&config.sessions_table)?;
        valid_identifier(&config.messages_table)?;
        let mut tiberius_config = parse_url(&config.url)?;
        // NEW-90 (feature 009): `trust_cert()` unconditionally skipped TLS server-certificate-
        // chain validation with no way to opt back into it -- now gated on `trust_server_
        // certificate`, defaulting to `true` (today's behavior, backward compatible).
        if config.trust_server_certificate {
            tiberius_config.trust_cert();
        }
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
                     sender BIGINT NOT NULL, target BIGINT NOT NULL, creation_time BIGINT NULL)"
                ),
                &[],
            )
            .await
            .map_err(backend)?;
        // Best-effort add for tables created before this column existed (GAP-38/FR-017,
        // feature 005) — errors (most commonly "column already exists") are intentionally
        // swallowed; a freshly created table above already has the column.
        let _ = client
            .execute(
                format!("ALTER TABLE {sessions} ADD creation_time BIGINT NULL"),
                &[],
            )
            .await;
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
                     INSERT INTO {sessions} (session_id, sender, target, creation_time) \
                     VALUES (@P1, 1, 1, @P2)"
                ),
                &[&self.session_id, &now_unix()],
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
                format!(
                    "UPDATE {sessions} SET sender = 1, target = 1, creation_time = @P1 \
                     WHERE session_id = @P2"
                ),
                &[&now_unix(), &self.session_id],
            )
            .await
            .map_err(backend)?;
        Ok(())
    }
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        let sessions = &self.sessions_table;
        let mut client = self.client.lock().await;
        let row = client
            .query(
                format!("SELECT creation_time FROM {sessions} WHERE session_id = @P1"),
                &[&self.session_id],
            )
            .await
            .map_err(backend)?
            .into_row()
            .await
            .map_err(backend)?
            .ok_or_else(|| backend(format!("no row for session_id {:?}", self.session_id)))?;
        let secs: Option<i64> = row.get(0);
        Ok(secs.and_then(|s| time::OffsetDateTime::from_unix_timestamp(s).ok()))
    }
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        // GAP-39/FR-018 (feature 005): a real BEGIN/COMMIT transaction — the client is already
        // serialized behind this store's own mutex, but that alone doesn't survive a mid-sequence
        // crash; an explicit transaction does.
        let messages = &self.messages_table;
        let sessions = &self.sessions_table;
        let seq_i = seq as i64;
        let next = seq_i + 1;
        let mut client = self.client.lock().await;
        client
            .simple_query("BEGIN TRANSACTION")
            .await
            .map_err(backend)?;
        let result = async {
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
            client
                .execute(
                    format!("UPDATE {sessions} SET sender = @P1 WHERE session_id = @P2"),
                    &[&next, &self.session_id],
                )
                .await
                .map_err(backend)?;
            Ok::<(), StoreError>(())
        }
        .await;
        match result {
            Ok(()) => {
                // BUG-41 (feature 007): a failed COMMIT previously returned its error directly,
                // leaving the transaction open on this mutex-held connection -- every subsequent
                // operation on it would then be stuck behind an uncommitted, unrolled-back
                // transaction. Mirror the statement-failure branch below: attempt a rollback
                // before propagating the error. The commit error is converted to an owned
                // `String` immediately (rather than kept as the borrowing `QueryStream` result)
                // so the following `ROLLBACK` call can borrow `client` again.
                let commit_err = client
                    .simple_query("COMMIT TRANSACTION")
                    .await
                    .err()
                    .map(|e| e.to_string());
                match commit_err {
                    None => Ok(()),
                    Some(msg) => {
                        let _ = client.simple_query("ROLLBACK TRANSACTION").await;
                        Err(StoreError::Backend(msg))
                    }
                }
            }
            Err(e) => {
                let _ = client.simple_query("ROLLBACK TRANSACTION").await;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- T042 (US4, feature 006): real QuickFIX/J MSSQL JDBC grammar (BUG-10/FR-019) ---

    #[test]
    fn semicolon_form_with_user_password_properties_parses() {
        let config = parse_url(
            "sqlserver://localhost:1433;databaseName=quickfixj;user=sa;password=Secret1!",
        )
        .expect("real QuickFIX/J semicolon-delimited MSSQL URL should parse");
        // `tiberius::Config` doesn't expose getters for host/port/database/user directly, but a
        // successful parse (vs. the pre-fix raw split_once('@') panic-adjacent Err) is itself the
        // regression signal -- BUG-10 reported this exact URL shape failing to parse at all.
        let _ = config;
    }

    #[test]
    fn semicolon_form_without_port_defaults_to_1433() {
        parse_url("sqlserver://localhost;databaseName=quickfixj;user=sa;password=Secret1!")
            .expect("semicolon-form URL without an explicit port should default to 1433");
    }

    #[test]
    fn semicolon_form_with_spliced_credentials_ahead_of_host_also_parses() {
        // Matches what splice_credentials (truefix-config) produces when JdbcUser/JdbcPassword
        // are configured alongside a bare semicolon-form JdbcURL.
        parse_url("sqlserver://sa:Secret1!@localhost:1433;databaseName=quickfixj")
            .expect("semicolon-form URL with spliced user:password@ should also parse");
    }

    #[test]
    fn semicolon_form_missing_database_name_is_a_clean_error() {
        let err = match parse_url("sqlserver://localhost;user=sa;password=Secret1!") {
            Ok(_) => panic!("expected an error for a missing databaseName property"),
            Err(e) => e,
        };
        assert!(format!("{err}").contains("databaseName"));
    }

    #[test]
    fn path_based_shorthand_form_still_parses_unchanged() {
        parse_url("sqlserver://sa:Secret1!@localhost:1433/quickfixj")
            .expect("the existing TrueFix-native path-based shorthand must remain accepted");
    }
}
