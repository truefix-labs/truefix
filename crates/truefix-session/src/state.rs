//! The sans-IO session state machine.
//!
//! The engine consumes [`Event`]s and returns [`Action`]s, performing no I/O itself, so it is
//! deterministic and unit-testable; the transport drives it. Stage S3 adds inbound sequence
//! validation, an in-memory store of sent messages, ResendRequest/SequenceReset handling
//! (gap fill, PossDup/OrigSendingTime), chunked resend, ResetSeqNumFlag, and NextExpectedMsgSeqNum.
//! Persistence beyond memory and scheduling arrive in later stages.

use core::cmp::Ordering;
use std::collections::BTreeMap;

use truefix_core::{Field, Message};

use crate::admin;
use crate::config::{Role, SessionConfig};
use crate::session_id::SessionId;
use crate::tags::{
    GAP_FILL_FLAG, HEART_BT_INT, MSG_SEQ_NUM, NEW_SEQ_NO, POSS_DUP_FLAG, RESET_SEQ_NUM_FLAG,
};
use crate::time_util::now_utc_timestamp;

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
    logon_sent: bool,
    resend_requested: bool,
    /// Sent messages, by sequence number, for resend.
    store: BTreeMap<u64, Message>,
    /// Received out-of-order messages (seq > expected), awaiting gap fill.
    queue: BTreeMap<u64, Message>,
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
            logon_sent: false,
            resend_requested: false,
            store: BTreeMap::new(),
            queue: BTreeMap::new(),
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

    /// The next outbound sequence number that will be used.
    pub fn next_out_seq(&self) -> u64 {
        self.next_out_seq
    }

    /// The next inbound sequence number expected.
    pub fn next_in_seq(&self) -> u64 {
        self.next_in_seq
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

    /// Send an application message: stamps the header (seq, comp IDs, SendingTime), stores it for
    /// possible resend, and returns the send action.
    pub fn send_app(&mut self, mut msg: Message) -> Vec<Action> {
        let seq = self.next_seq();
        msg.header.set(Field::string(8, &self.config.begin_string));
        msg.header
            .set(Field::string(49, &self.config.sender_comp_id));
        msg.header
            .set(Field::string(56, &self.config.target_comp_id));
        msg.header.set(Field::int(MSG_SEQ_NUM, seq as i64));
        msg.header.set(Field::string(52, &now_utc_timestamp()));
        vec![self.send_stored(msg)]
    }

    fn hb(&self) -> u32 {
        self.config.heartbeat_interval.max(1)
    }

    fn next_seq(&mut self) -> u64 {
        let s = self.next_out_seq;
        self.next_out_seq = self.next_out_seq.saturating_add(1);
        s
    }

    /// Send a normally-sequenced message: store it by its MsgSeqNum for resend, reset the send
    /// timer.
    fn send_stored(&mut self, msg: Message) -> Action {
        self.ticks_since_send = 0;
        if let Some(seq) = msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
        {
            self.store.insert(seq as u64, msg.clone());
        }
        Action::Send(msg)
    }

    /// Send without allocating a new sequence number or storing (used for resend responses).
    fn send_raw(&mut self, msg: Message) -> Action {
        self.ticks_since_send = 0;
        Action::Send(msg)
    }

    fn reset_sequences(&mut self, full: bool) {
        self.next_in_seq = 1;
        self.queue.clear();
        self.resend_requested = false;
        if full {
            self.next_out_seq = 1;
            self.store.clear();
        }
    }

    fn on_connected(&mut self) -> Vec<Action> {
        self.state = SessionState::AwaitingLogon;
        match self.config.role {
            Role::Initiator => {
                let next_exp = self.maybe_next_expected();
                let seq = self.next_seq();
                let msg = admin::logon(&self.config, seq, next_exp);
                self.logon_sent = true;
                vec![self.send_stored(msg)]
            }
            Role::Acceptor => Vec::new(),
        }
    }

    fn maybe_next_expected(&self) -> Option<u64> {
        if self.config.enable_next_expected_msg_seq_num {
            Some(self.next_in_seq)
        } else {
            None
        }
    }

    fn on_received(&mut self, msg: Message) -> Vec<Action> {
        self.ticks_since_recv = 0;
        self.test_request_outstanding = false;
        let mt = msg.msg_type().map(str::to_owned);

        if mt.as_deref() == Some("A") {
            return self.on_logon(msg);
        }
        if mt.as_deref() == Some("4") {
            return self.on_sequence_reset(&msg);
        }

        let seq = match msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
        {
            Some(s) => s as u64,
            None => {
                let s = self.next_seq();
                let rej = admin::reject(&self.config, s, 0, "missing or invalid MsgSeqNum");
                return vec![self.send_stored(rej)];
            }
        };
        let poss_dup = msg.header.get(POSS_DUP_FLAG).and_then(|f| f.as_str().ok()) == Some("Y");

        match seq.cmp(&self.next_in_seq) {
            Ordering::Equal => {
                let mut actions = self.process_in_order(&msg, mt.as_deref());
                self.next_in_seq = self.next_in_seq.saturating_add(1);
                actions.extend(self.drain_queue());
                actions
            }
            Ordering::Greater => {
                self.queue.insert(seq, msg);
                if self.resend_requested {
                    Vec::new()
                } else {
                    self.resend_requested = true;
                    let begin = self.next_in_seq;
                    vec![self.request_resend(begin)]
                }
            }
            Ordering::Less => {
                if poss_dup {
                    Vec::new()
                } else {
                    let s = self.next_seq();
                    let lo = admin::logout(&self.config, s, Some("MsgSeqNum too low"));
                    self.state = SessionState::Disconnected;
                    vec![self.send_stored(lo), Action::Disconnect]
                }
            }
        }
    }

    fn process_in_order(&mut self, msg: &Message, mt: Option<&str>) -> Vec<Action> {
        match mt {
            Some("1") => self.answer_test_request(msg),
            Some("2") => self.on_resend_request(msg),
            Some("5") => self.on_logout_msg(),
            _ => Vec::new(),
        }
    }

    fn drain_queue(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        while let Some(msg) = self.queue.remove(&self.next_in_seq) {
            let mt = msg.msg_type().map(str::to_owned);
            actions.extend(self.process_in_order(&msg, mt.as_deref()));
            self.next_in_seq = self.next_in_seq.saturating_add(1);
        }
        if self.queue.is_empty() {
            self.resend_requested = false;
        }
        actions
    }

    fn request_resend(&mut self, begin: u64) -> Action {
        let chunk = u64::from(self.config.resend_request_chunk_size);
        let end = if chunk > 0 {
            begin + chunk - 1
        } else {
            0 // 0 = to infinity
        };
        let seq = self.next_seq();
        let rr = admin::resend_request(&self.config, seq, begin, end);
        self.send_stored(rr)
    }

    fn on_resend_request(&mut self, msg: &Message) -> Vec<Action> {
        let begin = msg
            .body
            .get(crate::tags::BEGIN_SEQ_NO)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map_or(0, |s| s as u64);
        let end_req = msg
            .body
            .get(crate::tags::END_SEQ_NO)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s >= 0)
            .map_or(0, |s| s as u64);
        let last = self.next_out_seq.saturating_sub(1);
        let end = if end_req == 0 || end_req > last {
            last
        } else {
            end_req
        };
        if begin == 0 || begin > end {
            return Vec::new();
        }
        self.build_resend(begin, end)
    }

    fn build_resend(&mut self, begin: u64, end: u64) -> Vec<Action> {
        let mut actions = Vec::new();
        let mut gap_start: Option<u64> = None;
        let mut seq = begin;
        while seq <= end {
            let stored = self.store.get(&seq).cloned();
            let is_app = stored
                .as_ref()
                .is_some_and(|m| !is_admin_type(m.msg_type()));
            if is_app {
                if let Some(gs) = gap_start.take() {
                    let action = self.gap_fill(gs, seq);
                    actions.push(action);
                }
                if let Some(m) = stored {
                    let resent = admin::prepare_resend(&m);
                    actions.push(self.send_raw(resent));
                }
            } else if gap_start.is_none() {
                gap_start = Some(seq);
            }
            seq = seq.saturating_add(1);
        }
        if let Some(gs) = gap_start.take() {
            let new_seq = end.saturating_add(1);
            let action = self.gap_fill(gs, new_seq);
            actions.push(action);
        }
        actions
    }

    fn gap_fill(&mut self, from_seq: u64, new_seq_no: u64) -> Action {
        let m = admin::sequence_reset(&self.config, from_seq, new_seq_no, true);
        self.send_raw(m)
    }

    fn on_sequence_reset(&mut self, msg: &Message) -> Vec<Action> {
        let gap_fill = msg.body.get(GAP_FILL_FLAG).and_then(|f| f.as_str().ok()) == Some("Y");
        let new_seq = msg
            .body
            .get(NEW_SEQ_NO)
            .and_then(|f| f.as_int().ok())
            .filter(|&n| n > 0)
            .map(|n| n as u64);
        if let Some(ns) = new_seq {
            if gap_fill {
                if ns >= self.next_in_seq {
                    self.next_in_seq = ns;
                }
            } else {
                self.next_in_seq = ns;
            }
        }
        self.drain_queue()
    }

    fn on_logon(&mut self, msg: Message) -> Vec<Action> {
        if msg
            .body
            .get(RESET_SEQ_NUM_FLAG)
            .and_then(|f| f.as_str().ok())
            == Some("Y")
        {
            self.reset_sequences(!self.logon_sent);
        }

        let mut actions = Vec::new();
        match self.config.role {
            Role::Acceptor if self.state == SessionState::AwaitingLogon => {
                if let Some(hbi) = msg
                    .body
                    .get(HEART_BT_INT)
                    .and_then(|f| f.as_int().ok())
                    .filter(|&h| h > 0)
                {
                    self.config.heartbeat_interval = hbi as u32;
                }
                self.state = SessionState::LoggedOn;
                let next_exp = self.maybe_next_expected();
                let seq = self.next_seq();
                let resp = admin::logon(&self.config, seq, next_exp);
                self.logon_sent = true;
                actions.push(self.send_stored(resp));
            }
            Role::Initiator if self.state == SessionState::AwaitingLogon => {
                self.state = SessionState::LoggedOn;
            }
            _ => {}
        }

        // Sequence accounting for the Logon message itself.
        if let Some(seq) = msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map(|s| s as u64)
        {
            match seq.cmp(&self.next_in_seq) {
                Ordering::Equal => {
                    self.next_in_seq = self.next_in_seq.saturating_add(1);
                    actions.extend(self.drain_queue());
                }
                Ordering::Greater => {
                    if !self.resend_requested {
                        self.resend_requested = true;
                        let begin = self.next_in_seq;
                        actions.push(self.request_resend(begin));
                    }
                }
                Ordering::Less => {}
            }
        }
        actions
    }

    fn answer_test_request(&mut self, msg: &Message) -> Vec<Action> {
        if self.state != SessionState::LoggedOn {
            return Vec::new();
        }
        let id = msg
            .body
            .get(crate::tags::TEST_REQ_ID)
            .and_then(|f| f.as_str().ok())
            .map(str::to_owned);
        let seq = self.next_seq();
        let hb = admin::heartbeat(&self.config, seq, id.as_deref());
        vec![self.send_stored(hb)]
    }

    fn on_logout_msg(&mut self) -> Vec<Action> {
        if self.state == SessionState::AwaitingLogout {
            self.enter_disconnected();
            vec![Action::Disconnect]
        } else {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, None);
            let send = self.send_stored(lo);
            self.enter_disconnected();
            vec![send, Action::Disconnect]
        }
    }

    fn on_start_logout(&mut self) -> Vec<Action> {
        if self.state == SessionState::LoggedOn {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, None);
            self.state = SessionState::AwaitingLogout;
            vec![self.send_stored(lo)]
        } else {
            Vec::new()
        }
    }

    fn enter_disconnected(&mut self) {
        self.state = SessionState::Disconnected;
        if self.config.reset_on_disconnect || self.config.reset_on_logout {
            self.reset_sequences(true);
            self.logon_sent = false;
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
                    self.enter_disconnected();
                    return vec![Action::Disconnect];
                }
                if self.ticks_since_recv > hb && !self.test_request_outstanding {
                    let seq = self.next_seq();
                    let tr = admin::test_request(&self.config, seq, "TEST");
                    self.test_request_outstanding = true;
                    actions.push(self.send_stored(tr));
                }
                if self.ticks_since_send >= hb {
                    let seq = self.next_seq();
                    let h = admin::heartbeat(&self.config, seq, None);
                    actions.push(self.send_stored(h));
                }
            }
            SessionState::AwaitingLogout if self.ticks_since_send >= 2 * hb + 2 => {
                self.enter_disconnected();
                return vec![Action::Disconnect];
            }
            _ => {}
        }
        actions
    }
}

fn is_admin_type(mt: Option<&str>) -> bool {
    matches!(mt, Some("0" | "1" | "2" | "3" | "4" | "5" | "A"))
}
