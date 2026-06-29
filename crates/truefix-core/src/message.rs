//! A FIX message: header, body, and trailer field maps.

use crate::error::DecodeError;
use crate::field_map::FieldMap;
use crate::tags::{BEGIN_STRING, MSG_TYPE};

/// A FIX message split into header, body, and trailer regions.
///
/// Decoding routes session-level header tags and trailer tags to their regions; everything
/// else lands in the body. Encoding always emits the canonical order
/// `8, 9, 35, <rest of header>, <body>, <trailer except 10>, 10`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    /// Header fields (BeginString, BodyLength, MsgType, session fields, ...).
    pub header: FieldMap,
    /// Application/body fields.
    pub body: FieldMap,
    /// Trailer fields (SignatureLength, Signature, CheckSum).
    pub trailer: FieldMap,
}

impl Message {
    /// Create an empty message.
    pub fn new() -> Self {
        Self::default()
    }

    /// The BeginString (tag 8) as a string, if present and valid UTF-8.
    pub fn begin_string(&self) -> Option<&str> {
        self.header.get(BEGIN_STRING).and_then(|f| f.as_str().ok())
    }

    /// The MsgType (tag 35) as a string, if present and valid UTF-8.
    pub fn msg_type(&self) -> Option<&str> {
        self.header.get(MSG_TYPE).and_then(|f| f.as_str().ok())
    }

    /// Encode to wire bytes (computes BodyLength and CheckSum).
    pub fn encode(&self) -> Vec<u8> {
        crate::codec::encode(self)
    }

    /// Decode from wire bytes (verifies BodyLength and CheckSum).
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        crate::codec::decode(bytes)
    }
}
