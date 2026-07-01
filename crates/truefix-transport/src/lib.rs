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

mod framing;

use std::collections::HashMap;
use std::future::pending;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use truefix_core::{decode, Message};
use truefix_log::Log;
use truefix_session::{
    Action, Application, Event, Schedule, Session, SessionConfig, SessionId, SessionState,
    SessionStatus,
};
use truefix_store::MessageStore;

use framing::frame_length;

/// TCP socket options applied to each connection.
#[derive(Debug, Clone, Copy)]
pub struct SocketOptions {
    /// Disable Nagle's algorithm (TCP_NODELAY).
    pub tcp_no_delay: bool,
}

impl Default for SocketOptions {
    fn default() -> Self {
        Self { tcp_no_delay: true }
    }
}

impl SocketOptions {
    /// Apply the options to a connected stream (best-effort).
    pub fn apply(&self, stream: &TcpStream) {
        let _ = stream.set_nodelay(self.tcp_no_delay);
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
        if let Ok(mut map) = self.inner.lock() {
            if let Some(entry) = map.get_mut(id) {
                entry.status = status;
            }
        }
    }

    fn mark_disconnected(&self, id: &SessionId) {
        if let Ok(mut map) = self.inner.lock() {
            if let Some(entry) = map.get_mut(id) {
                entry.connected = false;
            }
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
    let stream = TcpStream::connect(addr).await?;
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
    let tcp = TcpStream::connect(addr).await?;
    services.socket_options.apply(&tcp);
    let connector = tokio_rustls::TlsConnector::from(tls);
    let stream = connector.connect(server_name, tcp).await?;
    Ok(spawn_session_io(stream, config, app, services))
}

/// A listening acceptor that serves one static session configuration per connection.
pub struct Acceptor<A: Application + 'static> {
    listener: TcpListener,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
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
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            listener,
            config,
            app,
            services,
        })
    }

    /// The bound local address (useful when binding to port 0).
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Spawn the accept loop. Each connection is served as a session.
    pub fn serve(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Ok((stream, _peer)) = self.listener.accept().await {
                let _ = spawn_session(
                    stream,
                    self.config.clone(),
                    self.app.clone(),
                    self.services.clone(),
                );
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

#[allow(clippy::too_many_arguments)]
async fn run_connection<A, S>(
    mut stream: S,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    control_tx: mpsc::Sender<Control>,
    mut control: mpsc::Receiver<Control>,
    mut buf: Vec<u8>,
) where
    A: Application + 'static,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut session = Session::new(config);
    if let Some((dict, opts)) = &services.validator {
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
            if let Ok(stored) = store.get(1, out - 1).await {
                let msgs = stored
                    .into_iter()
                    .filter_map(|(seq, bytes)| decode(&bytes).ok().map(|m| (seq, m)));
                session.seed_sent_messages(msgs);
            }
        }
    }

    if let Some(monitor) = &services.monitor {
        monitor.register(id.clone(), session.status(), control_tx);
    }

    let mut chunk = [0u8; 4096];
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut logged_on = false;
    let mut control_open = true;
    let mut closing = false;

    if dispatch(
        &mut session,
        Event::Connected,
        &mut stream,
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

    // Drain any bytes pre-read while routing the connection (acceptor: the inbound Logon).
    if !closing
        && !buf.is_empty()
        && drain_messages(
            &mut buf,
            &mut session,
            &mut stream,
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
            read = stream.read(&mut chunk) => match read {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Some(slice) = chunk.get(..n) {
                        buf.extend_from_slice(slice);
                    }
                    if drain_messages(&mut buf, &mut session, &mut stream, &app, &id, &services, &mut logged_on)
                        .await
                        .is_err()
                    {
                        closing = true;
                    }
                }
            },
            _ = ticker.tick() => {
                if dispatch(&mut session, Event::Tick, &mut stream, &app, &id, &services, &mut logged_on)
                    .await
                    .is_err()
                {
                    closing = true;
                }
            },
            ctrl = recv_control(&mut control, control_open) => match ctrl {
                Some(Control::Logout) => {
                    if dispatch(&mut session, Event::StartLogout, &mut stream, &app, &id, &services, &mut logged_on)
                        .await
                        .is_err()
                    {
                        closing = true;
                    }
                }
                Some(Control::Reset) => {
                    session.reset();
                    if let Some(store) = &services.store {
                        let _ = store.reset().await;
                    }
                    if let Some(monitor) = &services.monitor {
                        monitor.update(&id, session.status());
                    }
                }
                Some(Control::Send(msg)) => {
                    if send_app(&mut session, *msg, &mut stream, &app, &id, &services)
                        .await
                        .is_err()
                    {
                        closing = true;
                    }
                }
                None => control_open = false,
            },
        }
    }

    let _ = stream.shutdown().await;
    if let Some(monitor) = &services.monitor {
        monitor.mark_disconnected(&id);
    }
    if logged_on {
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

#[allow(clippy::too_many_arguments)]
async fn drain_messages<A, S>(
    buf: &mut Vec<u8>,
    session: &mut Session,
    stream: &mut S,
    app: &Arc<A>,
    id: &SessionId,
    services: &Services,
    logged_on: &mut bool,
) -> Result<(), ()>
where
    A: Application + 'static,
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        match frame_length(buf) {
            Ok(Some(total)) => {
                let raw: Vec<u8> = buf.drain(..total).collect();
                if let Ok(msg) = decode(&raw) {
                    if let Some(log) = &services.log {
                        log.on_incoming(&String::from_utf8_lossy(&raw));
                    }
                    // Authentication / admin acceptance: an admin message the application rejects
                    // (e.g. a Logon with bad credentials) sends a Logout with the reject's text (if
                    // any) and tears the session down (FR-016).
                    if is_admin(&msg) {
                        if let Err(reject) = app.from_admin(&msg, id).await {
                            if let Some(log) = &services.log {
                                log.on_event(&format!(
                                    "{id}: admin message rejected by application: {reject}"
                                ));
                            }
                            let actions = session.reject_logon(&reject);
                            let _ =
                                perform_actions(actions, session, stream, app, id, services).await;
                            return Err(());
                        }
                    } else if let Err(breject) = app.from_app(&msg, id).await {
                        // Dictionary-driven business rejects are handled separately; this is the
                        // application-supplied one (FR-016). Sequence processing still proceeds.
                        let action = session.business_reject(&msg, &breject);
                        let _ =
                            perform_actions(vec![action], session, stream, app, id, services).await;
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
                    .await?;
                } else {
                    // The frame length was determinable but the content failed to decode (bad
                    // checksum/body-length/tag): honor RejectGarbledMessage (FR-006).
                    dispatch(
                        session,
                        Event::Garbled,
                        stream,
                        app,
                        id,
                        services,
                        logged_on,
                    )
                    .await?;
                }
            }
            Ok(None) => return Ok(()),
            Err(_) => {
                buf.clear();
                return Ok(());
            }
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
        app.on_logon(id).await;
        if let Some(log) = &services.log {
            log.on_event(&format!("{id}: logon"));
        }
    }

    let disconnect = perform_actions(actions, session, stream, app, id, services).await?;

    // Persist sequence numbers for restart continuity (best-effort).
    if let Some(store) = &services.store {
        let _ = store.set_next_sender_seq(session.next_out_seq()).await;
        let _ = store.set_next_target_seq(session.next_in_seq()).await;
    }

    if let Some(monitor) = &services.monitor {
        monitor.update(id, session.status());
    }

    if disconnect || session.state() == SessionState::Disconnected {
        Err(())
    } else {
        Ok(())
    }
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
                let bytes = msg.encode();
                if let Some(log) = &services.log {
                    log.on_outgoing(&String::from_utf8_lossy(&bytes));
                }
                if stream.write_all(&bytes).await.is_err() {
                    return Err(());
                }
                persist_sent(&msg, &bytes, session, services).await;
            }
            Action::Disconnect => disconnect = true,
        }
    }
    let _ = stream.flush().await;
    Ok(disconnect)
}

