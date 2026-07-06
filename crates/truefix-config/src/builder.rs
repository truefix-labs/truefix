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
    /// This session is the acceptor group's dynamic-session template (`DynamicSession=Y` or
    /// `AcceptorTemplate` set — US2, feature 005, BUG-03/FR-006). Acceptor-only; always `false` for
    /// initiators.
    pub acceptor_template: bool,
    /// Only accept connections from these remote IP addresses (`AllowedRemoteAddresses` — US2,
    /// feature 005, BUG-03/FR-006). Acceptor-only; empty when unset.
    pub allowed_remote_addresses: Vec<IpAddr>,
    /// `.cfg`-selectable `ScreenLog`/`TracingLog`/`CompositeLog` (`Log` key, GAP-21); `None` means
    /// the pre-existing File/SQL-only inference from `FileLogPath`/`JdbcURL` (unchanged default).
    pub log_kind: Option<LogKind>,
    /// A real FIXT 1.1 transport/application dictionary split (T079/GAP-18c part 2), present only
    /// when both `AppDataDictionary` and `TransportDataDictionary` are set — see
    /// `resolve_fixt_dictionaries`'s doc. `None` otherwise, including the common case where only
    /// one of the two (or the generic `DataDictionary`) is set, which [`Self::validator`] alone
    /// already covers.
    pub fixt_dictionaries: Option<truefix_dict::FixtDictionaries>,
    /// NEW-96 (feature 009): `LogMessageWhenSessionNotFound` — acceptor-only, logs a diagnostic
    /// when an inbound message's Logon matches neither a static session nor a dynamic template,
    /// before the connection is refused. Was registered `Impl` in the key registry but had no
    /// `.cfg` parsing path at all (`Services.log_message_when_session_not_found`, which the
    /// mechanism genuinely consumes, was permanently `false` via `Services::default()`).
    pub log_message_when_session_not_found: bool,
}

/// Which non-File/SQL `truefix-log` backend to build, selected via the `Log` config key
/// (GAP-21). `File`/`Sql` continue to be selected implicitly by `FileLogPath`/`JdbcURL` presence,
/// as before `Log` existed — this only adds the three backends that had no `.cfg` trigger at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    /// `ScreenLog` (stdout).
    Screen,
    /// `TracingLog` (the `tracing` facade, QFJ's SLF4J equivalent).
    Tracing,
    /// `CompositeLog` fanning out to every other backend this session also has configured
    /// (`FileLogPath` and/or `JdbcURL`), plus `Screen` and `Tracing`.
    Composite,
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

/// As [`bool_key`], but a value that's present and neither a recognized affirmative (`Y`) nor
/// negative (`N`) form is a typed [`ConfigError`] (BUG-64/FR-036, feature 007) rather than
/// silently treated as `false` — e.g. `SocketUseSSL=true` would otherwise silently disable TLS
/// with no diagnostic at all. Deliberately a *separate* function from `bool_key`, not a blanket
/// conversion of it: `resolve_lenient`'s own `ContinueInitializationOnError` read (this file, near
/// the top) is intentionally a "simple, never-failing boolean key" — it's consulted specifically
/// to decide whether to tolerate *another* key's failure in the same session, so making it
/// fallible too would undermine the fallback path it exists for. Applied narrowly to
/// `SocketUseSSL` (BUG-64's own named example) rather than every boolean key in this file.
fn strict_bool_key(
    map: &Map,
    key: &str,
    session: &str,
    default: bool,
) -> Result<bool, ConfigError> {
    match map.get(key).map(String::as_str) {
        None => Ok(default),
        Some(v) if v.eq_ignore_ascii_case("Y") => Ok(true),
        Some(v) if v.eq_ignore_ascii_case("N") => Ok(false),
        Some(other) => Err(ConfigError::InvalidValue {
            key: key.to_owned(),
            session: session.to_owned(),
            reason: format!("expected Y/N, got {other:?}"),
        }),
    }
}

