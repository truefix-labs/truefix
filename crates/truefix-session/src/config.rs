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
    /// ResendRequest chunk size; `0` means request the whole range at once.
    pub resend_request_chunk_size: u32,
    /// Whether to use NextExpectedMsgSeqNum (789) on logon (FIX ≥ 4.4).
    pub enable_next_expected_msg_seq_num: bool,
    /// Whether to validate inbound SendingTime against the local clock.
    pub check_latency: bool,
    /// Maximum tolerated SendingTime latency, in seconds.
    pub max_latency: u32,
    /// Seconds to wait for the Logon handshake before giving up (LogonTimeout).
    pub logon_timeout: u32,
    /// Seconds to wait for the counterparty Logout before disconnecting (LogoutTimeout).
    pub logout_timeout: u32,
    /// Initiator reconnect interval, in seconds (ReconnectInterval).
    pub reconnect_interval: u32,
    /// Optional activity schedule (StartTime/EndTime/Weekdays/NonStop).
    pub schedule: Option<crate::schedule::Schedule>,
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
            resend_request_chunk_size: 0,
            enable_next_expected_msg_seq_num: false,
            check_latency: true,
            max_latency: 120,
            logon_timeout: 10,
            logout_timeout: 10,
            reconnect_interval: 5,
            schedule: None,
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
