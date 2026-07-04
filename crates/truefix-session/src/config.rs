//! Session configuration (programmatic; the `.cfg` mapping lives in `truefix-config`).

use crate::session_id::SessionId;

/// Whether this side initiates (connects out) or accepts (listens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Buy-side / connects out.
    Initiator,
    /// Sell-side / listens.
    Acceptor,
}

/// Minimal session configuration needed for the S2 logon/heartbeat/logout flow.
///
/// Sequence recovery, persistence, scheduling, and the full Appendix A key surface arrive in
/// later stages; this is the bootstrap subset.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// BeginString, e.g. `"FIX.4.4"`.
    pub begin_string: String,
    /// This engine's SenderCompID.
    pub sender_comp_id: String,
    /// The counterparty's TargetCompID.
    pub target_comp_id: String,
    /// Heartbeat interval in seconds (HeartBtInt, tag 108).
    pub heartbeat_interval: u32,
    /// Initiator or acceptor.
    pub role: Role,
    /// Whether to set ResetSeqNumFlag (141=Y) on the logon (resets both sequence numbers to 1).
    pub reset_on_logon: bool,
    /// Whether to reset sequence numbers on logout.
    pub reset_on_logout: bool,
    /// Whether to reset sequence numbers on disconnect.
    pub reset_on_disconnect: bool,
    /// Whether to refresh state from the store on logon.
    pub refresh_on_logon: bool,
    /// Whether to persist sent messages for replay (PersistMessages). When `false`, sent messages
    /// are not stored, so a ResendRequest is satisfied with a SequenceReset-GapFill (FR-003).
    pub persist_messages: bool,
    /// ResendRequest chunk size; `0` means request the whole range at once.
    pub resend_request_chunk_size: u32,
    /// Whether to use NextExpectedMsgSeqNum (789) on logon (FIX ≥ 4.4).
    pub enable_next_expected_msg_seq_num: bool,
    /// Whether to stamp LastMsgSeqNumProcessed (369) on every outbound message.
    pub enable_last_msg_seq_num_processed: bool,
    /// Whether to validate inbound SendingTime against the local clock.
    pub check_latency: bool,
    /// Maximum tolerated SendingTime latency, in seconds.
    pub max_latency: u32,
    /// Whether to validate inbound SenderCompID/TargetCompID against the session (CheckCompID).
    pub check_comp_id: bool,
    /// Whether a garbled (undecodable) frame draws a session Reject (RejectGarbledMessage). When
    /// `false` (the default), garbled frames are silently dropped without advancing the sequence
    /// number (FR-006).
    pub reject_garbled_message: bool,
    /// Multiplier applied to the heartbeat interval before probing a silent peer with a
    /// TestRequest (TestRequestDelayMultiplier; the probe fires after
    /// `heartbeat_interval * this` idle ticks, default matches `heartbeat_interval + 1`).
    pub test_request_delay_multiplier: f64,
    /// Heartbeat-interval multiplier after which a silent peer is disconnected
    /// (HeartBeatTimeoutMultiplier; the timeout is `heartbeat_interval * this + 2` ticks).
    /// BUG-36/FR-033 (feature 007): `f64`, not `u32` — QFJ's own `HeartBtIntTimeoutMultiplier` is a
    /// `double` (default `1.4`), so a `.cfg` value with fractional precision (e.g. `1.4`) must be
    /// honored rather than truncated to `1`, which would silently produce a shorter timeout than
    /// configured.
    pub heartbeat_timeout_multiplier: f64,
    /// Precision of the SendingTime the engine emits (TimeStampPrecision).
    pub timestamp_precision: TimeStampPrecision,
    /// Seconds to wait for the Logon handshake before giving up (LogonTimeout).
    pub logon_timeout: u32,
    /// Seconds to wait for the counterparty Logout before disconnecting (LogoutTimeout).
    pub logout_timeout: u32,
    /// Initiator reconnect interval, in seconds (ReconnectInterval).
    pub reconnect_interval: u32,
    /// Optional activity schedule (StartTime/EndTime/Weekdays/NonStop).
    pub schedule: Option<crate::schedule::Schedule>,
    /// Send a fresh ResendRequest even while one is already outstanding, instead of suppressing it
    /// (SendRedundantResendRequests). Default `false` (today's suppress-while-pending behaviour).
    pub send_redundant_resend_requests: bool,
    /// Recognised for QuickFIX/J config-key parity (ClosedResendInterval). TrueFix's session state
    /// machine processes one event at a time with no concurrent resend-servicing path, so there is
    /// no open/closed-interval race for this to resolve — accepted but intentionally a no-op.
    pub closed_resend_interval: bool,
    /// Perform a full reset (sequence numbers + durable store) in addition to the normal
    /// reject/disconnect handling when an inbound message fails validation or identity/latency
    /// checks (ResetOnError). Default `false`.
    pub reset_on_error: bool,
    /// Disconnect when an inbound application message fails dictionary validation (session or
    /// business reject), in addition to sending the reject (DisconnectOnError). Identity/latency
    /// failures already disconnect unconditionally, independent of this flag. Default `false`.
    pub disconnect_on_error: bool,
    /// Skip the heartbeat-timeout disconnect and test-request-on-silence checks entirely
    /// (DisableHeartBeatCheck). Default `false`.
    pub disable_heart_beat_check: bool,
    /// Recognised for QuickFIX/J config-key parity (RejectMessageOnUnhandledException). TrueFix's
    /// typed-error architecture (Constitution Principle I: no panics on critical paths) has no
    /// unhandled-exception class this would apply to — accepted but intentionally a no-op.
    pub reject_message_on_unhandled_exception: bool,
    /// Custom `(tag, value)` pairs appended to every outbound Logon (`LogonTag`, `LogonTag1`,
    /// `LogonTag2`, … — GAP-12), in ascending numeric-suffix order.
    pub logon_tags: Vec<(u32, String)>,
    /// Recognised for QuickFIX/J config-key parity (MaxScheduledWriteRequests). TrueFix's session
    /// state machine returns actions synchronously for the transport to write immediately — there
    /// is no internal outbound write queue for this to bound — accepted but intentionally a no-op.
    pub max_scheduled_write_requests: Option<u32>,
    /// `ContinueInitializationOnError` (US4, feature 004, FR-005): whether a multi-session engine
    /// keeps starting *other* sessions when this one's configuration or startup fails — an
    /// `Engine::start` (multi-session bring-up) concern in the `truefix` crate, honored there; this
    /// field itself has no effect on `Session`'s own per-connection runtime behaviour, it only
    /// carries the setting from `.cfg` for `Engine::start` to read.
    pub continue_initialization_on_error: bool,
    /// Force a full reset (sequence numbers + durable store) at connect time when the durable store
    /// reports it recovered from corruption (ForceResendWhenCorruptedStore) — the safest recovery
    /// action when local history can't be fully trusted, rather than resending possibly-incomplete
    /// data. Default `false`.
    pub force_resend_when_corrupted_store: bool,
    /// Reject (Logout + disconnect) an inbound message with `MsgSeqNum` lower than expected and
    /// `PossDupFlag=Y` when it's missing `OrigSendingTime` (US3, feature 005, GAP-08/FR-009) — in
    /// addition to the unconditional check (always active regardless of this switch) that rejects
    /// such a message when `OrigSendingTime` is present but later than the message's own
    /// `SendingTime` (a replay/falsification signal). Default `false`, matching QuickFIX/J's
    /// `RequiresOrigSendingTime` default.
    pub requires_orig_sending_time_on_low_seq: bool,
    /// Bound the inbound *application*-message channel to at most this many pending messages
    /// (`InChanCapacity`; US14, FR-019). Administrative/session-level traffic (heartbeat,
    /// TestRequest, ResendRequest, Logon, Logout, garbled-frame handling) always travels on a
    /// separate, unbounded, priority-drained channel, so it is never starved by a full application
    /// queue. `None` (the default) preserves today's unbounded behavior for both.
    pub in_chan_capacity: Option<usize>,
    /// Optional SenderSubID (GAP-47/FR-012, feature 005): part of the session identity, alongside
    /// [`Self::sender_comp_id`].
    pub sender_sub_id: Option<String>,
    /// Optional SenderLocationID (GAP-47/FR-012, feature 005).
    pub sender_location_id: Option<String>,
    /// Optional TargetSubID (GAP-47/FR-012, feature 005).
    pub target_sub_id: Option<String>,
    /// Optional TargetLocationID (GAP-47/FR-012, feature 005).
    pub target_location_id: Option<String>,
    /// Optional session qualifier (GAP-47/FR-012/FR-013, feature 005): disambiguates two sessions
    /// that otherwise share the same BeginString/SenderCompID/TargetCompID, letting both start as
    /// distinct, independently addressable sessions.
    pub session_qualifier: Option<String>,
    /// Stepped reconnect-backoff delays, in seconds (GAP-14/FR-014, feature 005): the reconnect
    /// loop indexes into this by attempt number, clamped (sticking) at the last element once
    /// exhausted. Empty (the default) preserves the single-`reconnect_interval` behavior
    /// unchanged (`ReconnectInterval` also accepts a space/comma-separated list, which populates
    /// this field instead).
    pub reconnect_interval_steps: Vec<u32>,
    /// Local address an initiator binds before connecting out (GAP-15/FR-015, feature 005;
    /// `SocketLocalHost`+`SocketLocalPort`). `None` (the default) preserves today's
    /// OS-chosen-ephemeral-port behavior.
    pub local_bind_addr: Option<std::net::SocketAddr>,
    /// Bound an initiator's connect attempt (GAP-16/FR-016, feature 005; `SocketConnectTimeout`).
    /// `None` (the default) preserves today's unbounded-wait behavior.
    pub connect_timeout: Option<std::time::Duration>,
}

