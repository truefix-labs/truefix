//! Map parsed [`SessionSettings`] into runnable engine configuration (FR-013/014/015).
//!
//! Resolution is all-or-nothing: the first invalid/missing key aborts with a typed [`ConfigError`]
//! naming the key and session, so no partially-configured engine is produced.

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::time::Duration;

use time::Weekday;
use truefix_session::{Role, Schedule, SessionConfig, TimeStampPrecision};
use truefix_store::StoreConfig;

use crate::{ConfigError, SessionSettings};

/// Whether a session initiates or accepts connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    /// Listens for inbound connections.
    Acceptor,
    /// Connects out to a peer.
    Initiator,
}

/// A fully-resolved per-session configuration ready to start.
#[derive(Debug, Clone)]
pub struct ResolvedSession {
    /// The session-layer configuration.
    pub session: SessionConfig,
    /// Initiator or acceptor.
    pub connection: ConnectionType,
    /// The accept (bind) or connect (target) address.
    pub address: SocketAddr,
    /// The message store to build for this session.
    pub store: StoreConfig,
    /// TLS material and settings, present when `SocketUseSSL=Y` (FR-017).
    pub tls: Option<TlsSpec>,
    /// Socket-level tuning options (FR-019).
    pub socket_options: SocketOptionsSpec,
    /// Additional backup endpoints (initiator only), in failover order, parsed from numbered
    /// `SocketConnectHost<N>`/`SocketConnectPort<N>` keys (FR-019). Empty when no backups are
    /// configured.
    pub failover_addresses: Vec<SocketAddr>,
    /// A file-backed log to build for this session, present when `FileLogPath` is set (FR-026).
    pub log: Option<LogSpec>,
    /// A SQL-backed log to build for this session, present when `JdbcURL` is set (US3, feature 004,
    /// FR-003). Mutually exclusive with [`log`](Self::log) — see `resolve_log`'s doc.
    pub sql_log: Option<SqlLogSpec>,
    /// Forward-proxy configuration for an initiator connection, present when `ProxyType` is set
    /// (US12, FR-016).
    pub proxy: Option<ProxySpec>,
    /// PROXY protocol (v1/v2) is trusted only from these physical source addresses (US12,
    /// FR-015); empty when `UseTCPProxy` is unset/`N`, or `TrustedProxyAddresses` is absent.
    pub trusted_proxy_addresses: Vec<IpAddr>,
    /// `SocketSynchronousWrites` + `SocketSynchronousWriteTimeout` (US12, FR-017): present when
    /// synchronous writes are enabled with a configured timeout.
    pub sync_write_timeout: Option<Duration>,
    /// The inbound dictionary validator, present when `UseDataDictionary=Y` (US2, feature 004,
    /// FR-002). `None` (the default) means no dictionary validation is wired, matching today's
    /// behavior.
    pub validator: Option<(
        truefix_dict::DataDictionary,
        truefix_dict::ValidationOptions,
    )>,
}

/// Forward-proxy configuration for an initiator connection, resolved from configuration (US12,
/// FR-016). Data-only mirror of `truefix_transport::ProxyConfig`; the mechanism that performs the
/// SOCKS4/SOCKS5/HTTP-CONNECT handshake lives in `truefix-transport`, which depends on this crate
/// for the shared spec shape (same pattern as [`TlsSpec`]/[`SocketOptionsSpec`]).
#[derive(Debug, Clone)]
pub struct ProxySpec {
    /// Which proxy protocol to speak. Maps from `ProxyType`.
    pub proxy_type: ProxyKind,
    /// The proxy server's host. Maps from `ProxyHost`.
    pub host: String,
    /// The proxy server's port. Maps from `ProxyPort`.
    pub port: u16,
    /// Optional username (SOCKS5 password auth, or SOCKS4 user ID). Maps from `ProxyUser`.
    pub username: Option<String>,
    /// Optional password (SOCKS5 password auth only). Maps from `ProxyPassword`.
    pub password: Option<String>,
}

/// Which forward-proxy protocol to use (`ProxyType`; US12, FR-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyKind {
    /// SOCKS4 (optionally with a user ID).
    Socks4,
    /// SOCKS5 (optionally with username/password authentication).
    Socks5,
    /// HTTP CONNECT.
    HttpConnect,
}

/// A file-backed log resolved from configuration (FR-026). Data-only mirror of
/// `truefix_log::FileLogOptions`; the mechanism that builds a live `Log` from this lives in
/// `truefix-log`, which the facade (already depending on both crates) converts through.
#[derive(Debug, Clone)]
pub struct LogSpec {
    /// Directory holding `messages.log`/`event.log`. Maps from `FileLogPath`.
    pub dir: PathBuf,
    /// `FileLogHeartbeats`.
    pub include_heartbeats: bool,
    /// `FileIncludeTimeStampForMessages`.
    pub include_timestamp: bool,
    /// `FileIncludeMilliseconds`.
    pub include_milliseconds: bool,
}

/// A SQL-backed log resolved from configuration (US3, feature 004, FR-003). Data-only mirror of
/// `truefix_log::SqlLogConfig`/`MssqlLogConfig`; independent of, and mutually exclusive with,
/// [`LogSpec`] (File-log) — see `resolve_log`'s doc for the precedence rule when both `JdbcURL` and
/// `FileLogPath` are present. The mechanism that builds a live `Log` from this (dispatched by
/// `url`'s scheme, same as the store side) lives in the facade.
#[derive(Debug, Clone)]
pub struct SqlLogSpec {
    /// The `JdbcURL` value; scheme carries backend selection (SQL vs MSSQL).
    pub url: String,
    /// `JdbcLogIncomingTable`, default `"log_incoming"` (matching `SqlLogConfig::new`'s default).
    pub incoming_table: String,
    /// `JdbcLogOutgoingTable`, default `"log_outgoing"`.
    pub outgoing_table: String,
    /// `JdbcLogEventTable`, default `"log_event"`.
    pub event_table: String,
    /// `JdbcLogHeartBeats`, default `true`.
    pub include_heartbeats: bool,
}

