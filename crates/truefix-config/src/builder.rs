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
    /// Forward-proxy configuration for an initiator connection, present when `ProxyType` is set
    /// (US12, FR-016).
    pub proxy: Option<ProxySpec>,
    /// PROXY protocol (v1/v2) is trusted only from these physical source addresses (US12,
    /// FR-015); empty when `UseTCPProxy` is unset/`N`, or `TrustedProxyAddresses` is absent.
    pub trusted_proxy_addresses: Vec<IpAddr>,
    /// `SocketSynchronousWrites` + `SocketSynchronousWriteTimeout` (US12, FR-017): present when
    /// synchronous writes are enabled with a configured timeout.
    pub sync_write_timeout: Option<Duration>,
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
    let log = resolve_log(map);
    let proxy = resolve_proxy(map, &session)?;
    let trusted_proxy_addresses = resolve_trusted_proxy_addresses(map, &session)?;
    let sync_write_timeout = resolve_sync_write_timeout(map, &session)?;

    Ok(ResolvedSession {
        session: cfg,
        connection,
        address,
        store,
        tls,
        socket_options,
        failover_addresses,
        log,
        proxy,
        trusted_proxy_addresses,
        sync_write_timeout,
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
fn resolve_log(map: &Map) -> Option<LogSpec> {
    let dir = PathBuf::from(map.get("FileLogPath")?);
    Some(LogSpec {
        dir,
        include_heartbeats: bool_key(map, "FileLogHeartbeats", true),
        include_timestamp: bool_key(map, "FileIncludeTimeStampForMessages", false),
        include_milliseconds: bool_key(map, "FileIncludeMilliseconds", false),
    })
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

/// Resolve `FileStorePath`/`FileStoreSync`/`FileStoreMaxCachedMsgs` (FR-025): a bare
/// `FileStorePath` builds a plain [`StoreConfig::File`]; adding `FileStoreMaxCachedMsgs` opts into
/// [`StoreConfig::CachedFile`] (its bound applies once you've asked to tune it).
fn resolve_store(map: &Map, session: &str) -> Result<StoreConfig, ConfigError> {
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
