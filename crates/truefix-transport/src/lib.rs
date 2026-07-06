//! `truefix-transport` — tokio TCP Initiator and Acceptor that drive a [`Session`].
//!
//! Single- and multi-session initiator/acceptor over TCP or TLS (rustls), framing FIX messages off
//! the stream and pumping the sans-IO [`Session`] engine. Supports routing by SessionID, dynamic
//! sessions, allow-listing, reconnect, optional [`Services`] (persistent [`MessageStore`], [`Log`],
//! socket options, [`Monitor`]), and an operational monitoring surface.
//!
//! Design: `specs/001-fix-engine-parity/`.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

mod metrics_export;
mod proxy;
mod tls_config;

pub use proxy::{ProxyConfig, ProxyError, ProxyType};
pub use tls_config::{
    TlsConfigError, TlsSpec, TlsVersion, build_client_config, build_server_config,
};

use std::collections::HashMap;
use std::future::pending;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, split};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval};

use truefix_config::SocketEndpoint;
use truefix_core::{DecodeError, Message, decode, decode_with_groups};
use truefix_log::Log;
use truefix_session::{
    Action, Application, Event, Role, Schedule, Session, SessionConfig, SessionId, SessionState,
    SessionStatus,
};
use truefix_store::MessageStore;

// B30 (feature 006): `truefix-transport` previously carried its own near-identical copy of
// `frame_length` — de-duplicated in favor of `truefix-core`'s (already public and already used by
// `truefix-at`), so the BUG-13 max-body-length bound only needs to be maintained in one place.
use truefix_core::frame_length;

/// TCP socket options applied to each connection (US10; FR-019).
#[derive(Debug, Clone, Copy)]
pub struct SocketOptions {
    /// Disable Nagle's algorithm (`SocketTcpNoDelay`).
    pub tcp_no_delay: bool,
    /// Enable TCP keepalive (`SocketKeepAlive`).
    pub keep_alive: bool,
    /// `SO_REUSEADDR` on the acceptor's listening socket (`SocketReuseAddress`); has no effect on
    /// an already-connected initiator stream, only on a freshly-bound listener.
    pub reuse_address: bool,
    /// `SO_LINGER` timeout (`SocketLinger`); `None` leaves the OS default in place.
    pub linger: Option<Duration>,
    /// `SO_OOBINLINE` (`SocketOobInline`).
    pub oob_inline: bool,
    /// `SO_RCVBUF` size in bytes (`SocketReceiveBufferSize`); `None` leaves the OS default.
    pub recv_buffer_size: Option<usize>,
    /// `SO_SNDBUF` size in bytes (`SocketSendBufferSize`); `None` leaves the OS default.
    pub send_buffer_size: Option<usize>,
    /// IP_TOS / traffic class (`SocketTrafficClass`); `None` leaves the OS default.
    pub traffic_class: Option<u32>,
}

impl Default for SocketOptions {
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

impl SocketOptions {
    /// Apply the connection-level options to a connected/accepted stream (best-effort: each
    /// option that fails to apply — e.g. unsupported on the platform — is silently skipped rather
    /// than aborting the connection).
    pub fn apply(&self, stream: &TcpStream) {
        let _ = stream.set_nodelay(self.tcp_no_delay);
        let sock = socket2::SockRef::from(stream);
        let _ = sock.set_keepalive(self.keep_alive);
        if let Some(linger) = self.linger {
            let _ = sock.set_linger(Some(linger));
        }
        let _ = sock.set_out_of_band_inline(self.oob_inline);
        if let Some(size) = self.recv_buffer_size {
            let _ = sock.set_recv_buffer_size(size);
        }
        if let Some(size) = self.send_buffer_size {
            let _ = sock.set_send_buffer_size(size);
        }
        if let Some(tos) = self.traffic_class {
            let _ = sock.set_tos(tos);
        }
        // `reuse_address` applies to a listener at bind time (see `bind_listener_with_options`),
        // not to an already-connected stream.
    }
}

/// Bind a `TcpListener`, optionally setting `SO_REUSEADDR` before binding (`SocketReuseAddress`).
/// Plain `TcpListener::bind` cannot express this — the option must be set on the raw socket
/// before the bind syscall.
fn bind_listener_with_options(addr: SocketAddr, reuse_address: bool) -> io::Result<TcpListener> {
    let domain = if addr.is_ipv4() {
        socket2::Domain::IPV4
    } else {
        socket2::Domain::IPV6
    };
    let socket = socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
    if reuse_address {
        socket.set_reuse_address(true)?;
    }
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    TcpListener::from_std(socket.into())
}

/// Connect out to `addr` (GAP-15/FR-015 + GAP-16/FR-016, feature 005), optionally binding to
/// `local_bind_addr` first (`SocketLocalHost`/`SocketLocalPort`) and/or bounding the whole attempt
/// by `connect_timeout` (`SocketConnectTimeout`). `None` for either preserves today's behavior
/// (OS-chosen ephemeral port, unbounded wait) exactly.
async fn tcp_connect(
    addr: SocketAddr,
    local_bind_addr: Option<SocketAddr>,
    connect_timeout: Option<Duration>,
) -> io::Result<TcpStream> {
    let connect = async move {
        match local_bind_addr {
            None => TcpStream::connect(addr).await,
            Some(local) => {
                // A blocking bind-then-connect on a worker thread avoids the nonblocking-connect
                // EINPROGRESS/readiness dance; connects are infrequent enough that the blocking
                // thread cost is immaterial.
                tokio::task::spawn_blocking(move || -> io::Result<std::net::TcpStream> {
                    let domain = if addr.is_ipv4() {
                        socket2::Domain::IPV4
                    } else {
                        socket2::Domain::IPV6
                    };
                    let socket = socket2::Socket::new(
                        domain,
                        socket2::Type::STREAM,
                        Some(socket2::Protocol::TCP),
                    )?;
                    socket.bind(&local.into())?;
                    socket.connect(&addr.into())?;
                    socket.set_nonblocking(true)?;
                    Ok(socket.into())
                })
                .await
                .unwrap_or_else(|e| Err(io::Error::other(e)))
                .and_then(TcpStream::from_std)
            }
        }
    };
    with_connect_timeout(connect_timeout, connect).await
}

/// GAP-16/FR-016 (feature 005): bound `fut` by `dur` when set, surfacing a typed
/// `ErrorKind::TimedOut` on expiry rather than blocking indefinitely. `None` preserves the
/// unbounded wait. Split out from [`tcp_connect`] so the timeout-wrapping behavior itself is
/// unit-testable with `tokio::time::pause()`, without depending on real network timing.
async fn with_connect_timeout<F, T>(dur: Option<Duration>, fut: F) -> io::Result<T>
where
    F: std::future::Future<Output = io::Result<T>>,
{
    match dur {
        None => fut.await,
        Some(dur) => tokio::time::timeout(dur, fut).await.unwrap_or_else(|_| {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "connect attempt timed out",
            ))
        }),
    }
}

/// Optional per-session services: a persistent message store, a log, and socket options.
#[derive(Clone, Default)]
pub struct Services {
    /// Persistent store for sequence-number continuity (and future cross-restart resend).
    pub store: Option<Arc<dyn MessageStore>>,
    /// Message/event log.
    pub log: Option<Arc<dyn Log>>,
    /// TCP socket options.
    pub socket_options: SocketOptions,
    /// Operational monitor.
    pub monitor: Option<Monitor>,
    /// Optional inbound application-message validator (dictionary + toggles).
    pub validator: Option<(
        truefix_dict::DataDictionary,
        truefix_dict::ValidationOptions,
    )>,
    /// A real FIXT 1.1 transport/application dictionary split (T079/GAP-18c part 2), taking
    /// precedence over `validator` for application-message validation (see
    /// `Session::set_fixt_dictionaries`'s doc) when set.
    pub fixt_dictionaries: Option<(
        truefix_dict::FixtDictionaries,
        truefix_dict::ValidationOptions,
    )>,
    /// Log a message via `log` when an inbound connection's Logon matches neither a static session
    /// nor a dynamic template, before the connection is refused (`LogMessageWhenSessionNotFound`;
    /// acceptor/routing-level, so it lives here rather than on `SessionConfig`).
    pub log_message_when_session_not_found: bool,
    /// PROXY protocol (v1/v2) is parsed only from a connection whose *physical* peer address is
    /// in this set (`UseTCPProxy` + `TrustedProxyAddresses`; FR-015); empty disables PROXY
    /// protocol entirely. Applied by both [`Acceptor`] and [`AcceptorBuilder`].
    pub trusted_proxy_addresses: Vec<IpAddr>,
    /// `SocketSynchronousWrites` + `SocketSynchronousWriteTimeout` (FR-017): when set, an
    /// outbound write exceeding this duration surfaces a typed timeout (logged and the
    /// connection is torn down) rather than blocking indefinitely. `None` (the default) leaves
    /// writes unbounded, as before.
    pub sync_write_timeout: Option<Duration>,
    /// Optional hostname resolver override. Production uses Tokio's asynchronous resolver; this
    /// hook also makes reconnect-time DNS behavior deterministic in integration tests.
    pub address_resolver: Option<Arc<dyn AddressResolver>>,
}

/// Asynchronous hostname resolution used by reconnecting initiators.
#[async_trait::async_trait]
pub trait AddressResolver: Send + Sync {
    /// Resolve every address currently associated with `endpoint`.
    async fn resolve(&self, endpoint: &SocketEndpoint) -> io::Result<Vec<SocketAddr>>;
}

async fn resolve_endpoint(
    endpoint: &SocketEndpoint,
    resolver: Option<&dyn AddressResolver>,
) -> io::Result<Vec<SocketAddr>> {
    if let Some(resolver) = resolver {
        return resolver.resolve(endpoint).await;
    }
    tokio::net::lookup_host((endpoint.host(), endpoint.port()))
        .await
        .map(Iterator::collect)
}

async fn connect_endpoint(
    endpoint: &SocketEndpoint,
    resolver: Option<&dyn AddressResolver>,
    local_bind_addr: Option<SocketAddr>,
    connect_timeout: Option<Duration>,
) -> io::Result<TcpStream> {
    let addrs = resolve_endpoint(endpoint, resolver).await?;
    let mut last_error = None;
    for addr in addrs {
        match tcp_connect(addr, local_bind_addr, connect_timeout).await {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "could not resolve address {}:{}",
                endpoint.host(),
                endpoint.port()
            ),
        )
    }))
}

/// Control messages to a running session task.
enum Control {
    /// Begin a graceful logout.
    Logout,
    /// Operationally reset the session's sequence numbers and store.
    Reset,
    /// Send an application message on this session.
    Send(Box<Message>),
}

/// Operational monitoring (FR-L): live session status plus reset / force-logout actions
/// (the JMX-equivalent surface).
#[derive(Clone, Default)]
pub struct Monitor {
    inner: Arc<Mutex<HashMap<SessionId, MonitorEntry>>>,
}

struct MonitorEntry {
    status: SessionStatus,
    connected: bool,
    control: mpsc::Sender<Control>,
}

impl Monitor {
    /// Create an empty monitor.
    pub fn new() -> Self {
        Self::default()
    }

    fn register(&self, id: SessionId, status: SessionStatus, control: mpsc::Sender<Control>) {
        if let Ok(mut map) = self.inner.lock() {
            map.insert(
                id,
                MonitorEntry {
                    status,
                    connected: true,
                    control,
                },
            );
        }
    }

    fn update(&self, id: &SessionId, status: SessionStatus) {
        if let Ok(mut map) = self.inner.lock()
            && let Some(entry) = map.get_mut(id)
        {
            entry.status = status;
        }
    }

    fn mark_disconnected(&self, id: &SessionId) {
        if let Ok(mut map) = self.inner.lock()
            && let Some(entry) = map.get_mut(id)
        {
            entry.connected = false;
        }
    }

    /// The latest status snapshot for `id`.
    pub fn status(&self, id: &SessionId) -> Option<SessionStatus> {
        self.inner.lock().ok()?.get(id).map(|e| e.status)
    }