/// Socket-level tuning options resolved from configuration (FR-019). Data-only mirror of
/// `truefix_transport::SocketOptions`; the mechanism that applies these to a live socket (via
/// `socket2`) lives in `truefix-transport`, which depends on this crate for the shared shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketOptionsSpec {
    /// `TCP_NODELAY`. Maps from `SocketTcpNoDelay`.
    pub tcp_no_delay: bool,
    /// `SO_KEEPALIVE`. Maps from `SocketKeepAlive`.
    pub keep_alive: bool,
    /// `SO_REUSEADDR` (acceptor bind only). Maps from `SocketReuseAddress`.
    pub reuse_address: bool,
    /// `SO_LINGER` timeout. Maps from `SocketLinger` (seconds; absent/negative = disabled).
    pub linger: Option<Duration>,
    /// `SO_OOBINLINE`. Maps from `SocketOobInline`.
    pub oob_inline: bool,
    /// `SO_RCVBUF` size in bytes. Maps from `SocketReceiveBufferSize`.
    pub recv_buffer_size: Option<usize>,
    /// `SO_SNDBUF` size in bytes. Maps from `SocketSendBufferSize`.
    pub send_buffer_size: Option<usize>,
    /// `IP_TOS` traffic class. Maps from `SocketTrafficClass`.
    pub traffic_class: Option<u32>,
}

impl Default for SocketOptionsSpec {
    fn default() -> Self {
        Self {
            tcp_no_delay: true,
            keep_alive: false,
            reuse_address: false,
            linger: None,
            oob_inline: false,
            recv_buffer_size: None,
            send_buffer_size: None,
            traffic_class: None,
        }
    }
}

/// TLS material and settings resolved from configuration (FR-017). The mechanism that consumes
/// this (building `rustls::{Server,Client}Config`) lives in `truefix-transport`, which depends on
/// this crate for the shared spec shape.
#[derive(Debug, Clone)]
pub struct TlsSpec {
    /// PEM file containing the certificate chain and private key (combined). Maps from
    /// `SocketKeyStore`. `None` when [`key_store_bytes`](Self::key_store_bytes) is set instead —
    /// exactly one of the two must be present.
    pub key_store_path: Option<PathBuf>,
    /// Inline PEM bytes containing the certificate chain and private key (combined), in place of
    /// a file path (US12, FR-017). Maps from `SocketKeyStoreBytes`.
    pub key_store_bytes: Option<Vec<u8>>,
    /// PEM file of CA certificates (trust roots) used to verify the peer. Maps from
    /// `SocketTrustStore`.
    pub trust_store_path: Option<PathBuf>,
    /// Inline PEM bytes of CA certificates, in place of a file path (US12, FR-017). Maps from
    /// `SocketTrustStoreBytes`.
    pub trust_store_bytes: Option<Vec<u8>>,
    /// Require and verify a client certificate (mTLS; server side). Maps from `NeedClientAuth`.
    pub need_client_auth: bool,
    /// Minimum TLS protocol version to accept/offer. Maps from `EnabledProtocols`.
    pub min_version: Option<TlsVersion>,
    /// SNI server name an initiator presents. Maps from `SNIHostName` (when `UseSNI=Y`).
    pub server_name: Option<String>,
    /// Restrict the TLS handshake to these cipher suites (US12, FR-017); empty preserves rustls's
    /// default suite set. Maps from `CipherSuites` (comma-separated names, e.g.
    /// `TLS13_AES_128_GCM_SHA256`).
    pub cipher_suites: Vec<String>,
}

/// A minimum TLS protocol version (`EnabledProtocols`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    /// TLS 1.2 (and above).
    Tls12,
    /// TLS 1.3 only.
    Tls13,
}

/// [`SessionSettings::resolve_lenient`]'s return shape: the successfully-resolved sessions, plus
/// `(session label, error)` pairs for any sessions skipped via `ContinueInitializationOnError=Y`
/// (US4, feature 004, FR-005).
pub type LenientResolve = (Vec<ResolvedSession>, Vec<(String, ConfigError)>);

impl SessionSettings {
    /// Resolve every `[SESSION]` into a runnable [`ResolvedSession`] (FR-013/014). All-or-nothing:
    /// the first invalid or missing key aborts with a typed [`ConfigError`] (FR-015).
    pub fn resolve(&self) -> Result<Vec<ResolvedSession>, ConfigError> {
        self.sessions()
            .iter()
            .enumerate()
            .map(|(i, map)| resolve_one(map, i))
            .collect()
    }

    /// As [`Self::resolve`], but tolerates a resolution failure for a session whose own
    /// `ContinueInitializationOnError=Y` (US4, feature 004, FR-005) — that session's error is
    /// collected into the second return value (labeled by `SenderCompID->TargetCompID`) instead of
    /// aborting the whole resolve. A session without the flag set still aborts the *entire* resolve
    /// on its first error, exactly as [`Self::resolve`] does — this is deliberately per-session
    /// opt-in, not an engine-wide override (the flag is read directly from that session's own raw
    /// `.cfg` map, since `ContinueInitializationOnError` itself is a simple, never-failing boolean
    /// key — readable even when *other* keys in the same session are broken).
    pub fn resolve_lenient(&self) -> Result<LenientResolve, ConfigError> {
        let mut resolved = Vec::new();
        let mut skipped = Vec::new();
        for (i, map) in self.sessions().iter().enumerate() {
            match resolve_one(map, i) {
                Ok(rs) => resolved.push(rs),
                Err(err) => {
                    if bool_key(map, "ContinueInitializationOnError", false) {
                        skipped.push((label(map, i), err));
                    } else {
                        return Err(err);
                    }
                }
            }
        }
        Ok((resolved, skipped))
    }
}

