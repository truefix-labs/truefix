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
use truefix_dict::{DataDictionary, FixtDictionaries, ValidationOptions};

use crate::admin;
use crate::config::{Role, SessionConfig};
use crate::session_id::SessionId;
use crate::tags::{
    APPL_VER_ID, DEFAULT_APPL_VER_ID, GAP_FILL_FLAG, HEART_BT_INT, MSG_SEQ_NUM, NEW_SEQ_NO,
    NEXT_EXPECTED_MSG_SEQ_NUM, ORIG_SENDING_TIME, POSS_DUP_FLAG, RESET_SEQ_NUM_FLAG, SENDING_TIME,
    SESSION_STATUS,
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
    /// The transport connection was lost (TCP drop, not a graceful Logout) (BUG-33/FR-009). Honors
    /// `ResetOnDisconnect`/`ResetOnLogout` exactly as any other path into `enter_disconnected()`
    /// does, so an unexpected drop resets sequence numbers when configured to, the same as a
    /// Logout-driven disconnect would.
    ///
    /// NEW-56 (feature 009): carries the reason so `enter_disconnected` can consult only the
    /// applicable flag (`reset_on_logout` for a completed graceful Logout exchange,
    /// `reset_on_disconnect` for everything else), rather than `||`-combining both regardless of
    /// cause. The transport dispatches this event exactly once per connection teardown
    /// (`truefix-transport::run_connection`'s unconditional final dispatch) using whichever
    /// reason [`Session::last_disconnect_reason`] already recorded (if an internal handler like
    /// `on_logout_msg`/a schedule-or-heartbeat-timeout in `on_tick` already ran), or
    /// [`DisconnectReason::Other`] otherwise (a raw TCP drop, or an error-driven path that sets
    /// `state` directly and uses the separate `reset_on_error` mechanism instead).
    Disconnected(DisconnectReason),
}

/// Why a session is tearing down (NEW-56, feature 009) — determines which of
/// `reset_on_logout`/`reset_on_disconnect` `enter_disconnected` consults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    /// A graceful Logout exchange completed (this session sent or received a Logout while
    /// `LoggedOn`/`AwaitingLogout`).
    GracefulLogout,
    /// Anything else: a raw TCP drop, a timeout (logon/logout/heartbeat), or a schedule-window
    /// exit.
    Other,
}

/// The kind of BeginString/CheckCompID mismatch `identity_problem` detected. Distinguished (rather
/// than a single `&'static str` reason) because NEW-87 (feature 009) gives the two cases different
/// wire behavior: a CompID mismatch gets a session `Reject` before the `Logout` (matching QFJ's
/// `doBadCompID`); an incorrect BeginString stays Logout-only (QFJ has no equivalent
/// `doBadBeginString` two-message path).
enum IdentityProblem {
    /// The inbound message's BeginString(8) doesn't match this session's configured version.
    BeginString,
    /// `CheckCompID` is enabled and the inbound SenderCompID(49)/TargetCompID(56) doesn't match.
    CompId,
}