fn is_admin(msg: &Message) -> bool {
    matches!(
        msg.msg_type(),
        Some("0" | "1" | "2" | "3" | "4" | "5" | "A")
    )
}

/// Persist an original outbound transmission to the store for restart-survivable resend (FR-001/002).
/// Skips resends (PossDupFlag=Y keep the original's seq and must not overwrite it) and honours
/// `PersistMessages` (FR-003).
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
        let _ = store.save(seq as u64, bytes).await;
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
    if let Some(store) = &services.store {
        let _ = store.set_next_sender_seq(session.next_out_seq()).await;
    }
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
}

impl<A: Application + 'static> AcceptorBuilder<A> {
    /// Bind a multi-session acceptor.
    pub async fn bind(addr: SocketAddr, app: Arc<A>) -> io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            listener,
            app,
            sessions: HashMap::new(),
            template: None,
            allowed_remotes: None,
            services: Services::default(),
            tls: None,
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

    /// Register a static session (keyed by its SessionID).
    #[must_use]
    pub fn with_session(mut self, config: SessionConfig) -> Self {
        self.sessions.insert(config.session_id(), config);
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

    /// Attach store/log/socket services.
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
        });
        let app = self.app;
        let services = self.services;
        let listener = self.listener;
        let tls = self.tls.map(tokio_rustls::TlsAcceptor::from);
        tokio::spawn(async move {
            while let Ok((stream, peer)) = listener.accept().await {
                if let Some(allowed) = &registry.allowed_remotes {
                    if !allowed.contains(&peer.ip()) {
                        continue; // refuse: not in the allow-list
                    }
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

async fn route_and_run<A, S>(
    mut stream: S,
    app: Arc<A>,
    registry: Arc<Registry>,
    services: Services,
) where
    A: Application + 'static,
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // Read until the first complete message (the Logon) so we can route by SessionID.
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let logon = loop {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if let Some(slice) = chunk.get(..n) {
                    buf.extend_from_slice(slice);
                }
            }
        }
        match frame_length(&buf) {
            Ok(Some(total)) => match buf.get(..total).map(decode) {
                Some(Ok(msg)) => break msg,
                _ => return,
            },
            Ok(None) => continue,
            Err(_) => return,
        }
    };

    let begin = logon.begin_string().unwrap_or_default().to_owned();
    let their_sender = field_str(&logon, 49);
    let their_target = field_str(&logon, 56);
    // Our SessionID reverses the counterparty's comp IDs.
    let sid = SessionId::new(begin.clone(), their_target.clone(), their_sender.clone());

    let config = registry.sessions.get(&sid).cloned().or_else(|| {
        registry
            .template
            .as_ref()
            .map(|t| dynamic_config(t, &begin, &their_target, &their_sender))
    });
    let Some(config) = config else {
        return; // no static match and no template -> refuse
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

fn dynamic_config(
    template: &SessionConfig,
    begin: &str,
    our_sender: &str,
    our_target: &str,
) -> SessionConfig {
    let mut config = template.clone();
    config.begin_string = begin.to_owned();
    config.sender_comp_id = our_sender.to_owned();
    config.target_comp_id = our_target.to_owned();
    config
}

// ===========================================================================================
// Reconnecting initiator (S6).
// ===========================================================================================

/// Handle to a reconnecting initiator loop.
pub struct ReconnectHandle {
    stop: Arc<AtomicBool>,
    task: JoinHandle<()>,
}

impl ReconnectHandle {
    /// Signal the loop to stop after the current attempt.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Wait for the loop to finish.
    pub async fn join(self) {
        let _ = self.task.await;
    }
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
    let stop = Arc::new(AtomicBool::new(false));
    let loop_stop = stop.clone();
    let interval = Duration::from_secs(u64::from(config.reconnect_interval).max(1));
    let task = tokio::spawn(async move {
        while !loop_stop.load(Ordering::SeqCst) {
            if let Ok(stream) = TcpStream::connect(addr).await {
                let (control_tx, control_rx) = mpsc::channel(8);
                run_connection(
                    stream,
                    config.clone(),
                    app.clone(),
                    services.clone(),
                    control_tx,
                    control_rx,
                    Vec::new(),
                )
                .await;
            }
            if loop_stop.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(interval).await;
        }
    });
    ReconnectHandle { stop, task }
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
    let task = tokio::spawn(async move {
        let mut current: Option<SessionHandle> = None;
        let mut was_in_session = false;
        while !loop_stop.load(Ordering::SeqCst) {
            let in_session = schedule.is_in_session(time::OffsetDateTime::now_utc());
            match (in_session, current.is_some()) {
                (true, false) => {
                    if !was_in_session {
                        // Entering a session window: reset persisted state.
                        if let Some(store) = &services.store {
                            let _ = store.reset().await;
                        }
                    }
                    if let Ok(handle) =
                        connect_initiator_with(addr, config.clone(), app.clone(), services.clone())
                            .await
                    {
                        current = Some(handle);
                    }
                }
                (false, true) => {
                    if let Some(handle) = current.take() {
                        handle.logout().await;
                    }
                }
                _ => {}
            }
            was_in_session = in_session;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if let Some(handle) = current.take() {
            handle.logout().await;
        }
    });
    ScheduledHandle { stop, task }
}
