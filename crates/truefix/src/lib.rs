//! `truefix` — the engine facade.
//!
//! Re-exports the public surface of the layered crates so integrators depend on one crate.
//! Implement [`Application`] for your callbacks, build a [`SessionConfig`], and run an
//! [`Acceptor`] or [`connect_initiator`].
//!
//! Audit 006 additions: [`Engine::shutdown`] cancels already-accepted acceptor session tasks, not
//! only listener loops (NEW-149); [`Engine::skipped_sessions`] reports which configured sessions
//! failed to start and why (NEW-143); and dynamic acceptor identities each get their own
//! per-identity store/services factory rather than sharing state (NEW-148).
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
    self as core, DecodeError, Field, FieldError, FieldMap, Group, Message, MessageCracker,
    MessageFactory, decode, encode,
};
pub use truefix_session::{
    self as session, Action, Application, Event, Role, Session, SessionConfig, SessionId,
    SessionState,
};
pub use truefix_transport::{
    self as transport, Acceptor, Monitor, SessionHandle, connect_initiator,
};

pub use truefix_config as config;
pub use truefix_dict as dict;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::task::JoinHandle;

use truefix_config::{
    ConfigError, ConnectionType, ProxyKind, ProxySpec, ResolvedSession, SessionSettings,
    SocketEndpoint, SocketOptionsSpec,
};
use truefix_transport::{
    AcceptorBuilder, ProxyConfig, ProxyType, ReconnectHandle, Services, SocketOptions,
    build_client_config, build_server_config, connect_initiator_reconnecting_endpoints,
    connect_initiator_reconnecting_endpoints_tls, connect_initiator_tls,
    connect_initiator_via_proxy, connect_initiator_via_proxy_tls, connect_initiator_with,
};

async fn resolve_socket_endpoint(endpoint: &SocketEndpoint) -> std::io::Result<SocketAddr> {
    tokio::net::lookup_host((endpoint.host(), endpoint.port()))
        .await?
        .next()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "could not resolve address {}:{}",
                    endpoint.host(),
                    endpoint.port()
                ),
            )
        })
}

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
        backlog: spec.backlog,
    }
}

/// `truefix-config::ProxySpec` -> `truefix-transport::ProxyConfig` (US12, FR-016). The proxy
/// server's own address (`spec.host`/`spec.port`) is resolved here (DNS if needed); the FIX
/// counterparty's address arrives already-resolved as `rs.address` (`resolve_address` resolves it
/// at config-resolution time, same as every other connection target in this crate), so the SOCKS/
/// HTTP-CONNECT handshake tunnels to that resolved `SocketAddr` rather than deferring DNS
/// resolution of the FIX counterparty to the proxy side.
async fn to_transport_proxy_config(spec: &ProxySpec) -> std::io::Result<ProxyConfig> {
    let proxy_addr = tokio::net::lookup_host((spec.host.as_str(), spec.port))
        .await?
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
            max_size_bytes: spec.max_size_bytes,
        },
    )
    .map_err(|e| EngineError::Log(e.to_string()))?;
    Ok(Arc::new(truefix_log::SessionPrefixLog::new(
        session_id.to_owned(),
        file_log,
    )))
}