    /// Whether `id`'s connection is currently up.
    pub fn is_connected(&self, id: &SessionId) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|m| m.get(id).map(|e| e.connected))
            .unwrap_or(false)
    }

    /// All known session ids.
    pub fn sessions(&self) -> Vec<SessionId> {
        self.inner
            .lock()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn control_for(&self, id: &SessionId) -> Option<mpsc::Sender<Control>> {
        self.inner.lock().ok()?.get(id).map(|e| e.control.clone())
    }

    /// Request a graceful logout of `id`.
    pub async fn force_logout(&self, id: &SessionId) -> bool {
        match self.control_for(id) {
            Some(c) => c.send(Control::Logout).await.is_ok(),
            None => false,
        }
    }

    /// Request an operational sequence reset of `id`.
    pub async fn reset(&self, id: &SessionId) -> bool {
        match self.control_for(id) {
            Some(c) => c.send(Control::Reset).await.is_ok(),
            None => false,
        }
    }

    /// Send an application `message` on session `id`. The engine stamps the header
    /// (sequence number, comp IDs, SendingTime).
    pub async fn send_app(&self, id: &SessionId, message: Message) -> bool {
        match self.control_for(id) {
            Some(c) => c.send(Control::Send(Box::new(message))).await.is_ok(),
            None => false,
        }
    }
}

/// A handle to a running session task.
pub struct SessionHandle {
    control: mpsc::Sender<Control>,
    task: JoinHandle<()>,
}

impl SessionHandle {
    /// Request a graceful logout.
    pub async fn logout(&self) {
        let _ = self.control.send(Control::Logout).await;
    }

    /// Send an application message on this session.
    pub async fn send(&self, message: Message) {
        let _ = self.control.send(Control::Send(Box::new(message))).await;
    }

    /// Wait for the session task to finish.
    pub async fn join(self) {
        let _ = self.task.await;
    }

    /// Synchronously, non-gracefully abort the underlying connection task (BUG-27/FR-013, feature
    /// 007) — no Logout is sent; this is a hard stop, not a courtesy disconnect. Used where an
    /// async `.logout().await` isn't available (`Engine::shutdown()`'s synchronous signature,
    /// `impl Drop for Engine`) or isn't appropriate (a caller that already knows the connection is
    /// unresponsive). Prefer `logout()` when a graceful stop is possible.
    pub fn abort(&self) {
        self.task.abort();
    }

    /// Whether the underlying connection task has already finished (BUG-25, feature 007) —
    /// non-consuming, unlike `join`, so a caller can poll it repeatedly (e.g.
    /// `run_scheduled_initiator`'s reconnect loop, to detect a dropped connection and become
    /// eligible to reconnect).
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }
}

/// Connect out as an initiator and run the session.
pub async fn connect_initiator<A>(
    addr: SocketAddr,
    config: SessionConfig,
    app: Arc<A>,
) -> io::Result<SessionHandle>
where
    A: Application + 'static,
{
    connect_initiator_with(addr, config, app, Services::default()).await
}

/// Connect out as an initiator with persistent store/log services.
pub async fn connect_initiator_with<A>(
    addr: SocketAddr,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
) -> io::Result<SessionHandle>
where
    A: Application + 'static,
{
    let stream = tcp_connect(addr, config.local_bind_addr, config.connect_timeout).await?;
    Ok(spawn_session(stream, config, app, services))
}

/// Connect out as an initiator over TLS (FR-F6).
///
/// `server_name` is the SNI/host name presented to the server (e.g. `"example.com"`).
pub async fn connect_initiator_tls<A>(
    addr: SocketAddr,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    tls: Arc<rustls::ClientConfig>,
    server_name: rustls::pki_types::ServerName<'static>,
) -> io::Result<SessionHandle>
where
    A: Application + 'static,
{
    let tcp = tcp_connect(addr, config.local_bind_addr, config.connect_timeout).await?;
    services.socket_options.apply(&tcp);
    let connector = tokio_rustls::TlsConnector::from(tls);
    let stream = connector.connect(server_name, tcp).await?;
    Ok(spawn_session_io(stream, config, app, services))
}

/// Connect out as an initiator through a configured forward proxy (SOCKS4/SOCKS5/HTTP CONNECT;
/// FR-016). Unlike `connect_initiator*`'s `SocketAddr`, `target_host`/`target_port` is resolved on
/// the proxy side, not locally — the proxy performs the DNS lookup (or, for SOCKS4, the caller is
/// expected to have already resolved the address, per the protocol's own limitation).
pub async fn connect_initiator_via_proxy<A>(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
) -> Result<SessionHandle, ProxyError>
where
    A: Application + 'static,
{
    let stream =
        connect_through_proxy_with_timeout(proxy, target_host, target_port, config.connect_timeout)
            .await?;
    services.socket_options.apply(&stream);
    Ok(spawn_session_io(stream, config, app, services))
}

/// BUG-19 (feature 006): bound the proxy connect path by `connect_timeout`, matching the
/// direct/TLS initiator paths (`tcp_connect`/`with_connect_timeout`) — a proxy that accepts the
/// TCP connection but never completes its SOCKS/CONNECT handshake would otherwise hang the
/// connect attempt indefinitely.
async fn connect_through_proxy_with_timeout(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
    connect_timeout: Option<Duration>,
) -> Result<TcpStream, ProxyError> {
    match connect_timeout {
        None => proxy::connect_through_proxy(proxy, target_host, target_port).await,
        Some(dur) => tokio::time::timeout(
            dur,
            proxy::connect_through_proxy(proxy, target_host, target_port),
        )
        .await
        .unwrap_or_else(|_| {
            Err(ProxyError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                "proxy connect timed out",
            )))
        }),
    }
}

/// As [`connect_initiator_via_proxy`], but wraps the tunneled stream in TLS afterward (FR-016 +
/// FR-017).
#[allow(clippy::too_many_arguments)]
pub async fn connect_initiator_via_proxy_tls<A>(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    tls: Arc<rustls::ClientConfig>,
    server_name: rustls::pki_types::ServerName<'static>,
) -> Result<SessionHandle, ProxyError>
where
    A: Application + 'static,
{
    let tcp =
        connect_through_proxy_with_timeout(proxy, target_host, target_port, config.connect_timeout)
            .await?;
    services.socket_options.apply(&tcp);
    let connector = tokio_rustls::TlsConnector::from(tls);
    let stream = connector
        .connect(server_name, tcp)
        .await
        .map_err(ProxyError::Io)?;
    Ok(spawn_session_io(stream, config, app, services))
}

/// A listening acceptor that serves one static session configuration per connection.
pub struct Acceptor<A: Application + 'static> {
    listener: TcpListener,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    tls: Option<Arc<rustls::ServerConfig>>,
}

/// BUG-32/FR-008 (feature 007): unlike `AcceptorBuilder`'s multi-session `Registry`, a single-session
/// `Acceptor` has exactly one identity (`config`'s fixed `SenderCompID`/`TargetCompID`), so a bare
/// `bool` behind a mutex is enough to track whether that one identity currently has an active
/// connection.
type SingleSessionActive = Arc<std::sync::Mutex<bool>>;

/// Clears the single-session active flag when the connection's task finishes, regardless of how it
/// finishes (normal completion, error, or panic-unwind) — mirrors `ActiveGuard` in `route_and_run`.
struct SingleSessionActiveGuard {
    active: SingleSessionActive,
}
impl Drop for SingleSessionActiveGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.active.lock() {
            *guard = false;
        }
    }
}

impl<A: Application + 'static> Acceptor<A> {
    /// Bind to `addr`.
    pub async fn bind(addr: SocketAddr, config: SessionConfig, app: Arc<A>) -> io::Result<Self> {
        Self::bind_with(addr, config, app, Services::default()).await
    }

    /// Bind to `addr` with persistent store/log services.
    pub async fn bind_with(
        addr: SocketAddr,
        config: SessionConfig,
        app: Arc<A>,
        services: Services,
    ) -> io::Result<Self> {
        let listener = bind_listener_with_options(addr, services.socket_options.reuse_address)?;
        Ok(Self {
            listener,
            config,
            app,
            services,
            tls: None,
        })
    }

    /// Serve TLS using `config` (FR-F6/FR-017).
    #[must_use]
    pub fn with_tls(mut self, config: Arc<rustls::ServerConfig>) -> Self {
        self.tls = Some(config);
        self
    }

    /// The bound local address (useful when binding to port 0).
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Spawn the accept loop. Each connection is served as a session.
    pub fn serve(self) -> JoinHandle<()> {
        let tls = self.tls.map(tokio_rustls::TlsAcceptor::from);
        let active: SingleSessionActive = Arc::new(std::sync::Mutex::new(false));
        tokio::spawn(async move {
            while let Ok((mut stream, peer)) = self.listener.accept().await {
                // PROXY protocol (FR-015): a single-session acceptor has no allow-list to gate
                // on, but a trusted upstream's header is still stripped so its bytes are never
                // mistaken for the start of the FIX stream.
                let _ = proxy::strip_trusted_proxy_header(
                    &mut stream,
                    peer,
                    &self.services.trusted_proxy_addresses,
                )
                .await;
                self.services.socket_options.apply(&stream);

                // BUG-32/FR-008 (feature 007): refuse a second connection while this acceptor's
                // one session identity already has an active connection, for the same reason
                // `route_and_run`'s `Registry.active` check exists on the multi-session path — a
                // competing connection for the same identity would corrupt shared sequence-number
                // bookkeeping.
                {
                    let mut guard = match active.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    if *guard {
                        if let Some(log) = &self.services.log {
                            log.on_event(&format!(
                                "refused a second connection for already-connected session {:?}/{:?}",
                                self.config.sender_comp_id, self.config.target_comp_id
                            ));
                        }
                        continue;
                    }
                    *guard = true;
                }

                let config = self.config.clone();
                let app = self.app.clone();
                let services = self.services.clone();
                let guard = SingleSessionActiveGuard {
                    active: active.clone(),
                };
                match &tls {
                    Some(acceptor) => {
                        let acceptor = acceptor.clone();
                        tokio::spawn(async move {
                            let _guard = guard;
                            if let Ok(tls_stream) = acceptor.accept(stream).await {
                                let handle = spawn_session_io(tls_stream, config, app, services);
                                let _ = handle.task.await;
                            }
                        });
                    }
                    None => {
                        tokio::spawn(async move {
                            let _guard = guard;
                            let handle = spawn_session_io(stream, config, app, services);
                            let _ = handle.task.await;
                        });
                    }
                }
            }
        })
    }
}

fn spawn_session<A>(
    stream: TcpStream,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
) -> SessionHandle
where
    A: Application + 'static,
{
    services.socket_options.apply(&stream);
    spawn_session_io(stream, config, app, services)
}

fn spawn_session_io<A, S>(
    stream: S,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
) -> SessionHandle
where
    A: Application + 'static,
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (control_tx, control_rx) = mpsc::channel(8);
    let task = {
        let control_tx = control_tx.clone();
        tokio::spawn(async move {
            run_connection(
                stream,
                config,
                app,
                services,
                control_tx,
                control_rx,
                Vec::new(),
            )
            .await;
        })
    };
    SessionHandle {
        control: control_tx,
        task,
    }
}

/// An item forwarded from the reader task to the session-processing loop: either a fully decoded
/// message, or a garbled (undecodable) frame that still needs `RejectGarbledMessage` handling
/// (US14, FR-019).
enum Inbound {
    /// A fully decoded inbound message.
    Message(Message),
    /// A frame whose length was determinable but whose content failed to decode.
    Garbled,
}

/// The inbound *application*-message channel sender, present only when `in_chan_capacity` is
/// `Some(n)` (US14, FR-019). When `None` (default), there is no separate application channel at
/// all: every message — admin or application — is routed through the single, always-unbounded
/// admin channel below, in strict wire order, reproducing pre-US14 behavior exactly.
///
/// **Trade-off, accepted deliberately** (spec Clarifications) when a capacity *is* configured:
/// administrative traffic is drained with priority ahead of the bounded application channel, so
/// under backpressure a message that arrives on the wire *after* a still-queued application
/// message can be processed *before* it. The session's existing out-of-order/gap-fill handling (a
/// `ResendRequest`) already tolerates this safely — it's the same mechanism that recovers from
/// genuine network reordering — but it *is* a real behavior change from strict wire order, which
/// is exactly why the whole split is opt-in (`in_chan_capacity: None` never engages it).
type AppSender = mpsc::Sender<Inbound>;

