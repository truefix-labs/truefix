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
    pub heartbeat_timeout_multiplier: u32,
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
    /// An optional custom `(tag, value)` pair appended to every outbound Logon (LogonTag).
    pub logon_tag: Option<(u32, String)>,
    /// Recognised for QuickFIX/J config-key parity (MaxScheduledWriteRequests). TrueFix's session
    /// state machine returns actions synchronously for the transport to write immediately — there
    /// is no internal outbound write queue for this to bound — accepted but intentionally a no-op.
    pub max_scheduled_write_requests: Option<u32>,
    /// Recognised for QuickFIX/J config-key parity (ContinueInitializationOnError). This governs
    /// whether a multi-session engine keeps starting *other* sessions when one session's
    /// configuration fails during startup — an `Engine::start` (multi-session bring-up) concern in
    /// the `truefix` crate, not a per-connection `Session` runtime behaviour, so it has no effect
    /// here; the field exists so the setting round-trips through `SessionConfig`.
    pub continue_initialization_on_error: bool,
    /// Force a full reset (sequence numbers + durable store) at connect time when the durable store
    /// reports it recovered from corruption (ForceResendWhenCorruptedStore) — the safest recovery
    /// action when local history can't be fully trusted, rather than resending possibly-incomplete
    /// data. Default `false`.
    pub force_resend_when_corrupted_store: bool,
    /// Bound the inbound *application*-message channel to at most this many pending messages
    /// (`InChanCapacity`; US14, FR-019). Administrative/session-level traffic (heartbeat,
    /// TestRequest, ResendRequest, Logon, Logout, garbled-frame handling) always travels on a
    /// separate, unbounded, priority-drained channel, so it is never starved by a full application
    /// queue. `None` (the default) preserves today's unbounded behavior for both.
    pub in_chan_capacity: Option<usize>,
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
            heartbeat_timeout_multiplier: 2,
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
            logon_tag: None,
            max_scheduled_write_requests: None,
            continue_initialization_on_error: false,
            force_resend_when_corrupted_store: false,
            in_chan_capacity: None,
        }
    }

    /// The [`SessionId`] this configuration describes.
    pub fn session_id(&self) -> SessionId {
        SessionId::new(
            self.begin_string.clone(),
            self.sender_comp_id.clone(),
            self.target_comp_id.clone(),
        )
    }
}