/// Build a `ScreenLog`/`TracingLog`/`CompositeLog` from a `.cfg`-resolved `LogKind` (GAP-21).
/// `Composite` fans out to `Screen` + `Tracing`, plus `File` when `file_spec` is also present
/// (`truefix_log::build_log`'s pre-existing `LogConfig::Composite` already does the fan-out; this
/// only needed a `.cfg`-reachable trigger, confirmed via `truefix_log::LogConfig` before assuming
/// new backend code was needed — GAP-21's own framing). NEW-126 (audit 006): the `Composite`
/// branch's `File` component now threads `file_spec`'s own output-switch values through
/// `LogConfig::File`'s `options` field, instead of always building a hardcoded-default `FileLog`.
fn build_log_kind(
    kind: truefix_config::LogKind,
    file_spec: &Option<truefix_config::LogSpec>,
    session_id: &str,
) -> Result<Arc<dyn truefix_log::Log>, EngineError> {
    let log_config = match kind {
        truefix_config::LogKind::Screen => truefix_log::LogConfig::Screen,
        truefix_config::LogKind::Tracing => truefix_log::LogConfig::Tracing,
        truefix_config::LogKind::Composite => {
            let mut parts = vec![
                truefix_log::LogConfig::Screen,
                truefix_log::LogConfig::Tracing,
            ];
            if let Some(spec) = file_spec {
                parts.push(truefix_log::LogConfig::File {
                    dir: spec.dir.clone(),
                    options: truefix_log::FileLogOptions {
                        include_heartbeats: spec.include_heartbeats,
                        include_timestamp: spec.include_timestamp,
                        include_milliseconds: spec.include_milliseconds,
                        max_size_bytes: spec.max_size_bytes,
                    },
                });
            }
            truefix_log::LogConfig::Composite(parts)
        }
    };
    let log = truefix_log::build_log(&log_config).map_err(|e| EngineError::Log(e.to_string()))?;
    Ok(Arc::new(truefix_log::SessionPrefixLog::new(
        session_id.to_owned(),
        log,
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
        // NEW-90 (feature 009): no `.cfg` key wires this yet (out of this fix's scope, which only
        // adds the config option at the Rust API level per FR-020's literal wording) -- preserves
        // today's unconditional `trust_cert()` behavior for every `.cfg`-driven deployment.
        trust_server_certificate: true,
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
    /// Every log built for a session this `Engine` started (NEW-91, feature 009), for
    /// [`Engine::logs`] — a caller wanting entries queued in an async log backend flushed before
    /// tearing down should iterate it and await [`truefix_log::Log::shutdown`] on each, mirroring
    /// the existing `initiators()`-then-`.logout().await` pattern for a graceful plain-initiator
    /// stop, since `Engine::shutdown()`/`Drop` both stay synchronous by design (BUG-21/FR-023).
    logs: Vec<Arc<dyn truefix_log::Log>>,
    /// Operational monitor shared by every session this `Engine` started (NEW-122/NEW-123, audit
    /// 006) — every acceptor and initiator session (static, dynamic, or grouped) is registered
    /// here, giving a caller session-state query (`status`/`is_connected`/`sessions`) and control
    /// (`force_logout`/`reset`/`send_app`) addressable by `SessionId`, including acceptor sessions
    /// that previously had no per-session handle at all.
    monitor: Monitor,
    /// Sessions skipped at configuration-resolution time via `ContinueInitializationOnError=Y`
    /// (NEW-143, audit 006) — previously only logged via `tracing::error!` and then discarded, with
    /// no programmatic access for a caller that wants to enumerate what failed to start.
    skipped_sessions: Vec<(String, ConfigError)>,
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
        // NEW-123 (audit 006): a single `Monitor` shared by every session this `Engine` starts,
        // so `Engine::monitor()` can query/control any of them by `SessionId` after start.
        let monitor = Monitor::new();
        let mut acceptors = Vec::new();
        let mut initiators = Vec::new();
        let mut failover_initiators = Vec::new();
        // NEW-91 (feature 009): every log built while starting sessions below, so a caller
        // wanting a graceful flush-on-shutdown can iterate `Engine::logs()` and await
        // `Log::shutdown()` on each — mirroring the existing `initiators()`-then-`.logout().await`
        // pattern for a graceful plain-initiator stop, since `Engine::shutdown()`/`Drop` both stay
        // synchronous by design (BUG-21/FR-023).
        let mut logs: Vec<Arc<dyn truefix_log::Log>> = Vec::new();

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
        let mut acceptor_addr_counts: HashMap<SocketEndpoint, usize> = HashMap::new();
        for rs in &resolved {
            if rs.connection == ConnectionType::Acceptor {
                *acceptor_addr_counts.entry(rs.address.clone()).or_insert(0) += 1;
            }
        }
        let mut singles = Vec::new();
        let mut acceptor_groups: HashMap<SocketEndpoint, Vec<ResolvedSession>> = HashMap::new();
        for rs in resolved {
            let is_grouped = rs.connection == ConnectionType::Acceptor
                && (acceptor_addr_counts.get(&rs.address).copied().unwrap_or(0) > 1
                    || rs.acceptor_template
                    || !rs.allowed_remote_addresses.is_empty());
            if is_grouped {
                acceptor_groups
                    .entry(rs.address.clone())
                    .or_default()
                    .push(rs);
            } else {
                singles.push(rs);
            }
        }

        // T164/T165 (feature 009, NEW-92): `HashMap` iteration order is randomized (each `HashMap`
        // gets its own freshly-keyed hasher state, so even repeated calls within one process can
        // differ), so two runs over the same config could start this engine's acceptor-group
        // listeners in a different order — mostly harmless, but an observable nondeterminism
        // (log/startup-event ordering, listen-backlog readiness timing under load) with no upside.
        let acceptor_groups = sorted_by_endpoint(acceptor_groups);
        for (addr, members) in acceptor_groups {
            // BUG-61/FR-022 (feature 007): store (feature 006, BUG-07) and now
            // log/validator/fixt_dictionaries are each resolved per statically-registered member
            // via `AcceptorBuilder::with_session_store`/`with_session_services` below, rather than
            // silently inherited from the group-wide `Services` built from `primary` alone.
            // `socket_options`/TLS remain a disclosed group-wide simplification (one physical
            // accept loop per group; see the per-session-services doc in `truefix-transport` for
            // why socket options specifically can't be safely resolved this late without unsafe
            // raw-fd handling), same as any session matched via the dynamic template (which has no
            // fixed identity to key a per-session override by in the first place).
            // BUG-16/FR-022 (feature 006): strictest-member-wins — the group tolerates a startup
            // failure only if EVERY member opts in, not just the first one. Resolved via
            // `/speckit-clarify`: errs toward surfacing failures (Constitution Principle I,
            // Production-Ready First) rather than one lenient member silently overriding a
            // later, stricter member's preference.
            let continue_on_error = members
                .iter()
                .all(|m| m.session.continue_initialization_on_error);
            let result: Result<(), EngineError> = async {
                let bind_addr = resolve_socket_endpoint(&addr)
                    .await
                    .map_err(|e| EngineError::Io(e.to_string()))?;
                let template_count = members.iter().filter(|m| m.acceptor_template).count();
                if template_count > 1 {
                    return Err(EngineError::Config(
                        ConfigError::AmbiguousAcceptorTemplate { addr: bind_addr },
                    ));
                }
                // BUG-07/FR-011 (feature 006): SessionQualifier has no wire tag, so two sessions
                // in this group whose wire-extractable identity (BeginString/Sender/Target/SubID/
                // LocationID) is otherwise identical and differ only by SessionQualifier can never
                // be disambiguated by a live inbound connection -- reject at resolve time instead
                // of leaving one of them silently unroutable, resolved via `/speckit-clarify`.
                for (i, a) in members.iter().enumerate() {
                    for b in members.iter().skip(i + 1) {
                        let wire_identity_matches = a.session.begin_string
                            == b.session.begin_string
                            && a.session.sender_comp_id == b.session.sender_comp_id
                            && a.session.target_comp_id == b.session.target_comp_id
                            && a.session.sender_sub_id == b.session.sender_sub_id
                            && a.session.sender_location_id == b.session.sender_location_id
                            && a.session.target_sub_id == b.session.target_sub_id
                            && a.session.target_location_id == b.session.target_location_id;
                        if wire_identity_matches
                            && a.session.session_qualifier != b.session.session_qualifier
                        {
                            return Err(EngineError::Config(
                                ConfigError::AmbiguousSessionQualifier {
                                    addr: bind_addr,
                                    session_a: format!(
                                        "{}:{}->{}",
                                        a.session.begin_string,
                                        a.session.sender_comp_id,
                                        a.session.target_comp_id
                                    ),
                                    session_b: format!(
                                        "{}:{}->{}",
                                        b.session.begin_string,
                                        b.session.sender_comp_id,
                                        b.session.target_comp_id
                                    ),
                                },
                            ));
                        }
                    }
                }
                let primary = members.first().ok_or_else(|| {
                    EngineError::Io(format!("acceptor group at {bind_addr} has no sessions"))
                })?;
                let primary_session_id = primary.session.session_id();
                let session_id = format!(
                    "{}:{}->{}",
                    primary.session.begin_string,
                    primary.session.sender_comp_id,
                    primary.session.target_comp_id
                );
                let primary_store: Arc<dyn truefix_store::MessageStore> =
                    Arc::from(truefix_store::build_store(&primary.store).await.map_err(
                        |e: truefix_store::StoreError| EngineError::Store(e.to_string()),
                    )?);
                let log = if let Some(kind) = primary.log_kind {
                    Some(build_log_kind(kind, &primary.log, &session_id)?)
                } else if let Some(spec) = &primary.sql_log {
                    Some(build_sql_log(spec, &session_id).await?)
                } else if let Some(spec) = &primary.log {
                    Some(build_log(spec, &session_id)?)
                } else {
                    None
                };
                let services = Services {
                    // Fallback for any connection matched via the dynamic template, which has no
                    // fixed identity to key a per-session store by. Every statically-registered
                    // member below gets its own independent store instead (BUG-07-adjacent
                    // finding, feature 006: sharing one store instance across concurrently
                    // connected sessions in a group corrupts each session's own sequence-number
                    // bookkeeping — unlike socket options/TLS, store state is never legitimately
                    // group-wide).
                    store: Some(primary_store.clone()),
                    socket_options: to_transport_socket_options(primary.socket_options),
                    monitor: Some(monitor.clone()),
                    log,
                    trusted_proxy_addresses: primary.trusted_proxy_addresses.clone(),
                    sync_write_timeout: primary.sync_write_timeout,
                    validator: primary.validator.clone(),
                    fixt_dictionaries: primary.fixt_dictionaries.clone().map(|dicts| {
                        let opts =
                            primary.validator.as_ref().map_or_else(
                                truefix_dict::ValidationOptions::default,
                                |(_, opts)| *opts,
                            );
                        (dicts, opts)
                    }),
                    // NEW-96 (feature 009): was permanently `false` via `Services::default()` --
                    // no `.cfg` -> `Services` path existed for this key at all.
                    log_message_when_session_not_found: primary.log_message_when_session_not_found,
                    ..Services::default()
                };
                if let Some(log) = &services.log {
                    logs.push(log.clone());
                }

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

                let mut builder = AcceptorBuilder::bind_with(bind_addr, app.clone(), services)
                    .await
                    .map_err(|e| EngineError::Io(e.to_string()))?;
                if !allow.is_empty() {
                    builder = builder.allow_remotes(allow);
                }
                for m in members {
                    if m.acceptor_template {
                        let template_session = m.session.clone();
                        let dynamic_store = m.store.clone();
                        let dynamic_log_kind = m.log_kind;
                        let dynamic_log = m.log.clone();
                        let dynamic_sql_log = m.sql_log.clone();
                        let dynamic_socket_options = m.socket_options;
                        let dynamic_trusted_proxy_addresses = m.trusted_proxy_addresses.clone();
                        let dynamic_sync_write_timeout = m.sync_write_timeout;
                        let dynamic_validator = m.validator.clone();
                        let dynamic_fixt_dictionaries = m.fixt_dictionaries.clone();
                        let dynamic_log_message_when_session_not_found =
                            m.log_message_when_session_not_found;
                        let dynamic_monitor = monitor.clone();
                        builder = builder
                            .with_dynamic_template(template_session)
                            .with_dynamic_services_factory(move |sid, _config| {
                                let store = dynamic_store.clone();
                                let log_kind = dynamic_log_kind;
                                let log = dynamic_log.clone();
                                let sql_log = dynamic_sql_log.clone();
                                let socket_options = dynamic_socket_options;
                                let trusted_proxy_addresses =
                                    dynamic_trusted_proxy_addresses.clone();
                                let validator = dynamic_validator.clone();
                                let fixt_dictionaries = dynamic_fixt_dictionaries.clone();
                                let monitor = dynamic_monitor.clone();
                                async move {
                                    let store: Arc<dyn truefix_store::MessageStore> = Arc::from(
                                        truefix_store::build_store(&store)
                                            .await
                                            .map_err(|e| e.to_string())?,
                                    );
                                    let session_id = sid.to_string();
                                    let log = if let Some(kind) = log_kind {
                                        Some(
                                            build_log_kind(kind, &log, &session_id)
                                                .map_err(|e| e.to_string())?,
                                        )
                                    } else if let Some(spec) = &sql_log {
                                        Some(
                                            build_sql_log(spec, &session_id)
                                                .await
                                                .map_err(|e| e.to_string())?,
                                        )
                                    } else if let Some(spec) = &log {
                                        Some(
                                            build_log(spec, &session_id)
                                                .map_err(|e| e.to_string())?,
                                        )
                                    } else {
                                        None
                                    };
                                    let fixt_dictionaries = fixt_dictionaries.map(|dicts| {
                                        let opts = validator.as_ref().map_or_else(
                                            truefix_dict::ValidationOptions::default,
                                            |(_, opts)| *opts,
                                        );
                                        (dicts, opts)
                                    });
                                    Ok(Services {
                                        store: Some(store),
                                        socket_options: to_transport_socket_options(socket_options),
                                        monitor: Some(monitor),
                                        log,
                                        trusted_proxy_addresses,
                                        sync_write_timeout: dynamic_sync_write_timeout,
                                        validator,
                                        fixt_dictionaries,
                                        log_message_when_session_not_found:
                                            dynamic_log_message_when_session_not_found,
                                        ..Services::default()
                                    })
                                }
                            });
                    } else {
                        let member_session_id = m.session.session_id();
                        // Give every statically-registered member its own store: reuse the
                        // already-built `primary_store` for the primary member (avoids building
                        // it twice) and build a fresh one from each other member's own `.store`
                        // config, closing the sequence-number cross-contamination this loop used
                        // to allow (BUG-07-adjacent finding, feature 006).
                        let member_store: Arc<dyn truefix_store::MessageStore> =
                            if member_session_id == primary_session_id {
                                primary_store.clone()
                            } else {
                                Arc::from(truefix_store::build_store(&m.store).await.map_err(
                                    |e: truefix_store::StoreError| {
                                        EngineError::Store(e.to_string())
                                    },
                                )?)
                            };
                        // BUG-61/FR-022 (feature 007): also resolve this member's own
                        // log/validator/fixt_dictionaries instead of silently inheriting the
                        // group-wide (primary's) ones -- the primary member itself is skipped here
                        // since `services` above was already built from it directly.
                        if member_session_id != primary_session_id {
                            let member_session_id_str = format!(
                                "{}:{}->{}",
                                m.session.begin_string,
                                m.session.sender_comp_id,
                                m.session.target_comp_id
                            );
                            let member_log = if let Some(kind) = m.log_kind {
                                Some(build_log_kind(kind, &m.log, &member_session_id_str)?)
                            } else if let Some(spec) = &m.sql_log {
                                Some(build_sql_log(spec, &member_session_id_str).await?)
                            } else if let Some(spec) = &m.log {
                                Some(build_log(spec, &member_session_id_str)?)
                            } else {
                                None
                            };
                            if let Some(log) = &member_log {
                                logs.push(log.clone());
                            }
                            let member_fixt_dictionaries =
                                m.fixt_dictionaries.clone().map(|dicts| {
                                    let opts = m.validator.as_ref().map_or_else(
                                        truefix_dict::ValidationOptions::default,
                                        |(_, opts)| *opts,
                                    );
                                    (dicts, opts)
                                });
                            builder = builder.with_session_services(
                                member_session_id.clone(),
                                Services {
                                    log: member_log,
                                    validator: m.validator.clone(),
                                    fixt_dictionaries: member_fixt_dictionaries,
                                    monitor: Some(monitor.clone()),
                                    ..Services::default()
                                },
                            );
                        }
                        builder = builder
                            .with_session_store(member_session_id, member_store)
                            .with_session(m.session);
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
                        addr = ?addr,
                        error = %e,
                        "skipping acceptor group after a startup error (ContinueInitializationOnError=Y)"
                    );
                } else {
                    // BUG-11/FR-021 (feature 006): clean up every session that already started
                    // successfully before propagating the error, instead of dropping the local
                    // handle `Vec`s and leaving them running (uncontrollable but not disconnected
                    // — `JoinHandle::drop` doesn't abort a task, and `SessionHandle` has no
                    // `Drop` impl).
                    cleanup_partial_start(&acceptors, &initiators, &failover_initiators).await;
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
                let log = if let Some(kind) = rs.log_kind {
                    Some(build_log_kind(kind, &rs.log, &session_id)?)
                } else if let Some(spec) = &rs.sql_log {
                    Some(build_sql_log(spec, &session_id).await?)
                } else if let Some(spec) = &rs.log {
                    Some(build_log(spec, &session_id)?)
                } else {
                    None
                };
                let services = Services {
                    store: Some(Arc::from(store)),
                    socket_options: to_transport_socket_options(rs.socket_options),
                    monitor: Some(monitor.clone()),
                    log,
                    trusted_proxy_addresses: rs.trusted_proxy_addresses.clone(),
                    sync_write_timeout: rs.sync_write_timeout,
                    validator: rs.validator.clone(),
                    fixt_dictionaries: rs.fixt_dictionaries.clone().map(|dicts| {
                        let opts = rs.validator.as_ref().map_or_else(
                            truefix_dict::ValidationOptions::default,
                            |(_, opts)| *opts,
                        );
                        (dicts, opts)
                    }),
                    // NEW-96 (feature 009): threaded through for consistency with the grouped
                    // acceptor path, though only `route_and_run` (the multi-session routing path)
                    // actually consumes it -- inert for a single-session `Acceptor`/initiator.
                    log_message_when_session_not_found: rs.log_message_when_session_not_found,
                    ..Services::default()
                };
                if let Some(log) = &services.log {
                    logs.push(log.clone());
                }
                match rs.connection {
                    ConnectionType::Acceptor => {
                        let bind_addr = resolve_socket_endpoint(&rs.address)
                            .await
                            .map_err(|e| EngineError::Io(e.to_string()))?;
                        let mut acc =
                            Acceptor::bind_with(bind_addr, rs.session, app.clone(), services)
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
                        let proxy = if let Some(spec) = rs.proxy.as_ref() {
                            Some(
                                to_transport_proxy_config(spec)
                                    .await
                                    .map_err(|e| EngineError::Proxy(e.to_string()))?,
                            )
                        } else {
                            None
                        };

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
                                addrs.push(rs.address.clone());
                                addrs.extend(rs.failover_addresses.iter().cloned());
                                let handle = if let Some(tls) = &rs.tls {
                                    let client_cfg = build_client_config(tls)
                                        .map_err(|e| EngineError::Tls(e.to_string()))?;
                                    let host = tls.server_name.clone().unwrap_or_default();
                                    let server_name =
                                        rustls::pki_types::ServerName::try_from(host)
                                            .map_err(|e| EngineError::Tls(e.to_string()))?;
                                    connect_initiator_reconnecting_endpoints_tls(
                                        addrs,
                                        rs.session,
                                        app.clone(),
                                        services,
                                        client_cfg,
                                        server_name,
                                    )
                                } else {
                                    connect_initiator_reconnecting_endpoints(
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
                                    rs.address.host(),
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
                                rs.address.host(),
                                rs.address.port(),
                                rs.session,
                                app.clone(),
                                services,
                            )
                            .await
                            .map_err(|e| EngineError::Proxy(e.to_string()))?,
                            (Some(tls), None) => {
                                let target_addr = resolve_socket_endpoint(&rs.address)
                                    .await
                                    .map_err(|e| EngineError::Io(e.to_string()))?;
                                let client_cfg = build_client_config(tls)
                                    .map_err(|e| EngineError::Tls(e.to_string()))?;
                                let host = tls.server_name.clone().unwrap_or_default();
                                let server_name = rustls::pki_types::ServerName::try_from(host)
                                    .map_err(|e| EngineError::Tls(e.to_string()))?;
                                connect_initiator_tls(
                                    target_addr,
                                    rs.session,
                                    app.clone(),
                                    services,
                                    client_cfg,
                                    server_name,
                                )
                                .await
                                .map_err(|e| EngineError::Io(e.to_string()))?
                            }
                            (None, None) => {
                                let target_addr = resolve_socket_endpoint(&rs.address)
                                    .await
                                    .map_err(|e| EngineError::Io(e.to_string()))?;
                                connect_initiator_with(
                                    target_addr,
                                    rs.session,
                                    app.clone(),
                                    services,
                                )
                                .await
                                .map_err(|e| EngineError::Io(e.to_string()))?
                            }
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
                    // BUG-11/FR-021 (feature 006): see the acceptor-group loop's identical
                    // cleanup call above.
                    cleanup_partial_start(&acceptors, &initiators, &failover_initiators).await;
                    return Err(e);
                }
            }
        }
        Ok(Self {
            acceptors,
            initiators,
            failover_initiators,
            logs,
            monitor,
            skipped_sessions: skipped,
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

    /// Every log built for a session this `Engine` started (NEW-91, feature 009). `shutdown()`/
    /// `Drop` don't flush these (both stay synchronous by design, see [`Engine::shutdown`]'s own
    /// doc) — a caller wanting queued async-log entries flushed before tearing down should await
    /// [`truefix_log::Log::shutdown`] on each of these first, the same way a caller wanting a
    /// graceful plain-initiator stop calls `.logout().await` on each of [`Engine::initiators`].
    pub fn logs(&self) -> &[Arc<dyn truefix_log::Log>] {
        &self.logs
    }

    /// The operational monitor shared by every session this `Engine` started (NEW-122/NEW-123,
    /// audit 006): `monitor().sessions()` lists every currently-connected `SessionId` (acceptor or
    /// initiator, static or dynamically-instantiated), `monitor().status(id)`/`is_connected(id)`
    /// query a specific one, and `monitor().force_logout(id)`/`reset(id)`/`send_app(id, msg)`
    /// control it — including acceptor sessions, which previously had no per-session handle from
    /// `Engine` at all.
    pub fn monitor(&self) -> &Monitor {
        &self.monitor
    }

    /// Sessions skipped at configuration-resolution time via a session's own
    /// `ContinueInitializationOnError=Y` (NEW-143, audit 006): each entry pairs the session's label
    /// (`SenderCompID->TargetCompID`) with the [`ConfigError`] that caused it to be skipped.
    /// Previously only logged via `tracing::error!` inside [`Engine::start`] and then discarded,
    /// with no programmatic access for a caller wanting to enumerate what failed to start.
    pub fn skipped_sessions(&self) -> &[(String, ConfigError)] {
        &self.skipped_sessions
    }

    /// Abort all acceptor listeners, stop all failover-initiator reconnect loops, and (BUG-27/
    /// FR-013, feature 007) abort every plain (non-failover) initiator too — every session task
    /// this `Engine` started is stopped when this returns.
    ///
    /// The plain-initiator stop is a synchronous, non-graceful `SessionHandle::abort()` (no
    /// Logout is sent) — this method's own signature stays synchronous (BUG-21/FR-023, feature
    /// 006's doc-comment-accuracy fix predates this: earlier versions read as if this were already
    /// a full stop, before BUG-27 actually made it one). Callers wanting a *graceful* stop for
    /// plain initiators should call `.logout().await` on each handle from `Self::initiators()`
    /// themselves, before calling `shutdown()`.
    pub fn shutdown(&self) {
        abort_everything(&self.acceptors, &self.initiators, &self.failover_initiators);
    }
}

impl Drop for Engine {
    /// BUG-27/FR-013 (feature 007): a safety net for callers that drop an `Engine` value without
    /// calling `shutdown()` explicitly — previously every spawned task (acceptors, initiators,
    /// failover loops) was silently detached and kept running forever. `Drop::drop` cannot
    /// `.await`, so this performs the same synchronous, non-graceful abort `shutdown()` does, not
    /// a substitute for calling `shutdown()` (or logging out gracefully) when that's possible.
    fn drop(&mut self) {
        abort_everything(&self.acceptors, &self.initiators, &self.failover_initiators);
    }
}

/// Shared by [`Engine::shutdown`] and `impl Drop for Engine` (BUG-27/FR-013, feature 007):
/// synchronously, non-gracefully stop every session task this `Engine` started.
fn abort_everything(
    acceptors: &[JoinHandle<()>],
    initiators: &[SessionHandle],
    failover_initiators: &[ReconnectHandle],
) {
    abort_acceptors_and_stop_failover(acceptors, failover_initiators);
    for initiator in initiators {
        initiator.abort();
    }
}

/// Shared by [`Engine::shutdown`] and [`Engine::start`]'s partial-failure cleanup: abort every
/// acceptor listener and stop every failover-initiator reconnect loop. Synchronous, so it cannot
/// also stop plain initiators (see [`Engine::shutdown`]'s doc comment) — [`cleanup_partial_start`]
/// layers that on top for the one caller (`Engine::start`) that can afford to `.await` it.
fn abort_acceptors_and_stop_failover(
    acceptors: &[JoinHandle<()>],
    failover_initiators: &[ReconnectHandle],
) {
    for acceptor in acceptors {
        acceptor.abort();
    }
    for initiator in failover_initiators {
        initiator.stop();
    }
}

/// BUG-11/FR-021 (feature 006): clean up every session `Engine::start` already started
/// successfully before it returns an error for a later session, so the caller never loses control
/// of a running-but-unreachable session. Unlike [`Engine::shutdown`], this runs inside `start`'s
/// own async context, so it can also gracefully log out plain (non-failover) initiators.
async fn cleanup_partial_start(
    acceptors: &[JoinHandle<()>],
    initiators: &[SessionHandle],
    failover_initiators: &[ReconnectHandle],
) {
    abort_acceptors_and_stop_failover(acceptors, failover_initiators);
    for initiator in initiators {
        initiator.logout().await;
    }
}

/// Sort `HashMap` entries by their `SocketEndpoint` key's `(host, port)` (T164/T165, feature 009,
/// NEW-92) — `HashMap` iteration order is randomized (a freshly-keyed hasher per instance), so
/// consuming one directly would start `Engine::start`'s acceptor-group listeners in a different,
/// unreproducible order across runs over the identical config.
fn sorted_by_endpoint<T>(map: HashMap<SocketEndpoint, T>) -> Vec<(SocketEndpoint, T)> {
    let mut entries: Vec<_> = map.into_iter().collect();
    entries.sort_by(|(a, _), (b, _)| (a.host(), a.port()).cmp(&(b.host(), b.port())));
    entries
}

#[cfg(test)]
mod acceptor_group_start_order_tests {
    use super::sorted_by_endpoint;
    use truefix_config::SocketEndpoint;

    /// T164 (feature 009, NEW-92): a failing-test-first check of `sorted_by_endpoint` in
    /// isolation — `Engine::start`'s actual acceptor-group construction has no public
    /// constructor path for `ResolvedSession` reachable from an external integration test, so
    /// this exercises the exact sort this fix added directly, the same way
    /// `scheduled_initiator_nonblocking_connect_tests` (`crates/truefix-transport/src/lib.rs`)
    /// unit-tests connect-timeout logic that isn't practically triggerable end-to-end either.
    #[test]
    fn sorted_by_endpoint_is_stable_regardless_of_input_order() {
        let a = SocketEndpoint::new("127.0.0.1", 1000);
        let b = SocketEndpoint::new("127.0.0.1", 2000);
        let c = SocketEndpoint::new("192.168.0.1", 500);

        let forward: std::collections::HashMap<SocketEndpoint, u32> =
            [(a.clone(), 1), (b.clone(), 2), (c.clone(), 3)]
                .into_iter()
                .collect();
        let reverse: std::collections::HashMap<SocketEndpoint, u32> =
            [(c.clone(), 3), (b.clone(), 2), (a.clone(), 1)]
                .into_iter()
                .collect();

        let expected = vec![(a, 1), (b, 2), (c, 3)]; // host "127.0.0.1" < "192.168.0.1"; 1000 < 2000
        assert_eq!(sorted_by_endpoint(forward), expected);
        assert_eq!(sorted_by_endpoint(reverse), expected);
    }
}
