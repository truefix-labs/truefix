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
    /// Set by [`Self::decode`] when the wire byte stream violated header/body/trailer sectioning
    /// (a field classified into an earlier section arrived after a later-section field had
    /// already been seen) or the third field wasn't MsgType(35) — i.e. `ValidateFieldsOutOfOrder`
    /// (FR-006). Always `false` for a `Message` built in code (assumed well-formed); the 3
    /// separate `FieldMap`s preserve *within-section* wire order but classify by static tag
    /// identity, so cross-section interleaving is only observable during decode itself.
    pub(crate) fields_out_of_order: bool,
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

    /// Whether decoding observed header/body/trailer fields out of their wire-sectioning order,
    /// or a third field other than MsgType(35) (`ValidateFieldsOutOfOrder`; FR-006). Always
    /// `false` for a `Message` built in code rather than decoded from the wire.
    pub fn fields_out_of_order(&self) -> bool {
        self.fields_out_of_order
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

    /// Copy `original`'s routing header fields onto `self` (a reply/reject being built),
    /// reversed: `original`'s `OnBehalfOfCompID(115)` becomes `self`'s `DeliverToCompID(128)` and
    /// vice versa; likewise `OnBehalfOfSubID(116)`/`DeliverToSubID(129)` and
    /// `OnBehalfOfLocationID(144)`/`DeliverToLocationID(145)` (Appendix B `ReverseRoute`).
    ///
    /// A routing tag absent on `original` is left unset on `self` — no reversal is attempted and
    /// no error is raised (`ReverseRouteWithEmptyRoutingTags`: a tag present with an empty value
    /// still reverses, since presence — not content — governs).
    pub fn reverse_route(&mut self, original: &Message) {
        for (on_behalf_of, deliver_to) in [(115u32, 128u32), (116, 129), (144, 145)] {
            if let Some(f) = original.header.get(on_behalf_of) {
                self.header.set(crate::field::Field::new(
                    deliver_to,
                    f.value_bytes().to_vec(),
                ));
            }
            if let Some(f) = original.header.get(deliver_to) {
                self.header.set(crate::field::Field::new(
                    on_behalf_of,
                    f.value_bytes().to_vec(),
                ));
            }
        }
    }
}