/// Capacity of the admin-message channel (BUG-53/FR-045, feature 007) — see its construction site
/// in `run_connection` for the rationale. Not user-configurable (unlike `in_chan_capacity`): this
/// is a memory-safety bound, not a behavior trade-off an integrator should need to tune.
const ADMIN_CHANNEL_CAPACITY: usize = 256;

/// Build the application-message channel per `SessionConfig::in_chan_capacity` (US14, FR-019).
/// `None` capacity returns `(None, _)` — the returned receiver's sole sender is dropped
/// immediately, so its `recv()` resolves to `None` on the processing loop's very first poll and
/// that channel is permanently inert; see [`AppSender`]'s doc for why.
fn app_channel(capacity: Option<usize>) -> (Option<AppSender>, mpsc::Receiver<Inbound>) {
    match capacity {
        Some(n) => {
            let (tx, rx) = mpsc::channel(n.max(1));
            (Some(tx), rx)
        }
        None => {
            let (_tx, rx) = mpsc::channel(1);
            (None, rx)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_connection<A, S>(
    stream: S,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    control_tx: mpsc::Sender<Control>,
    mut control: mpsc::Receiver<Control>,
    buf: Vec<u8>,
) where
    A: Application + 'static,
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let in_chan_capacity = config.in_chan_capacity;
    let mut session = Session::new(config);
    if let Some((dicts, opts)) = &services.fixt_dictionaries {
        session.set_fixt_dictionaries(dicts.clone(), *opts);
    } else if let Some((dict, opts)) = &services.validator {
        session.set_dictionary(dict.clone(), *opts);
    }
    let id = session.id().clone();
    app.on_create(&id).await;

    // Restore sequence numbers from the store (continuity across restarts).
    if let Some(store) = &services.store {
        let out = store.next_sender_seq().await.unwrap_or(1);
        let inn = store.next_target_seq().await.unwrap_or(1);
        session.seed_sequences(out, inn);
        // Re-hydrate previously-sent message bodies so a post-restart ResendRequest can replay them
        // (FR-001/002). The store is the source of truth; the session's in-memory map is rebuilt.
        if out > 1 {
            match store.get(1, out - 1).await {
                Ok(stored) => {
                    let msgs = stored
                        .into_iter()
                        .filter_map(|(seq, bytes)| decode(&bytes).ok().map(|m| (seq, m)));
                    session.seed_sent_messages(msgs);
                }
                // BUG-08/FR-013 (feature 006): a failed resend re-hydration read previously
                // failed silently, skipping re-hydration as if the store simply had nothing to
                // offer — now routed through the log as an operator-visible event.
                Err(e) => {
                    if let Some(log) = &services.log {
                        log.on_event(&format!("{id}: failed to re-hydrate sent messages: {e}"));
                    }
                }
            }
        }
        // ForceResendWhenCorruptedStore: recovered-but-untrusted history isn't safe to resend from,
        // so force a full reset (both sides realign via a fresh ResetSeqNumFlag logon) instead
        // (FR-008).
        if store.was_corrupted() && session.force_resend_when_corrupted_store() {
            app.on_before_reset(&id).await;
            session.reset();
            if let Err(e) = store.reset().await
                && let Some(log) = &services.log
            {
                log.on_event(&format!("{id}: store reset failed: {e}"));
            }
        }
    }

    if let Some(monitor) = &services.monitor {
        monitor.register(id.clone(), session.status(), control_tx);
    }

    // Split the stream: a spawned reader task owns the read half — it only frames/decodes/
    // classifies inbound bytes and forwards them onto the admin/application channels below, no
    // session-state logic — while this task keeps the write half and does all session processing
    // (US14, FR-019). This lets admin traffic (heartbeat, TestRequest, Logon/Logout, garbled-frame
    // handling) keep flowing, prioritized, even while a slow `Application` hook or a saturated
    // bounded application channel is stalling app-message processing.
    let (read_half, mut write_half) = split(stream);
    // BUG-53/FR-045 (feature 007): bounded, not unbounded — an unbounded admin channel let a peer
    // that keeps sending admin-classified traffic (heartbeats, TestRequests, garbled frames) faster
    // than this connection's processing loop can drain it grow this queue's memory without limit,
    // a DoS vector. `ADMIN_CHANNEL_CAPACITY` is generous enough that normal admin traffic (which is
    // driven by heartbeat intervals, not line-rate) never comes close to filling it; only a
    // genuinely abusive/malfunctioning peer applies backpressure, which then naturally propagates
    // to the reader (its `.send().await` below blocks) and from there to the TCP connection itself
    // (the reader stops draining the socket), rather than growing this process's memory.
    let (admin_tx, mut admin_rx) = mpsc::channel::<Inbound>(ADMIN_CHANNEL_CAPACITY);
    let (app_tx, mut app_rx) = app_channel(in_chan_capacity);
    let reader_services = services.clone();
    let reader_id = id.clone();
    let read_task = tokio::spawn(read_loop(
        read_half,
        buf,
        admin_tx,
        app_tx,
        reader_services,
        reader_id,
    ));
    // BUG-50/FR-043 (feature 007): abort the reader task whenever this connection's own task ends
    // for *any* reason, including being aborted externally mid-flight (`SessionHandle::abort`/
    // `ReconnectHandle::stop`). Without this, the reader task -- which owns the split `ReadHalf`
    // -- keeps running orphaned even after this function's own future is torn down; since
    // `tokio::io::split`'s underlying stream is only actually closed once *both* halves are
    // dropped, a peer would never observe the connection close at all (only our write half would
    // be gone, not the socket itself). `Drop` runs even when this async fn's own future is
    // cancelled mid-poll (not just on a normal return), so this guard covers every exit path.
    struct AbortReaderOnDrop(JoinHandle<()>);
    impl Drop for AbortReaderOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _read_task_guard = AbortReaderOnDrop(read_task);

    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut logged_on = false;
    let mut control_open = true;
    let mut closing = false;
    // Set once the reader task has ended *and* its channel has been fully drained (mpsc::recv
    // keeps yielding buffered items after the sender drops, only returning `None` once truly
    // empty) — the natural, no-message-lost way to detect "nothing more will ever arrive".
    let mut admin_done = false;
    let mut app_done = false;

    if dispatch(
        &mut session,
        Event::Connected,
        &mut write_half,
        &app,
        &id,
        &services,
        &mut logged_on,
    )
    .await
    .is_err()
    {
        closing = true;
    }

    while !closing {
        tokio::select! {
            biased;
            inbound = admin_rx.recv(), if !admin_done => match inbound {
                Some(item) => {
                    if handle_inbound(item, &mut session, &mut write_half, &app, &id, &services, &mut logged_on)
                        .await
                        .is_err()
                    {
                        closing = true;
                    }
                }
                None => admin_done = true,
            },
            inbound = app_rx.recv(), if !app_done => match inbound {
                Some(item) => {
                    if handle_inbound(item, &mut session, &mut write_half, &app, &id, &services, &mut logged_on)
                        .await
                        .is_err()
                    {
                        closing = true;
                    }
                }
                None => app_done = true,
            },
            _ = ticker.tick() => {
                if dispatch(&mut session, Event::Tick, &mut write_half, &app, &id, &services, &mut logged_on)
                    .await
                    .is_err()
                {
                    closing = true;
                }
            },
            ctrl = recv_control(&mut control, control_open) => match ctrl {
                Some(Control::Logout) => {
                    if dispatch(&mut session, Event::StartLogout, &mut write_half, &app, &id, &services, &mut logged_on)
                        .await
                        .is_err()
                    {
                        closing = true;
                    }
                }
                Some(Control::Reset) => {
                    app.on_before_reset(&id).await;
                    session.reset();
                    if let Some(store) = &services.store
                        && let Err(e) = store.reset().await
                            && let Some(log) = &services.log {
                                log.on_event(&format!("{id}: store reset failed: {e}"));
                            }
                    metrics_export::record_status(&id, &session.status());
                    if let Some(monitor) = &services.monitor {
                        monitor.update(&id, session.status());
                    }
                }
                Some(Control::Send(msg)) => {
                    if send_app(&mut session, *msg, &mut write_half, &app, &id, &services)
                        .await
                        .is_err()
                    {
                        closing = true;
                    }
                }
                None => control_open = false,
            },
        }
        if admin_done && app_done {
            closing = true;
        }
    }

    // BUG-33/FR-009 (feature 007): a raw TCP drop (as opposed to a graceful Logout) never used to
    // reach the state machine at all, so `ResetOnDisconnect`/`ResetOnLogout` were silently ignored
    // on that path. Dispatching `Event::Disconnected` here — through the same `dispatch`/
    // `perform_actions` pipeline every other event already uses — reaches `enter_disconnected()`
    // and propagates any resulting `Action::ResetStore` to the store exactly as it would for a
    // Logout-driven disconnect. This always returns `Err(())` (the event always leaves the session
    // in `Disconnected` state), so its result is intentionally discarded.
    //
    // NEW-56 (feature 009): this dispatch runs for *every* teardown, including ones an internal
    // handler (`on_logout_msg`, or a logon/logout/heartbeat/schedule timeout in `on_tick`) already
    // fully processed with its own correct reason. Re-using that already-recorded reason (rather
    // than a fixed default) makes this redundant second call idempotent instead of potentially
    // consulting the wrong flag; `Other` covers the remaining case (a raw TCP drop, or an
    // error-driven path that sets `state` directly and uses the separate `reset_on_error`
    // mechanism instead — neither is a completed graceful Logout).
    let reason = session
        .last_disconnect_reason()
        .unwrap_or(truefix_session::DisconnectReason::Other);
    let _ = dispatch(
        &mut session,
        Event::Disconnected(reason),
        &mut write_half,
        &app,
        &id,
        &services,
        &mut logged_on,
    )
    .await;

    let _ = write_half.shutdown().await;
    if let Some(monitor) = &services.monitor {
        monitor.mark_disconnected(&id);
    }
    // BUG-93/FR-029 (feature 007): fire on_logout whenever a Logon was sent or received on this
    // connection, not only once it reached full `logged_on` state -- matching QFJ (`logonReceived
    // || logonSent`) and QFGo, rather than silently skipping the callback for a connection that
    // dropped mid-handshake.
    if logged_on || session.logon_sent() || session.logon_received() {
        app.on_logout(&id).await;
        if let Some(log) = &services.log {
            log.on_event(&format!("{id}: logout"));
        }
    }
}

/// Await a control message, or never resolve once the channel is closed (avoids a busy loop).
async fn recv_control(control: &mut mpsc::Receiver<Control>, open: bool) -> Option<Control> {
    if open {
        control.recv().await
    } else {
        pending::<Option<Control>>().await
    }
}

/// Reads bytes off `read_half`, frames complete messages, decodes them, classifies each as
/// administrative or application traffic, and forwards it onto the corresponding channel — pure
/// I/O and framing, no session-state logic (that lives in [`handle_inbound`], run by
/// [`run_connection`]'s processing loop). Ends when the stream closes/errors, or the processing
/// loop has dropped its receivers (connection torn down from elsewhere).
///
/// Administrative items are forwarded via the bounded admin channel (`ADMIN_CHANNEL_CAPACITY`,
/// BUG-53/FR-045) — sized generously so ordinary admin traffic never blocks in practice, but
/// bounded so a peer that floods admin-classified messages faster than the processing loop can
/// drain them applies backpressure here (and from here, to the socket read below) instead of
/// growing this process's memory without limit. Application items, when a bounded channel is
/// configured (`in_chan_capacity: Some(n)`), are *not* sent inline: decoding stages them into
/// `pending_app` instead, and a separate loop below delivers them via `Sender::reserve()` —
/// cancel-safe, so it can run concurrently (via `select!`) with continuing to read more bytes.
/// This is what actually keeps admin traffic flowing ahead of a *separately* backed-up application
/// channel (US14, FR-019): an implementation that instead `.await`s each application send inline,
/// in wire order, would still let one blocked send stall every frame decoded *after* it in the
/// stream — including a later admin message — since a single sequential loop can't decode/forward
/// anything past a point it's stuck awaiting.
async fn read_loop<S: AsyncRead + Unpin>(
    mut read_half: S,
    mut buf: Vec<u8>,
    admin_tx: mpsc::Sender<Inbound>,
    app_tx: Option<AppSender>,
    services: Services,
    id: SessionId,
) {
    let mut chunk = [0u8; 4096];
    let mut pending_app: std::collections::VecDeque<Inbound> = std::collections::VecDeque::new();
    let has_app_channel = app_tx.is_some();

    // Drain any bytes pre-read while routing the connection (acceptor: the inbound Logon) before
    // blocking on the socket for more.
    if classify_buffered(
        &mut buf,
        &admin_tx,
        has_app_channel,
        &mut pending_app,
        &services,
        &id,
    )
    .await
    .is_err()
    {
        return;
    }

    let Some(app_tx) = app_tx else {
        // No bounded application channel at all (in_chan_capacity: None): a plain read loop,
        // identical to pre-US14 behavior (`pending_app` never gets used in this branch, since
        // `classify_buffered` sends everything straight through `admin_tx` when `has_app_channel`
        // is `false`).
        loop {
            match read_half.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    if let Some(slice) = chunk.get(..n) {
                        buf.extend_from_slice(slice);
                    }
                    if classify_buffered(
                        &mut buf,
                        &admin_tx,
                        false,
                        &mut pending_app,
                        &services,
                        &id,
                    )
                    .await
                    .is_err()
                    {
                        return;
                    }
                }
            }
        }
    };

    loop {
        tokio::select! {
            biased;
            permit = app_tx.reserve(), if !pending_app.is_empty() => {
                match permit {
                    Ok(permit) => {
                        if let Some(item) = pending_app.pop_front() {
                            permit.send(item);
                        }
                    }
                    Err(_) => return, // the processing loop's application receiver was dropped
                }
            }
            read = read_half.read(&mut chunk) => match read {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    if let Some(slice) = chunk.get(..n) {
                        buf.extend_from_slice(slice);
                    }
                    if classify_buffered(&mut buf, &admin_tx, true, &mut pending_app, &services, &id)
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            },
        }
    }
}

