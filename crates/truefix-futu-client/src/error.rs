use thiserror::Error;

/// Result type used by the Futu client crate.
pub type FutuResult<T> = Result<T, FutuError>;

/// Errors produced while encoding, decoding, transporting, or communicating
/// with the Futu OpenD daemon.
#[derive(Debug, Error)]
pub enum FutuError {
    /// A network I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Frame header does not start with the expected `b"FT"` magic bytes.
    #[error("frame magic mismatch: got {0:?}")]
    BadMagic([u8; 2]),

    /// Decrypted body SHA1 doesn't match the value in the frame header.
    #[error("frame body SHA1 mismatch")]
    Sha1Mismatch,

    /// Encryption or decryption failed.
    #[error("encryption error: {0}")]
    Crypto(String),

    /// Protobuf deserialization failed.
    #[error("protobuf decode error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),

    /// OpenD returned a non-zero `retType` in its response.
    #[error("OpenD error: retType={ret_type}, msg={ret_msg:?}")]
    OpenDError {
        /// The non-zero return code from OpenD.
        ret_type: i32,
        /// Optional human-readable message from OpenD.
        ret_msg: Option<String>,
    },

    /// A request timed out waiting for an OpenD response.
    #[error("request timed out after {timeout_ms}ms")]
    Timeout {
        /// The configured timeout in milliseconds.
        timeout_ms: u64,
    },

    /// The `serial_no` u32 counter overflowed.
    #[error("serial number overflow")]
    SerialOverflow,

    /// The `ConnectionActor` task has shut down; the command channel is closed.
    #[error("actor shut down")]
    ActorGone,
}

impl From<tokio::sync::oneshot::error::RecvError> for FutuError {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        FutuError::ActorGone
    }
}
