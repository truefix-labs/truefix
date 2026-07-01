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
use truefix_dict::{DataDictionary, ValidationOptions};

use crate::admin;
use crate::config::{Role, SessionConfig};
use crate::session_id::SessionId;
use crate::tags::{
    GAP_FILL_FLAG, HEART_BT_INT, MSG_SEQ_NUM, NEW_SEQ_NO, NEXT_EXPECTED_MSG_SEQ_NUM, POSS_DUP_FLAG,
    RESET_SEQ_NUM_FLAG,
};
use crate::time_util::now_utc_timestamp_prec;

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
    /// A frame failed to decode (bad checksum/body-length/tag). Honors `RejectGarbledMessage`
    /// (FR-006): emits a session Reject when enabled, otherwise produces no action (silent drop,
    /// sequence number not advanced).
    Garbled,
}

/// A point-in-time snapshot of a session, for monitoring (FR-L1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStatus {
    /// Lifecycle state.
    pub state: SessionState,
    /// Whether the session is logged on.
    pub logged_on: bool,
    /// Next outbound sequence number.
    pub next_sender_seq: u64,
    /// Next inbound sequence number expected.
    pub next_target_seq: u64,
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
    ticks_awaiting: u32,
    test_request_outstanding: bool,
    logon_sent: bool,
    resend_requested: bool,
    /// Sent messages, by sequence number, for resend.
    store: BTreeMap<u64, Message>,
    /// Received out-of-order messages (seq > expected), awaiting gap fill.
    queue: BTreeMap<u64, Message>,
    /// Optional inbound application-message validator (dictionary + toggles).
    validator: Option<(DataDictionary, ValidationOptions)>,
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
            ticks_awaiting: 0,
            test_request_outstanding: false,
            logon_sent: false,
            resend_requested: false,
            store: BTreeMap::new(),
            queue: BTreeMap::new(),
            validator: None,
        }
    }

    /// Enable inbound application-message validation against `dict` using `opts`. Admin messages
    /// are not validated here. Invalid messages produce a Reject (session-level) or
    /// BusinessMessageReject (business-level) instead of being delivered.
    pub fn set_dictionary(&mut self, dict: DataDictionary, opts: ValidationOptions) {
        self.validator = Some((dict, opts));
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

    /// Seed the sequence numbers from a persistent store (call before [`Event::Connected`]).
    /// Has no effect once a logon has been sent.
    pub fn seed_sequences(&mut self, next_out: u64, next_in: u64) {
        if !self.logon_sent {
            self.next_out_seq = next_out.max(1);
            self.next_in_seq = next_in.max(1);
        }
    }

    /// Re-hydrate previously-sent messages from a persistent store so a ResendRequest can replay
    /// them after a process restart (FR-001/002). Call before [`Event::Connected`]; no effect once a
    /// logon has been sent or when message persistence is disabled.
    pub fn seed_sent_messages(&mut self, messages: impl IntoIterator<Item = (u64, Message)>) {
        if self.logon_sent || !self.config.persist_messages {
            return;
        }
        for (seq, msg) in messages {
            self.store.insert(seq, msg);
        }
    }

    /// Whether sent messages are persisted for replay (PersistMessages; FR-003).
    pub fn persist_messages(&self) -> bool {
        self.config.persist_messages
    }

    /// A monitoring snapshot of the session (FR-L1).
    pub fn status(&self) -> SessionStatus {
        SessionStatus {
            state: self.state,
            logged_on: self.is_logged_on(),
            next_sender_seq: self.next_out_seq,
            next_target_seq: self.next_in_seq,
        }
    }

    /// Operational reset: clear stored messages and set both sequence numbers back to 1 (FR-L2).
    pub fn reset(&mut self) {
        self.reset_sequences(true);
        self.logon_sent = false;
    }

    /// Drive the engine with an [`Event`], producing [`Action`]s.
    pub fn handle(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::Connected => self.on_connected(),
            Event::Received(msg) => self.on_received(msg),
            Event::Tick => self.on_tick(),
            Event::StartLogout => self.on_start_logout(),
            Event::Garbled => self.on_garbled(),
        }
    }

    /// A frame failed to decode. Per `RejectGarbledMessage` (FR-006), either emit a session
    /// Reject (35=3, SessionRejectReason=0) or silently drop (no sequence number is available to
    /// advance, so a drop is always sequence-neutral).
    fn on_garbled(&mut self) -> Vec<Action> {
        if !self.config.reject_garbled_message || self.state != SessionState::LoggedOn {
            return Vec::new();
        }
        let seq = self.next_seq();
        let rej = admin::reject_with_reason(&self.config, seq, 0, 0, "garbled message");
        vec![self.send_stored(rej)]
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
        msg.header.set(Field::string(
            52,
            &now_utc_timestamp_prec(self.config.timestamp_precision),
        ));
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
    fn send_stored(&mut self, mut msg: Message) -> Action {
        self.ticks_since_send = 0;
        if self.config.enable_last_msg_seq_num_processed {
            // LastMsgSeqNumProcessed (369): the last inbound sequence number we have processed.
            let last = self.next_in_seq.saturating_sub(1);
            msg.header.set(Field::int(369, last as i64));
        }
        if self.config.persist_messages {
            if let Some(seq) = msg
                .header
                .get(MSG_SEQ_NUM)
                .and_then(|f| f.as_int().ok())
                .filter(|&s| s > 0)
            {
                self.store.insert(seq as u64, msg.clone());
            }
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

    /// Detect a BeginString or CompID mismatch (CheckCompID). Returns a Logout reason on failure.
    fn identity_problem(&self, msg: &Message) -> Option<&'static str> {
        if let Some(bs) = msg.header.get(8).and_then(|f| f.as_str().ok()) {
            if bs != self.config.begin_string {
                return Some("Incorrect BeginString");
            }
        }
        if self.config.check_comp_id {
            // Inbound SenderCompID(49) is the peer's sender = our target; TargetCompID(56) = ours.
            // Only a present-but-mismatched value is a CompID problem (absence is a separate concern).
            if let Some(sender) = msg.header.get(49).and_then(|f| f.as_str().ok()) {
                if sender != self.config.target_comp_id {
                    return Some("CompID problem");
                }
            }
            if let Some(target) = msg.header.get(56).and_then(|f| f.as_str().ok()) {
                if target != self.config.sender_comp_id {
                    return Some("CompID problem");
                }
            }
        }
        None
    }

    fn latency_ok(&self, msg: &Message) -> bool {
        if !self.config.check_latency {
            return true;
        }
        let Some(field) = msg.header.get(crate::tags::SENDING_TIME) else {
            return true;
        };
        let Ok(ts) = field.as_utc_timestamp() else {
            return true; // unparseable SendingTime is a dictionary/validation concern
        };
        let now = time::OffsetDateTime::now_utc();
        (now - ts).whole_seconds().abs() <= i64::from(self.config.max_latency)
    }

    fn on_connected(&mut self) -> Vec<Action> {
        self.ticks_awaiting = 0;
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

        // CheckLatency / MaxLatency: a stale SendingTime aborts the session.
        if !self.latency_ok(&msg) {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, Some("SendingTime accuracy problem"));
            self.state = SessionState::Disconnected;
            return vec![self.send_stored(lo), Action::Disconnect];
        }

        // BeginString / CheckCompID: a mismatched identity aborts the session (FR-007).
        if let Some(reason) = self.identity_problem(&msg) {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, Some(reason));
            self.state = SessionState::Disconnected;
            return vec![self.send_stored(lo), Action::Disconnect];
        }

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
                if let Some(reject) = self.validate_app(&msg) {
                    let mut actions = vec![reject];
                    self.next_in_seq = self.next_in_seq.saturating_add(1);
                    actions.extend(self.drain_queue());
                    return actions;
                }
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

    /// Validate an inbound application message; on failure, return a Reject/BusinessMessageReject
    /// action. Admin messages and the no-dictionary case return `None`.
    fn validate_app(&mut self, msg: &Message) -> Option<Action> {
        let error = {
            let (dict, opts) = self.validator.as_ref()?;
            if is_admin_type(msg.msg_type()) {
                return None;
            }
            dict.validate(msg, opts).err()
        }?;
        let ref_seq = msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map_or(0, |s| s as u64);
        let ref_mt = msg.msg_type().map(str::to_owned);
        let seq = self.next_seq();
        let mut reject = if error.business {
            admin::business_message_reject(
                &self.config,
                seq,
                ref_seq,
                ref_mt.as_deref(),
                error.reason.code(),
                &error.text,
            )
        } else {
            admin::reject_with_reason(&self.config, seq, ref_seq, error.reason.code(), &error.text)
        };
        reject.reverse_route(msg);
        Some(self.send_stored(reject))
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

        // Account for the Logon's own sequence number *before* building our response, so that
        // NextExpectedMsgSeqNum (789) and LastMsgSeqNumProcessed (369) on the response reflect
        // having consumed this Logon. The resulting actions (queue drain / ResendRequest) are
        // emitted *after* the Logon response below to preserve wire ordering.
        let disposition = msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map(|s| (s as u64).cmp(&self.next_in_seq));
        if disposition == Some(Ordering::Equal) {
            self.next_in_seq = self.next_in_seq.saturating_add(1);
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

        // Emit the deferred sequence actions after the Logon response.
        match disposition {
            Some(Ordering::Equal) => actions.extend(self.drain_queue()),
            Some(Ordering::Greater) if !self.resend_requested => {
                self.resend_requested = true;
                let begin = self.next_in_seq;
                actions.push(self.request_resend(begin));
            }
            _ => {}
        }

        // NextExpectedMsgSeqNum (789): the peer reports the next sequence number it expects from
        // us. If it is behind our outbound counter, resend the gap directly (avoids a round-trip).
        if let Some(next_expected) = msg
            .body
            .get(NEXT_EXPECTED_MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&n| n > 0)
            .map(|n| n as u64)
        {
            if next_expected < self.next_out_seq {
                let end = self.next_out_seq.saturating_sub(1);
                actions.extend(self.build_resend(next_expected, end));
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
        let mut hb = admin::heartbeat(&self.config, seq, id.as_deref());
        hb.reverse_route(msg);
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
            self.ticks_awaiting = 0;
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
        self.ticks_awaiting = self.ticks_awaiting.saturating_add(1);
        let hb = self.hb();
        let mut actions = Vec::new();
        match self.state {
            SessionState::AwaitingLogon
                if self.ticks_awaiting >= self.config.logon_timeout.max(1) =>
            {
                self.enter_disconnected();
                return vec![Action::Disconnect];
            }
            SessionState::LoggedOn => {
                if self.ticks_since_recv >= hb * self.config.heartbeat_timeout_multiplier.max(1) + 2
                {
                    self.enter_disconnected();
                    return vec![Action::Disconnect];
                }
                let test_request_delay = f64::from(hb) * self.config.test_request_delay_multiplier;
                if (self.ticks_since_recv as f64) > test_request_delay
                    && !self.test_request_outstanding
                {
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
            SessionState::AwaitingLogout
                if self.ticks_awaiting >= self.config.logout_timeout.max(1) =>
            {
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
