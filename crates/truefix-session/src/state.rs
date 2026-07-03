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
    GAP_FILL_FLAG, HEART_BT_INT, MSG_SEQ_NUM, NEW_SEQ_NO, NEXT_EXPECTED_MSG_SEQ_NUM,
    ORIG_SENDING_TIME, POSS_DUP_FLAG, RESET_SEQ_NUM_FLAG, SENDING_TIME, SESSION_STATUS,
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
    /// Resend a previously-sent application message during gap-fill resend, carrying its original
    /// sequence number (US3, feature 005, GAP-07/FR-007). Distinct from [`Action::Send`] so the
    /// transport can give `Application::to_app` a veto that — unlike a vetoed live send — must be
    /// backed by a compensating `SequenceReset-GapFill` for the sequence number rather than a bare
    /// discard, since a resend's sequence number was already promised to the counterparty in an
    /// earlier connection. See [`Session::gap_fill_after_veto`].
    Resend(Message, u64),
    /// Close the connection.
    Disconnect,
    /// A full sequence/message-history reset occurred in-engine (logon-time `ResetSeqNumFlag`, or
    /// a disconnect-triggered `ResetOnDisconnect`/`ResetOnLogout`); the transport MUST clear the
    /// durable store (if attached) so its persisted state stays consistent with the in-memory
    /// reset (FR-004). Distinct from the explicit `Session::reset()` API, whose one call site
    /// already pairs a store reset manually.
    ResetStore,
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
    /// GAP-09/FR-011 (feature 005): the full known upper bound of an in-progress inbound gap,
    /// tracked only while `resend_request_chunk_size > 0`. `None` when no chunked resend is
    /// outstanding.
    resend_target: Option<u64>,
    /// GAP-09/FR-011 (feature 005): the end sequence number of the currently outstanding
    /// chunked `ResendRequest`, paired with `resend_target`.
    resend_chunk_end: Option<u64>,
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
            resend_target: None,
            resend_chunk_end: None,
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

    /// Whether this session is configured to refresh sequence numbers from the durable store
    /// right after logon completes (`RefreshOnLogon`).
    pub fn refresh_on_logon(&self) -> bool {
        self.config.refresh_on_logon
    }

    /// Whether this session is configured to force a full reset when the durable store reports it
    /// recovered from corruption (`ForceResendWhenCorruptedStore`).
    pub fn force_resend_when_corrupted_store(&self) -> bool {
        self.config.force_resend_when_corrupted_store
    }

    /// Force sequence numbers to `next_out`/`next_in`, unconditionally (unlike [`Self::seed_sequences`],
    /// which only applies before a logon has been sent). Used by the transport to honour
    /// `RefreshOnLogon`: re-sync from the durable store immediately after logon completes, in case
    /// it changed between connect time and logon (FR-008).
    pub fn refresh_sequences(&mut self, next_out: u64, next_in: u64) {
        self.next_out_seq = next_out.max(1);
        self.next_in_seq = next_in.max(1);
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

    /// Discard a previously-stored sent message (e.g. suppressed by an outbound `to_app` callback
    /// returning `DoNotSend`), so a future ResendRequest gap-fills that sequence number instead of
    /// replaying content that was never actually transmitted (FR-016).
    pub fn discard_sent(&mut self, seq: u64) {
        self.store.remove(&seq);
    }

    /// PossDup anti-replay check (US3, feature 005, GAP-08/FR-008/FR-009): a stale-but-legitimate
    /// resend's `OrigSendingTime` is never later than its own `SendingTime`. Returns `Some(reject)`
    /// when `OrigSendingTime` is present and later than `SendingTime` (unconditional — a real
    /// replay/falsification signal), or when it's missing and
    /// `requires_orig_sending_time_on_low_seq` is set (FR-009); `None` otherwise (matches today's
    /// silent-drop behavior).
    fn poss_dup_anti_replay_violation(&self, msg: &Message) -> Option<truefix_core::Reject> {
        let orig_sending_time = msg
            .header
            .get(ORIG_SENDING_TIME)
            .and_then(|f| f.as_utc_timestamp().ok());
        match orig_sending_time {
            Some(orig) => {
                let sending_time = msg
                    .header
                    .get(SENDING_TIME)
                    .and_then(|f| f.as_utc_timestamp().ok());
                if sending_time.is_some_and(|st| orig > st) {
                    Some(truefix_core::Reject {
                        reason: 5, // SessionRejectReason: Value is incorrect (out of range) for this tag
                        ref_tag: Some(ORIG_SENDING_TIME),
                        text: Some(
                            "OrigSendingTime is later than SendingTime on a PossDup message"
                                .to_owned(),
                        ),
                        session_status: None,
                    })
                } else {
                    None
                }
            }
            None if self.config.requires_orig_sending_time_on_low_seq => {
                Some(truefix_core::Reject {
                    reason: 1, // SessionRejectReason: Required tag missing
                    ref_tag: Some(ORIG_SENDING_TIME),
                    text: Some("OrigSendingTime is required on a PossDup message".to_owned()),
                    session_status: None,
                })
            }
            None => None,
        }
    }

    /// Refuse the session in response to an admin/logon callback rejection
    /// (`Application::from_admin` returning `Err(Reject)`): send a Logout carrying the rejection
    /// text (if any) and disconnect (FR-016). If `reject` carries a `SessionStatus` (tag 573)
    /// reason, it is stamped onto the outbound Logout (US10, FR-013).
    pub fn reject_logon(&mut self, reject: &truefix_core::Reject) -> Vec<Action> {
        let seq = self.next_seq();
        let mut lo = admin::logout(&self.config, seq, reject.text.as_deref());
        if let Some(status) = reject.session_status {
            lo.body.set(Field::int(SESSION_STATUS, i64::from(status)));
        }
        self.state = SessionState::Disconnected;
        vec![self.send_stored(lo), Action::Disconnect]
    }

    /// Emit a Business Message Reject (35=j) in response to `original`, using an
    /// application-supplied reason/ref-tag/text (`Application::from_app` returning
    /// `Err(BusinessReject)`; FR-016/FR-008).
    pub fn business_reject(
        &mut self,
        original: &Message,
        reject: &truefix_core::BusinessReject,
    ) -> Action {
        let ref_seq = original
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map_or(0, |s| s as u64);
        let ref_mt = original.msg_type().map(str::to_owned);
        let seq = self.next_seq();
        let mut msg = admin::business_message_reject(
            &self.config,
            seq,
            ref_seq,
            ref_mt.as_deref(),
            reject.ref_tag,
            reject.reason,
            reject.text.as_deref().unwrap_or(""),
        );
        msg.reverse_route(original);
        self.send_stored(msg)
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
        let rej = admin::reject_with_reason(&self.config, seq, 0, None, 0, "garbled message");
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

    /// Resend a previously-sent application message during gap-fill resend (US3, feature 005,
    /// GAP-07/FR-007), carrying its original sequence number so the transport can substitute a
    /// compensating `GapFill` (via [`Session::gap_fill_after_veto`]) if `Application::to_app` vetoes
    /// it, instead of the bare discard a vetoed live [`Action::Send`] gets.
    fn send_resend(&mut self, msg: Message, seq: u64) -> Action {
        self.ticks_since_send = 0;
        Action::Resend(msg, seq)
    }

    /// Build the compensating `SequenceReset-GapFill` for a single resend-originated message the
    /// transport's `Application::to_app` callback vetoed (US3, feature 005, GAP-07/FR-007). Call
    /// this instead of a bare `discard_sent` when the vetoed [`Action::Resend`] carried a real
    /// sequence number that was already promised to the counterparty in an earlier connection — an
    /// unfilled gap there (unlike a vetoed *live* send, which never promised anything) would leave
    /// the counterparty's own sequence tracking permanently stuck.
    pub fn gap_fill_after_veto(&mut self, seq: u64) -> Action {
        self.gap_fill(seq, seq.saturating_add(1))
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

    /// When `ResetOnError` is configured, perform a full reset in response to an inbound-message
    /// processing error (identity/latency failure, or a dictionary-validation reject) and return
    /// the `Action::ResetStore` signal so the durable store stays consistent (FR-008/FR-004). A
    /// no-op (returns nothing) when the switch is off.
    fn reset_on_error(&mut self) -> Vec<Action> {
        if !self.config.reset_on_error {
            return Vec::new();
        }
        self.reset_sequences(true);
        self.logon_sent = false;
        vec![Action::ResetStore]
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
            let mut actions = vec![self.send_stored(lo)];
            actions.extend(self.reset_on_error());
            actions.push(Action::Disconnect);
            return actions;
        }

        // BeginString / CheckCompID: a mismatched identity aborts the session (FR-007).
        if let Some(reason) = self.identity_problem(&msg) {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, Some(reason));
            self.state = SessionState::Disconnected;
            let mut actions = vec![self.send_stored(lo)];
            actions.extend(self.reset_on_error());
            actions.push(Action::Disconnect);
            return actions;
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
                    actions.extend(self.reset_on_error());
                    if self.config.disconnect_on_error {
                        self.state = SessionState::Disconnected;
                        actions.push(Action::Disconnect);
                    } else {
                        actions.extend(self.drain_queue());
                    }
                    return actions;
                }
                let mut actions = self.process_in_order(&msg, mt.as_deref());
                self.next_in_seq = self.next_in_seq.saturating_add(1);
                actions.extend(self.drain_queue());
                actions
            }
            Ordering::Greater => {
                self.queue.insert(seq, msg);
                // GAP-09/FR-011 (feature 005): track this arrival as (at least) the full known
                // extent of the gap, so a chunked resend can auto-continue past its first chunk.
                if self.config.resend_request_chunk_size > 0 {
                    self.resend_target = Some(self.resend_target.map_or(seq, |t| t.max(seq)));
                }
                if self.resend_requested && !self.config.send_redundant_resend_requests {
                    Vec::new()
                } else {
                    self.resend_requested = true;
                    let begin = self.next_in_seq;
                    vec![self.request_resend(begin)]
                }
            }
            Ordering::Less => {
                if poss_dup {
                    // GAP-08/FR-008/FR-009 (feature 005): anti-replay check. A stale-but-legitimate
                    // resend always carries an `OrigSendingTime` no later than its own `SendingTime`
                    // — one later than the other indicates a replayed/falsified message. Distinct
                    // from, and runs *before*, the dictionary-level `requires_orig_sending_time`
                    // toggle (`ValidationOptions`, TODO-09) — that toggle only fires inside
                    // `validate_app`, which this early-drop branch never reaches. Reuses
                    // `reject_logon` (Logout + disconnect), the same severity as a duplicate Logon
                    // (`on_logon`, below) — resolved via `/speckit-clarify`, not routed through
                    // `Application::from_admin` since this is a purely session-internal protocol
                    // violation.
                    match self.poss_dup_anti_replay_violation(&msg) {
                        Some(reject) => self.reject_logon(&reject),
                        None => Vec::new(),
                    }
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
                error.ref_tag,
                error.reason.code(),
                &error.text,
            )
        } else {
            admin::reject_with_reason(
                &self.config,
                seq,
                ref_seq,
                error.ref_tag,
                error.reason.code(),
                &error.text,
            )
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
        actions.extend(self.maybe_continue_chunked_resend());
        actions
    }

    /// GAP-09/FR-011 (feature 005): once `next_in_seq` has advanced past the currently
    /// outstanding chunk's end (`resend_chunk_end`) but the full known gap (`resend_target`)
    /// isn't closed yet, automatically issue the next chunk's `ResendRequest` — a well-behaved
    /// reference-engine responder never re-requests the remainder unprompted, so the requester
    /// must drive continuation itself (research.md §8). No-op when no chunked resend is
    /// outstanding.
    fn maybe_continue_chunked_resend(&mut self) -> Vec<Action> {
        let Some(target) = self.resend_target else {
            return Vec::new();
        };
        if self.next_in_seq > target {
            self.resend_target = None;
            self.resend_chunk_end = None;
            return Vec::new();
        }
        match self.resend_chunk_end {
            Some(chunk_end) if self.next_in_seq > chunk_end => {
                vec![self.request_resend(self.next_in_seq)]
            }
            _ => Vec::new(),
        }
    }

    fn request_resend(&mut self, begin: u64) -> Action {
        let chunk = u64::from(self.config.resend_request_chunk_size);
        let end = if chunk > 0 {
            let capped = begin + chunk - 1;
            self.resend_chunk_end = Some(self.resend_target.map_or(capped, |t| t.min(capped)));
            self.resend_chunk_end.unwrap_or(capped)
        } else {
            self.resend_chunk_end = None;
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
                    actions.push(self.send_resend(resent, seq));
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
        // GAP-18a/FR-010 (feature 005): reject a duplicate Logon on an already-logged-on session
        // (Logout + disconnect) instead of silently no-op'ing it — the same severity as the
        // PossDup anti-replay violation above (resolved via `/speckit-clarify`). Checked first,
        // before any of this function's reset/sequence-advancement side effects, so a spurious
        // duplicate never partially mutates session state.
        if self.state == SessionState::LoggedOn {
            return self.reject_logon(&truefix_core::Reject {
                reason: 0,
                ref_tag: None,
                text: Some("session is already logged on".to_owned()),
                session_status: None,
            });
        }

        let mut store_reset = false;
        if msg
            .body
            .get(RESET_SEQ_NUM_FLAG)
            .and_then(|f| f.as_str().ok())
            == Some("Y")
        {
            let full = !self.logon_sent;
            self.reset_sequences(full);
            store_reset = full;
        }

        // Account for the Logon's own sequence number *before* building our response, so that
        // NextExpectedMsgSeqNum (789) and LastMsgSeqNumProcessed (369) on the response reflect
        // having consumed this Logon. The resulting actions (queue drain / ResendRequest) are
        // emitted *after* the Logon response below to preserve wire ordering.
        let logon_seq = msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map(|s| s as u64);
        let disposition = logon_seq.map(|s| s.cmp(&self.next_in_seq));
        if disposition == Some(Ordering::Equal) {
            self.next_in_seq = self.next_in_seq.saturating_add(1);
        }

        let mut actions = Vec::new();
        if store_reset {
            actions.push(Action::ResetStore);
        }
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
                if self.config.resend_request_chunk_size > 0 {
                    if let Some(s) = logon_seq {
                        self.resend_target = Some(self.resend_target.map_or(s, |t| t.max(s)));
                    }
                }
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
            let reset = self.enter_disconnected();
            let mut actions: Vec<Action> = reset.into_iter().collect();
            actions.push(Action::Disconnect);
            actions
        } else {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, None);
            let send = self.send_stored(lo);
            let reset = self.enter_disconnected();
            let mut actions = vec![send];
            actions.extend(reset);
            actions.push(Action::Disconnect);
            actions
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

    /// Transition to `Disconnected`, honoring `ResetOnDisconnect`/`ResetOnLogout`. Returns
    /// `Some(Action::ResetStore)` when a full reset happened, so the caller can include it in the
    /// actions it returns (FR-004) — the durable store is otherwise unaware this reset occurred.
    fn enter_disconnected(&mut self) -> Option<Action> {
        self.state = SessionState::Disconnected;
        if self.config.reset_on_disconnect || self.config.reset_on_logout {
            self.reset_sequences(true);
            self.logon_sent = false;
            Some(Action::ResetStore)
        } else {
            None
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
                let reset = self.enter_disconnected();
                let mut actions: Vec<Action> = reset.into_iter().collect();
                actions.push(Action::Disconnect);
                return actions;
            }
            SessionState::LoggedOn if !self.config.disable_heart_beat_check => {
                if self.ticks_since_recv >= hb * self.config.heartbeat_timeout_multiplier.max(1) + 2
                {
                    let reset = self.enter_disconnected();
                    let mut actions: Vec<Action> = reset.into_iter().collect();
                    actions.push(Action::Disconnect);
                    return actions;
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
            // DisableHeartBeatCheck: still send heartbeats on our own cadence, but never probe or
            // time out a silent peer.
            SessionState::LoggedOn => {
                if self.ticks_since_send >= hb {
                    let seq = self.next_seq();
                    let h = admin::heartbeat(&self.config, seq, None);
                    actions.push(self.send_stored(h));
                }
            }
            SessionState::AwaitingLogout
                if self.ticks_awaiting >= self.config.logout_timeout.max(1) =>
            {
                let reset = self.enter_disconnected();
                let mut actions: Vec<Action> = reset.into_iter().collect();
                actions.push(Action::Disconnect);
                return actions;
            }
            _ => {}
        }
        actions
    }
}

fn is_admin_type(mt: Option<&str>) -> bool {
    matches!(mt, Some("0" | "1" | "2" | "3" | "4" | "5" | "A"))
}
