//! Session identity.

use core::fmt;

/// Identifies a FIX session: BeginString + Sender/Target comp IDs (+ optional sub/location
/// IDs and a qualifier). Used to route connections and look up sessions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId {
    /// BeginString, e.g. `"FIX.4.4"`.
    pub begin_string: String,
    /// SenderCompID (this engine's identity).
    pub sender_comp_id: String,
    /// Optional SenderSubID.
    pub sender_sub_id: Option<String>,
    /// Optional SenderLocationID.
    pub sender_location_id: Option<String>,
    /// TargetCompID (the counterparty).
    pub target_comp_id: String,
    /// Optional TargetSubID.
    pub target_sub_id: Option<String>,
    /// Optional TargetLocationID.
    pub target_location_id: Option<String>,
    /// Optional session qualifier (disambiguates multiple sessions with the same comp IDs).
    pub session_qualifier: Option<String>,
}

impl SessionId {
    /// Create a session id from the three required components.
    pub fn new(
        begin_string: impl Into<String>,
        sender_comp_id: impl Into<String>,
        target_comp_id: impl Into<String>,
    ) -> Self {
        Self {
            begin_string: begin_string.into(),
            sender_comp_id: sender_comp_id.into(),
            sender_sub_id: None,
            sender_location_id: None,
            target_comp_id: target_comp_id.into(),
            target_sub_id: None,
            target_location_id: None,
            session_qualifier: None,
        }
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}->{}",
            self.begin_string, self.sender_comp_id, self.target_comp_id
        )?;
        if let Some(q) = &self.session_qualifier {
            write!(f, ":{q}")?;
        }
        Ok(())
    }
}