/// `LogonTag=<tag>=<value>`, `LogonTag1=<tag>=<value>`, `LogonTag2=<tag>=<value>`, … (e.g.
/// `LogonTag=9001=HOUSE-ID`), each appended to every outbound Logon in ascending numeric-suffix
/// order (`LogonTag` itself sorts first, GAP-12); none present → empty `Vec`.
fn resolve_logon_tags(map: &Map, session: &str) -> Result<Vec<(u32, String)>, ConfigError> {
    let parse_one = |key: &str, raw: &str| -> Result<(u32, String), ConfigError> {
        let (tag, value) = raw
            .split_once('=')
            .ok_or_else(|| ConfigError::InvalidValue {
                key: key.to_owned(),
                session: session.to_owned(),
                reason: format!("expected `<tag>=<value>`, got {raw:?}"),
            })?;
        let tag: u32 = tag.parse().map_err(|_| ConfigError::InvalidValue {
            key: key.to_owned(),
            session: session.to_owned(),
            reason: format!("expected an integer tag, got {tag:?}"),
        })?;
        Ok((tag, value.to_owned()))
    };

    let mut result = Vec::new();
    if let Some(raw) = map.get("LogonTag") {
        result.push(parse_one("LogonTag", raw)?);
    }
    let mut suffix = 1u32;
    loop {
        let key = format!("LogonTag{suffix}");
        let Some(raw) = map.get(&key) else {
            break;
        };
        result.push(parse_one(&key, raw)?);
        suffix += 1;
    }
    Ok(result)
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
    // BUG-63/FR-036 (feature 007): case-insensitive, matching every other parser in this file
    // (`bool_key`/`strict_bool_key`'s `eq_ignore_ascii_case`) — previously required exact
    // uppercase, so a perfectly reasonable `.cfg` value like `TimeStampPrecision=seconds` failed
    // with `InvalidValue` even though it's unambiguous.
    match map.get(key).map(|v| v.to_ascii_uppercase()).as_deref() {
        None => Ok(default),
        Some("SECONDS") => Ok(TimeStampPrecision::Seconds),
        Some("MILLIS") => Ok(TimeStampPrecision::Milliseconds),
        Some("MICROS") => Ok(TimeStampPrecision::Microseconds),
        Some("NANOS") => Ok(TimeStampPrecision::Nanoseconds),
        Some(_) => Err(ConfigError::InvalidValue {
            key: key.to_owned(),
            session: session.to_owned(),
            reason: format!(
                "expected SECONDS/MILLIS/MICROS/NANOS, got {:?}",
                map.get(key).map(String::as_str).unwrap_or_default()
            ),
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
            });
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
    let (reconnect_interval, reconnect_interval_steps) = resolve_reconnect_interval(map, &session)?;
    cfg.reconnect_interval = reconnect_interval;
    cfg.reconnect_interval_steps = reconnect_interval_steps;
    cfg.resend_request_chunk_size = u32_key(map, "ResendRequestChunkSize", &session, 0)?;
    cfg.enable_last_msg_seq_num_processed = bool_key(map, "EnableLastMsgSeqNumProcessed", false);
    cfg.enable_next_expected_msg_seq_num = bool_key(map, "EnableNextExpectedMsgSeqNum", false);
    cfg.check_comp_id = bool_key(map, "CheckCompID", cfg.check_comp_id);
    cfg.reject_garbled_message = bool_key(map, "RejectGarbledMessage", cfg.reject_garbled_message);
    cfg.heartbeat_timeout_multiplier = f64_key(
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
    // 005/T027 (GAP-08/FR-009): the same `RequiresOrigSendingTime` key QuickFIX/J uses also gates
    // the session-layer anti-replay check (state.rs's low-seq+PossDup branch), in addition to the
    // pre-existing dictionary-level `ValidationOptions.requires_orig_sending_time` check below —
    // the two run at different layers (session state machine vs. `validate()`) but share one key.
    cfg.requires_orig_sending_time_on_low_seq = bool_key(map, "RequiresOrigSendingTime", false);
    cfg.logon_tags = resolve_logon_tags(map, &session)?;
    cfg.in_chan_capacity = usize_key(map, "InChanCapacity", &session, None)?;
    // GAP-47/FR-012/FR-013 (feature 005): full session-identity keys.
    cfg.sender_sub_id = map.get("SenderSubID").cloned();
    cfg.sender_location_id = map.get("SenderLocationID").cloned();
    cfg.target_sub_id = map.get("TargetSubID").cloned();
    cfg.target_location_id = map.get("TargetLocationID").cloned();
    cfg.session_qualifier = map.get("SessionQualifier").cloned();
    // GAP-15/FR-015 + GAP-16/FR-016 (feature 005): initiator connect robustness.
    cfg.local_bind_addr = resolve_local_bind_addr(map, &session)?;
    cfg.connect_timeout = match map.get("SocketConnectTimeout") {
        None => None,
        Some(_) => Some(std::time::Duration::from_secs(u64::from(u32_key(
            map,
            "SocketConnectTimeout",
            &session,
            0,
        )?))),
    };

    let address = resolve_address(map, connection, &session)?;
    let store = resolve_store(map, &session)?;
    let tls = resolve_tls(map, &session)?;
    let socket_options = resolve_socket_options(map, &session)?;
    let failover_addresses = match connection {
        ConnectionType::Initiator => resolve_failover_addresses(map, &session)?,
        ConnectionType::Acceptor => Vec::new(),
    };
    let (log, sql_log) = resolve_log(map, &session);
    let log_kind = resolve_log_kind(map, &session)?;
    let proxy = resolve_proxy(map, &session)?;
    let trusted_proxy_addresses = resolve_trusted_proxy_addresses(map, &session)?;
    let sync_write_timeout = resolve_sync_write_timeout(map, &session)?;
    let validator = resolve_validator(map, &session)?;
    let fixt_dictionaries = resolve_fixt_dictionaries(map, &session)?;
    let acceptor_template =
        bool_key(map, "DynamicSession", false) || map.contains_key("AcceptorTemplate");
    let allowed_remote_addresses = resolve_allowed_remote_addresses(map, &session)?;
    // NEW-96 (feature 009): parse the key that was previously registered `Impl` but never read.
    let log_message_when_session_not_found =
        bool_key(map, "LogMessageWhenSessionNotFound", false);

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
        acceptor_template,
        allowed_remote_addresses,
        log_kind,
        fixt_dictionaries,
        log_message_when_session_not_found,
    })
}