type Map = BTreeMap<String, String>;

fn label(map: &Map, index: usize) -> String {
    match (map.get("SenderCompID"), map.get("TargetCompID")) {
        (Some(s), Some(t)) => format!("{s}->{t}"),
        _ => format!("#{index}"),
    }
}

fn required<'a>(map: &'a Map, key: &str, session: &str) -> Result<&'a str, ConfigError> {
    map.get(key)
        .map(String::as_str)
        .ok_or_else(|| ConfigError::MissingRequired {
            key: key.to_owned(),
            session: session.to_owned(),
        })
}

fn u32_key(map: &Map, key: &str, session: &str, default: u32) -> Result<u32, ConfigError> {
    match map.get(key) {
        None => Ok(default),
        Some(v) => v.parse().map_err(|_| ConfigError::InvalidValue {
            key: key.to_owned(),
            session: session.to_owned(),
            reason: format!("expected an integer, got {v:?}"),
        }),
    }
}

/// A `Y`/`N` boolean (case-insensitive); absent → `default`.
fn bool_key(map: &Map, key: &str, default: bool) -> bool {
    map.get(key)
        .map(|v| v.eq_ignore_ascii_case("Y"))
        .unwrap_or(default)
}

/// `LogonTag=<tag>=<value>` (e.g. `LogonTag=9001=HOUSE-ID`); absent → `None`.
fn resolve_logon_tag(map: &Map, session: &str) -> Result<Option<(u32, String)>, ConfigError> {
    let Some(raw) = map.get("LogonTag") else {
        return Ok(None);
    };
    let (tag, value) = raw
        .split_once('=')
        .ok_or_else(|| ConfigError::InvalidValue {
            key: "LogonTag".to_owned(),
            session: session.to_owned(),
            reason: format!("expected `<tag>=<value>`, got {raw:?}"),
        })?;
    let tag: u32 = tag.parse().map_err(|_| ConfigError::InvalidValue {
        key: "LogonTag".to_owned(),
        session: session.to_owned(),
        reason: format!("expected an integer tag, got {tag:?}"),
    })?;
    Ok(Some((tag, value.to_owned())))
}

fn f64_key(map: &Map, key: &str, session: &str, default: f64) -> Result<f64, ConfigError> {
    match map.get(key) {
        None => Ok(default),
        Some(v) => v.parse().map_err(|_| ConfigError::InvalidValue {
            key: key.to_owned(),
            session: session.to_owned(),
            reason: format!("expected a number, got {v:?}"),
        }),
    }
}

fn usize_key(
    map: &Map,
    key: &str,
    session: &str,
    default: Option<usize>,
) -> Result<Option<usize>, ConfigError> {
    match map.get(key) {
        None => Ok(default),
        Some(v) => v.parse().map(Some).map_err(|_| ConfigError::InvalidValue {
            key: key.to_owned(),
            session: session.to_owned(),
            reason: format!("expected a non-negative integer, got {v:?}"),
        }),
    }
}

/// `SocketLinger` (seconds); a negative value means "disabled" (`None`), matching QuickFIX/J.
fn linger_key(map: &Map, key: &str, session: &str) -> Result<Option<Duration>, ConfigError> {
    match map.get(key) {
        None => Ok(None),
        Some(v) => {
            let secs: i64 = v.parse().map_err(|_| ConfigError::InvalidValue {
                key: key.to_owned(),
                session: session.to_owned(),
                reason: format!("expected an integer number of seconds, got {v:?}"),
            })?;
            Ok((secs >= 0).then(|| Duration::from_secs(secs as u64)))
        }
    }
}

fn precision_key(
    map: &Map,
    key: &str,
    session: &str,
    default: TimeStampPrecision,
) -> Result<TimeStampPrecision, ConfigError> {
    match map.get(key).map(String::as_str) {
        None => Ok(default),
        Some("SECONDS") => Ok(TimeStampPrecision::Seconds),
        Some("MILLIS") => Ok(TimeStampPrecision::Milliseconds),
        Some("MICROS") => Ok(TimeStampPrecision::Microseconds),
        Some("NANOS") => Ok(TimeStampPrecision::Nanoseconds),
        Some(other) => Err(ConfigError::InvalidValue {
            key: key.to_owned(),
            session: session.to_owned(),
            reason: format!("expected SECONDS/MILLIS/MICROS/NANOS, got {other:?}"),
        }),
    }
}

