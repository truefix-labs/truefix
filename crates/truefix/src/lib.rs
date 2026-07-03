//! `truefix` — the engine facade.
//!
//! Re-exports the public surface of the layered crates so integrators depend on one crate.
//! Implement [`Application`] for your callbacks, build a [`SessionConfig`], and run an
//! [`Acceptor`] or [`connect_initiator`].
//!
//! Design: `specs/001-fix-engine-parity/`.
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub use truefix_core::{
    self as core, decode, encode, DecodeError, Field, FieldError, FieldMap, Group, Message,
    MessageCracker, MessageFactory,
};
pub use truefix_session::{
    self as session, Action, Application, Event, Role, Session, SessionConfig, SessionId,
    SessionState,
};
pub use truefix_transport::{self as transport, connect_initiator, Acceptor, SessionHandle};

pub use truefix_config as config;
pub use truefix_dict as dict;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::task::JoinHandle;

use truefix_config::{
    ConfigError, ConnectionType, ProxyKind, ProxySpec, ResolvedSession, SessionSettings,
    SocketOptionsSpec,
};
use truefix_transport::{
    build_client_config, build_server_config, connect_initiator_reconnecting_multi,
    connect_initiator_reconnecting_multi_tls, connect_initiator_tls, connect_initiator_via_proxy,
    connect_initiator_via_proxy_tls, connect_initiator_with, AcceptorBuilder, ProxyConfig,
    ProxyType, ReconnectHandle, Services, SocketOptions,
};

fn to_transport_socket_options(spec: SocketOptionsSpec) -> SocketOptions {
    SocketOptions {
        tcp_no_delay: spec.tcp_no_delay,
        keep_alive: spec.keep_alive,
        reuse_address: spec.reuse_address,
        linger: spec.linger,
        oob_inline: spec.oob_inline,
        recv_buffer_size: spec.recv_buffer_size,
        send_buffer_size: spec.send_buffer_size,
        traffic_class: spec.traffic_class,
    }
}

/// `truefix-config::ProxySpec` -> `truefix-transport::ProxyConfig` (US12, FR-016). The proxy
/// server's own address (`spec.host`/`spec.port`) is resolved here (DNS if needed); the FIX
/// counterparty's address arrives already-resolved as `rs.address` (`resolve_address` resolves it
/// at config-resolution time, same as every other connection target in this crate), so the SOCKS/
/// HTTP-CONNECT handshake tunnels to that resolved `SocketAddr` rather than deferring DNS
/// resolution of the FIX counterparty to the proxy side.
fn to_transport_proxy_config(spec: &ProxySpec) -> std::io::Result<ProxyConfig> {
    use std::net::ToSocketAddrs;
    let proxy_addr = (spec.host.as_str(), spec.port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "could not resolve proxy address {}:{}",
                    spec.host, spec.port
                ),
            )
        })?;
    let proxy_type = match spec.proxy_type {
        ProxyKind::Socks4 => ProxyType::Socks4,
        ProxyKind::Socks5 => ProxyType::Socks5,
        ProxyKind::HttpConnect => ProxyType::HttpConnect,
    };
    Ok(ProxyConfig {
        proxy_type,
        proxy_addr,
        username: spec.username.clone(),
        password: spec.password.clone(),
    })
}

/// An error starting the engine from configuration (FR-015).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// A configuration mapping error (missing/invalid key, unknown ConnectionType).
    #[error(transparent)]
    Config(#[from] truefix_config::ConfigError),
    /// A message store could not be built.
    #[error("store: {0}")]
    Store(String),
    /// A network bind/connect failure.
    #[error("io: {0}")]
    Io(String),
    /// A TLS configuration could not be built from the session's `TlsSpec` (FR-017).
    #[error("tls: {0}")]
    Tls(String),
    /// A log could not be built from the session's `LogSpec` (FR-026).
    #[error("log: {0}")]
    Log(String),
    /// A forward-proxy connection could not be established (US12, FR-016).
    #[error("proxy: {0}")]
    Proxy(String),
}

fn build_log(
    spec: &truefix_config::LogSpec,
    session_id: &str,
) -> Result<Arc<dyn truefix_log::Log>, EngineError> {
    let file_log = truefix_log::FileLog::open_with_options(
        &spec.dir,
        truefix_log::FileLogOptions {
            include_heartbeats: spec.include_heartbeats,
            include_timestamp: spec.include_timestamp,
            include_milliseconds: spec.include_milliseconds,
        },
    )
    .map_err(|e| EngineError::Log(e.to_string()))?;
    Ok(Arc::new(truefix_log::SessionPrefixLog::new(
        session_id.to_owned(),
        file_log,
    )))
}

