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

use std::sync::Arc;

use tokio::task::JoinHandle;

use truefix_config::{ConnectionType, ProxyKind, ProxySpec, SessionSettings, SocketOptionsSpec};
use truefix_transport::{
    build_client_config, build_server_config, connect_initiator_tls, connect_initiator_via_proxy,
    connect_initiator_via_proxy_tls, connect_initiator_with, ProxyConfig, ProxyType, Services,
    SocketOptions,
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

/// A running engine: the started acceptor listeners and initiator sessions (FR-013/014).
///
/// Built from a [`SessionSettings`] via [`Engine::start`]; each `[SESSION]` is routed by its
/// `ConnectionType` to an acceptor or initiator with a store built from its configuration.
pub struct Engine {
    acceptors: Vec<JoinHandle<()>>,
    initiators: Vec<SessionHandle>,
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
        let resolved = settings.resolve()?;
        let mut acceptors = Vec::new();
        let mut initiators = Vec::new();
        for rs in resolved {
            let store = truefix_store::build_store(&rs.store)
                .await
                .map_err(|e| EngineError::Store(e.to_string()))?;
            let session_id = format!(
                "{}:{}->{}",
                rs.session.begin_string, rs.session.sender_comp_id, rs.session.target_comp_id
            );
            let log = rs
                .log
                .as_ref()
                .map(|spec| build_log(spec, &session_id))
                .transpose()?;
            let services = Services {
                store: Some(Arc::from(store)),
                socket_options: to_transport_socket_options(rs.socket_options),
                log,
                trusted_proxy_addresses: rs.trusted_proxy_addresses.clone(),
                sync_write_timeout: rs.sync_write_timeout,
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
                        (None, None) => {
                            connect_initiator_with(rs.address, rs.session, app.clone(), services)
                                .await
                                .map_err(|e| EngineError::Io(e.to_string()))?
                        }
                    };
                    initiators.push(handle);
                }
            }
        }
        Ok(Self {
            acceptors,
            initiators,
        })
    }

    /// The started initiator session handles (for logout/join).
    pub fn initiators(&self) -> &[SessionHandle] {
        &self.initiators
    }

    /// Abort all acceptor listeners.
    pub fn shutdown(&self) {
        for acceptor in &self.acceptors {
            acceptor.abort();
        }
    }
}