/// Resolve `AllowedRemoteAddresses` (US2, feature 005, BUG-03/FR-006): a comma-separated list of IP
/// addresses this session's acceptor group only accepts connections from. Empty (the default) means
/// no restriction, matching today's behavior. Mirrors `resolve_trusted_proxy_addresses`'s parsing
/// exactly.
fn resolve_allowed_remote_addresses(map: &Map, session: &str) -> Result<Vec<IpAddr>, ConfigError> {
    match map.get("AllowedRemoteAddresses") {
        None => Ok(Vec::new()),
        Some(v) => v
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<IpAddr>().map_err(|_| ConfigError::InvalidValue {
                    key: "AllowedRemoteAddresses".to_owned(),
                    session: session.to_owned(),
                    reason: format!("expected an IP address, got {s:?}"),
                })
            })
            .collect(),
    }
}

/// Resolve `UseDataDictionary` + `DataDictionary`/`AppDataDictionary`/`TransportDataDictionary` +
/// the five already-implemented `ValidationOptions` toggles into a single-dictionary validator
/// (US2, feature 004, FR-002). `None` when `UseDataDictionary` is absent/`N`, matching today's
/// behavior (no dictionary validation wired) unchanged.
///
/// When both `AppDataDictionary` and `TransportDataDictionary` are present (a real FIXT 1.1 split
/// config), this single-dictionary value is the **transport** dictionary specifically — matching
/// its actual use here (session/structural validation, header/trailer group decode), which under
/// FIXT lives in the transport dictionary, not the application one (T079/GAP-18c part 2; this
/// priority order was the actual "aliasing" bug — previously `AppDataDictionary` would win when
/// both were set, silently validating admin messages against the wrong dictionary). See
/// `resolve_fixt_dictionaries` for the real transport/application pair, used for **application**
/// message validation, which needs both.
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
        .or_else(|| map.get("TransportDataDictionary"))
        .or_else(|| map.get("AppDataDictionary"))
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
        // NEW-10 (feature 009): 5 more keys registered as `Impl` in `keys.rs` but never actually
        // read here -- an operator setting any of these to a non-default value saw no effect
        // despite the registry claiming full implementation.
        validate_fields_have_values: bool_key(map, "ValidateFieldsHaveValues", true),
        validate_unordered_group_fields: bool_key(map, "ValidateUnorderedGroupFields", true),
        validate_user_defined_fields: bool_key(map, "ValidateUserDefinedFields", false),
        allow_unknown_msg_fields: bool_key(map, "AllowUnknownMsgFields", false),
        first_field_in_group_is_delimiter: bool_key(map, "FirstFieldInGroupIsDelimiter", true),
        ..truefix_dict::ValidationOptions::default()
    };
    Ok(Some((dict, opts)))
}

