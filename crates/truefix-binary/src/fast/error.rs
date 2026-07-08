//! FAST codec errors and observability helpers.

/// A typed FAST codec error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FastCodecError {
    /// Input ended before the requested byte offset.
    #[error("FAST input truncated at byte {offset}")]
    Truncated {
        /// Byte offset that could not be read.
        offset: usize,
    },
    /// A stop-bit integer exceeded the supported width or never terminated.
    #[error("invalid FAST stop-bit integer at byte {offset}")]
    InvalidStopBit {
        /// Byte offset where the invalid integer started.
        offset: usize,
    },
    /// Presence-map bits were inconsistent with the template.
    #[error("FAST presence map mismatch for field {field} at byte {offset}")]
    PresenceMapMismatch {
        /// Field tag.
        field: u32,
        /// Byte offset.
        offset: usize,
    },
    /// The inline template id was not loaded.
    #[error("unknown FAST template id {id}")]
    UnknownTemplateId {
        /// Missing template id.
        id: u32,
    },
    /// A field value could not be represented.
    #[error("FAST field {field} value is unsupported: {reason}")]
    UnsupportedValue {
        /// Field tag.
        field: u32,
        /// Reason.
        reason: String,
    },
}

/// Emit the structured decode-failure tracing and metrics event required by FR-013.
pub fn report_decode_failure(session: &str, template_id: Option<u32>, error: &FastCodecError) {
    tracing::error!(
        target: "truefix_binary",
        session = session,
        template_id = template_id,
        error = %error,
        "FAST decode failure"
    );
    metrics::counter!("truefix_binary_decode_failures_total", "session" => session.to_owned())
        .increment(1);
}

/// Emit the structured unknown-template tracing and metrics event required by FR-013.
pub fn report_unknown_template(session: &str, template_id: u32) {
    tracing::error!(
        target: "truefix_binary",
        session = session,
        template_id = template_id,
        "FAST unknown template id"
    );
    metrics::counter!("truefix_binary_unknown_templates_total", "session" => session.to_owned())
        .increment(1);
}
