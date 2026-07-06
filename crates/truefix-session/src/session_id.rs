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

    /// Create a session id from all eight components (GAP-47/FR-012, feature 005): the three
    /// required identity fields plus SenderSubID/SenderLocationID/TargetSubID/TargetLocationID/
    /// SessionQualifier, which together let two sessions sharing the same
    /// BeginString/SenderCompID/TargetCompID be distinguished (FR-013).
    #[allow(clippy::too_many_arguments)]
    pub fn new_full(
        begin_string: impl Into<String>,
        sender_comp_id: impl Into<String>,
        sender_sub_id: Option<String>,
        sender_location_id: Option<String>,
        target_comp_id: impl Into<String>,
        target_sub_id: Option<String>,
        target_location_id: Option<String>,
        session_qualifier: Option<String>,
    ) -> Self {
        Self {
            begin_string: begin_string.into(),
            sender_comp_id: sender_comp_id.into(),
            sender_sub_id,
            sender_location_id,
            target_comp_id: target_comp_id.into(),
            target_sub_id,
            target_location_id,
            session_qualifier,
        }
    }
}

/// Append `/sub_id` and `/location_id` when present (T170/T171, feature 009, NEW-39) — two
/// sessions sharing the same BeginString/CompIDs but differing only by SubID/LocationID
/// previously rendered identically via `Display`, making them indistinguishable in logs/error
/// messages/anywhere else this is the only identifying text shown.
fn write_comp_id_with_qualifiers(
    f: &mut fmt::Formatter<'_>,
    comp_id: &str,
    sub_id: Option<&str>,
    location_id: Option<&str>,
) -> fmt::Result {
    write!(f, "{comp_id}")?;
    if let Some(sub) = sub_id {
        write!(f, "/{sub}")?;
    }
    if let Some(loc) = location_id {
        write!(f, "/{loc}")?;
    }
    Ok(())
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.begin_string)?;
        write_comp_id_with_qualifiers(
            f,
            &self.sender_comp_id,
            self.sender_sub_id.as_deref(),
            self.sender_location_id.as_deref(),
        )?;
        write!(f, "->")?;
        write_comp_id_with_qualifiers(
            f,
            &self.target_comp_id,
            self.target_sub_id.as_deref(),
            self.target_location_id.as_deref(),
        )?;
        if let Some(q) = &self.session_qualifier {
            write!(f, ":{q}")?;
        }
        Ok(())
    }
}