/// Resolve a real `truefix_dict::FixtDictionaries` (transport + application dictionaries kept
/// genuinely separate, T079/GAP-18c part 2) when both `AppDataDictionary` and
/// `TransportDataDictionary` are present — i.e. a real FIXT 1.1 dual-dictionary `.cfg`, as opposed
/// to the single-dictionary aliasing `resolve_validator` above still performs for the common
/// non-FIXT (or "one dictionary named either key") case. `None` unless both are present, so this
/// is purely additive — it never changes `resolve_validator`'s own return value or the sessions
/// that only ever set one of the two keys.
fn resolve_fixt_dictionaries(
    map: &Map,
    session: &str,
) -> Result<Option<truefix_dict::FixtDictionaries>, ConfigError> {
    if !bool_key(map, "UseDataDictionary", false) {
        return Ok(None);
    }
    let (Some(app_value), Some(transport_value)) = (
        map.get("AppDataDictionary"),
        map.get("TransportDataDictionary"),
    ) else {
        return Ok(None);
    };
    let app_dict = load_dictionary_value(app_value, session)?;
    let transport_dict = load_dictionary_value(transport_value, session)?;
    let mut dicts = truefix_dict::FixtDictionaries::new(transport_dict);
    let app_ver_id = map
        .get("DefaultApplVerID")
        .cloned()
        .unwrap_or_else(|| app_dict.version().to_owned());
    dicts = dicts
        .with_application(app_ver_id.clone(), app_dict)
        .with_default_appl_ver_id(app_ver_id);
    Ok(Some(dicts))
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
            });
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
        // BUG-04 (feature 005): strip a `jdbc:`-prefixed URL down to the sqlx-native form
        // `connect_sql_log`/`connect_mssql_log`'s own scheme-sniffing understands, splicing in
        // JdbcUser/JdbcPassword when the URL doesn't already embed credentials — mirrors
        // `resolve_store`'s `jdbc_store_config` treatment exactly.
        let stripped = url.strip_prefix("jdbc:").unwrap_or(url);
        let url = splice_credentials(
            stripped,
            map.get("JdbcUser").map(String::as_str),
            map.get("JdbcPassword").map(String::as_str),
        );
        return (
            None,
            Some(SqlLogSpec {
                url,
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

/// Resolve the `Log` config key (GAP-21): `Screen`/`Tracing`/`Composite` (case-insensitive) select
/// a `ScreenLog`/`TracingLog`/`CompositeLog` backend, none of which had a `.cfg` trigger before this
/// (`resolve_log` above only ever inferred File or SQL, from `FileLogPath`/`JdbcURL` presence).
/// `File`/`Sql` are also accepted as explicit (redundant) synonyms for that pre-existing inference,
/// resolving to `None` — i.e. a no-op, matching behavior from before `Log` existed. Absent → `None`.
fn resolve_log_kind(map: &Map, session: &str) -> Result<Option<LogKind>, ConfigError> {
    let Some(v) = map.get("Log") else {
        return Ok(None);
    };
    match v.to_ascii_lowercase().as_str() {
        "screen" => Ok(Some(LogKind::Screen)),
        "tracing" => Ok(Some(LogKind::Tracing)),
        "composite" => Ok(Some(LogKind::Composite)),
        "file" | "sql" => Ok(None),
        _ => Err(ConfigError::InvalidValue {
            key: "Log".to_owned(),
            session: session.to_owned(),
            reason: format!("expected Screen/Tracing/Composite/File/Sql, got {v:?}"),
        }),
    }
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
                });
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

/// `ReconnectInterval` (GAP-14/FR-014, feature 005): a single integer (unchanged behavior) or a
/// space/comma-separated list of integers, matching QuickFIX/J's own `.cfg` grammar for this key.
/// Returns `(first_value, steps)`; `steps` is empty for the single-integer form (preserving
/// today's fixed-interval reconnect behavior verbatim).
fn resolve_reconnect_interval(map: &Map, session: &str) -> Result<(u32, Vec<u32>), ConfigError> {
    let Some(raw) = map.get("ReconnectInterval") else {
        return Ok((30, Vec::new()));
    };
    let tokens: Vec<&str> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    let mut steps = Vec::with_capacity(tokens.len());
    for t in &tokens {
        let v: u32 = t.parse().map_err(|_| ConfigError::InvalidValue {
            key: "ReconnectInterval".to_owned(),
            session: session.to_owned(),
            reason: format!(
                "expected an integer or space/comma-separated list of integers, got {raw:?}"
            ),
        })?;
        steps.push(v);
    }
    match steps.first() {
        None => Ok((30, Vec::new())),
        Some(&first) if steps.len() == 1 => Ok((first, Vec::new())),
        Some(&first) => Ok((first, steps)),
    }
}

/// `SocketLocalHost`+`SocketLocalPort` (GAP-15/FR-015, feature 005): the local address an
/// initiator binds before connecting out. Both keys must be set together (an unpaired
/// `SocketLocalPort` is ambiguous, matching the numbered-failover-endpoint convention above).
fn resolve_local_bind_addr(map: &Map, session: &str) -> Result<Option<SocketAddr>, ConfigError> {
    let (host, port_str) = match (map.get("SocketLocalHost"), map.get("SocketLocalPort")) {
        (None, None) => return Ok(None),
        (Some(h), Some(p)) => (h.as_str(), p.as_str()),
        _ => {
            return Err(ConfigError::InvalidValue {
                key: "SocketLocalHost/SocketLocalPort".to_owned(),
                session: session.to_owned(),
                reason: "both SocketLocalHost and SocketLocalPort must be set together".to_owned(),
            });
        }
    };
    let port: u16 = port_str.parse().map_err(|_| ConfigError::InvalidValue {
        key: "SocketLocalPort".to_owned(),
        session: session.to_owned(),
        reason: format!("expected a port number, got {port_str:?}"),
    })?;
    let addr = (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.next())
        .ok_or_else(|| ConfigError::InvalidValue {
            key: "SocketLocalHost".to_owned(),
            session: session.to_owned(),
            reason: format!("could not resolve address {host}:{port}"),
        })?;
    Ok(Some(addr))
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
    if !strict_bool_key(map, "SocketUseSSL", session, false)? {
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
            });
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
        return jdbc_store_config(
            map,
            url,
            session,
            map.get("JdbcUser").map(String::as_str),
            map.get("JdbcPassword").map(String::as_str),
        );
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
///
/// A second, `jdbc:`-prefixed form (BUG-04, feature 005) is recognized as a distinct, disjoint
/// group before these sqlx-native checks — the real URL grammar QuickFIX/J's own `.cfg` files use
/// (`jdbc:postgresql://...`, `jdbc:mysql://...`, etc.), never sqlx's bare-scheme form.
fn is_sql_scheme(url: &str) -> bool {
    url.starts_with("postgres://")
        || url.starts_with("postgresql://")
        || url.starts_with("mysql://")
        || url.starts_with("sqlite:")
}

fn is_mssql_scheme(url: &str) -> bool {
    url.starts_with("mssql://") || url.starts_with("sqlserver://")
}

/// The `jdbc:`-prefixed sibling forms of [`is_sql_scheme`]/[`is_mssql_scheme`] (BUG-04).
///
/// `jdbc:h2:` is deliberately NOT recognized here (BUG-10/FR-018, feature 006, resolved via
/// `/speckit-clarify`: no H2 backend is implemented) — it previously advertised SQL-family
/// support with no corresponding `connect_pool` branch, so it silently fell through to an
/// unconditional SQLite-file-path interpretation, opening/creating a bogus file literally named
/// e.g. `h2:mem:quickfixj` and persisting real session data into it with no error at all. An
/// unrecognized scheme now correctly surfaces as `UnsupportedBackend` instead.
fn is_jdbc_sql_scheme(url: &str) -> bool {
    url.starts_with("jdbc:postgres://")
        || url.starts_with("jdbc:postgresql://")
        || url.starts_with("jdbc:mysql://")
        || url.starts_with("jdbc:sqlite:")
}

fn is_jdbc_mssql_scheme(url: &str) -> bool {
    url.starts_with("jdbc:mssql://") || url.starts_with("jdbc:sqlserver://")
}

fn jdbc_scheme_of(url: &str) -> String {
    let url = url.strip_prefix("jdbc:").unwrap_or(url);
    url.split_once(':')
        .map_or_else(|| url.to_owned(), |(s, _)| s.to_owned())
}

/// Splice `user`/`password` into `url`'s authority when `url` doesn't already embed credentials
/// (no `@`-delimited userinfo before the host) and both are present. Real QuickFIX/J `.cfg` files
/// supply `JdbcURL` (credential-less) plus separate `JdbcUser`/`JdbcPassword` keys
/// (`JdbcUtil.java:69-72`) rather than embedding credentials in the URL string the way TrueFix's
/// own sqlx-native form does.
fn splice_credentials(url: &str, user: Option<&str>, password: Option<&str>) -> String {
    let (user, password) = match (user, password) {
        (Some(u), Some(p)) => (u, p),
        _ => return url.to_owned(),
    };
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };
    if rest.contains('@') {
        // Already credentialed — never override an explicit inline credential.
        return url.to_owned();
    }
    // GAP-55/FR-020 (feature 006): percent-encode the reserved URL-authority characters before
    // splicing — an unescaped `@`, `:`, or `/` in `JdbcUser`/`JdbcPassword` would otherwise
    // corrupt the resulting URL's own delimiter structure (e.g. a password containing `@` would
    // be parsed as if it started the host portion).
    format!(
        "{scheme}://{}:{}@{rest}",
        percent_encode_userinfo(user),
        percent_encode_userinfo(password)
    )
}

/// Percent-encode the URL-authority-reserved characters (`%` itself, `@`, `:`, `/`) in a
/// userinfo component (username or password) before splicing it into a URL. Deliberately
/// minimal — just the characters that would otherwise corrupt this specific splice, not a
/// general-purpose URL-encoding implementation (no new dependency for this one call site,
/// consistent with this workspace's dependency-minimization discipline).
fn percent_encode_userinfo(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '@' => out.push_str("%40"),
            ':' => out.push_str("%3A"),
            '/' => out.push_str("%2F"),
            _ => out.push(c),
        }
    }
    out
}

