//! SBE codec errors and observability helpers.

/// A typed SBE codec error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SbeCodecError {
    /// Input ended before the requested byte offset.
    #[error("SBE input truncated at byte {offset}")]
    Truncated {
        /// Byte offset that could not be read.
        offset: usize,
    },
    /// Declared block length differs from the schema.
    #[error("SBE block length mismatch: declared {declared}, actual {actual}")]
    BlockLengthMismatch {
        /// Declared block length.
        declared: u32,
        /// Schema block length.
        actual: u32,
    },
    /// The inline template id was not loaded.
    #[error("unknown SBE template id {id}")]
    UnknownTemplateId {
        /// Missing template id.
        id: u32,
    },
    /// A VarData length would read past input.
    #[error("SBE varData {field:?} length mismatch: declared {declared}, actual {actual}")]
    VarDataLengthMismatch {
        /// Field name.
        field: String,
        /// Declared length.
        declared: u32,
        /// Available length.
        actual: u32,
    },
    /// A field value could not be represented.
    #[error("SBE field {field} value is unsupported: {reason}")]
    UnsupportedValue {
        /// Field tag.
        field: u32,
        /// Reason.
        reason: String,
    },
}

/// Emit the structured decode-failure tracing and metrics event required by FR-013.
pub fn report_decode_failure(session: &str, template_id: Option<u32>, error: &SbeCodecError) {
    tracing::error!(
        target: "truefix_binary",
        session = session,
        template_id = template_id,
        error = %error,
        "SBE decode failure"
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
        "SBE unknown template id"
    );
    metrics::counter!("truefix_binary_unknown_templates_total", "session" => session.to_owned())
        .increment(1);
}