/// Frame and classify every complete message currently in `buf`. Administrative items (and
/// garbled frames) are forwarded via `admin_tx` — bounded (`ADMIN_CHANNEL_CAPACITY`, BUG-53/
/// FR-045), so `.send().await` here can itself apply backpressure to this function's caller
/// ([`read_loop`]) when a peer floods admin traffic faster than the processing loop drains it,
/// rather than growing memory without limit. Application items are appended to `pending_app` for
/// [`read_loop`] to deliver at its own pace, rather than sent here, so an application-channel
/// backlog specifically never blocks this function. When `has_app_channel` is `false`
/// (`in_chan_capacity: None`), every item — admin or application — goes through `admin_tx`
/// instead, in strict wire order, reproducing pre-US14 behavior exactly; see [`AppSender`]'s doc.
/// `Err` means the processing loop is gone (its admin receiver dropped).
async fn classify_buffered(
    buf: &mut Vec<u8>,
    admin_tx: &mpsc::Sender<Inbound>,
    has_app_channel: bool,
    pending_app: &mut std::collections::VecDeque<Inbound>,
    services: &Services,
    id: &SessionId,
) -> Result<(), ()> {
    loop {
        match frame_length(buf) {
            Ok(Some(total)) => {
                let raw: Vec<u8> = buf.drain(..total).collect();
                // GAP-26/FR-032 (feature 006): when a dictionary is attached, decode with group
                // awareness (`decode_with_groups`, already correct since feature 005 but never
                // invoked from any production path until now) so a header/trailer repeating group
                // (e.g. NoHops) is structured instead of silently misrouted to the flat body.
                // Scoped to header/trailer groups only via `HeaderTrailerGroupsOnly` — the runtime
                // `DataDictionary` also declares body-level groups (e.g. NoPartyIDs), and
                // structuring those into `Member::Group` here would silently break
                // `validate_groups`'s existing flat-`body.fields()`-based walk (and any codegen-
                // generated accessor reading a body group's member tags directly) — a much larger,
                // riskier change than this fix's actual scope (GAP-26 is specifically about
                // header/trailer groups being invisible, not a mandate to restructure body-group
                // handling too).
                // NEW-58 (feature 009): under a FIXT 1.1 dual-dictionary configuration,
                // `services.validator` is unset (the session uses `fixt_dictionaries` instead --
                // see `run_connection`'s `session.set_fixt_dictionaries` call) -- so this dispatch
                // must also check `fixt_dictionaries`, or header/trailer repeating groups (e.g.
                // NoHops) never get group-aware decoding under FIXT 1.1 at all.
                let decoded = match (&services.validator, &services.fixt_dictionaries) {
                    (Some((dict, _)), _) => {
                        decode_with_groups(&raw, &HeaderTrailerGroupsOnly(dict))
                    }
                    (None, Some((dicts, _))) => {
                        decode_with_groups(&raw, &HeaderTrailerGroupsOnly(dicts.transport()))
                    }
                    (None, None) => decode(&raw),
                };
                if let Ok(msg) = decoded {
                    metrics_export::record_received(id);
                    if let Some(log) = &services.log {
                        log.on_incoming(&String::from_utf8_lossy(&raw));
                    }
                    if !has_app_channel || is_admin(&msg) {
                        admin_tx.send(Inbound::Message(msg)).await.map_err(|_| ())?;
                    } else {
                        pending_app.push_back(Inbound::Message(msg));
                    }
                } else {
                    // The frame length was determinable but the content failed to decode (bad
                    // checksum/body-length/tag): honor RejectGarbledMessage (FR-006) —
                    // session-level, so it always goes through the (bounded) admin channel.
                    admin_tx.send(Inbound::Garbled).await.map_err(|_| ())?;
                }
            }
            Ok(None) => return Ok(()),
            // BUG-13/FR-024 (feature 006): a declared BodyLength beyond the sane maximum closes
            // the connection outright — this is the one framing error that must not just recover
            // and keep the connection open, since it's the actual DoS signal.
            Err(DecodeError::BodyLengthTooLarge { .. }) => return Err(()),
            // BUG-100/FR-014 (feature 007): a declared BodyLength of exactly 0 gets the same
            // hard-close treatment, rather than the generic malformed-frame skip-and-resync
            // recovery below (which would otherwise just discard it and wait indefinitely for
            // more bytes, since a zero-length body can't be resynced past).
            Err(DecodeError::ZeroBodyLength) => return Err(()),
            Err(_) => {
                // B14/FR-028 (feature 006): discard only the malformed prefix that caused this
                // framing error, not the entire buffer — a legitimate message that happens to
                // follow a few garbled/non-FIX bytes in the same read must not be discarded along
                // with them. Recovery scans for the next plausible frame start ("8=") past at
                // least one byte (guaranteeing forward progress); if none is found,
                // nothing in the buffer is recoverable and it's cleared as before.
                let skip = next_frame_start(buf).unwrap_or(buf.len());
                buf.drain(..skip);
                if buf.is_empty() {
                    return Ok(());
                }
                // Otherwise loop and re-attempt framing on the remainder.
            }
        }
    }
}

/// Scan `buf` (from byte offset 1 onward, guaranteeing forward progress even when byte 0 itself
/// looks like a frame start) for the next `"8="` — a plausible start of a new FIX message, per
/// [`classify_buffered`]'s malformed-prefix recovery (B14/FR-028, feature 006). The preceding
/// garbage need not end in SOH (NEW-74/FR-043, feature 009).
fn next_frame_start(buf: &[u8]) -> Option<usize> {
    (1..buf.len().saturating_sub(1)).find(|&i| buf.get(i..i + 2) == Some(b"8="))
}

/// A [`truefix_core::GroupSpec`] adapter that only reports a group definition for count tags the
/// version-agnostic codec already classifies as header/trailer (GAP-26/FR-032, feature 006) —
/// every other (body-level) group is hidden, so `decode_with_groups` leaves the body exactly as
/// flat as plain `decode()` would, preserving `validate_groups`'s existing body-group validation
/// (which walks `body.fields()`, invisible to `Member::Group` entries) and every codegen-generated
/// body-group accessor unchanged.
struct HeaderTrailerGroupsOnly<'a>(&'a truefix_dict::DataDictionary);

impl truefix_core::GroupSpec for HeaderTrailerGroupsOnly<'_> {
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])> {
        if truefix_core::tags::is_header(count_tag) || truefix_core::tags::is_trailer(count_tag) {
            self.0.group_of(count_tag)
        } else {
            None
        }
    }
}

/// Run the application hooks and drive the session state machine for one dequeued inbound item —
/// the per-message logic `drain_messages` used to run inline before US14's reader/processor
/// split; now run by the processing loop after a message has already been read, framed, and
/// classified by the (separately-scheduled) reader task.
#[allow(clippy::too_many_arguments)]
async fn handle_inbound<A, S>(
    inbound: Inbound,
    session: &mut Session,
    stream: &mut S,
    app: &Arc<A>,
    id: &SessionId,
    services: &Services,
    logged_on: &mut bool,
) -> Result<(), ()>
where
    A: Application + 'static,
    S: AsyncWrite + Unpin,
{
    match inbound {
        Inbound::Message(msg) => {
            // BUG-34/FR-010 (feature 007): a message the session layer is about to reject
            // outright (stale latency, mismatched identity, or a too-low sequence number with no
            // PossDupFlag — `Session::would_reject_before_processing`'s read-only precheck) must
            // never reach `from_admin`/`from_app` at all; go straight to `dispatch`, which
            // performs the actual reject/teardown.
            //
            // For every other message, `from_admin`/`from_app` still runs *before* `dispatch`,
            // preserving existing semantics load-bearing elsewhere (e.g. `auth.rs`'s
            // Username/Password check on Logon: the callback must be able to veto a Logon
            // *before* the session commits to it and fires `on_logon` — `dispatch` performs both
            // the state transition and the `on_logon` callback in one call, so reordering that
            // relative to `from_admin` would let an unauthenticated Logon log on before the
            // rejection is even checked).
            if session.would_reject_before_processing(&msg) {
                return dispatch(
                    session,
                    Event::Received(msg),
                    stream,
                    app,
                    id,
                    services,
                    logged_on,
                )
                .await;
            }

            if is_admin(&msg) {
                if let Err(reject) = app.from_admin(&msg, id).await {
                    if let Some(log) = &services.log {
                        log.on_event(&format!(
                            "{id}: admin message rejected by application: {reject}"
                        ));
                    }
                    let actions = session.reject_logon(&reject);
                    let _ = perform_actions(actions, session, stream, app, id, services).await;
                    return Err(());
                }
            } else if let Err(breject) = app.from_app(&msg, id).await {
                // Dictionary-driven business rejects are handled separately; this is the
                // application-supplied one (FR-016). Sequence processing still proceeds.
                let action = session.business_reject(&msg, &breject);
                let _ = perform_actions(vec![action], session, stream, app, id, services).await;
            }
            dispatch(
                session,
                Event::Received(msg),
                stream,
                app,
                id,
                services,
                logged_on,
            )
            .await
        }
        Inbound::Garbled => {
            dispatch(
                session,
                Event::Garbled,
                stream,
                app,
                id,
                services,
                logged_on,
            )
            .await
        }
    }
}