fn jdbc_store_config(
    map: &Map,
    url: &str,
    session: &str,
    user: Option<&str>,
    password: Option<&str>,
) -> Result<StoreConfig, ConfigError> {
    if is_sql_scheme(url) {
        return sql_store_config(map, url, session);
    }
    if is_mssql_scheme(url) {
        return mssql_store_config(map, url, session);
    }
    if is_jdbc_sql_scheme(url) {
        let stripped = url.strip_prefix("jdbc:").unwrap_or(url);
        return sql_store_config(map, &splice_credentials(stripped, user, password), session);
    }
    if is_jdbc_mssql_scheme(url) {
        let stripped = url.strip_prefix("jdbc:").unwrap_or(url);
        return mssql_store_config(map, &splice_credentials(stripped, user, password), session);
    }
    Err(ConfigError::UnsupportedBackend {
        session: session.to_owned(),
        scheme: jdbc_scheme_of(url),
    })
}

/// The `JdbcStoreSessionsTableName`/`JdbcStoreMessagesTableName`/
/// `JdbcSessionIdDefaultPropertyValue` keys shared by `sql_store_config`/`mssql_store_config`
/// (US8, feature 005, FR-021).
#[cfg(any(feature = "sql", feature = "mssql"))]
fn jdbc_table_name_keys(map: &Map) -> (Option<String>, Option<String>, Option<String>) {
    (
        map.get("JdbcStoreSessionsTableName").cloned(),
        map.get("JdbcStoreMessagesTableName").cloned(),
        map.get("JdbcSessionIdDefaultPropertyValue").cloned(),
    )
}