/// Build a `SqlLog`/`MssqlLog` from a `.cfg`-resolved `SqlLogSpec` (US3, feature 004, FR-003),
/// dispatched by `url`'s scheme — the same scheme-sniffing `resolve_store`/`resolve_log` already do
/// in `truefix-config`, duplicated here rather than shared, since these are small, crate-local
/// checks (same accepted-duplication pattern `truefix-log::mssql`'s `parse_url` already uses versus
/// `truefix-store::mssql`'s, given `truefix-log` doesn't depend on `truefix-store`).
async fn build_sql_log(
    spec: &truefix_config::SqlLogSpec,
    session_id: &str,
) -> Result<Arc<dyn truefix_log::Log>, EngineError> {
    let url = &spec.url;
    let is_sql = url.starts_with("postgres://")
        || url.starts_with("postgresql://")
        || url.starts_with("mysql://")
        || url.starts_with("sqlite:");
    let is_mssql = url.starts_with("mssql://") || url.starts_with("sqlserver://");
    if is_sql {
        connect_sql_log(spec, session_id).await
    } else if is_mssql {
        connect_mssql_log(spec, session_id).await
    } else {
        Err(EngineError::Log(format!(
            "unsupported JdbcURL scheme in {url:?}"
        )))
    }
}

#[cfg(feature = "sql")]
async fn connect_sql_log(
    spec: &truefix_config::SqlLogSpec,
    session_id: &str,
) -> Result<Arc<dyn truefix_log::Log>, EngineError> {
    let log = truefix_log::SqlLog::connect_with_config(truefix_log::SqlLogConfig {
        url: spec.url.clone(),
        incoming_table: spec.incoming_table.clone(),
        outgoing_table: spec.outgoing_table.clone(),
        event_table: spec.event_table.clone(),
        include_heartbeats: spec.include_heartbeats,
        pool: truefix_log::SqlLogPoolOptions::default(),
        session_id: session_id.to_owned(),
    })
    .await
    .map_err(|e| EngineError::Log(e.to_string()))?;
    Ok(Arc::new(truefix_log::SessionPrefixLog::new(
        session_id.to_owned(),
        log,
    )))
}

#[cfg(not(feature = "sql"))]
async fn connect_sql_log(
    spec: &truefix_config::SqlLogSpec,
    _session_id: &str,
) -> Result<Arc<dyn truefix_log::Log>, EngineError> {
    Err(EngineError::Log(format!(
        "JdbcURL {:?} names a SQL backend, but the `sql` feature isn't compiled in",
        spec.url
    )))
}

#[cfg(feature = "mssql")]
async fn connect_mssql_log(
    spec: &truefix_config::SqlLogSpec,
    session_id: &str,
) -> Result<Arc<dyn truefix_log::Log>, EngineError> {
    let log = truefix_log::MssqlLog::connect_with_config(truefix_log::MssqlLogConfig {
        url: spec.url.clone(),
        incoming_table: spec.incoming_table.clone(),
        outgoing_table: spec.outgoing_table.clone(),
        event_table: spec.event_table.clone(),
        include_heartbeats: spec.include_heartbeats,
        session_id: session_id.to_owned(),
    })
    .await
    .map_err(|e| EngineError::Log(e.to_string()))?;
    Ok(Arc::new(truefix_log::SessionPrefixLog::new(
        session_id.to_owned(),
        log,
    )))
}

#[cfg(not(feature = "mssql"))]
async fn connect_mssql_log(
    spec: &truefix_config::SqlLogSpec,
    _session_id: &str,
) -> Result<Arc<dyn truefix_log::Log>, EngineError> {
    Err(EngineError::Log(format!(
        "JdbcURL {:?} names an MSSQL backend, but the `mssql` feature isn't compiled in",
        spec.url
    )))
}

/// A running engine: the started acceptor listeners and initiator sessions (FR-013/014).
///
/// Built from a [`SessionSettings`] via [`Engine::start`]; each `[SESSION]` is routed by its
/// `ConnectionType` to an acceptor or initiator with a store built from its configuration.
pub struct Engine {
    acceptors: Vec<JoinHandle<()>>,
    initiators: Vec<SessionHandle>,
    /// Initiators configured with one or more failover (backup) endpoints (US1, feature 004,
    /// FR-001) — kept separate from `initiators` because the underlying reconnecting connector
    /// returns a different handle type (`ReconnectHandle`, which lacks `SessionHandle`'s
    /// `logout()`/`send()`), so this is purely additive rather than changing `initiators()`'s
    /// existing type.
    failover_initiators: Vec<ReconnectHandle>,
}

