use thiserror::Error;

/// Result type used by the TWS API client crate.
pub type TwsApiResult<T> = Result<T, TwsApiError>;

/// Errors produced while encoding, decoding, or transporting TWS API frames.
#[derive(Debug, Error)]
pub enum TwsApiError {
    /// A caller attempted to encode a value containing non-printable ASCII.
    #[error("TWS API fields must be printable ASCII: {0:?}")]
    NonPrintableAscii(String),
    /// A caller attempted to send a missing field value.
    #[error("cannot encode a missing TWS API field")]
    MissingField,
    /// A frame length prefix exceeded the supported platform size.
    #[error("TWS API frame length {0} is too large for this platform")]
    FrameTooLarge(u32),
    /// A message was incomplete.
    #[error("incomplete TWS API frame: need {needed} bytes, have {available} bytes")]
    IncompleteFrame {
        /// Number of bytes needed to finish the frame.
        needed: usize,
        /// Number of bytes currently available.
        available: usize,
    },
    /// The peer returned a malformed handshake response.
    #[error("malformed TWS API handshake response")]
    MalformedHandshake,
    /// The peer advertised an unsupported server version.
    #[error("TWS/Gateway server version {server_version} is below required minimum {min_version}")]
    UnsupportedServerVersion {
        /// Advertised server version.
        server_version: i32,
        /// Required minimum version.
        min_version: i32,
    },
    /// A request used a field that the negotiated server version does not support.
    #[error(
        "TWS/Gateway server version {server_version} is below required minimum {min_version} for {parameter}"
    )]
    UnsupportedRequestParameter {
        /// Advertised server version.
        server_version: i32,
        /// Required minimum version.
        min_version: i32,
        /// Unsupported parameter name.
        parameter: &'static str,
    },
    /// A network operation failed.
    #[error("TWS API I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// An integer field could not be parsed.
    #[error("invalid TWS API integer field {field:?}: {source}")]
    InvalidInteger {
        /// Field content.
        field: String,
        /// Parse failure.
        source: std::num::ParseIntError,
    },
    /// A decimal field could not be parsed.
    #[error("invalid TWS API decimal field {field:?}: {source}")]
    InvalidDecimal {
        /// Field content.
        field: String,
        /// Parse failure.
        source: rust_decimal::Error,
    },
    /// A protobuf payload could not be decoded.
    #[error("invalid TWS API protobuf payload: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),
}