/// The 6 `Jdbc*Connection*`/`JdbcMaxActiveConnection`/`JdbcMinIdleConnection` pool-tuning keys
/// (US8, feature 005, FR-021); `None` when none of them are set (so `SqlStoreConfig`'s own
/// default `SqlPoolOptions` applies unchanged).
#[cfg(feature = "sql")]
fn jdbc_pool_options(
    map: &Map,
    session: &str,
) -> Result<Option<truefix_store::SqlPoolOptions>, ConfigError> {
    let any_set = [
        "JdbcMaxActiveConnection",
        "JdbcMinIdleConnection",
        "JdbcConnectionTimeout",
        "JdbcConnectionIdleTimeout",
        "JdbcMaxConnectionLifeTime",
        "JdbcConnectionKeepaliveTime",
    ]
    .iter()
    .any(|k| map.contains_key(*k));
    if !any_set {
        return Ok(None);
    }
    let defaults = truefix_store::SqlPoolOptions::default();
    Ok(Some(truefix_store::SqlPoolOptions {
        max_connections: u32_key(
            map,
            "JdbcMaxActiveConnection",
            session,
            defaults.max_connections,
        )?,
        min_connections: u32_key(
            map,
            "JdbcMinIdleConnection",
            session,
            defaults.min_connections,
        )?,
        acquire_timeout: match map.get("JdbcConnectionTimeout") {
            None => defaults.acquire_timeout,
            Some(_) => std::time::Duration::from_secs(u64::from(u32_key(
                map,
                "JdbcConnectionTimeout",
                session,
                0,
            )?)),
        },
        idle_timeout: match map.get("JdbcConnectionIdleTimeout") {
            None => defaults.idle_timeout,
            Some(_) => Some(std::time::Duration::from_secs(u64::from(u32_key(
                map,
                "JdbcConnectionIdleTimeout",
                session,
                0,
            )?))),
        },
        max_lifetime: match map.get("JdbcMaxConnectionLifeTime") {
            None => defaults.max_lifetime,
            Some(_) => Some(std::time::Duration::from_secs(u64::from(u32_key(
                map,
                "JdbcMaxConnectionLifeTime",
                session,
                0,
            )?))),
        },
        keepalive: match map.get("JdbcConnectionKeepaliveTime") {
            None => defaults.keepalive,
            Some(_) => Some(std::time::Duration::from_secs(u64::from(u32_key(
                map,
                "JdbcConnectionKeepaliveTime",
                session,
                0,
            )?))),
        },
    }))
}