fn resolve_one(map: &Map, index: usize) -> Result<ResolvedSession, ConfigError> {
    let session = label(map, index);
    let begin_string = required(map, "BeginString", &session)?.to_owned();
    let sender = required(map, "SenderCompID", &session)?.to_owned();
    let target = required(map, "TargetCompID", &session)?.to_owned();

    let (connection, role) = match required(map, "ConnectionType", &session)?
        .to_ascii_lowercase()
        .as_str()
    {
        "acceptor" => (ConnectionType::Acceptor, Role::Acceptor),
        "initiator" => (ConnectionType::Initiator, Role::Initiator),
        other => {
            return Err(ConfigError::UnknownConnectionType {
                session,
                value: other.to_owned(),
            })
        }
    };

    let mut cfg = SessionConfig::new(begin_string, sender, target, role);
    cfg.heartbeat_interval = u32_key(map, "HeartBtInt", &session, 30)?;
    cfg.reset_on_logon = bool_key(map, "ResetOnLogon", cfg.reset_on_logon);
    cfg.reset_on_logout = bool_key(map, "ResetOnLogout", false);
    cfg.reset_on_disconnect = bool_key(map, "ResetOnDisconnect", false);
    cfg.persist_messages = bool_key(map, "PersistMessages", true);
    cfg.check_latency = bool_key(map, "CheckLatency", true);
    cfg.max_latency = u32_key(map, "MaxLatency", &session, 120)?;
    cfg.logon_timeout = u32_key(map, "LogonTimeout", &session, 10)?;
    cfg.logout_timeout = u32_key(map, "LogoutTimeout", &session, 10)?;
    cfg.reconnect_interval = u32_key(map, "ReconnectInterval", &session, 30)?;
    cfg.resend_request_chunk_size = u32_key(map, "ResendRequestChunkSize", &session, 0)?;
    cfg.enable_last_msg_seq_num_processed = bool_key(map, "EnableLastMsgSeqNumProcessed", false);
    cfg.enable_next_expected_msg_seq_num = bool_key(map, "EnableNextExpectedMsgSeqNum", false);
    cfg.check_comp_id = bool_key(map, "CheckCompID", cfg.check_comp_id);
    cfg.reject_garbled_message = bool_key(map, "RejectGarbledMessage", cfg.reject_garbled_message);
    cfg.heartbeat_timeout_multiplier = u32_key(
        map,
        "HeartBeatTimeoutMultiplier",
        &session,
        cfg.heartbeat_timeout_multiplier,
    )?;
    cfg.test_request_delay_multiplier = f64_key(
        map,
        "TestRequestDelayMultiplier",
        &session,
        cfg.test_request_delay_multiplier,
    )?;
    cfg.timestamp_precision =
        precision_key(map, "TimeStampPrecision", &session, cfg.timestamp_precision)?;
    cfg.schedule = resolve_schedule(map, &session)?;
    cfg.send_redundant_resend_requests = bool_key(map, "SendRedundantResendRequests", false);
    cfg.reset_on_error = bool_key(map, "ResetOnError", false);
    cfg.disconnect_on_error = bool_key(map, "DisconnectOnError", false);
    cfg.disable_heart_beat_check = bool_key(map, "DisableHeartBeatCheck", false);
    cfg.refresh_on_logon = bool_key(map, "RefreshOnLogon", false);
    cfg.force_resend_when_corrupted_store = bool_key(map, "ForceResendWhenCorruptedStore", false);
    cfg.continue_initialization_on_error = bool_key(map, "ContinueInitializationOnError", false);
    cfg.logon_tag = resolve_logon_tag(map, &session)?;
    cfg.in_chan_capacity = usize_key(map, "InChanCapacity", &session, None)?;

    let address = resolve_address(map, connection, &session)?;
    let store = resolve_store(map, &session)?;
    let tls = resolve_tls(map, &session)?;
    let socket_options = resolve_socket_options(map, &session)?;
    let failover_addresses = match connection {
        ConnectionType::Initiator => resolve_failover_addresses(map, &session)?,
        ConnectionType::Acceptor => Vec::new(),
    };
    let (log, sql_log) = resolve_log(map, &session);
    let proxy = resolve_proxy(map, &session)?;
    let trusted_proxy_addresses = resolve_trusted_proxy_addresses(map, &session)?;
    let sync_write_timeout = resolve_sync_write_timeout(map, &session)?;
    let validator = resolve_validator(map, &session)?;

    Ok(ResolvedSession {
        session: cfg,
        connection,
        address,
        store,
        tls,
        socket_options,
        failover_addresses,
        log,
        sql_log,
        proxy,
        trusted_proxy_addresses,
        sync_write_timeout,
        validator,
    })
}

/// Resolve `UseDataDictionary` + `DataDictionary`/`AppDataDictionary`/`TransportDataDictionary` +
/// the five already-implemented `ValidationOptions` toggles into a validator (US2, feature 004,
/// FR-002). `None` when `UseDataDictionary` is absent/`N`, matching today's behavior (no dictionary
/// validation wired) unchanged.
///
/// **Scope note**: `Services.validator` (which this feeds) already only carries a single
/// `DataDictionary`, not `truefix_dict::FixtDictionaries`' separate transport/application pair —
/// this is a pre-existing limitation of the programmatic API, not something newly introduced by
/// `.cfg` wiring. `AppDataDictionary`/`TransportDataDictionary` are therefore treated as alternate
/// key names for the same single dictionary value, not a true FIXT dual-dictionary setup.
fn resolve_validator(
    map: &Map,
    session: &str,
) -> Result<
    Option<(
        truefix_dict::DataDictionary,
        truefix_dict::ValidationOptions,
    )>,
    ConfigError,
> {
    if !bool_key(map, "UseDataDictionary", false) {
        return Ok(None);
    }
    let value = map
        .get("DataDictionary")
        .or_else(|| map.get("AppDataDictionary"))
        .or_else(|| map.get("TransportDataDictionary"))
        .ok_or_else(|| ConfigError::MissingRequired {
            key: "DataDictionary".to_owned(),
            session: session.to_owned(),
        })?;
    let dict = load_dictionary_value(value, session)?;
    let opts = truefix_dict::ValidationOptions {
        validate_fields_out_of_order: bool_key(map, "ValidateFieldsOutOfOrder", false),
        validate_checksum: bool_key(map, "ValidateChecksum", true),
        validate_incoming_message: bool_key(map, "ValidateIncomingMessage", true),
        allow_pos_dup: bool_key(map, "AllowPosDup", true),
        requires_orig_sending_time: bool_key(map, "RequiresOrigSendingTime", false),
        ..truefix_dict::ValidationOptions::default()
    };
    Ok(Some((dict, opts)))
}

