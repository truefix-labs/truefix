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

use std::future::pending;
use std::net::SocketAddr;
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

/// Optional per-session services: a persistent message store and a log.
#[derive(Clone, Default)]
pub struct Services {
    /// Persistent store for sequence-number continuity (and future cross-restart resend).
    pub store: Option<Arc<dyn MessageStore>>,
    /// Message/event log.
    pub log: Option<Arc<dyn Log>>,
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
        run_connection(stream, config, app, services, control_rx).await;
    });
    SessionHandle {
        control: control_tx,
        task,
    }
}

async fn run_connection<A>(
    mut stream: TcpStream,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    mut control: mpsc::Receiver<Control>,
) where
    A: Application + 'static,
{
    let mut session = Session::new(config);
    let id = session.id().clone();
    app.on_create(&id).await;

    // Restore sequence numbers from the store (continuity across restarts).
    if let Some(store) = &services.store {
        let out = store.next_sender_seq().await.unwrap_or(1);
        let inn = store.next_target_seq().await.unwrap_or(1);
        session.seed_sequences(out, inn);
    }

    let mut buf: Vec<u8> = Vec::new();
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
