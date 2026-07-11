//! Connection lifecycle, heartbeat and bounded reconnect policy.

use std::time::{Duration, Instant};

use crate::types::websocket::RequestId;

/// Observable session lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Disconnected,
    Connecting,
    Authenticating,
    Resubscribing,
    Active,
    Backoff,
}

/// State machine used by all WebSocket entrypoints.
///
/// It deliberately does not retain a command queue: a connection loss makes a write's
/// completion unknown, so writes can never be replayed by this layer.
#[derive(Debug)]
pub struct Session {
    state: SessionState,
    authentication_required: bool,
    authenticated: bool,
    next_request_id: u64,
    reconnect_attempt: u8,
    heartbeat_deadline: Option<Instant>,
    awaiting_pong: bool,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            state: SessionState::Disconnected,
            authentication_required: false,
            authenticated: false,
            next_request_id: 0,
            reconnect_attempt: 0,
            heartbeat_deadline: None,
            awaiting_pong: false,
        }
    }
}

impl Session {
    pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
    const MAX_BACKOFF: Duration = Duration::from_secs(30);

    pub fn state(&self) -> SessionState {
        self.state
    }
    pub fn connecting(&mut self) {
        self.state = SessionState::Connecting;
    }
    pub fn login_required(&mut self) {
        self.authentication_required = true;
        self.authenticated = false;
        self.state = SessionState::Authenticating;
    }
    /// Records the outcome of a login acknowledgement.
    ///
    /// A rejected login must not transition the session into the authenticated
    /// recovery path: private subscriptions and writes remain unavailable until
    /// OKX has explicitly acknowledged a successful login.
    pub fn login_acknowledged(&mut self, success: bool) {
        if success {
            self.authenticated = true;
            self.state = SessionState::Resubscribing;
        } else {
            self.authenticated = false;
            self.state = SessionState::Authenticating;
        }
    }
    /// Marks a public connection ready, or completes authenticated subscription recovery.
    pub fn subscriptions_replayed(&mut self) {
        if !self.authentication_required || self.authenticated {
            self.state = SessionState::Active;
        }
    }
    pub fn can_write(&self) -> bool {
        self.authentication_required && self.authenticated && self.state == SessionState::Active
    }
    pub fn is_active(&self) -> bool {
        self.state == SessionState::Active
    }
    pub fn is_authenticated(&self) -> bool {
        self.authenticated
    }
    pub fn next_request_id(&mut self) -> RequestId {
        self.next_request_id = self.next_request_id.wrapping_add(1);
        RequestId(self.next_request_id.to_string())
    }
    /// Records an inbound OKX application message and resets the idle heartbeat timer.
    pub fn message_received(&mut self, now: Instant) {
        self.heartbeat_deadline = Some(now + Self::HEARTBEAT_INTERVAL);
        self.awaiting_pong = false;
    }
    /// Returns whether an application ping must be sent because the peer has been idle.
    ///
    /// The first call arms the idle timer. Once a ping is sent, the next expiry without a
    /// response disconnects the session rather than sending another ping.
    pub fn ping_due(&mut self, now: Instant) -> bool {
        match self.heartbeat_deadline {
            None => {
                self.heartbeat_deadline = Some(now + Self::HEARTBEAT_INTERVAL);
                false
            }
            Some(deadline) if now >= deadline => {
                if self.awaiting_pong {
                    self.disconnected();
                    false
                } else {
                    self.awaiting_pong = true;
                    self.heartbeat_deadline = Some(now + Self::HEARTBEAT_INTERVAL);
                    true
                }
            }
            Some(_) => false,
        }
    }
    pub fn pong_received(&mut self) {
        self.heartbeat_deadline = None;
        self.awaiting_pong = false;
    }
    /// Moves to bounded exponential reconnect backoff and invalidates authentication.
    pub fn disconnected(&mut self) {
        self.authenticated = false;
        self.heartbeat_deadline = None;
        self.awaiting_pong = false;
        self.reconnect_attempt = self.reconnect_attempt.saturating_add(1);
        self.state = SessionState::Backoff;
    }
    pub fn reconnect_delay(&self) -> Duration {
        let seconds = 1_u64 << u32::from(self.reconnect_attempt.saturating_sub(1).min(5));
        Duration::from_secs(seconds).min(Self::MAX_BACKOFF)
    }
    pub fn reconnecting(&mut self) {
        self.state = SessionState::Connecting;
    }
    pub fn connected(&mut self) {
        self.reconnect_attempt = 0;
        if self.authentication_required {
            self.login_required();
        } else {
            self.subscriptions_replayed();
        }
    }
}