/// `DataDictionary`'s value is either one of `truefix_dict::ALL_DICTS`'s bundled version strings
/// (e.g. `"FIX.4.4"`) or a filesystem path, tried in that order.
fn load_dictionary_value(
    value: &str,
    session: &str,
) -> Result<truefix_dict::DataDictionary, ConfigError> {
    if let Some((_, source)) = truefix_dict::ALL_DICTS.iter().find(|(v, _)| *v == value) {
        return truefix_dict::parse(source).map_err(|e| ConfigError::InvalidValue {
            key: "DataDictionary".to_owned(),
            session: session.to_owned(),
            reason: e.to_string(),
        });
    }
    truefix_dict::load_from_file(value).map_err(|e| ConfigError::InvalidValue {
        key: "DataDictionary".to_owned(),
        session: session.to_owned(),
        reason: e.to_string(),
    })
}

/// Resolve forward-proxy settings from `ProxyType`/`ProxyHost`/`ProxyPort`/`ProxyUser`/
/// `ProxyPassword` (US12, FR-016); `None` when `ProxyType` is absent.
fn resolve_proxy(map: &Map, session: &str) -> Result<Option<ProxySpec>, ConfigError> {
    let Some(proxy_type_str) = map.get("ProxyType") else {
        return Ok(None);
    };
    let proxy_type = match proxy_type_str.to_ascii_lowercase().as_str() {
        "socks4" => ProxyKind::Socks4,
        "socks5" => ProxyKind::Socks5,
        "httpconnect" | "http_connect" | "http" => ProxyKind::HttpConnect,
        other => {
            return Err(ConfigError::InvalidValue {
                key: "ProxyType".to_owned(),
                session: session.to_owned(),
                reason: format!(
                    "unknown proxy type {other:?} (expected Socks4, Socks5, or HttpConnect)"
                ),
            })
        }
    };
    let host = required(map, "ProxyHost", session)?.to_owned();
    let port_str = required(map, "ProxyPort", session)?;
    let port: u16 = port_str.parse().map_err(|_| ConfigError::InvalidValue {
        key: "ProxyPort".to_owned(),
        session: session.to_owned(),
        reason: format!("expected a port number, got {port_str:?}"),
    })?;
    Ok(Some(ProxySpec {
        proxy_type,
        host,
        port,
        username: map.get("ProxyUser").cloned(),
        password: map.get("ProxyPassword").cloned(),
    }))
}

/// Resolve `UseTCPProxy`/`TrustedProxyAddresses` (US12, FR-015): the physical source addresses a
/// PROXY protocol header is trusted from. Empty when `UseTCPProxy` is unset/`N`.
fn resolve_trusted_proxy_addresses(map: &Map, session: &str) -> Result<Vec<IpAddr>, ConfigError> {
    if !bool_key(map, "UseTCPProxy", false) {
        return Ok(Vec::new());
    }
    match map.get("TrustedProxyAddresses") {
        None => Ok(Vec::new()),
        Some(v) => v
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<IpAddr>().map_err(|_| ConfigError::InvalidValue {
                    key: "TrustedProxyAddresses".to_owned(),
                    session: session.to_owned(),
                    reason: format!("expected an IP address, got {s:?}"),
                })
            })
            .collect(),
    }
}

/// Resolve `SocketSynchronousWrites`/`SocketSynchronousWriteTimeout` (US12, FR-017); `None`
/// unless synchronous writes are enabled with a configured timeout.
fn resolve_sync_write_timeout(map: &Map, session: &str) -> Result<Option<Duration>, ConfigError> {
    if !bool_key(map, "SocketSynchronousWrites", false) {
        return Ok(None);
    }
    match map.get("SocketSynchronousWriteTimeout") {
        None => Ok(None),
        Some(v) => {
            let secs: u64 = v.parse().map_err(|_| ConfigError::InvalidValue {
                key: "SocketSynchronousWriteTimeout".to_owned(),
                session: session.to_owned(),
                reason: format!("expected a whole number of seconds, got {v:?}"),
            })?;
            Ok(Some(Duration::from_secs(secs)))
        }
    }
}

