//! The sans-IO session state machine.
//!
//! The engine consumes [`Event`]s (connection, inbound message, timer tick, logout request)
//! and returns [`Action`]s (send a message, disconnect). It performs no I/O itself, so it is
//! fully deterministic and unit-testable; the transport drives it. Sequence-gap recovery and
//! persistence arrive in Stage S3.

use truefix_core::Message;

use crate::admin;
use crate::config::{Role, SessionConfig};
use crate::session_id::SessionId;

/// Session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Not connected / logged out.
    Disconnected,
    /// Logon sent or awaited; not yet logged on.
    AwaitingLogon,
    /// Fully logged on.
    LoggedOn,
    /// Logout initiated; awaiting the counterparty's Logout.
    AwaitingLogout,
}

/// An input to the engine.
#[derive(Debug)]
pub enum Event {
    /// The transport connection is established.
    Connected,
    /// An inbound message was decoded.
    Received(Message),
    /// A periodic (≈1 second) timer tick.
    Tick,
    /// The application requested a graceful logout.
    StartLogout,
}

/// An output from the engine for the transport to perform.
#[derive(Debug)]
pub enum Action {
    /// Send this message to the counterparty.
    Send(Message),
    /// Close the connection.
    Disconnect,
}

/// A single FIX session.
pub struct Session {
    config: SessionConfig,
    id: SessionId,
    state: SessionState,
    next_out_seq: u64,
    next_in_seq: u64,
    ticks_since_send: u32,
    ticks_since_recv: u32,
    test_request_outstanding: bool,
}

impl Session {
    /// Create a new session in the `Disconnected` state.
    pub fn new(config: SessionConfig) -> Self {
        let id = config.session_id();
        Self {
            config,
            id,
            state: SessionState::Disconnected,
            next_out_seq: 1,
            next_in_seq: 1,
            ticks_since_send: 0,
            ticks_since_recv: 0,
            test_request_outstanding: false,
        }
    }

    /// The session's identity.
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    /// The current state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Whether the session is logged on.
    pub fn is_logged_on(&self) -> bool {
        self.state == SessionState::LoggedOn
    }

    /// Drive the engine with an [`Event`], producing [`Action`]s.
    pub fn handle(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::Connected => self.on_connected(),
            Event::Received(msg) => self.on_received(msg),
            Event::Tick => self.on_tick(),
            Event::StartLogout => self.on_start_logout(),
        }
    }

    fn hb(&self) -> u32 {
        self.config.heartbeat_interval.max(1)
    }

    fn next_seq(&mut self) -> u64 {
        let s = self.next_out_seq;
        self.next_out_seq = self.next_out_seq.saturating_add(1);
        s
    }

    fn send(&mut self, msg: Message) -> Action {
        self.ticks_since_send = 0;
        Action::Send(msg)
    }

    fn on_connected(&mut self) -> Vec<Action> {
        self.state = SessionState::AwaitingLogon;
        match self.config.role {
            Role::Initiator => {
                let seq = self.next_seq();
                let msg = admin::logon(&self.config, seq);
                vec![self.send(msg)]
            }
            Role::Acceptor => Vec::new(),
        }
    }

    fn on_received(&mut self, msg: Message) -> Vec<Action> {
        self.ticks_since_recv = 0;
        self.test_request_outstanding = false;
        self.next_in_seq = self.next_in_seq.saturating_add(1);
        match msg.msg_type() {
            Some("A") => self.on_logon(&msg),
            Some("1") => self.on_test_request(&msg),
            Some("5") => self.on_logout_msg(),
            _ => Vec::new(),
        }
    }

    fn on_logon(&mut self, msg: &Message) -> Vec<Action> {
        match self.config.role {
            Role::Acceptor if self.state == SessionState::AwaitingLogon => {
                if let Some(hbi) = msg
                    .body
                    .get(108)
                    .and_then(|f| f.as_int().ok())
                    .filter(|&h| h > 0)
                {
                    self.config.heartbeat_interval = hbi as u32;
                }
                self.state = SessionState::LoggedOn;
                let seq = self.next_seq();
                let resp = admin::logon(&self.config, seq);
                vec![self.send(resp)]
            }
            Role::Initiator if self.state == SessionState::AwaitingLogon => {
                self.state = SessionState::LoggedOn;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn on_test_request(&mut self, msg: &Message) -> Vec<Action> {
        if self.state != SessionState::LoggedOn {
            return Vec::new();
        }
        let id = msg
            .body
            .get(112)
            .and_then(|f| f.as_str().ok())
            .map(str::to_owned);
        let seq = self.next_seq();
        let hb = admin::heartbeat(&self.config, seq, id.as_deref());
        vec![self.send(hb)]
    }

    fn on_logout_msg(&mut self) -> Vec<Action> {
        if self.state == SessionState::AwaitingLogout {
            self.state = SessionState::Disconnected;
            vec![Action::Disconnect]
        } else {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, None);
            self.state = SessionState::Disconnected;
            vec![self.send(lo), Action::Disconnect]
        }
    }

    fn on_start_logout(&mut self) -> Vec<Action> {
        if self.state == SessionState::LoggedOn {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, None);
            self.state = SessionState::AwaitingLogout;
            vec![self.send(lo)]
        } else {
            Vec::new()
        }
    }

    fn on_tick(&mut self) -> Vec<Action> {
        self.ticks_since_send = self.ticks_since_send.saturating_add(1);
        self.ticks_since_recv = self.ticks_since_recv.saturating_add(1);
        let hb = self.hb();
        let mut actions = Vec::new();
        match self.state {
            SessionState::LoggedOn => {
                if self.ticks_since_recv >= 2 * hb + 2 {
                    self.state = SessionState::Disconnected;
                    return vec![Action::Disconnect];
                }
                if self.ticks_since_recv > hb && !self.test_request_outstanding {
                    let seq = self.next_seq();
                    let tr = admin::test_request(&self.config, seq, "TEST");
                    self.test_request_outstanding = true;
                    actions.push(self.send(tr));
                }
                if self.ticks_since_send >= hb {
                    let seq = self.next_seq();
                    let h = admin::heartbeat(&self.config, seq, None);
                    actions.push(self.send(h));
                }
            }
            SessionState::AwaitingLogout if self.ticks_since_send >= 2 * hb + 2 => {
                self.state = SessionState::Disconnected;
                return vec![Action::Disconnect];
            }
            _ => {}
        }
        actions
    }
}
