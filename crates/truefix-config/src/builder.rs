//! Map parsed [`SessionSettings`] into runnable engine configuration (FR-013/014/015).
//!
//! Resolution is all-or-nothing: the first invalid/missing key aborts with a typed [`ConfigError`]
//! naming the key and session, so no partially-configured engine is produced.

use std::collections::BTreeMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;

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
    if bool_key(map, "NonStopSession", false) {
        cfg.schedule = Some(Schedule::non_stop());
    }

    let address = resolve_address(map, connection, &session)?;
    let store = resolve_store(map);

    Ok(ResolvedSession {
        session: cfg,
        connection,
        address,
        store,
    })
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

fn resolve_store(map: &Map) -> StoreConfig {
    match map.get("FileStorePath") {
        Some(dir) => StoreConfig::File {
            dir: PathBuf::from(dir),
        },
        None => StoreConfig::Memory,
    }
}