/// Resolve a file-backed log from `FileLogPath` and its output-switch keys (FR-026); `None` when
/// `FileLogPath` is absent (matching the engine's pre-existing default of no log).
/// Resolve `JdbcURL`'s log side (US3, feature 004, FR-003) and `FileLogPath`/its output switches
/// (FR-026) into the mutually-exclusive `(log, sql_log)` pair. When both are configured, `JdbcURL`
/// wins — `FileLogPath` is ignored for that session's log, with a `tracing::warn!` noting the
/// precedence (an explicit, disclosed scope boundary: fanning out to both via `CompositeLog` was
/// considered and rejected as beyond this feature's "SQL backend selection reachable from `.cfg`"
/// scope; `CompositeLog` remains available programmatically for that combination).
fn resolve_log(map: &Map, session: &str) -> (Option<LogSpec>, Option<SqlLogSpec>) {
    if let Some(url) = map.get("JdbcURL") {
        if map.contains_key("FileLogPath") {
            tracing::warn!(
                session = %session,
                "both JdbcURL and FileLogPath are set; JdbcURL wins for this session's log"
            );
        }
        return (
            None,
            Some(SqlLogSpec {
                url: url.clone(),
                incoming_table: map
                    .get("JdbcLogIncomingTable")
                    .cloned()
                    .unwrap_or_else(|| "log_incoming".to_owned()),
                outgoing_table: map
                    .get("JdbcLogOutgoingTable")
                    .cloned()
                    .unwrap_or_else(|| "log_outgoing".to_owned()),
                event_table: map
                    .get("JdbcLogEventTable")
                    .cloned()
                    .unwrap_or_else(|| "log_event".to_owned()),
                include_heartbeats: bool_key(map, "JdbcLogHeartBeats", true),
            }),
        );
    }
    let Some(dir) = map.get("FileLogPath") else {
        return (None, None);
    };
    (
        Some(LogSpec {
            dir: PathBuf::from(dir),
            include_heartbeats: bool_key(map, "FileLogHeartbeats", true),
            include_timestamp: bool_key(map, "FileIncludeTimeStampForMessages", false),
            include_milliseconds: bool_key(map, "FileIncludeMilliseconds", false),
        }),
        None,
    )
}

/// Resolve the full socket-option set (FR-019): `SocketTcpNoDelay`/`SocketKeepAlive`/
/// `SocketReuseAddress`/`SocketLinger`/`SocketOobInline`/`SocketReceiveBufferSize`/
/// `SocketSendBufferSize`/`SocketTrafficClass`.
fn resolve_socket_options(map: &Map, session: &str) -> Result<SocketOptionsSpec, ConfigError> {
    let default = SocketOptionsSpec::default();
    Ok(SocketOptionsSpec {
        tcp_no_delay: bool_key(map, "SocketTcpNoDelay", default.tcp_no_delay),
        keep_alive: bool_key(map, "SocketKeepAlive", default.keep_alive),
        reuse_address: bool_key(map, "SocketReuseAddress", default.reuse_address),
        linger: linger_key(map, "SocketLinger", session)?,
        oob_inline: bool_key(map, "SocketOobInline", default.oob_inline),
        recv_buffer_size: usize_key(
            map,
            "SocketReceiveBufferSize",
            session,
            default.recv_buffer_size,
        )?,
        send_buffer_size: usize_key(
            map,
            "SocketSendBufferSize",
            session,
            default.send_buffer_size,
        )?,
        traffic_class: opt_u32_key(map, "SocketTrafficClass", session)?,
    })
}

fn opt_u32_key(map: &Map, key: &str, session: &str) -> Result<Option<u32>, ConfigError> {
    match map.get(key) {
        None => Ok(None),
        Some(v) => v.parse().map(Some).map_err(|_| ConfigError::InvalidValue {
            key: key.to_owned(),
            session: session.to_owned(),
            reason: format!("expected an integer, got {v:?}"),
        }),
    }
}

/// Resolve numbered backup endpoints `SocketConnectHost<N>`/`SocketConnectPort<N>` (N = 1, 2, ...;
/// the unnumbered `SocketConnectHost`/`SocketConnectPort` is the primary `address` and is not
/// repeated here) into failover order (FR-019). Stops at the first missing `N`.
fn resolve_failover_addresses(map: &Map, session: &str) -> Result<Vec<SocketAddr>, ConfigError> {
    let mut addrs = Vec::new();
    let mut n = 1u32;
    loop {
        let host_key = format!("SocketConnectHost{n}");
        let port_key = format!("SocketConnectPort{n}");
        let (host, port_str) = match (map.get(&host_key), map.get(&port_key)) {
            (Some(h), Some(p)) => (h.as_str(), p.as_str()),
            (None, None) => break,
            _ => {
                return Err(ConfigError::InvalidValue {
                    key: format!("{host_key}/{port_key}"),
                    session: session.to_owned(),
                    reason: "both host and port must be set for a numbered backup endpoint"
                        .to_owned(),
                })
            }
        };
        let port: u16 = port_str.parse().map_err(|_| ConfigError::InvalidValue {
            key: port_key.clone(),
            session: session.to_owned(),
            reason: format!("expected a port number, got {port_str:?}"),
        })?;
        let addr = (host, port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut it| it.next())
            .ok_or_else(|| ConfigError::InvalidValue {
                key: host_key.clone(),
                session: session.to_owned(),
                reason: format!("could not resolve address {host}:{port}"),
            })?;
        addrs.push(addr);
        n += 1;
    }
    Ok(addrs)
}

/// Decode a config value as inline PEM bytes (US12, FR-017): `.cfg` is line-oriented (one
/// `key=value` per line, see `SessionSettings::parse`), so a literal multi-line PEM block can't
/// appear as-is in a value — a literal `\n` two-character escape sequence stands in for a real
/// newline, decoded here.
fn pem_bytes_key(map: &Map, key: &str) -> Option<Vec<u8>> {
    map.get(key).map(|v| v.replace("\\n", "\n").into_bytes())
}