/// Run the engine for one event and perform the resulting actions. Returns `Err` if the
/// session should be torn down.
#[allow(clippy::too_many_arguments)]
async fn dispatch<A, S>(
    session: &mut Session,
    event: Event,
    stream: &mut S,
    app: &Arc<A>,
    id: &SessionId,
    services: &Services,
    logged_on: &mut bool,
) -> Result<(), ()>
where
    A: Application + 'static,
    S: AsyncWrite + Unpin,
{
    let actions = session.handle(event);

    if !*logged_on && session.is_logged_on() {
        *logged_on = true;
        // RefreshOnLogon: re-sync sequence numbers from the durable store right at logon
        // completion, in case it changed between connect time and logon (FR-008).
        if session.refresh_on_logon()
            && let Some(store) = &services.store
        {
            let out = store
                .next_sender_seq()
                .await
                .unwrap_or(session.next_out_seq());
            let inn = store
                .next_target_seq()
                .await
                .unwrap_or(session.next_in_seq());
            session.refresh_sequences(out, inn);
        }
        app.on_logon(id).await;
        if let Some(log) = &services.log {
            log.on_event(&format!("{id}: logon"));
        }
    }

    let disconnect = perform_actions(actions, session, stream, app, id, services).await?;

    // Persist sequence numbers for restart continuity (best-effort).
    if let Some(store) = &services.store {
        if let Err(e) = store.set_next_sender_seq(session.next_out_seq()).await
            && let Some(log) = &services.log
        {
            log.on_event(&format!("{id}: failed to persist next sender seq: {e}"));
        }
        if let Err(e) = store.set_next_target_seq(session.next_in_seq()).await
            && let Some(log) = &services.log
        {
            log.on_event(&format!("{id}: failed to persist next target seq: {e}"));
        }
    }

    metrics_export::record_status(id, &session.status());
    if let Some(monitor) = &services.monitor {
        monitor.update(id, session.status());
    }

    if disconnect || session.state() == SessionState::Disconnected {
        Err(())
    } else {
        Ok(())
    }
}

/// Encode, log, write, and persist an already-hook-approved outbound message. Shared by
/// [`Action::Send`], [`Action::Resend`]'s accepted path, and [`Action::Resend`]'s
/// vetoed-then-gap-filled path (US3, feature 005, GAP-07/FR-007).
async fn write_log_persist<S: AsyncWrite + Unpin>(
    msg: &Message,
    stream: &mut S,
    services: &Services,
    id: &SessionId,
    session: &mut Session,
) -> Result<(), ()> {
    let bytes = msg.encode();
    if let Some(log) = &services.log {
        log.on_outgoing(&String::from_utf8_lossy(&bytes));
    }
    write_with_timeout(stream, &bytes, services, id).await?;
    metrics_export::record_sent(id);
    persist_sent(msg, &bytes, session, services).await;
    Ok(())
}

/// Perform a batch of engine [`Action`]s: invoke the `to_admin`/`to_app` hooks (honoring an
/// outbound `DoNotSend` by discarding the message from the resend store instead of writing it),
/// log, encode, write, and persist each send. Returns whether a disconnect was requested.
async fn perform_actions<A, S>(
    actions: Vec<Action>,
    session: &mut Session,
    stream: &mut S,
    app: &Arc<A>,
    id: &SessionId,
    services: &Services,
) -> Result<bool, ()>
where
    A: Application + 'static,
    S: AsyncWrite + Unpin,
{
    let mut disconnect = false;
    for action in actions {
        match action {
            Action::Send(mut msg) => {
                if is_admin(&msg) {
                    app.to_admin(&mut msg, id).await;
                } else if app.to_app(&mut msg, id).await.is_err() {
                    if let Some(seq) = msg
                        .header
                        .get(34)
                        .and_then(|f| f.as_int().ok())
                        .filter(|&s| s > 0)
                    {
                        session.discard_sent(seq as u64);
                    }
                    continue;
                }
                write_log_persist(&msg, stream, services, id, session).await?;
            }
            Action::Resend(mut msg, seq) => {
                // GAP-07/FR-007 (feature 005): a resend-originated send goes through the same
                // `to_app` veto point a live send does, but unlike a vetoed live send (whose
                // sequence number was never promised to the counterparty), a vetoed resend's
                // sequence number *was* already promised in an earlier connection — silently
                // discarding it would leave the counterparty's own sequence tracking permanently
                // stuck waiting for a message that will never arrive. Substitute a compensating
                // GapFill instead of a bare discard.
                if app.to_app(&mut msg, id).await.is_err() {
                    session.discard_sent(seq);
                    if let Action::Send(mut gap_fill) = session.gap_fill_after_veto(seq) {
                        app.to_admin(&mut gap_fill, id).await;
                        write_log_persist(&gap_fill, stream, services, id, session).await?;
                    }
                    continue;
                }
                write_log_persist(&msg, stream, services, id, session).await?;
            }
            Action::Disconnect => disconnect = true,
            Action::ResetStore => {
                // The engine performed a full sequence/history reset internally (logon-time
                // ResetSeqNumFlag, or ResetOnDisconnect/ResetOnLogout) — clear the durable store
                // so it doesn't retain stale message bodies under recycled sequence numbers
                // (FR-004). `on_before_reset` fires here (rather than inside `Session`, which is
                // sans-IO and never holds an `Application` handle) — this is the earliest point
                // the transport learns of an internally-triggered reset, immediately before the
                // durable store itself is cleared (US10, FR-013).
                app.on_before_reset(id).await;
                if let Some(store) = &services.store
                    && let Err(e) = store.reset().await
                    && let Some(log) = &services.log
                {
                    log.on_event(&format!("{id}: store reset failed: {e}"));
                }
            }
        }
    }
    let _ = stream.flush().await;
    Ok(disconnect)
}

/// Write `bytes` to `stream`, honoring `services.sync_write_timeout` (`SocketSynchronousWrites` +
/// `SocketSynchronousWriteTimeout`; FR-017): a write exceeding the configured timeout is logged
/// as a distinct, typed timeout (rather than a generic I/O failure) and the connection is torn
/// down, instead of blocking indefinitely. `None` (the default) preserves the previous unbounded
/// behavior.
async fn write_with_timeout<S: AsyncWrite + Unpin>(
    stream: &mut S,
    bytes: &[u8],
    services: &Services,
    id: &SessionId,
) -> Result<(), ()> {
    match services.sync_write_timeout {
        Some(timeout) => match tokio::time::timeout(timeout, stream.write_all(bytes)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(()),
            Err(_elapsed) => {
                if let Some(log) = &services.log {
                    log.on_event(&format!(
                        "{id}: synchronous write timed out after {timeout:?}"
                    ));
                }
                Err(())
            }
        },
        None => stream.write_all(bytes).await.map_err(|_| ()),
    }
}

fn is_admin(msg: &Message) -> bool {
    matches!(
        msg.msg_type(),
        Some("0" | "1" | "2" | "3" | "4" | "5" | "A")
    )
}

/// Persist an original outbound transmission to the store for restart-survivable resend (FR-001/002).
/// Skips resends (PossDupFlag=Y keep the original's seq and must not overwrite it) and honours
/// `PersistMessages` (FR-003). Uses `save_and_advance_sender` (GAP-39/FR-018, feature 005) rather
/// than a bare `save`, atomically bundling this message's persistence with advancing the sender
/// counter past it, for backends that can offer that guarantee (SQL/MSSQL/redb) — closing the
/// crash window between "message saved" and "sequence advanced". The caller's own subsequent
/// aggregate `set_next_sender_seq(session.next_out_seq())` (after a whole batch of actions)
/// remains and is now redundant-but-harmless for the sender column specifically (idempotent: it
/// re-asserts the same value `save_and_advance_sender` already set).
async fn persist_sent(msg: &Message, bytes: &[u8], session: &Session, services: &Services) {
    if !session.persist_messages() {
        return;
    }
    let Some(store) = &services.store else {
        return;
    };
    if msg.header.get(43).and_then(|f| f.as_str().ok()) == Some("Y") {
        return; // resend / possible-duplicate — do not re-persist
    }
    if let Some(seq) = msg
        .header
        .get(34)
        .and_then(|f| f.as_int().ok())
        .filter(|&s| s > 0)
    {
        // BUG-08/FR-013 (feature 006): a failed persist previously failed silently, leaving the
        // session believing a message was durably saved for resend when it wasn't.
        if let Err(e) = store.save_and_advance_sender(seq as u64, bytes).await
            && let Some(log) = &services.log
        {
            log.on_event(&format!(
                "{}: failed to persist sent message: {e}",
                session.id()
            ));
        }
    }
}

/// Send an application message initiated by the application (via the control channel).
async fn send_app<A, S>(
    session: &mut Session,
    message: Message,
    stream: &mut S,
    app: &Arc<A>,
    id: &SessionId,
    services: &Services,
) -> Result<(), ()>
where
    A: Application + 'static,
    S: AsyncWrite + Unpin,
{
    let actions = session.send_app(message);
    perform_actions(actions, session, stream, app, id, services).await?;
    if let Some(store) = &services.store
        && let Err(e) = store.set_next_sender_seq(session.next_out_seq()).await
        && let Some(log) = &services.log
    {
        log.on_event(&format!("{id}: failed to persist next sender seq: {e}"));
    }
    metrics_export::record_status(id, &session.status());
    if let Some(monitor) = &services.monitor {
        monitor.update(id, session.status());
    }
    Ok(())
}

// ===========================================================================================
// Multi-session acceptor (routing by SessionID), dynamic sessions, and allow-listing (S6).
// ===========================================================================================

struct Registry {
    sessions: HashMap<SessionId, SessionConfig>,
    template: Option<SessionConfig>,
    allowed_remotes: Option<Vec<IpAddr>>,
    session_stores: HashMap<SessionId, Arc<dyn MessageStore>>,
    /// BUG-61/FR-022 (feature 007): per-session `log`/`validator`/`fixt_dictionaries`/
    /// `socket_options` overrides (attached via `AcceptorBuilder::with_session_services`),
    /// resolved once a connection's `SessionId` is known — previously every session in a group
    /// silently inherited these from whichever member happened to be built into the group-wide
    /// `Services` (the "primary"), so two sessions sharing a port configuring e.g. different log
    /// destinations would have the second session's traffic silently logged to the first's log
    /// file instead. `store` is deliberately excluded here — already independently handled by
    /// `session_stores` above (BUG-07, feature 006).
    session_services: HashMap<SessionId, Services>,
    /// BUG-32/FR-008 (feature 007): `SessionId`s with a currently-active connection — checked and
    /// inserted in `route_and_run` before a second connection presenting the same identity is
    /// routed, removed once that connection ends. Without this, a second connection for an
    /// already-connected `SessionId` was accepted as a competing session, corrupting sequence-
    /// number bookkeeping shared between the two.
    active: std::sync::Mutex<std::collections::HashSet<SessionId>>,
}

/// Builds and serves a multi-session acceptor with optional dynamic sessions and allow-listing.
pub struct AcceptorBuilder<A: Application + 'static> {
    listener: TcpListener,
    app: Arc<A>,
    sessions: HashMap<SessionId, SessionConfig>,
    template: Option<SessionConfig>,
    allowed_remotes: Option<Vec<IpAddr>>,
    services: Services,
    tls: Option<Arc<rustls::ServerConfig>>,
    session_stores: HashMap<SessionId, Arc<dyn MessageStore>>,
    session_services: HashMap<SessionId, Services>,
}

