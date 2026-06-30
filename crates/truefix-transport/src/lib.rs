//! `truefix-transport` — tokio TCP Initiator and Acceptor that drive a [`Session`].
//!
//! Stage S2–S5: single-session initiator/acceptor over plain TCP, framing FIX messages off the
//! stream and pumping the sans-IO [`Session`] engine, with optional [`Services`] (a persistent
//! [`MessageStore`] for sequence-number continuity across restarts, and a [`Log`]). Multi/dynamic
//! sessions, reconnect, and TLS arrive in later stages.
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
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use truefix_core::{decode, Message};
use truefix_log::Log;
use truefix_session::{
    Action, Application, Event, Session, SessionConfig, SessionId, SessionState,
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
}

/// Control messages to a running session task.
enum Control {
    /// Begin a graceful logout.
    Logout,
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
    let (control_tx, control_rx) = mpsc::channel(8);
    let task = tokio::spawn(async move {
        run_connection(stream, config, app, services, control_rx, Vec::new()).await;
    });
    SessionHandle {
        control: control_tx,
        task,
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_connection<A>(
    mut stream: TcpStream,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    mut control: mpsc::Receiver<Control>,
    mut buf: Vec<u8>,
) where
    A: Application + 'static,
{
    let mut session = Session::new(config);
    let id = session.id().clone();
    app.on_create(&id).await;
    services.socket_options.apply(&stream);

    // Restore sequence numbers from the store (continuity across restarts).
    if let Some(store) = &services.store {
        let out = store.next_sender_seq().await.unwrap_or(1);
        let inn = store.next_target_seq().await.unwrap_or(1);
        session.seed_sequences(out, inn);
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
                None => control_open = false,
            },
        }
    }

    let _ = stream.shutdown().await;
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
async fn drain_messages<A>(
    buf: &mut Vec<u8>,
    session: &mut Session,
    stream: &mut TcpStream,
    app: &Arc<A>,
    id: &SessionId,
    services: &Services,
    logged_on: &mut bool,
) -> Result<(), ()>
where
    A: Application + 'static,
{
    loop {
        match frame_length(buf) {
            Ok(Some(total)) => {
                let raw: Vec<u8> = buf.drain(..total).collect();
                if let Ok(msg) = decode(&raw) {
                    if let Some(log) = &services.log {
                        log.on_incoming(&String::from_utf8_lossy(&raw));
                    }
                    let inbound_ok = if is_admin(&msg) {
                        app.from_admin(&msg, id).await.is_ok()
                    } else {
                        app.from_app(&msg, id).await.is_ok()
                    };
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
                    if !inbound_ok {
                        return Err(());
                    }
                }
                // Garbled frames are dropped at this stage; RejectGarbledMessage arrives in S3.
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
async fn dispatch<A>(
    session: &mut Session,
    event: Event,
    stream: &mut TcpStream,
    app: &Arc<A>,
    id: &SessionId,
    services: &Services,
    logged_on: &mut bool,
) -> Result<(), ()>
where
    A: Application + 'static,
{
    let actions = session.handle(event);

    if !*logged_on && session.is_logged_on() {
        *logged_on = true;
        app.on_logon(id).await;
        if let Some(log) = &services.log {
            log.on_event(&format!("{id}: logon"));
        }
    }

    let mut disconnect = false;
    for action in actions {
        match action {
            Action::Send(mut msg) => {
                if is_admin(&msg) {
                    app.to_admin(&mut msg, id).await;
                } else {
                    app.to_app(&mut msg, id).await;
                }
                let bytes = msg.encode();
                if let Some(log) = &services.log {
                    log.on_outgoing(&String::from_utf8_lossy(&bytes));
                }
                if stream.write_all(&bytes).await.is_err() {
                    return Err(());
                }
            }
            Action::Disconnect => disconnect = true,
        }
    }
    let _ = stream.flush().await;

    // Persist sequence numbers for restart continuity (best-effort).
    if let Some(store) = &services.store {
        let _ = store.set_next_sender_seq(session.next_out_seq()).await;
        let _ = store.set_next_target_seq(session.next_in_seq()).await;
    }

    if disconnect || session.state() == SessionState::Disconnected {
        Err(())
    } else {
        Ok(())
    }
}

fn is_admin(msg: &Message) -> bool {
    matches!(
        msg.msg_type(),
        Some("0" | "1" | "2" | "3" | "4" | "5" | "A")
    )
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
        })
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
        tokio::spawn(async move {
            while let Ok((stream, peer)) = listener.accept().await {
                let registry = registry.clone();
                let app = app.clone();
                let services = services.clone();
                tokio::spawn(async move {
                    serve_connection(stream, peer, app, registry, services).await;
                });
            }
        })
    }
}

async fn serve_connection<A>(
    mut stream: TcpStream,
    peer: SocketAddr,
    app: Arc<A>,
    registry: Arc<Registry>,
    services: Services,
) where
    A: Application + 'static,
{
    if let Some(allowed) = &registry.allowed_remotes {
        if !allowed.contains(&peer.ip()) {
            return; // refuse: not in the allow-list
        }
    }
    services.socket_options.apply(&stream);

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

    let (_control_tx, control_rx) = mpsc::channel(1);
    run_connection(stream, config, app, services, control_rx, buf).await;
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
                let (_control_tx, control_rx) = mpsc::channel(1);
                run_connection(
                    stream,
                    config.clone(),
                    app.clone(),
                    services.clone(),
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