/// Resolve TLS settings when `SocketUseSSL=Y` (FR-017): key/trust-store paths (or inline PEM
/// bytes — US12), mTLS, minimum version, cipher suites, and SNI.
fn resolve_tls(map: &Map, session: &str) -> Result<Option<TlsSpec>, ConfigError> {
    if !bool_key(map, "SocketUseSSL", false) {
        return Ok(None);
    }
    let key_store_bytes = pem_bytes_key(map, "SocketKeyStoreBytes");
    let key_store_path = match (map.get("SocketKeyStore"), &key_store_bytes) {
        (Some(p), _) => Some(PathBuf::from(p)),
        (None, Some(_)) => None,
        (None, None) => {
            return Err(ConfigError::MissingRequired {
                key: "SocketKeyStore or SocketKeyStoreBytes".to_owned(),
                session: session.to_owned(),
            })
        }
    };
    let trust_store_bytes = pem_bytes_key(map, "SocketTrustStoreBytes");
    let trust_store_path = map.get("SocketTrustStore").map(PathBuf::from);
    let need_client_auth = bool_key(map, "NeedClientAuth", false);
    let min_version = match map.get("EnabledProtocols").map(String::as_str) {
        None => None,
        Some(v) if v.contains("1.3") && !v.contains("1.2") => Some(TlsVersion::Tls13),
        Some(_) => Some(TlsVersion::Tls12),
    };
    let server_name = if bool_key(map, "UseSNI", false) {
        Some(
            map.get("SNIHostName")
                .cloned()
                .unwrap_or_else(|| "localhost".to_owned()),
        )
    } else {
        None
    };
    let cipher_suites = map
        .get("CipherSuites")
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(TlsSpec {
        key_store_path,
        key_store_bytes,
        trust_store_path,
        trust_store_bytes,
        need_client_auth,
        min_version,
        server_name,
        cipher_suites,
    }))
}

fn resolve_address(
    map: &Map,
    connection: ConnectionType,
    session: &str,
) -> Result<SocketAddr, ConfigError> {
    let (host, port_key) = match connection {
        ConnectionType::Acceptor => (
            map.get("SocketAcceptAddress")
                .map(String::as_str)
                .unwrap_or("0.0.0.0"),
            "SocketAcceptPort",
        ),
        ConnectionType::Initiator => (
            required(map, "SocketConnectHost", session)?,
            "SocketConnectHost/Port",
        ),
    };
    let port_str = match connection {
        ConnectionType::Acceptor => required(map, "SocketAcceptPort", session)?,
        ConnectionType::Initiator => required(map, "SocketConnectPort", session)?,
    };
    let port: u16 = port_str.parse().map_err(|_| ConfigError::InvalidValue {
        key: port_key.to_owned(),
        session: session.to_owned(),
        reason: format!("expected a port number, got {port_str:?}"),
    })?;
    (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.next())
        .ok_or_else(|| ConfigError::InvalidValue {
            key: port_key.to_owned(),
            session: session.to_owned(),
            reason: format!("could not resolve address {host}:{port}"),
        })
}

/// Resolve `JdbcURL`/`FileStorePath`/`FileStoreSync`/`FileStoreMaxCachedMsgs` (FR-025, and US3
/// FR-003 for `JdbcURL`): `JdbcURL`, when present, is checked first and dispatches by URL scheme to
/// a SQL-family `StoreConfig` variant (requiring the matching Cargo feature); otherwise a bare
/// `FileStorePath` builds a plain [`StoreConfig::File`], and adding `FileStoreMaxCachedMsgs` opts
/// into [`StoreConfig::CachedFile`] (its bound applies once you've asked to tune it); absent both,
/// [`StoreConfig::Memory`].
fn resolve_store(map: &Map, session: &str) -> Result<StoreConfig, ConfigError> {
    if let Some(url) = map.get("JdbcURL") {
        return jdbc_store_config(url, session);
    }
    let Some(dir) = map.get("FileStorePath") else {
        return Ok(StoreConfig::Memory);
    };
    let dir = PathBuf::from(dir);
    let sync = bool_key(map, "FileStoreSync", true);
    Ok(match map.get("FileStoreMaxCachedMsgs") {
        Some(_) => {
            let max_cached_msgs =
                usize_key(map, "FileStoreMaxCachedMsgs", session, Some(0))?.unwrap_or(0);
            StoreConfig::CachedFile {
                dir,
                options: truefix_store::FileStoreOptions {
                    sync,
                    max_cached_msgs,
                },
            }
        }
        None => StoreConfig::File {
            dir,
            options: truefix_store::FileStoreOptions {
                sync,
                max_cached_msgs: 0,
            },
        },
    })
}

/// `JdbcURL`'s scheme prefix, dispatched by string prefix — mirrors the scheme-sniffing
/// `SqlStore::connect`/`MssqlStore::connect` already do internally (research.md §3: no
/// `JdbcDriver`-class-based dispatch, since this codebase has no such registry to mirror).
fn is_sql_scheme(url: &str) -> bool {
    url.starts_with("postgres://")
        || url.starts_with("postgresql://")
        || url.starts_with("mysql://")
        || url.starts_with("sqlite:")
}

fn is_mssql_scheme(url: &str) -> bool {
    url.starts_with("mssql://") || url.starts_with("sqlserver://")
}

fn jdbc_scheme_of(url: &str) -> String {
    url.split_once(':')
        .map_or_else(|| url.to_owned(), |(s, _)| s.to_owned())
}

fn jdbc_store_config(url: &str, session: &str) -> Result<StoreConfig, ConfigError> {
    if is_sql_scheme(url) {
        return sql_store_config(url, session);
    }
    if is_mssql_scheme(url) {
        return mssql_store_config(url, session);
    }
    Err(ConfigError::UnsupportedBackend {
        session: session.to_owned(),
        scheme: jdbc_scheme_of(url),
    })
}

#[cfg(feature = "sql")]
fn sql_store_config(url: &str, _session: &str) -> Result<StoreConfig, ConfigError> {
    Ok(StoreConfig::Sql {
        url: url.to_owned(),
    })
}