impl<A: Application + 'static> AcceptorBuilder<A> {
    /// Bind a multi-session acceptor.
    ///
    /// Uses default [`Services`]. Call [`Self::bind_with`] when listener options such as
    /// `SocketReuseAddress` must be configured before binding.
    pub async fn bind(addr: SocketAddr, app: Arc<A>) -> io::Result<Self> {
        Self::bind_with(addr, app, Services::default()).await
    }

    /// Bind a multi-session acceptor with services available before the listener is created.
    pub async fn bind_with(addr: SocketAddr, app: Arc<A>, services: Services) -> io::Result<Self> {
        let listener = bind_listener_with_options(addr, services.socket_options.reuse_address)?;
        Ok(Self {
            listener,
            app,
            sessions: HashMap::new(),
            template: None,
            allowed_remotes: None,
            services,
            tls: None,
            session_stores: HashMap::new(),
            session_services: HashMap::new(),
        })
    }

    /// Terminate TLS on each accepted connection using `config` (FR-F6).
    #[must_use]
    pub fn with_tls(mut self, config: Arc<rustls::ServerConfig>) -> Self {
        self.tls = Some(config);
        self
    }

    /// The bound local address.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Return the effective `SO_REUSEADDR` setting on the bound listener.
    pub fn listener_reuse_address(&self) -> io::Result<bool> {
        socket2::SockRef::from(&self.listener).reuse_address()
    }

    /// Register a static session (keyed by its SessionID).
    #[must_use]
    pub fn with_session(mut self, config: SessionConfig) -> Self {
        self.sessions.insert(config.session_id(), config);
        self
    }

    /// Attach a message store specific to one statically-registered session (keyed by its
    /// SessionID), overriding the group-wide `Services.store` for that session only (found while
    /// implementing US2/BUG-07, feature 006: grouped acceptor sessions previously always shared
    /// one store instance via `Services`, which is correct for stateless per-connection concerns
    /// like socket options but silently corrupts per-session sequence-number bookkeeping when two
    /// sessions in one group are actually connected concurrently — each session's own store must
    /// be independent, unlike the other group-shared services). Sessions not given their own store
    /// here (including any connection matched via the dynamic template, which has no fixed
    /// identity to key by) continue to use the group-wide `Services.store`.
    #[must_use]
    pub fn with_session_store(
        mut self,
        session_id: SessionId,
        store: Arc<dyn MessageStore>,
    ) -> Self {
        self.session_stores.insert(session_id, store);
        self
    }

    /// Attach `log`/`socket_options`/`validator`/`fixt_dictionaries` specific to one
    /// statically-registered session (keyed by its SessionID), overriding the group-wide
    /// `Services` for that session only (BUG-61/FR-022, feature 007) — the same per-session
    /// override principle [`Self::with_session_store`] already applies to `store`, extended to
    /// the other fields that are just as legitimately per-session as sequence-number bookkeeping
    /// (e.g. two sessions sharing a port configuring different log destinations or dictionaries).
    /// `services.store` is ignored here (`store` stays governed exclusively by
    /// [`Self::with_session_store`]); every other field of `services` not set to its default is
    /// applied. Sessions not given an override here (including any connection matched via the
    /// dynamic template) continue to use the group-wide `Services`.
    #[must_use]
    pub fn with_session_services(mut self, session_id: SessionId, services: Services) -> Self {
        self.session_services.insert(session_id, services);
        self
    }

    /// Set a dynamic-session template: connections that don't match a static session are accepted
    /// using this template (AcceptorTemplate / DynamicSession).
    #[must_use]
    pub fn with_dynamic_template(mut self, template: SessionConfig) -> Self {
        self.template = Some(template);
        self
    }

    /// Only accept connections from these remote IP addresses (AllowedRemoteAddresses).
    #[must_use]
    pub fn allow_remotes(mut self, ips: Vec<IpAddr>) -> Self {
        self.allowed_remotes = Some(ips);
        self
    }

    /// Trust a PROXY protocol (v1/v2) header — parsed into the connection's effective source
    /// IP for the allow-list check above — only when the connection's *physical* peer address is
    /// one of `ips` (`UseTCPProxy` + `TrustedProxyAddresses`; FR-015). A connection from any other
    /// physical source never has its PROXY header parsed/trusted, even if one is present. Sugar
    /// over `services.trusted_proxy_addresses` (also settable directly via
    /// [`with_services`](Self::with_services)).
    #[must_use]
    pub fn trust_proxy_from(mut self, ips: Vec<IpAddr>) -> Self {
        self.services.trusted_proxy_addresses = ips;
        self
    }

    /// Attach store/log/socket services. Because the listener is already bound,
    /// `services.socket_options.reuse_address` cannot affect it; pass listener options through
    /// [`Self::bind_with`] instead.
    #[must_use]
    pub fn with_services(mut self, services: Services) -> Self {
        self.services = services;
        self
    }

    /// Spawn the accept loop.
    pub fn serve(self) -> JoinHandle<()> {
        let registry = Arc::new(Registry {
            sessions: self.sessions,
            template: self.template,
            allowed_remotes: self.allowed_remotes,
            session_stores: self.session_stores,
            session_services: self.session_services,
            active: std::sync::Mutex::new(std::collections::HashSet::new()),
        });
        let app = self.app;
        let services = self.services;
        let listener = self.listener;
        let tls = self.tls.map(tokio_rustls::TlsAcceptor::from);
        tokio::spawn(async move {
            while let Ok((mut stream, peer)) = listener.accept().await {
                // PROXY protocol (FR-015): only a physically-trusted upstream's header is parsed;
                // otherwise `effective_ip` is just the physical peer address, unchanged.
                let effective_ip = proxy::strip_trusted_proxy_header(
                    &mut stream,
                    peer,
                    &services.trusted_proxy_addresses,
                )
                .await
                .ip();
                if let Some(allowed) = &registry.allowed_remotes
                    && !allowed.contains(&effective_ip)
                {
                    continue; // refuse: not in the allow-list
                }
                services.socket_options.apply(&stream);
                let registry = registry.clone();
                let app = app.clone();
                let services = services.clone();
                match &tls {
                    Some(acceptor) => {
                        let acceptor = acceptor.clone();
                        tokio::spawn(async move {
                            if let Ok(tls_stream) = acceptor.accept(stream).await {
                                route_and_run(tls_stream, app, registry, services).await;
                            }
                        });
                    }
                    None => {
                        tokio::spawn(async move {
                            route_and_run(stream, app, registry, services).await;
                        });
                    }
                }
            }
        })
    }
}

/// NEW-93 (feature 009): the pre-Logon read loop in [`route_and_run`] has no `Session`/ticker of
/// its own yet (those only start once a Logon is decoded), so nothing bounds how long a silent or
/// SOH-less-garbage-trickling peer can hold the connection open. Picks the largest configured
/// `logon_timeout` across every statically-registered session and the dynamic template (whichever
/// session ends up matching should get at least as long as its own configured timeout), falling
/// back to `SessionConfig::new`'s own default when no session is registered yet.
fn prelogon_read_timeout(registry: &Registry) -> Duration {
    let default = SessionConfig::new("FIX.4.4", "S", "T", Role::Acceptor).logon_timeout;
    let max_configured = registry
        .sessions
        .values()
        .chain(registry.template.iter())
        .map(|c| c.logon_timeout)
        .max()
        .unwrap_or(default);
    Duration::from_secs(u64::from(max_configured.max(1)))
}

/// NEW-93 (feature 009): a defensive cap on the pre-Logon buffer, independent of the
/// `prelogon_read_timeout` above -- even if a peer keeps the connection alive within the timeout
/// window, its buffer must not grow past a bound no legitimate Logon could ever reach.
const PRELOGON_BUF_CAP: usize = truefix_core::MAX_BODY_LEN;

async fn route_and_run<A, S>(
    mut stream: S,
    app: Arc<A>,
    registry: Arc<Registry>,
    services: Services,
) where
    A: Application + 'static,
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let timeout = prelogon_read_timeout(&registry);
    let read_logon = async {
        // Read until the first complete message (the Logon) so we can route by SessionID.
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return None,
                Ok(n) => {
                    if let Some(slice) = chunk.get(..n) {
                        buf.extend_from_slice(slice);
                    }
                }
            }
            if buf.len() > PRELOGON_BUF_CAP {
                return None;
            }
            match frame_length(&buf) {
                Ok(Some(total)) => match buf.get(..total).map(decode) {
                    // BUG-52/FR-044 (feature 007): an acceptor must refuse a connection whose first
                    // inbound message isn't a Logon (QFJ/QFGo both close the socket immediately
                    // rather than routing/session-creating on anything else) -- previously any
                    // decodable first message was accepted here and used to derive a SessionId, so
                    // a non-Logon first message (or nothing at all) would silently fail routing
                    // further down instead of being rejected at the door.
                    Some(Ok(msg)) if msg.msg_type() == Some("A") => return Some((msg, buf)),
                    _ => return None,
                },
                Ok(None) => continue,
                Err(_) => return None,
            }
        }
    };
    let Ok(Some((logon, buf))) = tokio::time::timeout(timeout, read_logon).await else {
        // Either the timeout fired (NEW-93) or the loop itself gave up (EOF/error/non-Logon/
        // oversized buffer) -- in every case the connection is simply dropped, matching the
        // existing `return` behavior for those cases.
        return;
    };

    let begin = logon.begin_string().unwrap_or_default().to_owned();
    let their_sender = field_str(&logon, 49);
    let their_target = field_str(&logon, 56);
    // BUG-07/FR-010 (feature 006): also extract SubID/LocationID (tags 50/142/57/143) so a
    // session registered with those fields populated (GAP-47, feature 005) is actually routable —
    // the lookup key previously only used the 3 required fields, hardcoding the other 5 to `None`
    // regardless of what the inbound Logon carried, so any SubID/LocationID-distinguished session
    // could never match. Our SessionID reverses the counterparty's comp IDs, same as begin/target.
    let their_sender_sub_id = field_opt_str(&logon, 50);
    let their_sender_location_id = field_opt_str(&logon, 142);
    let their_target_sub_id = field_opt_str(&logon, 57);
    let their_target_location_id = field_opt_str(&logon, 143);
    let sid = SessionId::new_full(
        begin.clone(),
        their_target.clone(),
        their_target_sub_id.clone(),
        their_target_location_id.clone(),
        their_sender.clone(),
        their_sender_sub_id.clone(),
        their_sender_location_id.clone(),
        None, // SessionQualifier has no wire tag; disambiguated by the group-uniqueness check
              // enforced at config-resolve time instead (Engine::start).
    );

    let config = registry.sessions.get(&sid).cloned().or_else(|| {
        registry.template.as_ref().map(|t| {
            dynamic_config(
                t,
                &begin,
                (
                    &their_target,
                    their_target_sub_id.as_deref(),
                    their_target_location_id.as_deref(),
                ),
                (
                    &their_sender,
                    their_sender_sub_id.as_deref(),
                    their_sender_location_id.as_deref(),
                ),
            )
        })
    });
    let Some(config) = config else {
        if services.log_message_when_session_not_found
            && let Some(log) = &services.log
        {
            log.on_event(&format!(
                    "no session found for BeginString={begin} SenderCompID={their_sender} TargetCompID={their_target}"
                ));
        }
        return; // no static match and no template -> refuse
    };

    // US2/BUG-07 (feature 006): a statically-registered session with its own store (attached via
    // `AcceptorBuilder::with_session_store`) must use that store instead of the group-wide one —
    // sharing one store instance across multiple concurrently-connected sessions in a group
    // silently corrupts each session's own sequence-number bookkeeping (each is independent
    // per-session state, unlike socket options/TLS material, which are legitimately group-wide).
    let mut services = services;
    if let Some(store) = registry.session_stores.get(&sid) {
        services.store = Some(store.clone());
    }

    // BUG-61/FR-022 (feature 007): a statically-registered session with its own
    // log/validator/fixt_dictionaries (attached via `AcceptorBuilder::with_session_services`) uses
    // those instead of the group-wide `Services` — previously every session in a group silently
    // inherited these from whichever member was built into the group-wide `Services` (the
    // "primary"), so a second session configuring a different log destination or dictionary had
    // its traffic silently routed to the first session's log/validator instead. `store` is
    // untouched here (already resolved above, independently, by `session_stores`).
    //
    // `socket_options`/TLS remain a disclosed group-wide simplification here, same as the existing
    // TLS-is-listener-scoped note in `crates/truefix/src/lib.rs`'s acceptor-group builder: safely
    // re-applying per-session socket options this late would require reaching back into the
    // (possibly TLS-wrapped) generic `S` for its underlying `TcpStream`, which isn't expressible
    // without either an additional generic trait bound threaded through every caller of this
    // function or unsafe raw-fd handling (forbidden by this project's `unsafe_code = "forbid"`).
    // Socket options remain governed by the group-wide `Services`, applied once at accept time
    // (before any session identity is known) — a narrower, disclosed limitation of this fix.
    if let Some(overrides) = registry.session_services.get(&sid) {
        let store = services.store.take();
        services.log = overrides.log.clone();
        services.validator = overrides.validator.clone();
        services.fixt_dictionaries = overrides.fixt_dictionaries.clone();
        services.store = store;
    }

    // BUG-32/FR-008 (feature 007): refuse a second connection presenting a `SessionId` that
    // already has an active connection, rather than accepting it as a competing session (which
    // would corrupt sequence-number bookkeeping shared between the two). Checked-and-inserted
    // atomically under the same lock to avoid a race between two connections resolving to the
    // same `sid` concurrently.
    {
        let mut active = match registry.active.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !active.insert(sid.clone()) {
            if let Some(log) = &services.log {
                log.on_event(&format!(
                    "refused a second connection for already-connected session {sid:?}"
                ));
            }
            return;
        }
    }
    // RAII guard: whatever path `run_connection` returns through (normal completion, error,
    // panic-unwind), `sid` is removed from `active` so a legitimate future reconnect isn't
    // permanently refused.
    struct ActiveGuard {
        registry: Arc<Registry>,
        sid: SessionId,
    }
    impl Drop for ActiveGuard {
        fn drop(&mut self) {
            if let Ok(mut active) = self.registry.active.lock() {
                active.remove(&self.sid);
            }
        }
    }
    let _guard = ActiveGuard {
        registry: registry.clone(),
        sid: sid.clone(),
    };

    let (control_tx, control_rx) = mpsc::channel(8);
    run_connection(stream, config, app, services, control_tx, control_rx, buf).await;
}

