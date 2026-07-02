//! Typed errors for the core layer. No code path panics (Constitution Principle I).

use thiserror::Error;

/// Error converting a field's raw value to a typed Rust value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FieldError {
    /// The raw value is not valid UTF-8.
    #[error("field {tag}: value is not valid UTF-8")]
    NotUtf8 {
        /// The field tag.
        tag: u32,
    },
    /// The value is not a valid integer.
    #[error("field {tag}: expected an integer, got {value:?}")]
    NotInt {
        /// The field tag.
        tag: u32,
        /// The offending value.
        value: String,
    },
    /// The value is not a valid decimal.
    #[error("field {tag}: expected a decimal, got {value:?}")]
    NotDecimal {
        /// The field tag.
        tag: u32,
        /// The offending value.
        value: String,
    },
    /// The value is not a single character.
    #[error("field {tag}: expected a single character, got {value:?}")]
    NotChar {
        /// The field tag.
        tag: u32,
        /// The offending value.
        value: String,
    },
    /// The value is not a boolean (`Y`/`N`).
    #[error("field {tag}: expected 'Y' or 'N', got {value:?}")]
    NotBool {
        /// The field tag.
        tag: u32,
        /// The offending value.
        value: String,
    },
    /// The value is not a valid UTC timestamp.
    #[error("field {tag}: invalid UTC timestamp {value:?}")]
    NotTimestamp {
        /// The field tag.
        tag: u32,
        /// The offending value.
        value: String,
    },
    /// The value is not a valid UTC date-only (`YYYYMMDD`).
    #[error("field {tag}: invalid UTC date-only {value:?}")]
    NotDateOnly {
        /// The field tag.
        tag: u32,
        /// The offending value.
        value: String,
    },
    /// The value is not a valid UTC time-only (`HH:MM:SS[.sss...]`).
    #[error("field {tag}: invalid UTC time-only {value:?}")]
    NotTimeOnly {
        /// The field tag.
        tag: u32,
        /// The offending value.
        value: String,
    },
}

/// Error decoding raw bytes into a [`Message`](crate::Message).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// The input was empty.
    #[error("empty input")]
    Empty,
    /// The message does not start with BeginString (tag 8).
    #[error("message does not begin with BeginString (tag 8)")]
    MissingBeginString,
    /// BodyLength (tag 9) is missing or not a valid number.
    #[error("missing or invalid BodyLength (tag 9)")]
    InvalidBodyLength,
    /// The declared BodyLength does not match the actual body size.
    #[error("BodyLength mismatch: declared {declared}, actual {actual}")]
    BodyLengthMismatch {
        /// Value declared in tag 9.
        declared: usize,
        /// Actual measured body length.
        actual: usize,
    },
    /// CheckSum (tag 10) is missing or not a valid number.
    #[error("missing or invalid CheckSum (tag 10)")]
    MissingChecksum,
    /// The declared CheckSum does not match the computed one.
    #[error("CheckSum mismatch: declared {declared:03}, computed {computed:03}")]
    ChecksumMismatch {
        /// Value declared in tag 10.
        declared: u32,
        /// Locally computed checksum.
        computed: u32,
    },
    /// A field could not be parsed.
    #[error("garbled field near byte {offset}: {reason}")]
    GarbledField {
        /// Byte offset where the problem was detected.
        offset: usize,
        /// Human-readable reason.
        reason: &'static str,
    },
    /// The input ended before a field was complete.
    #[error("truncated input near byte {offset}")]
    Truncated {
        /// Byte offset where truncation was detected.
        offset: usize,
    },
    /// A tag number was not a valid positive integer.
    #[error("invalid tag number near byte {offset}")]
    InvalidTag {
        /// Byte offset of the offending tag.
        offset: usize,
    },
}

/// A session-level rejection returned from an admin/logon callback — the engine emits a Reject
/// (35=3). Typed, not stringly (Constitution Principle I; FR-016). `reason` is the numeric
/// SessionRejectReason (tag 373).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("reject (reason {reason}, ref tag {ref_tag:?}): {text:?}")]
pub struct Reject {
    /// SessionRejectReason (tag 373) numeric code.
    pub reason: u32,
    /// Optional referenced tag (RefTagID, 371).
    pub ref_tag: Option<u32>,
    /// Optional human-readable text (tag 58).
    pub text: Option<String>,
    /// Optional `SessionStatus` (tag 573) reason, propagated onto the outbound Logout when a
    /// `from_admin` callback refuses a Logon (US10, FR-013).
    pub session_status: Option<u16>,
}

/// Returned from an outbound callback to suppress a message: the engine does not send it and does
/// not store it as sent (it consumes no outbound sequence number). FR-016.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("do not send")]
pub struct DoNotSend;

/// A business-level rejection returned from an application callback — the engine emits a
/// BusinessMessageReject (35=j). `reason` is the numeric BusinessRejectReason (tag 380). FR-016.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("business reject (reason {reason}, ref tag {ref_tag:?}): {text:?}")]
pub struct BusinessReject {
    /// BusinessRejectReason (tag 380) numeric code.
    pub reason: u32,
    /// Optional referenced tag (RefTagID, 371).
    pub ref_tag: Option<u32>,
    /// Optional human-readable text (tag 58).
    pub text: Option<String>,
}
