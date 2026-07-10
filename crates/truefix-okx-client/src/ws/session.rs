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
    pub fn login_acknowledged(&mut self) {
        self.authenticated = true;
        self.state = SessionState::Resubscribing;
    }
    /// Marks a public connection ready, or completes authenticated subscription recovery.
    pub fn subscriptions_replayed(&mut self) {
        self.state = SessionState::Active;
    }
    pub fn can_write(&self) -> bool {
        self.authentication_required && self.authenticated && self.state == SessionState::Active
    }
    pub fn is_authenticated(&self) -> bool {
        self.authenticated
    }
    pub fn next_request_id(&mut self) -> RequestId {
        self.next_request_id = self.next_request_id.wrapping_add(1);
        RequestId(self.next_request_id.to_string())
    }
    /// Starts a ping deadline. A new ping is not due until the previous one is answered.
    pub fn ping_due(&mut self, now: Instant) -> bool {
        match self.heartbeat_deadline {
            None => {
                self.heartbeat_deadline = Some(now + Self::HEARTBEAT_INTERVAL);
                true
            }
            Some(deadline) if now >= deadline => {
                self.disconnected();
                false
            }
            Some(_) => false,
        }
    }
    pub fn pong_received(&mut self) {
        self.heartbeat_deadline = None;
    }
    /// Moves to bounded exponential reconnect backoff and invalidates authentication.
    pub fn disconnected(&mut self) {
        self.authenticated = false;
        self.heartbeat_deadline = None;
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