fn field_str(msg: &Message, tag: u32) -> String {
    msg.header
        .get(tag)
        .and_then(|f| f.as_str().ok())
        .unwrap_or_default()
        .to_owned()
}

/// Like [`field_str`], but `None` when the tag is absent (for `SessionId::new_full`'s optional
/// SubID/LocationID components) rather than collapsing absence to an empty string.
fn field_opt_str(msg: &Message, tag: u32) -> Option<String> {
    msg.header
        .get(tag)
        .and_then(|f| f.as_str().ok())
        .map(str::to_owned)
}

/// Build a per-connection `SessionConfig` from a dynamic-session template (`AcceptorTemplate` /
/// `DynamicSession`). GAP-19: the template acts as a wildcard match on all 7 identity fields, not
/// just BeginString/SenderCompID/TargetCompID — SubID/LocationID from the inbound Logon are
/// carried through too (`None` when the wire didn't send one), instead of silently staying
/// whatever the template itself happened to have (typically `None`, since a template's whole
/// point is that it doesn't know the counterparty's identity ahead of time).
/// `(CompID, SubID, LocationID)` — groups GAP-19's wildcard identity fields so `dynamic_config`
/// doesn't need 8 flat parameters (clippy's `too_many_arguments` caps at 7).
type IdentityTriple<'a> = (&'a str, Option<&'a str>, Option<&'a str>);

fn dynamic_config(
    template: &SessionConfig,
    begin: &str,
    our_sender: IdentityTriple<'_>,
    our_target: IdentityTriple<'_>,
) -> SessionConfig {
    let mut config = template.clone();
    config.begin_string = begin.to_owned();
    config.sender_comp_id = our_sender.0.to_owned();
    config.sender_sub_id = our_sender.1.map(str::to_owned);
    config.sender_location_id = our_sender.2.map(str::to_owned);
    config.target_comp_id = our_target.0.to_owned();
    config.target_sub_id = our_target.1.map(str::to_owned);
    config.target_location_id = our_target.2.map(str::to_owned);
    config
}

// ===========================================================================================
// Reconnecting initiator (S6).
// ===========================================================================================

/// Handle to a reconnecting initiator loop.
pub struct ReconnectHandle {
    stop: Arc<AtomicBool>,
    task: JoinHandle<()>,
    /// BUG-50/FR-043 (feature 007): an abort handle for the currently-active connection's own
    /// task, if any is live right now -- lets `stop()` reach in and abort it immediately, rather
    /// than only ever setting a flag the reconnect loop can't check again until that connection
    /// ends on its own. `AbortHandle` (unlike `JoinHandle`) is cheaply `Clone`, so it can be
    /// published here while the loop itself separately (and directly, not through this `Mutex`)
    /// awaits the connection's full `JoinHandle` to completion.
    current: Arc<std::sync::Mutex<Option<tokio::task::AbortHandle>>>,
}

impl ReconnectHandle {
    /// Signal the loop to stop after the current attempt, and abort a currently-active connection
    /// immediately if one is live right now (BUG-50/FR-043, feature 007) -- previously this only
    /// set a flag, checked at the top of the reconnect loop and after a connection's own task
    /// returned; while a connection was live, the loop was blocked awaiting it and never saw the
    /// flag until that connection ended on its own (which, absent a graceful close from either
    /// side, could be never).
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let guard = match self.current.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(handle) = guard.as_ref() {
            handle.abort();
        }
    }

    /// Wait for the loop to finish.
    pub async fn join(self) {
        let _ = self.task.await;
    }
}

/// GAP-14/FR-014 (feature 005): the reconnect delay for the `attempt`-th consecutive failed
/// connect since the last success (0-indexed). When `reconnect_interval_steps` is empty, always
/// returns the fixed `reconnect_interval` (today's behavior, unchanged). Otherwise indexes into
/// the steps array, clamped (sticking) at the last element once `attempt` exceeds it.
fn reconnect_delay(config: &SessionConfig, attempt: usize) -> Duration {
    let secs = if config.reconnect_interval_steps.is_empty() {
        config.reconnect_interval
    } else {
        let last = config.reconnect_interval_steps.len().saturating_sub(1);
        config
            .reconnect_interval_steps
            .get(attempt.min(last))
            .copied()
            .unwrap_or(config.reconnect_interval)
    };
    Duration::from_secs(u64::from(secs).max(1))
}

/// Connect out as an initiator, automatically reconnecting after `config.reconnect_interval`
/// seconds whenever the connection drops, until stopped.
pub fn connect_initiator_reconnecting<A>(
    addr: SocketAddr,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
) -> ReconnectHandle
where
    A: Application + 'static,
{
    connect_initiator_reconnecting_multi(vec![addr], config, app, services)
}

/// Connect out as an initiator against an ordered set of candidate endpoints
/// (`SocketConnectHost<N>`/`SocketConnectPort<N>`; FR-019), rotating to the next endpoint on each
/// (re)connect attempt and automatically reconnecting after `config.reconnect_interval` seconds
/// whenever the connection drops, until stopped. Endpoints are tried round-robin; if all are
/// unreachable the loop keeps retrying (starting again from the next endpoint) rather than
/// busy-looping or giving up.
pub fn connect_initiator_reconnecting_multi<A>(
    addrs: Vec<SocketAddr>,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
) -> ReconnectHandle
where
    A: Application + 'static,
{
    let endpoints = addrs
        .into_iter()
        .map(|addr| SocketEndpoint::new(addr.ip().to_string(), addr.port()))
        .collect();
    connect_initiator_reconnecting_endpoints(endpoints, config, app, services)
}

/// Connect as a reconnecting initiator while retaining configured hostnames. Every attempt
/// resolves the selected endpoint anew and tries all addresses returned by DNS (FR-030).
pub fn connect_initiator_reconnecting_endpoints<A>(
    endpoints: Vec<SocketEndpoint>,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
) -> ReconnectHandle
where
    A: Application + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let loop_stop = stop.clone();
    let current: Arc<std::sync::Mutex<Option<tokio::task::AbortHandle>>> =
        Arc::new(std::sync::Mutex::new(None));
    let loop_current = current.clone();
    let id = config.session_id();
    let task = tokio::spawn(async move {
        let mut connected_before = false;
        let mut next_endpoint = 0usize;
        let mut backoff_step = 0usize;
        while !loop_stop.load(Ordering::SeqCst) {
            let Some(endpoint) = endpoints.get(next_endpoint % endpoints.len().max(1)) else {
                break; // endpoints is empty; nothing to connect to
            };
            next_endpoint = next_endpoint.wrapping_add(1);
            if let Ok(stream) = connect_endpoint(
                endpoint,
                services.address_resolver.as_deref(),
                config.local_bind_addr,
                config.connect_timeout,
            )
            .await
            {
                // BUG-39/FR-021 (feature 007): apply the configured socket options here, matching
                // every other connection path (plain `connect_initiator`, the TLS variant below,
                // acceptor-side connections) -- previously this one path skipped it entirely.
                services.socket_options.apply(&stream);
                if connected_before {
                    metrics_export::record_reconnect(&id);
                }
                connected_before = true;
                // GAP-14/FR-014 (feature 005): a successful connect clears the backoff ladder —
                // stepped delays only escalate across *consecutive* failed attempts.
                backoff_step = 0;
                // BUG-51/FR-042 (feature 007): a successful connect also resets endpoint rotation
                // back to the primary (index 0) -- matching QFJ's `nextSocketAddressIndex = 0`.
                // Previously `next_endpoint` was only ever incremented, so after a connection to a
                // non-primary endpoint dropped, the next reconnect attempt kept rotating forward
                // instead of preferring the primary again.
                next_endpoint = 0;
                let (control_tx, control_rx) = mpsc::channel(8);
                // BUG-50/FR-043 (feature 007): run this connection as its own task (rather than
                // awaiting it directly in this loop) so its (cheaply `Clone`-able) `AbortHandle`
                // can be published to `current` -- letting `ReconnectHandle::stop()` reach in and
                // abort it immediately rather than only being checked once this connection ends
                // on its own. The full `JoinHandle` itself is awaited directly here, never through
                // the shared `Mutex` -- holding a `std::sync::Mutex` guard across an `.await`
                // would risk `stop()` (called synchronously, from another task) deadlocking
                // against it for the entire lifetime of the connection.
                let conn_task = tokio::spawn(run_connection(
                    stream,
                    config.clone(),
                    app.clone(),
                    services.clone(),
                    control_tx,
                    control_rx,
                    Vec::new(),
                ));
                {
                    let mut guard = match loop_current.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    *guard = Some(conn_task.abort_handle());
                }
                let _ = conn_task.await;
                {
                    let mut guard = match loop_current.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    *guard = None;
                }
            }
            if loop_stop.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(reconnect_delay(&config, backoff_step)).await;
            backoff_step = backoff_step.saturating_add(1);
        }
    });
    ReconnectHandle {
        stop,
        task,
        current,
    }
}

/// As [`connect_initiator_reconnecting_multi`], but performs a TLS handshake on every connection
/// attempt (US1, feature 004, FR-001) — mirroring [`connect_initiator_tls`]'s single-attempt
/// handshake, looped with the same rotation/backoff as the plain multi-endpoint connector.
pub fn connect_initiator_reconnecting_multi_tls<A>(
    addrs: Vec<SocketAddr>,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    tls: Arc<rustls::ClientConfig>,
    server_name: rustls::pki_types::ServerName<'static>,
) -> ReconnectHandle
where
    A: Application + 'static,
{
    let endpoints = addrs
        .into_iter()
        .map(|addr| SocketEndpoint::new(addr.ip().to_string(), addr.port()))
        .collect();
    connect_initiator_reconnecting_endpoints_tls(endpoints, config, app, services, tls, server_name)
}

/// TLS counterpart of [`connect_initiator_reconnecting_endpoints`]. Hostname resolution is
/// repeated before every TCP/TLS connection attempt.
pub fn connect_initiator_reconnecting_endpoints_tls<A>(
    endpoints: Vec<SocketEndpoint>,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    tls: Arc<rustls::ClientConfig>,
    server_name: rustls::pki_types::ServerName<'static>,
) -> ReconnectHandle
where
    A: Application + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let loop_stop = stop.clone();
    let current: Arc<std::sync::Mutex<Option<tokio::task::AbortHandle>>> =
        Arc::new(std::sync::Mutex::new(None));
    let loop_current = current.clone();
    let id = config.session_id();
    let connector = tokio_rustls::TlsConnector::from(tls);
    let task = tokio::spawn(async move {
        let mut connected_before = false;
        let mut next_endpoint = 0usize;
        let mut backoff_step = 0usize;
        while !loop_stop.load(Ordering::SeqCst) {
            let Some(endpoint) = endpoints.get(next_endpoint % endpoints.len().max(1)) else {
                break; // endpoints is empty; nothing to connect to
            };
            next_endpoint = next_endpoint.wrapping_add(1);
            if let Ok(tcp) = connect_endpoint(
                endpoint,
                services.address_resolver.as_deref(),
                config.local_bind_addr,
                config.connect_timeout,
            )
            .await
            {
                services.socket_options.apply(&tcp);
                if let Ok(stream) = connector.connect(server_name.clone(), tcp).await {
                    if connected_before {
                        metrics_export::record_reconnect(&id);
                    }
                    connected_before = true;
                    backoff_step = 0;
                    // BUG-51/FR-042 (feature 007): reset endpoint rotation back to the primary on
                    // a successful connect -- see the identical fix (and its full rationale) in
                    // the plain (non-TLS) `connect_initiator_reconnecting_multi` above.
                    next_endpoint = 0;
                    let (control_tx, control_rx) = mpsc::channel(8);
                    // BUG-50/FR-043 (feature 007): see the identical fix (and its full rationale)
                    // in the plain (non-TLS) `connect_initiator_reconnecting_multi` above.
                    let conn_task = tokio::spawn(run_connection(
                        stream,
                        config.clone(),
                        app.clone(),
                        services.clone(),
                        control_tx,
                        control_rx,
                        Vec::new(),
                    ));
                    {
                        let mut guard = match loop_current.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        *guard = Some(conn_task.abort_handle());
                    }
                    let _ = conn_task.await;
                    {
                        let mut guard = match loop_current.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        *guard = None;
                    }
                }
            }
            if loop_stop.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(reconnect_delay(&config, backoff_step)).await;
            backoff_step = backoff_step.saturating_add(1);
        }
    });
    ReconnectHandle {
        stop,
        task,
        current,
    }
}