#[cfg(not(feature = "sql"))]
fn sql_store_config(url: &str, session: &str) -> Result<StoreConfig, ConfigError> {
    Err(ConfigError::UnsupportedBackend {
        session: session.to_owned(),
        scheme: jdbc_scheme_of(url),
    })
}

#[cfg(feature = "mssql")]
fn mssql_store_config(url: &str, _session: &str) -> Result<StoreConfig, ConfigError> {
    Ok(StoreConfig::Mssql {
        url: url.to_owned(),
    })
}

#[cfg(not(feature = "mssql"))]
fn mssql_store_config(url: &str, session: &str) -> Result<StoreConfig, ConfigError> {
    Err(ConfigError::UnsupportedBackend {
        session: session.to_owned(),
        scheme: jdbc_scheme_of(url),
    })
}

/// Resolve `NonStopSession`/`StartTime`/`EndTime`/`Weekdays`/`TimeZone`/`StartDay`/`EndDay` into a
/// [`Schedule`] (FR-018/FR-E1). `None` means no schedule restriction (always active), matching the
/// engine's pre-existing default when no schedule keys are given.
fn resolve_schedule(map: &Map, session: &str) -> Result<Option<Schedule>, ConfigError> {
    if bool_key(map, "NonStopSession", false) {
        return Ok(Some(Schedule::non_stop()));
    }
    let start_time = map
        .get("StartTime")
        .map(|v| parse_time(v, "StartTime", session))
        .transpose()?;
    let end_time = map
        .get("EndTime")
        .map(|v| parse_time(v, "EndTime", session))
        .transpose()?;
    let start_day = map
        .get("StartDay")
        .map(|v| parse_weekday(v, "StartDay", session))
        .transpose()?;
    let end_day = map
        .get("EndDay")
        .map(|v| parse_weekday(v, "EndDay", session))
        .transpose()?;

    let schedule = match (start_day, end_day, start_time, end_time) {
        (Some(sd), Some(ed), Some(st), Some(et)) => Schedule::weekly(sd, st, ed, et),
        (None, None, Some(st), Some(et)) => Schedule::daily(st, et),
        (None, None, None, None) => return Ok(None),
        _ => {
            return Err(ConfigError::InvalidValue {
                key: "StartTime/EndTime/StartDay/EndDay".to_owned(),
                session: session.to_owned(),
                reason: "StartDay/EndDay require both, alongside StartTime/EndTime".to_owned(),
            })
        }
    };

    let schedule = match map.get("Weekdays") {
        Some(list) => {
            let days = list
                .split(',')
                .map(|d| parse_weekday(d.trim(), "Weekdays", session))
                .collect::<Result<Vec<_>, _>>()?;
            schedule.with_weekdays(days)
        }
        None => schedule,
    };
    let schedule = match map.get("TimeZone") {
        Some(tz) => schedule.with_utc_offset_seconds(parse_utc_offset(tz, session)?),
        None => schedule,
    };
    Ok(Some(schedule))
}

/// Parse `HH:MM:SS` (or `HH:MM`) as a local time-of-day.
fn parse_time(v: &str, key: &str, session: &str) -> Result<time::Time, ConfigError> {
    let bad = || ConfigError::InvalidValue {
        key: key.to_owned(),
        session: session.to_owned(),
        reason: format!("expected HH:MM:SS, got {v:?}"),
    };
    let mut parts = v.splitn(3, ':');
    let hour: u8 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(bad)?;
    let minute: u8 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(bad)?;
    let second: u8 = match parts.next() {
        Some(s) => s.parse().map_err(|_| bad())?,
        None => 0,
    };
    time::Time::from_hms(hour, minute, second).map_err(|_| bad())
}

/// Parse a weekday name (`Monday`/`Mon`, case-insensitive).
fn parse_weekday(v: &str, key: &str, session: &str) -> Result<Weekday, ConfigError> {
    let lower = v.trim().to_ascii_lowercase();
    let day = match lower.as_str() {
        "monday" | "mon" => Weekday::Monday,
        "tuesday" | "tue" => Weekday::Tuesday,
        "wednesday" | "wed" => Weekday::Wednesday,
        "thursday" | "thu" => Weekday::Thursday,
        "friday" | "fri" => Weekday::Friday,
        "saturday" | "sat" => Weekday::Saturday,
        "sunday" | "sun" => Weekday::Sunday,
        _ => {
            return Err(ConfigError::InvalidValue {
                key: key.to_owned(),
                session: session.to_owned(),
                reason: format!("expected a weekday name, got {v:?}"),
            })
        }
    };
    Ok(day)
}

/// Parse a `TimeZone` value as a signed numeric UTC offset (`+HH:MM`/`-HH:MM`). Named zones (e.g.
/// `America/New_York`) are not resolvable without a time-zone database and are rejected with a
/// typed error rather than silently defaulting to UTC.
fn parse_utc_offset(v: &str, session: &str) -> Result<i32, ConfigError> {
    let bad = || ConfigError::InvalidValue {
        key: "TimeZone".to_owned(),
        session: session.to_owned(),
        reason: format!("expected a numeric offset like +05:00/-03:00, got {v:?}"),
    };
    let (sign, rest) = match v.as_bytes().first() {
        Some(b'+') => (1i32, v.get(1..).unwrap_or("")),
        Some(b'-') => (-1i32, v.get(1..).unwrap_or("")),
        _ => return Err(bad()),
    };
    let mut parts = rest.splitn(2, ':');
    let hours: i32 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(bad)?;
    let minutes: i32 = match parts.next() {
        Some(s) => s.parse().map_err(|_| bad())?,
        None => 0,
    };
    Ok(sign * (hours * 3600 + minutes * 60))
}