impl IdentityProblem {
    fn text(&self) -> &'static str {
        match self {
            IdentityProblem::BeginString => "Incorrect BeginString",
            IdentityProblem::CompId => "CompID problem",
        }
    }
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
    /// BUG-93/FR-029 (feature 007): set as soon as an inbound Logon is received (regardless of
    /// whether it's later accepted or rejected), reset on each new connection — lets the transport
    /// layer know a handshake was at least attempted even if it never reached full `LoggedOn`,
    /// matching QFJ's `logonReceived`/QFGo's equivalent tracking.
    logon_received: bool,
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
    /// A FIXT 1.1 transport/application dictionary split (T079/GAP-18c part 2), taking precedence
    /// over `validator` for application-message validation when set: the application dictionary
    /// is selected per-message via [`Self::negotiated_appl_ver_id`]/tag 1128 rather than being a
    /// single fixed dictionary for the whole session.
    fixt_validator: Option<(FixtDictionaries, ValidationOptions)>,
    /// The `DefaultApplVerID` (tag 1137) negotiated from the most recent inbound Logon under FIXT
    /// 1.1 (T078/GAP-18c part 1); `None` before any Logon carrying tag 1137 has been received.
    negotiated_appl_ver_id: Option<String>,
    /// BUG-28/FR-006 (feature 007): set when this (initiator) session sends its own Logon with
    /// `ResetSeqNumFlag=Y`; cleared once the acceptor's response has been checked (echoed the flag,
    /// or its own `MsgSeqNum == 1`) or the connection ends. While set, a response Logon that does
    /// neither is a handshake failure.
    reset_on_logon_pending: bool,
    /// NEW-56 (feature 009): the reason `enter_disconnected` was last called with, if any, since
    /// the current connection began (`on_connected` clears it). Lets the transport layer's
    /// unconditional final `Event::Disconnected` dispatch (which runs for *every* teardown,
    /// including ones an internal handler like `on_logout_msg`/`on_tick` already fully processed)
    /// re-pass the same reason rather than guessing — making that redundant second call
    /// idempotent instead of potentially consulting the wrong flag.
    last_disconnect_reason: Option<DisconnectReason>,
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
            logon_received: false,
            resend_requested: false,
            resend_target: None,
            resend_chunk_end: None,
            store: BTreeMap::new(),
            queue: BTreeMap::new(),
            validator: None,
            fixt_validator: None,
            negotiated_appl_ver_id: None,
            reset_on_logon_pending: false,
            last_disconnect_reason: None,
        }
    }

    /// Enable inbound message validation against `dict` using `opts` — admin messages included
    /// (BUG-89/FR-011, feature 007). Invalid messages produce a Reject (session-level) or
    /// BusinessMessageReject (business-level) instead of being delivered.
    pub fn set_dictionary(&mut self, dict: DataDictionary, opts: ValidationOptions) {
        self.validator = Some((dict, opts));
    }

    /// Enable inbound application-message validation against a real FIXT 1.1 transport/
    /// application dictionary split (T079/GAP-18c part 2), taking precedence over
    /// [`Self::set_dictionary`]: the application dictionary is selected per-message (tag 1128, or
    /// the negotiated tag 1137 from Logon, or `dicts`' own baked-in `DefaultApplVerID`) rather
    /// than being one fixed dictionary for the whole session.
    pub fn set_fixt_dictionaries(&mut self, dicts: FixtDictionaries, opts: ValidationOptions) {
        self.fixt_validator = Some((dicts, opts));
    }

    /// The `DefaultApplVerID` (tag 1137) negotiated from the most recent inbound Logon under FIXT
    /// 1.1 (T078/GAP-18c part 1); `None` if no Logon carrying tag 1137 has been received yet.
    pub fn negotiated_appl_ver_id(&self) -> Option<&str> {
        self.negotiated_appl_ver_id.as_deref()
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

    /// Whether this session has sent its own Logon on the current connection (BUG-93/FR-029,
    /// feature 007) — true for an initiator immediately upon connecting, or for an acceptor once
    /// it sends its Logon response, regardless of whether the handshake ever fully completed.
    pub fn logon_sent(&self) -> bool {
        self.logon_sent
    }

    /// Whether this session has received an inbound Logon on the current connection (BUG-93/
    /// FR-029, feature 007), regardless of whether it was later accepted or rejected.
    pub fn logon_received(&self) -> bool {
        self.logon_received
    }

    /// The reason `enter_disconnected` was last called with on the current connection, if any
    /// (NEW-56, feature 009). `None` until some internal path (a completed graceful Logout
    /// exchange, or a logon/logout/heartbeat timeout) has torn the session down; the transport
    /// layer's unconditional final `Event::Disconnected` dispatch uses this to re-pass the same
    /// reason (making that dispatch idempotent) rather than guessing when it runs after an
    /// internal handler already processed the teardown.
    pub fn last_disconnect_reason(&self) -> Option<DisconnectReason> {
        self.last_disconnect_reason
    }

    /// The next outbound sequence number that will be used.
    pub fn next_out_seq(&self) -> u64 {
        self.next_out_seq
    }

    /// The next inbound sequence number expected.
    pub fn next_in_seq(&self) -> u64 {
        self.next_in_seq
    }

    /// The number of out-of-order messages currently held in the inbound gap-fill queue, awaiting
    /// either drain (once the gap closes) or discard (BUG-96/FR-027, feature 007: a gap-fill jump
    /// past them). Mainly useful for tests confirming the queue doesn't leak superseded entries.
    pub fn queued_len(&self) -> usize {
        self.queue.len()
    }

    /// Read-only pre-check, safe to call *before* the mutating `Event::Received(msg)` dispatch:
    /// would processing `msg` right now hit one of the session layer's immediate,
    /// unconditional-teardown paths — stale `SendingTime` (`CheckLatency`), a mismatched
    /// `BeginString`/CompID, or (for a non-Logon/non-SequenceReset message) a too-low
    /// `MsgSeqNum` with no `PossDupFlag` (BUG-34/FR-010)?
    ///
    /// Used by the transport layer to decide whether `Application::from_admin`/`from_app` should
    /// even be invoked for `msg` at all: the answer is "no" whenever this returns `true`, since
    /// the session is about to reject/tear down regardless of what the callback does. Not
    /// exhaustive — dictionary (`validate_app`) and in-order gap-fill checks are tightly coupled
    /// to advancing `next_in_seq` and can only run inside `on_received` itself — but covers the
    /// state-mutation-free checks that would otherwise let the callback observe a message the
    /// session has already doomed.
    pub fn would_reject_before_processing(&self, msg: &Message) -> bool {
        if !self.latency_ok(msg) {
            return true;
        }
        if self.identity_problem(msg).is_some() {
            return true;
        }
        let mt = msg.msg_type();
        if mt != Some("A")
            && mt != Some("4")
            && let Some(seq) = msg
                .header
                .get(MSG_SEQ_NUM)
                .and_then(|f| f.as_int().ok())
                .filter(|&s| s > 0)
        {
            let poss_dup = msg.header.get(POSS_DUP_FLAG).and_then(|f| f.as_str().ok()) == Some("Y");
            if !poss_dup && (seq as u64) < self.next_in_seq {
                return true;
            }
        }
        false
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

    /// PossDup anti-replay falsification check (BUG-57/FR-024, feature 007): applies to *any*
    /// `PossDupFlag=Y` message, not only a too-low (`Ordering::Less`) one — a replayed/falsified
    /// message (`OrigSendingTime` later than `SendingTime`) is exactly as real a threat at the
    /// expected sequence or higher, matching QFJ's `validatePossDup`, which runs unconditionally.
    /// Deliberately narrower than [`Self::poss_dup_anti_replay_violation`] below: it omits the
    /// missing-`OrigSendingTime`-on-a-too-low-message case (gated by
    /// `requires_orig_sending_time_on_low_seq`, whose name and existing single call site already
    /// scope it to the too-low case specifically) — only the unconditional falsification signal
    /// applies universally.
    fn poss_dup_falsification_check(&self, msg: &Message) -> Option<truefix_core::Reject> {
        let orig = msg
            .header
            .get(ORIG_SENDING_TIME)
            .and_then(|f| f.as_utc_timestamp().ok())?;
        let sending_time = msg
            .header
            .get(SENDING_TIME)
            .and_then(|f| f.as_utc_timestamp().ok())?;
        if orig > sending_time {
            Some(truefix_core::Reject {
                reason: 5, // SessionRejectReason: Value is incorrect (out of range) for this tag
                ref_tag: Some(ORIG_SENDING_TIME),
                text: Some(
                    "OrigSendingTime is later than SendingTime on a PossDup message".to_owned(),
                ),
                session_status: None,
            })
        } else {
            None
        }
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
        vec![self.send_logout(lo), Action::Disconnect]
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
            Event::Disconnected(reason) => self.enter_disconnected(reason).into_iter().collect(),
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
        let rej = admin::reject_with_reason(&self.config, seq, 0, None, None, 0, "garbled message");
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
        self.stamp_last_msg_seq_num_processed(&mut msg);
        if self.config.persist_messages
            && let Some(seq) = msg
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
    fn send_raw(&mut self, mut msg: Message) -> Action {
        self.ticks_since_send = 0;
        self.stamp_last_msg_seq_num_processed(&mut msg);
        Action::Send(msg)
    }

    fn send_logout(&mut self, msg: Message) -> Action {
        self.send_raw(msg)
    }

    fn stamp_last_msg_seq_num_processed(&self, msg: &mut Message) {
        if self.config.enable_last_msg_seq_num_processed && msg.header.get(369).is_none() {
            // LastMsgSeqNumProcessed (369): the last inbound sequence number we have processed.
            let last = self.next_in_seq.saturating_sub(1);
            msg.header.set(Field::int(369, last as i64));
        }
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
        // NEW-32 (feature 009): an operational reset mid-session must also clear any
        // in-progress resend/test-request bookkeeping tied to the sequence state being reset --
        // previously left stale, so a resend chunk or outstanding TestRequest from before the
        // reset could be misapplied to the freshly-reset sequence space afterward.
        self.resend_target = None;
        self.resend_chunk_end = None;
        self.test_request_outstanding = false;
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

    /// NEW-87 (feature 009): builds and sends a session-level `Reject` (373=`reject.reason`)
    /// referencing `msg`'s own `MsgSeqNum` — matching QFJ's `doBadTime`/`doBadCompID`, which each
    /// send this Reject *before* the Logout for a non-Logon message, rather than a Logout alone.
    fn build_session_reject(&mut self, msg: &Message, reject: &truefix_core::Reject) -> Action {
        let ref_seq = msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map_or(0, |s| s as u64);
        let ref_mt = msg.msg_type().map(str::to_owned);
        let seq = self.next_seq();
        let rej = admin::reject_with_reason(
            &self.config,
            seq,
            ref_seq,
            ref_mt.as_deref(),
            reject.ref_tag,
            reject.reason,
            reject.text.as_deref().unwrap_or(""),
        );
        self.send_stored(rej)
    }

    /// `build_session_reject` followed by `reject_logon` (Logout + disconnect) — for the
    /// PossDup-falsification/anti-replay paths, which (unlike the latency/CompID paths just above)
    /// don't call `reset_on_error()` even after this fix, matching their pre-existing behavior.
    /// Deliberately doesn't touch `reject_logon` itself, which stays Logout-only for its other
    /// callers (Logon-rejection paths keep QFJ's `logoutWithErrorMessage`-equivalent behavior, per
    /// the source audit's own note distinguishing that case from this one).
    fn session_reject_before_logout(
        &mut self,
        msg: &Message,
        reject: &truefix_core::Reject,
    ) -> Vec<Action> {
        let mut actions = vec![self.build_session_reject(msg, reject)];
        actions.extend(self.reject_logon(reject));
        actions
    }

    /// Detect a BeginString or CompID mismatch (CheckCompID). Returns a Logout reason on failure.
    fn identity_problem(&self, msg: &Message) -> Option<IdentityProblem> {
        if let Some(bs) = msg.header.get(8).and_then(|f| f.as_str().ok())
            && bs != self.config.begin_string
        {
            return Some(IdentityProblem::BeginString);
        }
        if self.config.check_comp_id {
            // Inbound SenderCompID(49) is the peer's sender = our target; TargetCompID(56) = ours.
            // Only a present-but-mismatched value is a CompID problem (absence is a separate concern).
            if let Some(sender) = msg.header.get(49).and_then(|f| f.as_str().ok())
                && sender != self.config.target_comp_id
            {
                return Some(IdentityProblem::CompId);
            }
            if let Some(target) = msg.header.get(56).and_then(|f| f.as_str().ok())
                && target != self.config.sender_comp_id
            {
                return Some(IdentityProblem::CompId);
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
            // B7/FR-009 (feature 006): an unparseable SendingTime fails the latency check rather
            // than passing it — a garbled value is not evidence of an acceptable send time.
            return false;
        };
        let now = time::OffsetDateTime::now_utc();
        (now - ts).whole_seconds().abs() <= i64::from(self.config.max_latency)
    }

    fn on_connected(&mut self) -> Vec<Action> {
        // B4/FR-007 (feature 006): reset per-connection timer/resend-tracking state so a reused
        // `Session` object doesn't carry stale values across a reconnect (a fresh connection has
        // received nothing yet and has no chunked resend outstanding).
        self.ticks_awaiting = 0;
        self.ticks_since_recv = 0;
        self.test_request_outstanding = false;
        self.resend_target = None;
        self.resend_chunk_end = None;
        self.reset_on_logon_pending = false;
        // NEW-56 (feature 009): a fresh connection has no teardown reason of its own yet -- avoid
        // the transport layer's final-dispatch fallback re-using a *prior* connection's reason.
        self.last_disconnect_reason = None;
        // BUG-93/FR-029 (feature 007): a fresh connection has neither sent nor received a Logon
        // yet (`logon_sent` gets set again below/in `on_logon` as this connection actually
        // progresses).
        self.logon_sent = false;
        self.logon_received = false;
        // BUG-35/FR-017 (feature 007): a fresh connection has no outstanding ResendRequest and no
        // out-of-order backlog of its own -- carrying `resend_requested=true` or a stale `queue`
        // over from a prior connection would silently suppress a ResendRequest for a genuinely new
        // gap on this new connection (since `send_redundant_resend_requests` defaults to `false`).
        self.resend_requested = false;
        self.queue.clear();
        self.state = SessionState::AwaitingLogon;
        match self.config.role {
            Role::Initiator => {
                let mut actions = Vec::new();
                // B1/FR-005 (feature 006): perform the ResetOnLogon full reset *before* composing
                // our own outbound Logon, not reactively when the counterparty's reset-flagged
                // reply arrives (by then `logon_sent` is already true and our Logon has already
                // been sent using whatever stale sequence number preceded the reset). Resetting
                // here guarantees our own Logon always carries a fresh seq 1, matching the
                // counterparty's own freshly-reset expectation, regardless of send/receive order.
                if self.config.reset_on_logon {
                    self.reset_sequences(true);
                    actions.push(Action::ResetStore);
                }
                // BUG-92/BUG-109/FR-025 (feature 007): send ResetSeqNumFlag=Y when ANY of
                // ResetOnLogon/ResetOnLogout/ResetOnDisconnect is configured AND both sequence
                // numbers are already 1 -- matching QFJ's `isResetNeeded()`/QFGo's
                // `shouldSendReset()`, rather than only ever consulting `reset_on_logon`. The
                // "both seqs == 1" gate matters for ResetOnLogout/ResetOnDisconnect specifically:
                // unlike ResetOnLogon (which just forced an actual reset immediately above, so
                // seqs are 1/1 by construction whenever it's set), those two don't reset anything
                // *here* -- they reset at a prior disconnect/logout, so this only claims
                // ResetSeqNumFlag=Y when that reset genuinely already happened (seqs still at 1),
                // not merely because the config is set on an otherwise-unrelated non-1 sequence.
                let should_send_reset = (self.config.reset_on_logon
                    || self.config.reset_on_logout
                    || self.config.reset_on_disconnect)
                    && self.next_out_seq == 1
                    && self.next_in_seq == 1;
                // BUG-28/FR-006 (feature 007): remember that our own Logon carries
                // ResetSeqNumFlag=Y so the response can be verified once it arrives (`on_logon`'s
                // `Role::Initiator` arm).
                self.reset_on_logon_pending = should_send_reset;
                let next_exp = self.maybe_next_expected();
                let seq = self.next_seq();
                let msg =
                    admin::logon_with_reset_flag(&self.config, seq, next_exp, should_send_reset);
                self.logon_sent = true;
                actions.push(self.send_stored(msg));
                actions
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

        // NEW-87 (feature 009): computed up-front (previously computed further down, after these
        // two checks) so both can tell a Logon apart from every other message type -- QFJ's
        // `doBadTime`/`doBadCompID` two-message shape (Reject, then Logout) only applies to a
        // non-Logon message; a Logon-specific bad-time/bad-CompID uses QFJ's
        // `logoutWithErrorMessage` instead, matching TrueFix's pre-existing Logout-only behavior
        // for that one case, which this fix must not change.
        let is_logon = msg.msg_type() == Some("A");

        // CheckLatency / MaxLatency: a stale SendingTime aborts the session.
        if !self.latency_ok(&msg) {
            let mut actions = Vec::new();
            if !is_logon {
                let reject = truefix_core::Reject {
                    reason: 10, // SessionRejectReason: SendingTime accuracy problem
                    ref_tag: Some(SENDING_TIME),
                    text: Some("SendingTime accuracy problem".to_owned()),
                    session_status: None,
                };
                actions.push(self.build_session_reject(&msg, &reject));
            }
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, Some("SendingTime accuracy problem"));
            self.state = SessionState::Disconnected;
            actions.push(self.send_logout(lo));
            actions.extend(self.reset_on_error());
            actions.push(Action::Disconnect);
            return actions;
        }

        // BeginString / CheckCompID: a mismatched identity aborts the session (FR-007).
        if let Some(problem) = self.identity_problem(&msg) {
            let mut actions = Vec::new();
            // NEW-87 (feature 009): only a non-Logon CompID mismatch gets the
            // Reject-before-Logout treatment, matching QFJ's `doBadCompID`
            // (SessionRejectReason=9, RefTagID=49) -- an incorrect BeginString stays Logout-only
            // (QFJ has no equivalent two-message path for it), and so does a Logon-specific CompID
            // mismatch (see `is_logon` above).
            if !is_logon && matches!(problem, IdentityProblem::CompId) {
                let reject = truefix_core::Reject {
                    reason: 9, // SessionRejectReason: CompID problem
                    ref_tag: Some(49),
                    text: Some(problem.text().to_owned()),
                    session_status: None,
                };
                actions.push(self.build_session_reject(&msg, &reject));
            }
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, Some(problem.text()));
            self.state = SessionState::Disconnected;
            actions.push(self.send_logout(lo));
            actions.extend(self.reset_on_error());
            actions.push(Action::Disconnect);
            return actions;
        }

        let mt = msg.msg_type().map(str::to_owned);

        // BUG-42/FR-018 (feature 007): `validLogonState` -- while not yet logged on, only
        // Logon/Logout/SequenceReset/Reject are legitimate (a counterparty may need to Logout or
        // gap-fill/Reject before completing its own Logon); anything else arriving this early is a
        // protocol violation and disconnects the session, matching QFJ's `validLogonState()`
        // rather than processing it normally (which, for a too-high sequence, would otherwise
        // queue it and send a ResendRequest -- itself a protocol violation, since no
        // ResendRequests should be sent before logon).
        if self.state != SessionState::LoggedOn
            && !matches!(mt.as_deref(), Some("A" | "5" | "4" | "3"))
        {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, Some("Logon state is not valid"));
            self.state = SessionState::Disconnected;
            let mut actions = vec![self.send_logout(lo)];
            actions.extend(self.reset_on_error());
            actions.push(Action::Disconnect);
            return actions;
        }

        if mt.as_deref() == Some("A") {
            return self.on_logon(msg);
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
                let rej = admin::reject(
                    &self.config,
                    s,
                    0,
                    mt.as_deref(),
                    "missing or invalid MsgSeqNum",
                );
                return vec![self.send_stored(rej)];
            }
        };
        let poss_dup = msg.header.get(POSS_DUP_FLAG).and_then(|f| f.as_str().ok()) == Some("Y");

        // BUG-57/FR-024 (feature 007): the anti-replay falsification check applies to any
        // PossDupFlag=Y message, not only a too-low one -- checked here, ahead of the
        // Equal/Greater/Less dispatch below, so it fires uniformly regardless of which of those
        // three this message's sequence happens to fall into. The `Ordering::Less` arm further
        // down still separately calls the fuller `poss_dup_anti_replay_violation` (this check plus
        // the too-low-specific missing-tag case) — by the time it does, this check has already
        // handled the falsification signal, so that call only meaningfully covers the missing-tag
        // case from here on.
        if poss_dup && let Some(reject) = self.poss_dup_falsification_check(&msg) {
            // NEW-87 (feature 009): a session Reject before the Logout, not a Logout alone.
            return self.session_reject_before_logout(&msg, &reject);
        }

        if mt.as_deref() == Some("4") {
            let gap_fill = msg.body.get(GAP_FILL_FLAG).and_then(|f| f.as_str().ok()) == Some("Y");
            if !gap_fill {
                // A plain SequenceReset-Reset's own MsgSeqNum is intentionally not checked
                // against `next_in_seq` (QFJ ignores it for Reset mode) -- unchanged from before
                // this fix.
                return self.on_sequence_reset(&msg);
            }
            // NEW-84 (feature 009): a gap-fill SequenceReset must go through the same too-high/
            // too-low dispatch every other message type gets (QFJ's `verify(sequenceReset,
            // isGapFill, isGapFill)` conditions both checks on `isGapFill`) -- previously it was
            // routed here unconditionally, applying `NewSeqNo` immediately regardless of its own
            // position in the stream, which could silently and permanently discard queued
            // messages a too-high/out-of-order gap-fill jumped past.
            return match seq.cmp(&self.next_in_seq) {
                Ordering::Equal => self.on_sequence_reset(&msg),
                Ordering::Greater => {
                    // Mirror the generic `Ordering::Greater` handling below: queue it and request
                    // a resend, rather than applying its `NewSeqNo` unconditionally.
                    let mut actions = Vec::new();
                    self.queue.insert(seq, msg);
                    if self.config.resend_request_chunk_size > 0 {
                        self.resend_target = Some(self.resend_target.map_or(seq, |t| t.max(seq)));
                    }
                    // NEW-18 (feature 009): no ResendRequests before logon -- a too-high pre-logon
                    // gap-fill SequenceReset must not trigger one either, matching the generic
                    // Ordering::Greater arm's identical guard below.
                    let pre_logon = self.state != SessionState::LoggedOn;
                    if !pre_logon
                        && (!self.resend_requested || self.config.send_redundant_resend_requests)
                    {
                        self.resend_requested = true;
                        let begin = self.next_in_seq;
                        actions.push(self.request_resend(begin));
                    }
                    actions
                }
                Ordering::Less => {
                    // Mirror the generic `Ordering::Less` handling below.
                    if poss_dup {
                        match self.poss_dup_anti_replay_violation(&msg) {
                            // NEW-87 (feature 009): a session Reject before the Logout.
                            Some(reject) => self.session_reject_before_logout(&msg, &reject),
                            None => Vec::new(),
                        }
                    } else {
                        let s = self.next_seq();
                        let lo = admin::logout(&self.config, s, Some("MsgSeqNum too low"));
                        self.state = SessionState::Disconnected;
                        let mut actions = vec![self.send_logout(lo)];
                        actions.extend(self.reset_on_error());
                        actions.push(Action::Disconnect);
                        actions
                    }
                }
            };
        }

        match seq.cmp(&self.next_in_seq) {
            Ordering::Equal => {
                if let Some(reject) = self.validate_app(&msg) {
                    let mut actions = vec![reject];
                    self.next_in_seq = self.next_in_seq.saturating_add(1);
                    actions.extend(self.reset_on_error());
                    if self.config.disconnect_on_error {
                        // B5/FR-008 (feature 006): send a Logout before disconnecting on a
                        // dictionary-validation failure, matching the identity/latency disconnect
                        // paths above instead of dropping the connection with only a Reject sent.
                        let seq = self.next_seq();
                        let lo = admin::logout(
                            &self.config,
                            seq,
                            Some("application message failed validation"),
                        );
                        actions.push(self.send_logout(lo));
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
                // NEW-85/FR-049 (feature 009): Logout is a terminal control message. Processing
                // it immediately avoids deadlocking teardown behind a sequence gap that may never
                // be filled; unlike ordinary too-high traffic it is neither queued nor answered
                // with a ResendRequest.
                if mt.as_deref() == Some("5") {
                    return self.on_logout_msg();
                }
                let mut actions = Vec::new();
                // BUG-29/FR-007 (feature 007): a too-high `ResendRequest` is answered immediately
                // rather than deferred to `drain_queue` once our own gap toward the counterparty
                // is filled -- otherwise two sessions each holding an outstanding `ResendRequest`
                // toward the other deadlock, each waiting on the other to resolve its own gap
                // first (QuickFIX/J's `verify` deliberately skips the too-high check for this one
                // message type, QFJ-673). It still participates in the normal queue/gap-fill
                // bookkeeping below, so it is also correctly reprocessed once our own gap
                // resolves (matching every other too-high message).
                if mt.as_deref() == Some("2") {
                    actions.extend(self.on_resend_request(&msg));
                }
                self.queue.insert(seq, msg);
                // GAP-09/FR-011 (feature 005): track this arrival as (at least) the full known
                // extent of the gap, so a chunked resend can auto-continue past its first chunk.
                if self.config.resend_request_chunk_size > 0 {
                    self.resend_target = Some(self.resend_target.map_or(seq, |t| t.max(seq)));
                }
                // NEW-18 (feature 009): residual issue from BUG-42's own fix -- `validLogonState`
                // lets pre-logon Logout(5)/Reject(3) admin messages through (alongside Logon and
                // gap-fill SequenceReset), but a too-high one of these still reached this arm and
                // emitted a ResendRequest, contradicting "no ResendRequests before logon".
                let pre_logon = self.state != SessionState::LoggedOn;
                if pre_logon {
                    // nothing further to add -- queued above, no resend request before logon
                } else if self.resend_requested && !self.config.send_redundant_resend_requests {
                    // nothing further to add
                } else {
                    self.resend_requested = true;
                    let begin = self.next_in_seq;
                    actions.push(self.request_resend(begin));
                }
                actions
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
                    // violation. NEW-87 (feature 009): now via `session_reject_before_logout`,
                    // sending a session Reject before the Logout instead of a Logout alone.
                    match self.poss_dup_anti_replay_violation(&msg) {
                        Some(reject) => self.session_reject_before_logout(&msg, &reject),
                        None => Vec::new(),
                    }
                } else {
                    let s = self.next_seq();
                    let lo = admin::logout(&self.config, s, Some("MsgSeqNum too low"));
                    self.state = SessionState::Disconnected;
                    let mut actions = vec![self.send_logout(lo)];
                    // BUG-68/FR-039 (feature 007): `reset_on_error()`, matching the
                    // latency-failure/identity-failure/validLogonState/dictionary-validation
                    // disconnect paths above and below -- previously this was the one hard
                    // sequence-violation disconnect path that skipped it, inconsistent when
                    // `ResetOnError` is configured.
                    actions.extend(self.reset_on_error());
                    actions.push(Action::Disconnect);
                    actions
                }
            }
        }
    }

    /// Feature 011 (FR-008/FR-009/FR-010): under FIXT 1.1, re-group `msg`'s body against its
    /// resolved application dictionary, in place, before delivery/validation.
    ///
    /// **Call this before `msg` reaches `Application::from_app`/`from_admin` — not after.**
    /// `truefix-transport`'s `handle_inbound` delivers a decoded message to the application
    /// *directly*, ahead of ever calling [`Session::handle`]/`on_received` (there is no
    /// `Action::Deliver` — delivery is not one of the `Action`s `handle` returns at all), so a
    /// caller that restructured only from inside the state machine would always be too late: by
    /// the time `on_received`/`validate_app` ran, the application had already seen the
    /// transport-dictionary-scoped body. This method exists to be called from `handle_inbound`
    /// itself, right after its pre-delivery reject precheck and before `from_app`/`from_admin`.
    ///
    /// The reader task (`classify_buffered`, `truefix-transport`) only has the transport
    /// dictionary available when it decodes a message — application-dictionary selection depends
    /// on `ApplVerID`, which for most messages is never carried on the wire at all and instead
    /// falls back to `negotiated_appl_ver_id`, session state the reader task (a separate,
    /// independently-scheduled tokio task) has no access to. So an application-layer message's
    /// body arrives already decoded, but only transport-dictionary-scoped (effectively flat for
    /// any body-level group the transport dictionary doesn't itself declare) — this method
    /// resolves the *application* dictionary (mirroring `validate_app`'s own `appl_ver_id`
    /// resolution precedence exactly, so both agree) and applies it to the message's actual
    /// structure via [`truefix_core::restructure_groups`], not just its later validation.
    ///
    /// No-ops for: a non-FIXT session (`fixt_validator: None` — a single `DataDictionary` session
    /// already structured the whole message, body included, at decode time, per FR-003); an admin
    /// (session-layer) message (already correctly structured against the transport dictionary,
    /// same as `validate_app`'s BUG-89 special case — resolving it against a per-message
    /// application dict would be wrong, not just redundant); and a message whose `ApplVerID`
    /// resolves to no registered application dictionary (FR-010 — left transport-scoped rather
    /// than rejected or misattributed). A restructuring failure (e.g. pathological nesting depth)
    /// is treated the same conservative way: `msg` keeps whatever structure it already had.
    pub fn restructure_fixt_application_body(&self, msg: &mut Message) {
        let Some((dicts, _)) = self.fixt_validator.as_ref() else {
            return;
        };
        if is_admin_type(msg.msg_type()) {
            return;
        }
        let msg_appl_ver_id = msg.header.get(APPL_VER_ID).and_then(|f| f.as_str().ok());
        let appl_ver_id = msg_appl_ver_id.or(self.negotiated_appl_ver_id.as_deref());
        let Some(dict) = dicts.application_for(appl_ver_id) else {
            return;
        };
        let _ = truefix_core::restructure_groups(&mut msg.body, dict);
    }

    /// Validate an inbound message; on failure, return a Reject/BusinessMessageReject action.
    /// The no-dictionary case returns `None`.
    ///
    /// BUG-89/FR-011 (feature 007): admin (session-level) messages are validated too, not
    /// skipped — a dictionary-invalid Logon/Heartbeat/etc. used to sail through unchecked
    /// regardless of whether a dictionary was configured. The resulting Reject/
    /// BusinessMessageReject split is still driven by the dictionary layer's own `error.business`
    /// flag (unchanged): in practice an admin-typed message's `MsgType` is always defined in the
    /// dictionary (it's how [`is_admin_type`] itself recognizes it), so the failures actually
    /// reachable here are field-membership/format/required ones, which `DataDictionary::validate`
    /// already classifies as session-level (`error.business == false`) — routing through the
    /// plain `Reject` path, matching QFJ/QFGo and this task's R1.13 decision, without needing a
    /// forced override.
    fn validate_app(&mut self, msg: &Message) -> Option<Action> {
        let error = {
            // T079/GAP-18c part 2: a real FIXT transport/application split, when configured,
            // takes precedence for *application* messages — the application dictionary is chosen
            // per-message by tag 1128 (ApplVerID), falling back to the tag-1137 value negotiated
            // from Logon (T078), falling back to whatever `FixtDictionaries` itself was built with
            // as `DefaultApplVerID`. A message whose resolved ApplVerID has no registered
            // application dictionary is treated like the no-dictionary case (skipped, not
            // rejected) — the same conservative default this session already applies when no
            // dictionary is configured at all. Admin messages always validate against the FIXT
            // *transport* dictionary instead (BUG-89) — they don't carry application semantics, so
            // resolving them against a per-message application dict would spuriously reject every
            // admin message as an undefined MsgType.
            if let Some((dicts, opts)) = self.fixt_validator.as_ref() {
                if is_admin_type(msg.msg_type()) {
                    dicts.transport().validate(msg, opts).err()
                } else {
                    let msg_appl_ver_id = msg.header.get(APPL_VER_ID).and_then(|f| f.as_str().ok());
                    let appl_ver_id = msg_appl_ver_id.or(self.negotiated_appl_ver_id.as_deref());
                    let dict = dicts.application_for(appl_ver_id)?;
                    dict.validate(msg, opts).err()
                }
            } else {
                let (dict, opts) = self.validator.as_ref()?;
                dict.validate(msg, opts).err()
            }
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
                ref_mt.as_deref(),
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
            // NEW-84 (feature 009): a previously-queued too-high gap-fill `SequenceReset` (see
            // `on_received`) applies its own `NewSeqNo` jump once it's reached in order, rather
            // than the uniform "+1" advance every other queued message type gets below --
            // mirrors the fresh in-order case (`on_sequence_reset`), including skipping
            // `validate_app` (SequenceReset has never gone through dictionary validation here,
            // consistent with the pre-existing direct-dispatch behavior for a fresh gap-fill).
            if msg.msg_type() == Some("4") {
                let new_seq = msg
                    .body
                    .get(NEW_SEQ_NO)
                    .and_then(|f| f.as_int().ok())
                    .filter(|&n| n > 0)
                    .map(|n| n as u64);
                self.apply_gap_fill_new_seq(new_seq);
                continue;
            }
            // B3/FR-006 (feature 006): a message drained from the out-of-order queue must be
            // validated exactly like an in-order message (mirroring `on_received`'s `Equal` branch)
            // — bypassing `validate_app` here would let an invalid message enqueued behind a
            // legitimate gap-filler slip through unchecked.
            if let Some(reject) = self.validate_app(&msg) {
                actions.push(reject);
                self.next_in_seq = self.next_in_seq.saturating_add(1);
                actions.extend(self.reset_on_error());
                if self.config.disconnect_on_error {
                    let seq = self.next_seq();
                    let lo = admin::logout(
                        &self.config,
                        seq,
                        Some("application message failed validation"),
                    );
                    actions.push(self.send_logout(lo));
                    self.state = SessionState::Disconnected;
                    actions.push(Action::Disconnect);
                    return actions;
                }
                continue;
            }
            let mt = msg.msg_type().map(str::to_owned);
            // NEW-86/FR-050 (feature 009): a too-high ResendRequest is answered immediately when
            // first received. It stays queued only so sequence accounting advances when the gap
            // closes; invoking its handler again here would send the same resend burst twice.
            if mt.as_deref() != Some("2") {
                actions.extend(self.process_in_order(&msg, mt.as_deref()));
            }
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
        // BUG-22/FR-004 (feature 006): a ResendRequest structurally missing BeginSeqNo(7) or
        // EndSeqNo(16) entirely is a required-field violation and must be rejected, distinct from
        // (and checked before) the existing, retained silent-skip for a well-formed request where
        // both tags are present but `BeginSeqNo > EndSeqNo` (spec Edge Cases). `ResendRequest` is
        // admin-typed and so never reaches the dictionary-level required-field check.
        let ref_seq = msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map_or(0, |s| s as u64);
        for (present_tag, tag) in [
            (
                msg.body.get(crate::tags::BEGIN_SEQ_NO).is_some(),
                crate::tags::BEGIN_SEQ_NO,
            ),
            (
                msg.body.get(crate::tags::END_SEQ_NO).is_some(),
                crate::tags::END_SEQ_NO,
            ),
        ] {
            if !present_tag {
                let seq = self.next_seq();
                let rej = admin::reject_with_reason(
                    &self.config,
                    seq,
                    ref_seq,
                    msg.msg_type(),
                    Some(tag),
                    1, // SessionRejectReason: Required tag missing
                    "required tag missing",
                );
                return vec![self.send_stored(rej)];
            }
        }

        // NEW-02 (feature 009): `BeginSeqNo=0` is a protocol-valid value meaning "from the
        // beginning" (sequence 1) -- QFJ/QFGo both treat it this way. A negative value is still
        // invalid (maps to the same `0` sentinel `begin == 0` below treats as "nothing to do"), but
        // a present `0` must map to `1`, not share that sentinel.
        let begin = msg
            .body
            .get(crate::tags::BEGIN_SEQ_NO)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s >= 0)
            .map_or(0, |s| if s == 0 { 1 } else { s as u64 });
        let end_req = msg
            .body
            .get(crate::tags::END_SEQ_NO)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s >= 0)
            .map_or(0, |s| s as u64);
        let last = self.next_out_seq.saturating_sub(1);
        // BUG-97/FR-041 (feature 007): `999999` is also a recognized "infinity" convention (QFJ's
        // `manageGapFill`/`sendResendRequest`, QFGo's `sendResendRequest`) for FIX.4.2-and-earlier
        // sessions -- previously only `end_req == 0` was treated as infinity, and while
        // `end_req > last` already makes `999999` behave the same way in the overwhelmingly common
        // case (`last` is almost always far below a million), a long-lived session that has
        // actually sent 999,999+ messages would otherwise treat `999999` as a literal (now
        // less-than-`last`) endpoint instead of the honored convention.
        let is_legacy_infinity_convention = end_req == 999_999
            && admin::fix_version(&self.config.begin_string)
                .is_some_and(|(major, minor)| major < 4 || (major == 4 && minor <= 2));
        let end = if end_req == 0 || end_req > last || is_legacy_infinity_convention {
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
            // BUG-88/FR-016 (feature 007): `ForceResendWhenCorruptedStore` also changes
            // resend-vs-gap-fill classification during a gap-fill resend (previously only
            // consulted at store-open — `force_resend_when_corrupted_store()`'s existing
            // `run_connection` call site) — an admin message is resent (with PossDup) instead of
            // folded into a GapFill, matching QuickFIX/J's `resendMessages()` two-effect
            // semantics.
            let is_app = stored.as_ref().is_some_and(|m| {
                !is_admin_type(m.msg_type()) || self.config.force_resend_when_corrupted_store
            });
            if is_app {
                if let Some(gs) = gap_start.take() {
                    let action = self.gap_fill(gs, seq);
                    actions.push(action);
                }
                if let Some(m) = stored {
                    let resent = admin::prepare_resend(&self.config, &m);
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
        let mut m = admin::sequence_reset(&self.config, from_seq, new_seq_no, true);
        if self.config.enable_last_msg_seq_num_processed {
            m.header.set(Field::int(369, self.next_in_seq as i64));
        }
        self.send_raw(m)
    }

    fn on_sequence_reset(&mut self, msg: &Message) -> Vec<Action> {
        if let Some(reject) = self.validate_app(msg) {
            let mut actions = vec![reject];
            let ref_seq = msg
                .header
                .get(MSG_SEQ_NUM)
                .and_then(|f| f.as_int().ok())
                .filter(|&s| s > 0)
                .map(|s| s as u64);
            if ref_seq.is_some_and(|s| s == self.next_in_seq) {
                self.next_in_seq = self.next_in_seq.saturating_add(1);
            }
            actions.extend(self.reset_on_error());
            if self.config.disconnect_on_error {
                let seq = self.next_seq();
                let lo = admin::logout(
                    &self.config,
                    seq,
                    Some("application message failed validation"),
                );
                actions.push(self.send_logout(lo));
                self.state = SessionState::Disconnected;
                actions.push(Action::Disconnect);
            }
            return actions;
        }

        let gap_fill = msg.body.get(GAP_FILL_FLAG).and_then(|f| f.as_str().ok()) == Some("Y");
        let new_seq_present = msg.body.get(NEW_SEQ_NO).is_some();
        // NEW-34 (feature 009): `n >= 0`, not `n > 0` -- a present `NewSeqNo=0` must still flow
        // through as `Some(0)` for the non-gap-fill path below, so it's rejected via the existing
        // "NewSeqNo is decreasing" check (0 is always less than `next_in_seq`, which starts at 1)
        // rather than being silently filtered out to `None` and treated as if NewSeqNo were
        // absent entirely -- previously a non-gap-fill SequenceReset with NewSeqNo=0 present was
        // accepted as a silent no-op, neither applying nor rejecting it.
        let new_seq = msg
            .body
            .get(NEW_SEQ_NO)
            .and_then(|f| f.as_int().ok())
            .filter(|&n| n >= 0)
            .map(|n| n as u64);
        let ref_seq = msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map_or(0, |s| s as u64);

        // NEW-70/FR-040 (feature 009): NewSeqNo is required by both SequenceReset modes.
        // Previously this check lived only in the plain-reset branch; a gap-fill missing tag 36
        // therefore returned no action and was silently accepted without updating sequence state.
        if !new_seq_present {
            let seq = self.next_seq();
            let rej = admin::reject_with_reason(
                &self.config,
                seq,
                ref_seq,
                msg.msg_type(),
                Some(NEW_SEQ_NO),
                1, // SessionRejectReason: Required tag missing
                "NewSeqNo is missing",
            );
            return vec![self.send_stored(rej)];
        }

        // BUG-06/FR-002/FR-003 (feature 006): plain-mode (non-gap-fill) SequenceReset anti-replay
        // hole. Unlike gap-fill mode, a decreasing NewSeqNo must be rejected rather than applied —
        // applying it would rewind `next_in_seq`, reopening the window to replay/re-accept
        // previously-processed sequence numbers (this message type bypasses the PossDup anti-replay
        // check every other inbound message type gets, since it's routed here directly from
        // `on_received` before that check runs). A missing NewSeqNo tag entirely is a required-field
        // violation, not a silent skip that still drains the queue.
        if !gap_fill {
            if let Some(ns) = new_seq {
                if ns < self.next_in_seq {
                    let seq = self.next_seq();
                    let rej = admin::reject_with_reason(
                        &self.config,
                        seq,
                        ref_seq,
                        msg.msg_type(),
                        Some(NEW_SEQ_NO),
                        5, // SessionRejectReason: Value is incorrect (out of range) for this tag
                        "NewSeqNo is decreasing",
                    );
                    return vec![self.send_stored(rej)];
                }
                self.next_in_seq = ns;
                self.queue.retain(|&seq, _| seq >= ns);
            }
            return self.drain_queue();
        }

        self.apply_gap_fill_new_seq(new_seq);
        self.drain_queue()
    }

    /// Apply a gap-fill `SequenceReset`'s `NewSeqNo` (a no-op if absent or decreasing) and
    /// discard any queued out-of-order messages it skips over (BUG-96/FR-027, matching QFJ's
    /// `dequeueMessagesUpTo(newSequence)`) — a queued message whose sequence falls below the
    /// jump's target would otherwise never match `next_in_seq` again and stay in the queue
    /// forever. Extracted so both the fresh in-order case (`on_sequence_reset`, above) and a
    /// previously-queued-then-dequeued gap-fill (`drain_queue`, NEW-84) share one implementation.
    /// Does not itself drain the queue — callers are responsible for that (directly, or via the
    /// natural continuation of an already-running `drain_queue` loop).
    fn apply_gap_fill_new_seq(&mut self, new_seq: Option<u64>) {
        if let Some(ns) = new_seq
            && ns >= self.next_in_seq
        {
            self.next_in_seq = ns;
            self.queue.retain(|&seq, _| seq >= ns);
        }
    }

    fn on_logon(&mut self, msg: Message) -> Vec<Action> {
        // BUG-93/FR-029 (feature 007): recorded unconditionally, before any of this function's
        // acceptance/rejection checks -- QFJ's `logonReceived` tracks that a Logon was received at
        // all, not only a validly-accepted one.
        self.logon_received = true;
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

        // BUG-90/FR-028 (feature 007): reject a stray Logon arriving while not actually awaiting
        // one (`AwaitingLogout` or `Disconnected` -- `LoggedOn` is already handled just above),
        // via the same Logout+disconnect path, rather than silently falling through to the
        // `_ => {}` catch-all below and (at best) doing nothing, matching QFJ's `nextLogon()`,
        // which disconnects on an unsolicited/out-of-sequence Logon.
        if self.state != SessionState::AwaitingLogon {
            return self.reject_logon(&truefix_core::Reject {
                reason: 0,
                ref_tag: None,
                text: Some("Logon received out of sequence".to_owned()),
                session_status: None,
            });
        }

        // BUG-05/FR-001 (feature 006): reject a Logon whose own MsgSeqNum is below our expected
        // next-incoming sequence number and carries no PossDup justification, via the same
        // Logout+disconnect path used for the duplicate-Logon case above — before any reset/
        // state-transition side effects run, so a spurious too-low-seq Logon never partially
        // mutates session state or gets a Logon reply of its own. Exempts a Logon carrying
        // `ResetSeqNumFlag=Y` (checked here, ahead of this function's later, more elaborate
        // reset-flag handling) — such a Logon legitimately carries `MsgSeqNum=1` regardless of
        // whatever we currently expect, precisely because it's requesting a fresh reset; without
        // this exemption, a genuine reconnect-with-reset would be wrongly rejected as a stale/
        // too-low message instead of honored as the reset it is.
        let logon_seq_early = msg
            .header
            .get(MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&s| s > 0)
            .map(|s| s as u64);
        let poss_dup = msg.header.get(POSS_DUP_FLAG).and_then(|f| f.as_str().ok()) == Some("Y");
        let requests_reset = msg
            .body
            .get(RESET_SEQ_NUM_FLAG)
            .and_then(|f| f.as_str().ok())
            == Some("Y");
        // NEW-03 (feature 009): an acceptor configured with `ResetOnLogon=Y` resets its sequence
        // numbers on *any* inbound Logon, independent of that Logon's own `ResetSeqNumFlag` — so
        // this exemption must apply here too, not only for `requests_reset`/`poss_dup`, or a
        // realistic trigger (peer reconnects with a fresh seq=1 Logon after our own sequences had
        // already advanced) would be rejected as "too low" before the reset logic further below
        // ever runs.
        let acceptor_forces_reset =
            self.config.role == Role::Acceptor && self.config.reset_on_logon;
        if !poss_dup
            && !requests_reset
            && !acceptor_forces_reset
            && logon_seq_early.is_some_and(|s| s.cmp(&self.next_in_seq) == Ordering::Less)
        {
            return self.reject_logon(&truefix_core::Reject {
                reason: 0,
                ref_tag: None,
                text: Some("MsgSeqNum too low".to_owned()),
                session_status: None,
            });
        }

        // BUG-44/FR-019 (feature 007): reject a Logon missing MsgSeqNum(34) entirely, rather than
        // silently accepting it — `disposition` would be `None` (matching neither `Some(Equal)`
        // nor `Some(Greater)`), so `next_in_seq` would never advance, desyncing the session while
        // still transitioning it to `LoggedOn` and sending a Logon response.
        if logon_seq_early.is_none() {
            return self.reject_logon(&truefix_core::Reject {
                reason: 0,
                ref_tag: None,
                text: Some("Required tag missing: MsgSeqNum".to_owned()),
                session_status: None,
            });
        }

        // BUG-95/FR-019 (feature 007): reject a Logon carrying a negative HeartBtInt(108), matching
        // QFJ's `next()` (`HeartBtInt must not be negative`) — the existing `.filter(|&h| h > 0)`
        // adoption below silently ignores a negative value (leaving whatever heartbeat interval
        // this session started with in place) instead of treating it as the protocol violation it
        // is.
        if let Some(hbi) = msg.body.get(HEART_BT_INT).and_then(|f| f.as_int().ok())
            && hbi < 0
        {
            return self.reject_logon(&truefix_core::Reject {
                reason: 0,
                ref_tag: None,
                text: Some("HeartBtInt must not be negative".to_owned()),
                session_status: None,
            });
        }

        // NEW-71/FR-041 (feature 009): dictionary validation normally enforces required Logon
        // fields, but it is optional. An acceptor without a dictionary must still require a
        // positive HeartBtInt rather than silently retaining its configured default when tag 108
        // is absent, malformed, or zero. Initiator-side Logon-response behavior is unchanged.
        if self.config.role == Role::Acceptor
            && msg
                .body
                .get(HEART_BT_INT)
                .and_then(|f| f.as_int().ok())
                .is_none_or(|hbi| hbi <= 0)
        {
            return self.reject_logon(&truefix_core::Reject {
                reason: 0,
                ref_tag: Some(HEART_BT_INT),
                text: Some("HeartBtInt is required and must be positive".to_owned()),
                session_status: None,
            });
        }

        // BUG-43/FR-019 (feature 007): reject a Logon whose NextExpectedMsgSeqNum(789) is higher
        // than what we've actually sent — the counterparty expects messages we never sent, which
        // can only mean the two sides have desynced; matching QFJ ("Tag 789 is higher than
        // expected"), rather than silently ignoring it (the existing NextExpectedMsgSeqNum handling
        // near the end of this function only ever handles the too-low case).
        if let Some(next_expected) = msg
            .body
            .get(NEXT_EXPECTED_MSG_SEQ_NUM)
            .and_then(|f| f.as_int().ok())
            .filter(|&n| n > 0)
            .map(|n| n as u64)
            && next_expected > self.next_out_seq
        {
            return self.reject_logon(&truefix_core::Reject {
                reason: 0,
                ref_tag: None,
                text: Some("Tag 789 is higher than expected".to_owned()),
                session_status: None,
            });
        }

        // BUG-86/FR-015 (feature 007): reject a Logon arriving outside a configured activity
        // schedule (trading-hours window) — same Logout+disconnect path as the other Logon
        // rejections above, before any reset/state-transition side effects run.
        if let Some(schedule) = &self.config.schedule
            && !schedule.is_in_session(time::OffsetDateTime::now_utc())
        {
            return self.reject_logon(&truefix_core::Reject {
                reason: 0,
                ref_tag: None,
                text: Some("Logon attempted outside of the configured session schedule".to_owned()),
                session_status: None,
            });
        }

        // BUG-89/FR-011 (feature 007): a dictionary-invalid Logon (missing required field, bad
        // enum/format, etc.) is rejected via the same dictionary-validation path any other admin
        // message now uses (`validate_app`, no longer skipped for admin types) — before any of
        // this function's reset/state-transition side effects run, so a malformed Logon never
        // partially logs the session on.
        if let Some(reject) = self.validate_app(&msg) {
            let mut actions = vec![reject];
            if logon_seq_early.is_some_and(|s| s == self.next_in_seq) {
                self.next_in_seq = self.next_in_seq.saturating_add(1);
            }
            // NEW-63 (feature 009): a dictionary-invalid Logon must always be followed by a
            // Logout + disconnect, matching every other Logon-rejection path in this function
            // (all routed through `reject_logon`) -- previously this was gated on
            // `disconnect_on_error`, so `disconnect_on_error=false` left only a bare session
            // Reject sent and the session stranded in `AwaitingLogon` instead of torn down.
            let seq = self.next_seq();
            let lo = admin::logout(
                &self.config,
                seq,
                Some("Logon failed dictionary validation"),
            );
            actions.push(self.send_logout(lo));
            self.state = SessionState::Disconnected;
            actions.push(Action::Disconnect);
            return actions;
        }

        // T078/GAP-18c part 1 (feature 006, FIXT 1.1): auto-extract the inbound Logon's own
        // DefaultApplVerID (tag 1137 — appears only on Logon, per `FIXT11.fixdict`'s message
        // definition; distinct from tag 1128 ApplVerID, a per-message header field any message may
        // carry to override it), remembering it for the life of the connection so later
        // per-message application-dictionary selection (`validate_app`, T079) can prefer it over
        // the static `DefaultApplVerID` .cfg value when a given message carries neither its own
        // 1128 nor anything else more specific. A Logon without tag 1137 leaves the prior
        // negotiated value untouched (there is nothing to renegotiate to).
        if let Some(appl_ver_id) = msg
            .body
            .get(DEFAULT_APPL_VER_ID)
            .and_then(|f| f.as_str().ok())
        {
            // NEW-88 (feature 009): validate against the registered `FixtDictionaries`
            // application keys before accepting, matching QFJ's `nextLogon` (which throws
            // `RejectLogon("Invalid DefaultApplVerID=...")` for an unregistered value) — an
            // invalid/typo'd value previously stored verbatim and silently disabled
            // per-message application validation for the rest of the connection instead of being
            // rejected up front. Only applies when a `FixtDictionaries` is actually configured;
            // there is nothing to validate against otherwise.
            if !appl_ver_id.is_empty()
                && let Some((dicts, _)) = &self.fixt_validator
                && dicts.application_for(Some(appl_ver_id)).is_none()
            {
                return self.reject_logon(&truefix_core::Reject {
                    reason: 5, // SessionRejectReason: Value is incorrect (out of range) for this tag
                    ref_tag: Some(DEFAULT_APPL_VER_ID),
                    text: Some(format!("Invalid DefaultApplVerID={appl_ver_id}")),
                    session_status: None,
                });
            }
            self.negotiated_appl_ver_id = Some(appl_ver_id.to_owned());
        }

        // BUG-28/FR-005 (feature 007): captured once, reused both to decide our own reset
        // (below) and — for the acceptor role — to echo on our Logon response later in this
        // function, instead of that response independently consulting `config.reset_on_logon`.
        let inbound_reset_flag = msg
            .body
            .get(RESET_SEQ_NUM_FLAG)
            .and_then(|f| f.as_str().ok())
            == Some("Y");
        let mut store_reset = false;
        // NEW-03 (feature 009): reset on `inbound_reset_flag` (what the peer requested) OR the
        // acceptor's own `ResetOnLogon` config (independent of what the peer requested) — the
        // response's own `ResetSeqNumFlag` echo further below is intentionally left keyed only to
        // `inbound_reset_flag` (BUG-28's fix), not `acceptor_forces_reset` — these are distinct
        // mechanisms per the audit's own note.
        if inbound_reset_flag || acceptor_forces_reset {
            let full = !self.logon_sent;
            self.reset_sequences(full);
            store_reset = full;
        }

        // Account for the Logon's own sequence number *before* building our response, so that
        // NextExpectedMsgSeqNum (789) and LastMsgSeqNumProcessed (369) on the response reflect
        // having consumed this Logon. The resulting actions (queue drain / ResendRequest) are
        // emitted *after* the Logon response below to preserve wire ordering.
        let logon_seq = logon_seq_early;
        let disposition = logon_seq.map(|s| s.cmp(&self.next_in_seq));
        if disposition == Some(Ordering::Equal) {
            self.next_in_seq = self.next_in_seq.saturating_add(1);
        }

        let mut actions = Vec::new();
        if store_reset {
            actions.push(Action::ResetStore);
        }
        if self.config.role == Role::Acceptor
            && self.state == SessionState::AwaitingLogon
            && disposition == Some(Ordering::Greater)
        {
            if !self.resend_requested || self.config.send_redundant_resend_requests {
                self.resend_requested = true;
                if self.config.resend_request_chunk_size > 0
                    && let Some(s) = logon_seq
                {
                    self.resend_target = Some(self.resend_target.map_or(s, |t| t.max(s)));
                }
                let begin = self.next_in_seq;
                actions.push(self.request_resend(begin));
            }
            return actions;
        }
        match self.config.role {
            Role::Acceptor if self.state == SessionState::AwaitingLogon => {
                if let Some(hbi) = msg
                    .body
                    .get(HEART_BT_INT)
                    .and_then(|f| f.as_int().ok())
                    .filter(|&h| h > 0)
                {
                    // BUG-67/FR-038 (feature 007): clamp, not `as u32` — a peer-supplied value
                    // beyond `u32::MAX` (`hbi` is `i64`) would otherwise silently truncate to an
                    // arbitrary, unrelated small number instead of the sane "very long heartbeat
                    // interval" the peer's oversized value actually implies.
                    self.config.heartbeat_interval = u32::try_from(hbi).unwrap_or(u32::MAX);
                }
                self.state = SessionState::LoggedOn;
                let next_exp = self.maybe_next_expected();
                let seq = self.next_seq();
                // BUG-28/FR-005 (feature 007): echo the inbound Logon's own `ResetSeqNumFlag`,
                // not `config.reset_on_logon` — the response must reflect what was actually
                // requested, not static session configuration.
                let resp =
                    admin::logon_with_reset_flag(&self.config, seq, next_exp, inbound_reset_flag);
                self.logon_sent = true;
                actions.push(self.send_stored(resp));
            }
            Role::Initiator if self.state == SessionState::AwaitingLogon => {
                // BUG-28/FR-006 (feature 007): if this session sent `ResetSeqNumFlag=Y` on its
                // own Logon (tracked via `self.reset_on_logon_pending`, set in `on_connected`),
                // the acceptor's response MUST echo it back, or be inferable as an acknowledged
                // reset via its own `MsgSeqNum == 1` — neither holding is a handshake failure,
                // routed through the same Logout+disconnect path as other Logon rejections.
                if self.reset_on_logon_pending && !inbound_reset_flag && logon_seq_early != Some(1)
                {
                    self.reset_on_logon_pending = false;
                    return self.reject_logon(&truefix_core::Reject {
                        reason: 0,
                        ref_tag: None,
                        text: Some(
                            "Expected Logon response to have reset sequence numbers".to_owned(),
                        ),
                        session_status: None,
                    });
                }
                self.reset_on_logon_pending = false;
                self.state = SessionState::LoggedOn;
            }
            _ => {}
        }

        // Emit the deferred sequence actions after the Logon response.
        match disposition {
            Some(Ordering::Equal) => actions.extend(self.drain_queue()),
            Some(Ordering::Greater) if !self.resend_requested => {
                self.resend_requested = true;
                if self.config.resend_request_chunk_size > 0
                    && let Some(s) = logon_seq
                {
                    self.resend_target = Some(self.resend_target.map_or(s, |t| t.max(s)));
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
            && next_expected < self.next_out_seq
        {
            let end = self.next_out_seq.saturating_sub(1);
            actions.extend(self.build_resend(next_expected, end));
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
            let reset = self.enter_disconnected(DisconnectReason::GracefulLogout);
            let mut actions: Vec<Action> = reset.into_iter().collect();
            actions.push(Action::Disconnect);
            actions
        } else {
            let seq = self.next_seq();
            let lo = admin::logout(&self.config, seq, None);
            let send = self.send_logout(lo);
            let reset = self.enter_disconnected(DisconnectReason::GracefulLogout);
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
            vec![self.send_logout(lo)]
        } else {
            Vec::new()
        }
    }

    /// Transition to `Disconnected`, honoring `ResetOnLogout` (a completed graceful Logout
    /// exchange) or `ResetOnDisconnect` (everything else) depending on `reason` (NEW-56, feature
    /// 009 — previously both flags were `||`-combined regardless of cause). Returns
    /// `Some(Action::ResetStore)` when a full reset happened, so the caller can include it in the
    /// actions it returns (FR-004) — the durable store is otherwise unaware this reset occurred.
    fn enter_disconnected(&mut self, reason: DisconnectReason) -> Option<Action> {
        self.state = SessionState::Disconnected;
        self.last_disconnect_reason = Some(reason);
        let should_reset = match reason {
            DisconnectReason::GracefulLogout => self.config.reset_on_logout,
            DisconnectReason::Other => self.config.reset_on_disconnect,
        };
        if should_reset {
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
                let reset = self.enter_disconnected(DisconnectReason::Other);
                let mut actions: Vec<Action> = reset.into_iter().collect();
                actions.push(Action::Disconnect);
                return actions;
            }
            SessionState::LoggedOn
                if self
                    .config
                    .schedule
                    .as_ref()
                    .is_some_and(|s| !s.is_in_session(time::OffsetDateTime::now_utc())) =>
            {
                // BUG-87/FR-015 (feature 007): an already-`LoggedOn` session is disconnected once
                // the current time crosses outside its configured schedule window, rather than
                // being left connected indefinitely past the window's end.
                // NEW-62 (feature 009): send a Logout before disconnecting, matching QFJ's own
                // session-end behavior. NEW-97 (audit 006): after sending it, wait for the peer
                // Logout (or LogoutTimeout) instead of immediately closing the transport.
                let seq = self.next_seq();
                let lo = admin::logout(&self.config, seq, Some("session schedule window closed"));
                let send = self.send_logout(lo);
                self.ticks_awaiting = 0;
                self.state = SessionState::AwaitingLogout;
                return vec![send];
            }
            SessionState::LoggedOn if !self.config.disable_heart_beat_check => {
                // BUG-36/FR-033 (feature 007): `f64` arithmetic throughout, not `u32` — besides
                // `heartbeat_timeout_multiplier` itself now needing to carry fractional precision
                // (see its doc), plain `u32` multiplication here could also overflow (panicking in
                // debug builds, silently wrapping in release) for large `hb`/multiplier values.
                // `f64` saturates to infinity instead of overflowing, so this comparison stays
                // correct (and simply never times out) even for pathological configured values.
                let timeout_ticks =
                    f64::from(hb) * self.config.heartbeat_timeout_multiplier.max(1.0) + 2.0;
                if (self.ticks_since_recv as f64) >= timeout_ticks {
                    let reset = self.enter_disconnected(DisconnectReason::Other);
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
                let reset = self.enter_disconnected(DisconnectReason::Other);
                let mut actions: Vec<Action> = reset.into_iter().collect();
                actions.push(Action::Disconnect);
                return actions;
            }
            // BUG-69/FR-040 (feature 007): still send heartbeats on the normal cadence while
            // waiting for the counterparty's Logout to arrive (and before our own `logout_timeout`
            // elapses, handled by the guarded arm above) — previously this fell through to the
            // catch-all below and sent nothing, so a long `logout_timeout` risked the peer
            // disconnecting us for heartbeat failure well before our own timeout ever fired.
            SessionState::AwaitingLogout if self.ticks_since_send >= hb => {
                let seq = self.next_seq();
                let h = admin::heartbeat(&self.config, seq, None);
                actions.push(self.send_stored(h));
            }
            _ => {}
        }
        actions
    }
}

fn is_admin_type(mt: Option<&str>) -> bool {
    matches!(mt, Some("0" | "1" | "2" | "3" | "4" | "5" | "A"))
}
