//! Conversion helpers between binary codecs and `truefix_core::Message`.

use crate::BinaryCodec;

/// Decode `bytes` into a TrueFix message using `codec`.
pub fn decode_into_message<C: BinaryCodec>(
    codec: &C,
    bytes: &[u8],
) -> Result<truefix_core::Message, C::Error> {
    codec.decode(bytes).map(|(message, _)| message)
}

/// Encode a TrueFix message using `codec` and `template_id`.
pub fn encode_from_message<C: BinaryCodec>(
    codec: &C,
    message: &truefix_core::Message,
    template_id: u32,
) -> Result<Vec<u8>, C::Error> {
    codec.encode(message, template_id)
}