// ===========================================================================================
// Scheduled initiator (S6 schedule wiring): connect only while in session (FR-E1/E2/E3).
// ===========================================================================================

/// Handle to a scheduled initiator loop.
pub struct ScheduledHandle {
    stop: Arc<AtomicBool>,
    task: JoinHandle<()>,
}

impl ScheduledHandle {
    /// Signal the loop to stop after the current check.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Wait for the loop to finish.
    pub async fn join(self) {
        let _ = self.task.await;
    }
}

/// NEW-14 (feature 009): polls `loop_stop` and the schedule's in-session status every 200ms
/// (matching `run_scheduled_initiator`'s own poll cadence), resolving as soon as either the stop
/// flag is set or the in-session status differs from `was_in_session` (a boundary crossed) —
/// raced via `tokio::select!` against an in-flight connect attempt so neither is blocked
/// indefinitely by the other.
async fn wait_for_stop_or_schedule_change(
    loop_stop: &AtomicBool,
    schedule: &Schedule,
    was_in_session: bool,
) {
    loop {
        if loop_stop.load(Ordering::SeqCst) {
            return;
        }
        let now = time::OffsetDateTime::now_utc();
        let in_session_now = schedule.non_stop || schedule.is_in_session(now);
        if in_session_now != was_in_session {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Run an initiator that connects only while the `schedule` is in session. On each transition
/// into a session window the store is reset (disconnect → reset sequence numbers → clear store →
/// reconnect, FR-E3); on a transition out of the window the session is logged out.
pub fn run_scheduled_initiator<A>(
    addr: SocketAddr,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    schedule: Schedule,
) -> ScheduledHandle
where
    A: Application + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let loop_stop = stop.clone();
    let id = config.session_id();
    let task = tokio::spawn(async move {
        let mut current: Option<SessionHandle> = None;
        // BUG-09/GAP-48 (feature 006): seed `was_in_session` from the store's persisted
        // creation time (already persisted correctly by every backend since GAP-38, feature 005,
        // but never consulted anywhere until now) instead of hardcoding `false`. A process
        // restart landing inside an already-active schedule window would otherwise be
        // indistinguishable from a genuinely-new session entering the window for the first time
        // — both produced `Enter` from `decide_schedule_action`, spuriously wiping sequence
        // numbers and resend history on every routine restart that happens to land mid-window.
        let mut was_in_session = if let Some(store) = &services.store {
            match store.creation_time().await {
                Ok(Some(created)) => schedule.non_stop || schedule.is_in_session(created),
                Ok(None) | Err(_) => false,
            }
        } else {
            false
        };
        let mut connected_before = false;
        // BUG-94/FR-026 (feature 007): consecutive failed *connect attempts* back off using the
        // same `ReconnectInterval`-step pattern `reconnect_delay` already applies elsewhere in
        // this file, instead of retrying at a fixed 200ms. Tracked independently of the loop's own
        // 200ms sleep below, which stays fast so schedule-boundary detection remains responsive --
        // only the connect *attempt* itself is skipped until `next_retry_at` elapses.
        let mut failed_attempts: usize = 0;
        let mut next_retry_at: Option<std::time::Instant> = None;
        while !loop_stop.load(Ordering::SeqCst) {
            let now = time::OffsetDateTime::now_utc();
            // The boundary-crossing decision (reset-on-entry / logout-on-exit) is a pure,
            // tested function in truefix-session (FR-018/FR-E3).
            match truefix_session::decide_schedule_action(&schedule, was_in_session, now) {
                truefix_session::ScheduleAction::Enter => {
                    if let Some(store) = &services.store
                        && let Err(e) = store.reset().await
                        && let Some(log) = &services.log
                    {
                        log.on_event(&format!("{id}: store reset failed: {e}"));
                    }
                }
                truefix_session::ScheduleAction::Exit => {
                    if let Some(handle) = current.take() {
                        handle.logout().await;
                    }
                }
                truefix_session::ScheduleAction::None => {}
            }
            was_in_session = schedule.non_stop || schedule.is_in_session(now);
            // BUG-25 (feature 007): a `SessionHandle` whose underlying connection task has
            // already finished (e.g. the TCP connection dropped) must free `current` so the
            // reconnect condition below can fire again -- previously `current` stayed `Some`
            // forever after any drop, since nothing ever re-checked liveness.
            if current.as_ref().is_some_and(SessionHandle::is_finished) {
                current = None;
            }
            // Independently of the boundary decision, keep retrying a connection while in
            // session and disconnected (e.g. after a transient network drop) -- but only once
            // this attempt's own backoff delay (if any) has elapsed.
            if was_in_session
                && current.is_none()
                && next_retry_at.is_none_or(|t| std::time::Instant::now() >= t)
            {
                // NEW-14 (feature 009): race the connect attempt against the stop flag and
                // schedule-boundary detection -- previously this `.await`ed inline, so with no
                // `connect_timeout` configured (the default), a hanging/black-holed connect
                // blocked `loop_stop`/schedule-boundary observation indefinitely, defeating the
                // 200ms poll cadence for as long as the connect attempt was outstanding.
                let connect_fut =
                    connect_initiator_with(addr, config.clone(), app.clone(), services.clone());
                let abandoned =
                    wait_for_stop_or_schedule_change(&loop_stop, &schedule, was_in_session);
                tokio::select! {
                    result = connect_fut => {
                        match result {
                            Ok(handle) => {
                                if connected_before {
                                    metrics_export::record_reconnect(&id);
                                }
                                connected_before = true;
                                current = Some(handle);
                                failed_attempts = 0;
                                next_retry_at = None;
                            }
                            Err(_) => {
                                let delay = reconnect_delay(&config, failed_attempts);
                                failed_attempts = failed_attempts.saturating_add(1);
                                next_retry_at = Some(std::time::Instant::now() + delay);
                            }
                        }
                    }
                    () = abandoned => {
                        // The stop flag or a schedule boundary fired mid-connect -- the connect
                        // future is dropped (cancelled) here; loop back around immediately so the
                        // boundary/stop-check logic above re-evaluates on the next iteration.
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if let Some(handle) = current.take() {
            handle.logout().await;
        }
    });
    ScheduledHandle { stop, task }
}

#[cfg(test)]
mod connect_timeout_tests {
    // T042 (US6, feature 005): `with_connect_timeout` (the wrapper `tcp_connect` uses around
    // every initiator connect attempt) bounds an otherwise-never-resolving connect by
    // `connect_timeout`, surfacing a typed `ErrorKind::TimedOut` rather than hanging forever
    // (GAP-16/FR-016). Uses `tokio::time::pause()` (virtual time) instead of a real black-holed
    // address, so this is deterministic and has no real-network dependency.
    use super::with_connect_timeout;
    use std::time::Duration;
    use tokio::io;

    #[tokio::test(start_paused = true)]
    async fn an_unbounded_connect_never_completes_without_a_timeout_configured() {
        let result = tokio::time::timeout(
            Duration::from_secs(60),
            with_connect_timeout(None, std::future::pending::<io::Result<()>>()),
        )
        .await;
        assert!(
            result.is_err(),
            "with no connect_timeout configured, the connect future should never resolve"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_configured_timeout_fires_before_an_unbounded_connect_resolves() {
        let result = with_connect_timeout(
            Some(Duration::from_secs(5)),
            std::future::pending::<io::Result<()>>(),
        )
        .await;
        let err = result.expect_err("expected the timeout to fire");
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test(start_paused = true)]
    async fn a_connect_that_resolves_before_the_timeout_is_unaffected() {
        let result = with_connect_timeout(Some(Duration::from_secs(5)), async {
            Ok::<_, io::Error>(42)
        })
        .await;
        assert_eq!(result.unwrap(), 42);
    }
}

#[cfg(test)]
mod scheduled_initiator_nonblocking_connect_tests {
    // T041/T042 (US1, feature 009, NEW-14): `wait_for_stop_or_schedule_change` is the mechanism
    // `run_scheduled_initiator` races an in-flight connect against, so `loop_stop`/schedule
    // boundaries are observed promptly instead of being blocked by a hanging connect attempt.
    // A genuinely-hanging real TCP connect isn't used here (OS-dependent, slow, and flaky across
    // environments) -- this tests the extracted helper directly against a never-resolving future
    // standing in for one, using `tokio::time::pause()` (virtual time) for determinism, matching
    // `connect_timeout_tests`'s existing pattern in this same file.
    use super::wait_for_stop_or_schedule_change;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;
    use truefix_session::Schedule;

    #[tokio::test(start_paused = true)]
    async fn resolves_promptly_once_the_stop_flag_is_set_even_with_a_pending_connect() {
        let stop = AtomicBool::new(true); // already set, standing in for a concurrent stop() call
        let schedule = Schedule::non_stop();
        let never_connects = std::future::pending::<()>();

        let result = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                _ = never_connects => unreachable!("connect must never resolve in this test"),
                () = wait_for_stop_or_schedule_change(&stop, &schedule, true) => (),
            }
        })
        .await;
        assert!(
            result.is_ok(),
            "wait_for_stop_or_schedule_change must resolve promptly once the stop flag is set, \
             not be blocked by a pending connect (NEW-14)"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn never_resolves_while_still_in_session_and_not_stopped() {
        let stop = AtomicBool::new(false);
        let schedule = Schedule::non_stop(); // always "in session" -- was_in_session never changes
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            wait_for_stop_or_schedule_change(&stop, &schedule, true),
        )
        .await;
        assert!(
            result.is_err(),
            "must not resolve while neither the stop flag nor the schedule boundary has changed"
        );
    }
}

#[cfg(test)]
mod reconnect_delay_tests {
    // T040 (US6, feature 005): `reconnect_delay` steps up across consecutive attempts and sticks
    // at the last configured value once exhausted (GAP-14/FR-014).
    use super::reconnect_delay;
    use std::time::Duration;
    use truefix_session::{Role, SessionConfig};

    fn cfg() -> SessionConfig {
        SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Initiator)
    }

    #[test]
    fn empty_steps_always_use_the_fixed_reconnect_interval() {
        let mut c = cfg();
        c.reconnect_interval = 7;
        for attempt in [0, 1, 5, 100] {
            assert_eq!(reconnect_delay(&c, attempt), Duration::from_secs(7));
        }
    }

    #[test]
    fn steps_index_by_attempt_number() {
        let mut c = cfg();
        c.reconnect_interval_steps = vec![2, 5, 10, 30];
        assert_eq!(reconnect_delay(&c, 0), Duration::from_secs(2));
        assert_eq!(reconnect_delay(&c, 1), Duration::from_secs(5));
        assert_eq!(reconnect_delay(&c, 2), Duration::from_secs(10));
        assert_eq!(reconnect_delay(&c, 3), Duration::from_secs(30));
    }

    #[test]
    fn steps_stick_at_the_last_value_once_exhausted() {
        let mut c = cfg();
        c.reconnect_interval_steps = vec![2, 5, 10, 30];
        for attempt in [4, 5, 100] {
            assert_eq!(reconnect_delay(&c, attempt), Duration::from_secs(30));
        }
    }
}