/// Sub-second precision of emitted SendingTime timestamps (TimeStampPrecision; FR-009).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeStampPrecision {
    /// Whole seconds (`YYYYMMDD-HH:MM:SS`).
    Seconds,
    /// Milliseconds (`.sss`) — the QuickFIX/J default.
    Milliseconds,
    /// Microseconds (`.ssssss`).
    Microseconds,
    /// Nanoseconds (`.sssssssss`).
    Nanoseconds,
}

impl SessionConfig {
    /// Convenience constructor.
    pub fn new(
        begin_string: impl Into<String>,
        sender_comp_id: impl Into<String>,
        target_comp_id: impl Into<String>,
        role: Role,
    ) -> Self {
        Self {
            begin_string: begin_string.into(),
            sender_comp_id: sender_comp_id.into(),
            target_comp_id: target_comp_id.into(),
            heartbeat_interval: 30,
            role,
            reset_on_logon: true,
            reset_on_logout: false,
            reset_on_disconnect: false,
            refresh_on_logon: false,
            persist_messages: true,
            resend_request_chunk_size: 0,
            enable_next_expected_msg_seq_num: false,
            enable_last_msg_seq_num_processed: false,
            check_latency: true,
            max_latency: 120,
            check_comp_id: true,
            reject_garbled_message: false,
            test_request_delay_multiplier: 1.0,
            heartbeat_timeout_multiplier: 2.0,
            timestamp_precision: TimeStampPrecision::Milliseconds,
            logon_timeout: 10,
            logout_timeout: 10,
            reconnect_interval: 5,
            schedule: None,
            send_redundant_resend_requests: false,
            closed_resend_interval: false,
            reset_on_error: false,
            disconnect_on_error: false,
            disable_heart_beat_check: false,
            reject_message_on_unhandled_exception: false,
            logon_tags: Vec::new(),
            max_scheduled_write_requests: None,
            continue_initialization_on_error: false,
            force_resend_when_corrupted_store: false,
            requires_orig_sending_time_on_low_seq: false,
            in_chan_capacity: None,
            sender_sub_id: None,
            sender_location_id: None,
            target_sub_id: None,
            target_location_id: None,
            session_qualifier: None,
            reconnect_interval_steps: Vec::new(),
            local_bind_addr: None,
            connect_timeout: None,
        }
    }

    /// The [`SessionId`] this configuration describes.
    pub fn session_id(&self) -> SessionId {
        SessionId::new_full(
            self.begin_string.clone(),
            self.sender_comp_id.clone(),
            self.sender_sub_id.clone(),
            self.sender_location_id.clone(),
            self.target_comp_id.clone(),
            self.target_sub_id.clone(),
            self.target_location_id.clone(),
            self.session_qualifier.clone(),
        )
    }
}