impl Engine {
    /// Build and start every session described by `settings`, sharing `app` across them.
    ///
    /// All-or-nothing: a configuration-resolution error returns before anything is started
    /// (FR-015). Each session gets a store built from its `FileStorePath`/store keys, and TLS
    /// (including mTLS) built from its `SocketUseSSL`/`SocketKeyStore`/... keys when
    /// `SocketUseSSL=Y` (FR-017) — no code-level TLS construction required.
    pub async fn start<A: Application + 'static>(
        settings: &SessionSettings,
        app: Arc<A>,
    ) -> Result<Self, EngineError> {
        // US4 (feature 004, FR-005): a resolution failure is also tolerated per-session via
        // `resolve_lenient` when that session's own `ContinueInitializationOnError=Y` — the flag
        // is read from the session's raw `.cfg` map (never fails to parse), since the session
        // itself may not have resolved far enough to have a `SessionConfig` to read it from
        // otherwise.
        let (resolved, skipped) = settings.resolve_lenient()?;
        for (session, err) in &skipped {
            tracing::error!(
                session = %session,
                error = %err,
                "skipping session after a configuration-resolution error (ContinueInitializationOnError=Y)"
            );
        }
        let mut acceptors = Vec::new();
        let mut initiators = Vec::new();
        let mut failover_initiators = Vec::new();

        // US2 (feature 005, BUG-03): group resolved acceptor sessions by bind address so multiple
        // `[SESSION]` blocks sharing one `SocketAcceptPort` are served by ONE listener. Before this,
        // every acceptor session — however many shared an address — got its own independent
        // `Acceptor::bind_with`, so a second bind on an already-bound address failed outright
        // ("address already in use"), and `AllowedRemoteAddresses`/`DynamicSession`/
        // `AcceptorTemplate` were parsed but never reached any code that could act on them. A
        // session is "grouped" (routed through `AcceptorBuilder` instead of the single-session
        // `Acceptor::bind_with` path) when its address is shared with another acceptor session, or
        // it individually sets `AcceptorTemplate`/`DynamicSession`/`AllowedRemoteAddresses` — every
        // other acceptor (the overwhelming common case: one session per port, no special keys) keeps
        // today's exact single-session path, unchanged.
        let mut acceptor_addr_counts: HashMap<SocketAddr, usize> = HashMap::new();
        for rs in &resolved {
            if rs.connection == ConnectionType::Acceptor {
                *acceptor_addr_counts.entry(rs.address).or_insert(0) += 1;
            }
        }
        let mut singles = Vec::new();
        let mut acceptor_groups: HashMap<SocketAddr, Vec<ResolvedSession>> = HashMap::new();
        for rs in resolved {
            let is_grouped = rs.connection == ConnectionType::Acceptor
                && (acceptor_addr_counts.get(&rs.address).copied().unwrap_or(0) > 1
                    || rs.acceptor_template
                    || !rs.allowed_remote_addresses.is_empty());
            if is_grouped {
                acceptor_groups.entry(rs.address).or_default().push(rs);
            } else {
                singles.push(rs);
            }
        }

        for (addr, members) in acceptor_groups {
            // `AcceptorBuilder` shares one `Services` (store/log/validator/...) across every
            // session it serves — a pre-existing constraint of the multi-session acceptor API, not
            // introduced here. The first group member's store/log/socket-options/validator become
            // the whole group's; a group whose members configure genuinely different stores has no
            // way to honor that today (same class of limitation as feature 004's JdbcURL
            // session_id-collision finding) — out of this feature's scope to lift.
            let continue_on_error = members
                .first()
                .is_some_and(|m| m.session.continue_initialization_on_error);
            let result: Result<(), EngineError> = async {
                let template_count = members.iter().filter(|m| m.acceptor_template).count();
                if template_count > 1 {
                    return Err(EngineError::Config(
                        ConfigError::AmbiguousAcceptorTemplate { addr },
                    ));
                }
                let primary = members.first().ok_or_else(|| {
                    EngineError::Io(format!("acceptor group at {addr} has no sessions"))
                })?;
                let session_id = format!(
                    "{}:{}->{}",
                    primary.session.begin_string,
                    primary.session.sender_comp_id,
                    primary.session.target_comp_id
                );
                let store = truefix_store::build_store(&primary.store)
                    .await
                    .map_err(|e| EngineError::Store(e.to_string()))?;
                let log = if let Some(spec) = &primary.sql_log {
                    Some(build_sql_log(spec, &session_id).await?)
                } else if let Some(spec) = &primary.log {
                    Some(build_log(spec, &session_id)?)
                } else {
                    None
                };
                let services = Services {
                    store: Some(Arc::from(store)),
                    socket_options: to_transport_socket_options(primary.socket_options),
                    log,
                    trusted_proxy_addresses: primary.trusted_proxy_addresses.clone(),
                    sync_write_timeout: primary.sync_write_timeout,
                    validator: primary.validator.clone(),
                    ..Services::default()
                };

                let mut allow: Vec<IpAddr> = Vec::new();
                for m in &members {
                    for ip in &m.allowed_remote_addresses {
                        if !allow.contains(ip) {
                            allow.push(*ip);
                        }
                    }
                }
                // TLS is necessarily listener-scoped (one physical accept loop per group) — the
                // first member that enables it wins; conflicting per-session TLS material within
                // one group isn't independently validated here (same disclosed simplification as
                // the shared-Services note above).
                let tls_spec = members.iter().find_map(|m| m.tls.as_ref()).cloned();

                let mut builder = AcceptorBuilder::bind(addr, app.clone())
                    .await
                    .map_err(|e| EngineError::Io(e.to_string()))?
                    .with_services(services);
                if !allow.is_empty() {
                    builder = builder.allow_remotes(allow);
                }
                for m in members {
                    if m.acceptor_template {
                        builder = builder.with_dynamic_template(m.session);
                    } else {
                        builder = builder.with_session(m.session);
                    }
                }
                if let Some(tls) = &tls_spec {
                    let server_cfg =
                        build_server_config(tls).map_err(|e| EngineError::Tls(e.to_string()))?;
                    builder = builder.with_tls(server_cfg);
                }
                acceptors.push(builder.serve());
                Ok(())
            }
            .await;
            if let Err(e) = result {
                if continue_on_error {
                    tracing::error!(
                        addr = %addr,
                        error = %e,
                        "skipping acceptor group after a startup error (ContinueInitializationOnError=Y)"
                    );
                } else {
                    return Err(e);
                }
            }
        }

        for rs in singles {
            let continue_on_error = rs.session.continue_initialization_on_error;
            let session_id = format!(
                "{}:{}->{}",
                rs.session.begin_string, rs.session.sender_comp_id, rs.session.target_comp_id
            );
            // The actual per-session startup work (store/log/bind/connect), isolated in its own
            // future so a failure here can also be tolerated when `continue_on_error` is set —
            // matching the resolution-time tolerance above, but for *runtime* startup failures
            // (e.g. a port already in use), which only surface once resolution has already
            // succeeded for this session.
            let result: Result<(), EngineError> = async {
                let store = truefix_store::build_store(&rs.store)
                    .await
                    .map_err(|e| EngineError::Store(e.to_string()))?;
                let log = if let Some(spec) = &rs.sql_log {
                    Some(build_sql_log(spec, &session_id).await?)
                } else if let Some(spec) = &rs.log {
                    Some(build_log(spec, &session_id)?)
                } else {
                    None
                };
                let services = Services {
                    store: Some(Arc::from(store)),
                    socket_options: to_transport_socket_options(rs.socket_options),
                    log,
                    trusted_proxy_addresses: rs.trusted_proxy_addresses.clone(),
                    sync_write_timeout: rs.sync_write_timeout,
                    validator: rs.validator.clone(),
                    ..Services::default()
                };
                match rs.connection {
                    ConnectionType::Acceptor => {
                        let mut acc =
                            Acceptor::bind_with(rs.address, rs.session, app.clone(), services)
                                .await
                                .map_err(|e| EngineError::Io(e.to_string()))?;
                        if let Some(tls) = &rs.tls {
                            let server_cfg = build_server_config(tls)
                                .map_err(|e| EngineError::Tls(e.to_string()))?;
                            acc = acc.with_tls(server_cfg);
                        }
                        acceptors.push(acc.serve());
                    }
                    ConnectionType::Initiator => {
                        let proxy = rs
                            .proxy
                            .as_ref()
                            .map(to_transport_proxy_config)
                            .transpose()
                            .map_err(|e| EngineError::Proxy(e.to_string()))?;

                        // US1 (feature 004, FR-001): route to the failover-capable reconnecting
                        // connector when backup endpoints are configured. Proxy+failover is out of
                        // scope (no reconnecting-multi-via-proxy connector exists) — a session with
                        // both takes the existing proxy path and its failover addresses are
                        // ignored, logged rather than silently dropped.
                        if !rs.failover_addresses.is_empty() {
                            if proxy.is_some() {
                                tracing::warn!(
                                    session = %session_id,
                                    "failover addresses configured alongside a proxy are ignored \
                                     (proxy+failover is not supported); using the proxy connection only"
                                );
                            } else {
                                let mut addrs =
                                    Vec::with_capacity(1 + rs.failover_addresses.len());
                                addrs.push(rs.address);
                                addrs.extend(rs.failover_addresses.iter().copied());
                                let handle = if let Some(tls) = &rs.tls {
                                    let client_cfg = build_client_config(tls)
                                        .map_err(|e| EngineError::Tls(e.to_string()))?;
                                    let host = tls.server_name.clone().unwrap_or_default();
                                    let server_name =
                                        rustls::pki_types::ServerName::try_from(host)
                                            .map_err(|e| EngineError::Tls(e.to_string()))?;
                                    connect_initiator_reconnecting_multi_tls(
                                        addrs,
                                        rs.session,
                                        app.clone(),
                                        services,
                                        client_cfg,
                                        server_name,
                                    )
                                } else {
                                    connect_initiator_reconnecting_multi(
                                        addrs,
                                        rs.session,
                                        app.clone(),
                                        services,
                                    )
                                };
                                failover_initiators.push(handle);
                                return Ok(());
                            }
                        }

                        let handle = match (&rs.tls, &proxy) {
                            (Some(tls), Some(proxy)) => {
                                let client_cfg = build_client_config(tls)
                                    .map_err(|e| EngineError::Tls(e.to_string()))?;
                                let host = tls.server_name.clone().unwrap_or_default();
                                let server_name = rustls::pki_types::ServerName::try_from(host)
                                    .map_err(|e| EngineError::Tls(e.to_string()))?;
                                connect_initiator_via_proxy_tls(
                                    proxy,
                                    &rs.address.ip().to_string(),
                                    rs.address.port(),
                                    rs.session,
                                    app.clone(),
                                    services,
                                    client_cfg,
                                    server_name,
                                )
                                .await
                                .map_err(|e| EngineError::Proxy(e.to_string()))?
                            }
                            (None, Some(proxy)) => connect_initiator_via_proxy(
                                proxy,
                                &rs.address.ip().to_string(),
                                rs.address.port(),
                                rs.session,
                                app.clone(),
                                services,
                            )
                            .await
                            .map_err(|e| EngineError::Proxy(e.to_string()))?,
                            (Some(tls), None) => {
                                let client_cfg = build_client_config(tls)
                                    .map_err(|e| EngineError::Tls(e.to_string()))?;
                                let host = tls.server_name.clone().unwrap_or_default();
                                let server_name = rustls::pki_types::ServerName::try_from(host)
                                    .map_err(|e| EngineError::Tls(e.to_string()))?;
                                connect_initiator_tls(
                                    rs.address,
                                    rs.session,
                                    app.clone(),
                                    services,
                                    client_cfg,
                                    server_name,
                                )
                                .await
                                .map_err(|e| EngineError::Io(e.to_string()))?
                            }
                            (None, None) => connect_initiator_with(
                                rs.address,
                                rs.session,
                                app.clone(),
                                services,
                            )
                            .await
                            .map_err(|e| EngineError::Io(e.to_string()))?,
                        };
                        initiators.push(handle);
                    }
                }
                Ok(())
            }
            .await;
            if let Err(e) = result {
                if continue_on_error {
                    tracing::error!(
                        session = %session_id,
                        error = %e,
                        "skipping session after a startup error (ContinueInitializationOnError=Y)"
                    );
                } else {
                    return Err(e);
                }
            }
        }
        Ok(Self {
            acceptors,
            initiators,
            failover_initiators,
        })
    }

    /// The started initiator session handles (for logout/join) — single-endpoint initiators only;
    /// see [`Engine::failover_initiators`] for initiators configured with backup endpoints (US1,
    /// feature 004).
    pub fn initiators(&self) -> &[SessionHandle] {
        &self.initiators
    }

    /// The started initiators configured with one or more failover (backup) endpoints (US1,
    /// feature 004, FR-001). A new, additive accessor — [`Engine::initiators`]'s existing shape is
    /// unchanged.
    pub fn failover_initiators(&self) -> &[ReconnectHandle] {
        &self.failover_initiators
    }

    /// Abort all acceptor listeners and stop all failover-initiator reconnect loops.
    pub fn shutdown(&self) {
        for acceptor in &self.acceptors {
            acceptor.abort();
        }
        for initiator in &self.failover_initiators {
            initiator.stop();
        }
    }
}
