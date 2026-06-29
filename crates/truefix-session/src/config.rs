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
    /// Whether to set ResetSeqNumFlag (141=Y) on the logon.
    pub reset_on_logon: bool,
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