#[cfg(feature = "sql")]
fn sql_store_config(map: &Map, url: &str, session: &str) -> Result<StoreConfig, ConfigError> {
    let (sessions_table, messages_table, session_id) = jdbc_table_name_keys(map);
    Ok(StoreConfig::Sql {
        url: url.to_owned(),
        sessions_table,
        messages_table,
        session_id,
        pool: jdbc_pool_options(map, session)?,
    })
}

#[cfg(not(feature = "sql"))]
fn sql_store_config(_map: &Map, url: &str, session: &str) -> Result<StoreConfig, ConfigError> {
    Err(ConfigError::UnsupportedBackend {
        session: session.to_owned(),
        scheme: jdbc_scheme_of(url),
    })
}

#[cfg(feature = "mssql")]
fn mssql_store_config(map: &Map, url: &str, _session: &str) -> Result<StoreConfig, ConfigError> {
    let (sessions_table, messages_table, session_id) = jdbc_table_name_keys(map);
    Ok(StoreConfig::Mssql {
        url: url.to_owned(),
        sessions_table,
        messages_table,
        session_id,
    })
}

#[cfg(not(feature = "mssql"))]
fn mssql_store_config(_map: &Map, url: &str, session: &str) -> Result<StoreConfig, ConfigError> {
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
            });
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
        Some(tz) => match parse_utc_offset(tz, session) {
            Ok(seconds) => schedule.with_utc_offset_seconds(seconds),
            // GAP-10: not a numeric offset -- try an IANA zone name (e.g. `America/New_York`)
            // before giving up.
            Err(numeric_err) => match time_tz::timezones::get_by_name(tz) {
                Some(named) => schedule.with_named_time_zone(named),
                None => return Err(numeric_err),
            },
        },
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
            });
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
        reason: format!(
            "expected a numeric offset like +05:00/-03:00 or an IANA zone name like \
             America/New_York, got {v:?}"
        ),
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
