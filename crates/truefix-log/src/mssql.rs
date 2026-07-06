//! MSSQL-backed log, behind the `mssql` feature (FR-020).
//!
//! Independent of the `sql` feature (PostgreSQL/MySQL/SQLite via `sqlx`, which has no official
//! MSSQL support): reached via `tiberius`, a separate TDS driver, mirroring
//! `truefix_store::MssqlStore`. Like [`crate::SqlLog`], the [`Log`] trait is synchronous and
//! infallible while `tiberius` is async: each entry is pushed onto an unbounded channel (a
//! non-blocking, sync send) and written by a background task holding the single `tiberius`
//! connection (no pooling — see `truefix-store`'s `MssqlStore` doc for why that's an accepted
//! simplification here).

use std::net::SocketAddr;

use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::{Log, LogError, is_heartbeat};

fn io_err<E: std::fmt::Display>(e: E) -> LogError {
    LogError::Io(e.to_string())
}

/// Parse an `mssql://user:password@host[:port]/database` (or `sqlserver://...`) URL. See
/// `truefix_store::MssqlStore`'s `parse_url` (duplicated here rather than shared, since
/// `truefix-log` doesn't depend on `truefix-store` — siblings in the layering).
///
/// NEW-09 (feature 009): ported the semicolon-delimited JDBC-URL form (BUG-10/FR-019) and
/// percent-decoded credentials (GAP-55/FR-020) from `truefix-store/src/mssql.rs`'s `parse_url` —
/// this copy previously had neither, an older/simpler version missing two entire features the
/// store version has.
fn parse_url(url: &str) -> Result<Config, LogError> {
    let rest = url
        .strip_prefix("mssql://")
        .or_else(|| url.strip_prefix("sqlserver://"))
        .ok_or_else(|| {
            io_err(format!(
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

    // BUG-70/FR-030 (feature 007): split at the *last* '@', not the first -- see the identical
    // fix in `truefix-store/src/mssql.rs`'s `parse_url` for the full rationale.
    let (userinfo, hostpart) = rest
        .rsplit_once('@')
        .ok_or_else(|| io_err("mssql URL must be user:password@host[:port]/database"))?;
    let (user, password) = userinfo
        .split_once(':')
        .ok_or_else(|| io_err("mssql URL must include user:password"))?;
    let (hostport, database) = hostpart
        .split_once('/')
        .ok_or_else(|| io_err("mssql URL must include /database"))?;
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

fn parse_host_port(hostport: &str) -> Result<(&str, u16), LogError> {
    match hostport.split_once(':') {
        Some((h, p)) => Ok((
            h,
            p.parse::<u16>()
                .map_err(|_| io_err(format!("invalid port {p:?}")))?,
        )),
        None => Ok((hostport, 1433)),
    }
}

/// The real QuickFIX/J MSSQL JDBC grammar: `host[:port][;databaseName=X;user=Y;password=Z;...]`,
/// optionally still carrying `user:password@` ahead of the host (as `splice_credentials` would
/// produce when `JdbcUser`/`JdbcPassword` are configured alongside a bare semicolon-form
/// `JdbcURL`). Unrecognized properties (e.g. `encrypt=true`) are accepted and ignored.
fn parse_semicolon_form(rest: &str) -> Result<Config, LogError> {
    let (authority, props_str) = rest
        .split_once(';')
        .ok_or_else(|| io_err("expected `;`-delimited properties"))?;
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
            .ok_or_else(|| io_err(format!("malformed property {prop:?}")))?;
        match k.trim().to_ascii_lowercase().as_str() {
            "databasename" => database = Some(v.to_owned()),
            "user" => prop_user = Some(v.to_owned()),
            "password" => prop_password = Some(v.to_owned()),
            _ => {} // ignore unrecognized properties
        }
    }
    let database =
        database.ok_or_else(|| io_err("mssql URL must include a databaseName property"))?;
    let (user, password) = match userinfo {
        Some(ui) => {
            let (u, p) = ui
                .split_once(':')
                .ok_or_else(|| io_err("mssql URL must include user:password"))?;
            (u.to_owned(), p.to_owned())
        }
        None => {
            let u = prop_user
                .ok_or_else(|| io_err("mssql URL must include a user property or user:password@"))?;
            let p = prop_password.ok_or_else(|| {
                io_err("mssql URL must include a password property or user:password@")
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

async fn connect_client(config: Config) -> Result<Client<Compat<TcpStream>>, LogError> {
    let addr: SocketAddr = tokio::net::lookup_host(config.get_addr())
        .await
        .map_err(io_err)?
        .next()
        .ok_or_else(|| io_err(format!("could not resolve {}", config.get_addr())))?;
    let tcp = TcpStream::connect(addr).await.map_err(io_err)?;
    tcp.set_nodelay(true).map_err(io_err)?;
    Client::connect(config, tcp.compat_write())
        .await
        .map_err(io_err)
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
        Err(io_err(format!("{s:?} is not a valid SQL table identifier")))
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

async fn ensure_table(client: &mut Client<Compat<TcpStream>>, table: &str) -> Result<(), LogError> {
    // GAP-41/FR-019 (feature 005): `logged_at`/`session_id` widen every row, mirroring `SqlLog`.
    client
        .execute(
            format!(
                "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='{table}' AND xtype='U') \
                 CREATE TABLE {table} (id BIGINT IDENTITY(1,1) PRIMARY KEY, text NVARCHAR(MAX) NOT NULL, \
                 logged_at BIGINT NULL, session_id NVARCHAR(255) NULL)"
            ),
            &[],
        )
        .await
        .map_err(io_err)?;
    // Best-effort add for a table created before these columns existed; swallowed on error
    // (most commonly "column already exists").
    let _ = client
        .execute(
            format!("ALTER TABLE {table} ADD logged_at BIGINT NULL"),
            &[],
        )
        .await;
    let _ = client
        .execute(
            format!("ALTER TABLE {table} ADD session_id NVARCHAR(255) NULL"),
            &[],
        )
        .await;
    Ok(())
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

async fn insert_text(
    client: &mut Client<Compat<TcpStream>>,
    table: &str,
    session_id: &str,
    text: &str,
) {
    // Best-effort: a write failure drops the entry rather than failing the session (matches the
    // synchronous, infallible `Log` trait contract, same as `SqlLog`'s `insert_text`).
    let _ = client
        .execute(
            format!("INSERT INTO {table} (text, logged_at, session_id) VALUES (@P1, @P2, @P3)"),
            &[&text, &now_unix(), &session_id],
        )
        .await;
}

/// Configuration for [`MssqlLog::connect_with_config`] (FR-020/024).
#[derive(Debug, Clone)]
pub struct MssqlLogConfig {
    /// The database URL (`mssql://user:password@host[:port]/database`).
    pub url: String,
    /// `JdbcLogIncomingTable`.
    pub incoming_table: String,
    /// `JdbcLogOutgoingTable`.
    pub outgoing_table: String,
    /// `JdbcLogEventTable`.
    pub event_table: String,
    /// `JdbcLogHeartBeats`: whether Heartbeat (`35=0`) messages are persisted.
    pub include_heartbeats: bool,
    /// The session identity stamped onto every row's `session_id` column (GAP-41/FR-019,
    /// feature 005). See `SqlLogConfig::session_id`'s doc for the rationale.
    pub session_id: String,
    /// NEW-90 (feature 009): whether to skip TLS server-certificate-chain validation
    /// (`tiberius::Config::trust_cert()`). Defaults to `true` (today's unconditional behavior,
    /// preserved for backward compatibility) — set to `false` to enable real validation.
    pub trust_server_certificate: bool,
}

impl MssqlLogConfig {
    /// A config using `url` with default table names and `session_id` `"default"`.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            incoming_table: "log_incoming".to_owned(),
            outgoing_table: "log_outgoing".to_owned(),
            event_table: "log_event".to_owned(),
            include_heartbeats: true,
            session_id: "default".to_owned(),
            trust_server_certificate: true,
        }
    }
}

/// An MSSQL-backed log writing to configurable incoming/outgoing/event tables via a background
/// task (FR-020).
pub struct MssqlLog {
    tx: mpsc::UnboundedSender<Entry>,
    include_heartbeats: bool,
}

impl MssqlLog {
    /// Connect to `url` with default table names (backward-compatible shorthand for
    /// [`Self::connect_with_config`]).
    pub async fn connect(url: &str) -> Result<Self, LogError> {
        Self::connect_with_config(MssqlLogConfig::new(url)).await
    }

    /// Connect using a full [`MssqlLogConfig`]: ensures the schema and spawns the background
    /// writer holding the single `tiberius` connection.
    pub async fn connect_with_config(config: MssqlLogConfig) -> Result<Self, LogError> {
        valid_identifier(&config.incoming_table)?;
        valid_identifier(&config.outgoing_table)?;
        valid_identifier(&config.event_table)?;
        let mut tiberius_config = parse_url(&config.url)?;
        // NEW-90 (feature 009): see the identical note in `truefix-store/src/mssql.rs`.
        if config.trust_server_certificate {
            tiberius_config.trust_cert();
        }
        let mut client = connect_client(tiberius_config).await?;
        ensure_table(&mut client, &config.incoming_table).await?;
        ensure_table(&mut client, &config.outgoing_table).await?;
        ensure_table(&mut client, &config.event_table).await?;

        let (tx, mut rx) = mpsc::unbounded_channel::<Entry>();
        let incoming_table = config.incoming_table;
        let outgoing_table = config.outgoing_table;
        let event_table = config.event_table;
        let session_id = config.session_id;
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                match entry {
                    Entry::Message { direction, text } => {
                        let table = if direction == "I" {
                            &incoming_table
                        } else {
                            &outgoing_table
                        };
                        insert_text(&mut client, table, &session_id, &text).await;
                    }
                    Entry::Event { text } => {
                        insert_text(&mut client, &event_table, &session_id, &text).await;
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

impl Log for MssqlLog {
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

#[cfg(test)]
mod percent_decode_tests {
    use super::*;

    // NEW-09 (feature 009): direct unit coverage for the ported `percent_decode` -- the
    // integration tests in `mssql_url_parsing_parity.rs` can only observe host reachability, not
    // decoded credential content.
    #[test]
    fn decodes_the_reserved_character_escapes() {
        assert_eq!(percent_decode("p%40ss"), "p@ss");
        assert_eq!(percent_decode("user%3Aname"), "user:name");
        assert_eq!(percent_decode("a%2Fb"), "a/b");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn parses_the_semicolon_form_with_user_password_properties() {
        let config = parse_url("mssql://127.0.0.1;databaseName=db;user=sa;password=p%40ss")
            .expect("should parse");
        assert_eq!(config.get_addr(), "127.0.0.1:1433");
    }
}
