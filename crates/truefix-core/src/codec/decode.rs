//! Decode wire bytes into a [`Message`], verifying BodyLength (tag 9) and CheckSum (tag 10).
//!
//! Decoding produces flat ordered fields (no dictionary-driven group parsing at this layer).
//! Binary length-prefixed data fields (e.g. RawDataLength/RawData) are handled so embedded
//! SOH bytes do not corrupt parsing. No path panics.

use crate::error::DecodeError;
use crate::field::Field;
use crate::message::Message;
use crate::tags::{
    data_field_for_length, is_header, is_trailer, BEGIN_STRING, BODY_LENGTH, CHECK_SUM, SOH,
};

/// A tokenized field: tag, raw value bytes, and the byte offset where the field began.
type Token = (u32, Vec<u8>, usize);

/// Decode `input` into a [`Message`].
pub fn decode(input: &[u8]) -> Result<Message, DecodeError> {
    if input.is_empty() {
        return Err(DecodeError::Empty);
    }

    let fields = tokenize(input)?;

    let first = fields.first().ok_or(DecodeError::Empty)?;
    if first.0 != BEGIN_STRING {
        return Err(DecodeError::MissingBeginString);
    }
    let second = fields.get(1).ok_or(DecodeError::InvalidBodyLength)?;
    if second.0 != BODY_LENGTH {
        return Err(DecodeError::InvalidBodyLength);
    }
    let declared_bl = parse_usize(&second.1).ok_or(DecodeError::InvalidBodyLength)?;

    let last = fields.last().ok_or(DecodeError::MissingChecksum)?;
    if last.0 != CHECK_SUM {
        return Err(DecodeError::MissingChecksum);
    }
    let declared_cs = parse_u32(&last.1).ok_or(DecodeError::MissingChecksum)?;

    // Body starts at the third field (just after `9=..<SOH>`) and ends just before `10=`.
    let cs_offset = last.2;
    let body_start = fields.get(2).map_or(cs_offset, |f| f.2);
    let actual_bl = cs_offset
        .checked_sub(body_start)
        .ok_or(DecodeError::InvalidBodyLength)?;
    if actual_bl != declared_bl {
        return Err(DecodeError::BodyLengthMismatch {
            declared: declared_bl,
            actual: actual_bl,
        });
    }

    let pre = input.get(..cs_offset).ok_or(DecodeError::MissingChecksum)?;
    let computed: u32 = pre.iter().map(|&b| u32::from(b)).sum::<u32>() & 0xFF;
    if computed != declared_cs {
        return Err(DecodeError::ChecksumMismatch {
            declared: declared_cs,
            computed,
        });
    }

    let mut msg = Message::new();
    for (tag, value, _) in fields {
        let field = Field::new(tag, value);
        if is_trailer(tag) {
            msg.trailer.add_field(field);
        } else if is_header(tag) {
            msg.header.add_field(field);
        } else {
            msg.body.add_field(field);
        }
    }
    Ok(msg)
}

/// Split `input` into `tag=value<SOH>` tokens, honoring length-prefixed binary data fields.
fn tokenize(input: &[u8]) -> Result<Vec<Token>, DecodeError> {
    let mut tokens = Vec::new();
    let mut pos = 0usize;
    // When the previous field was a length field, this holds the exact byte length of the
    // next (data) field's value.
    let mut pending_data_len: Option<usize> = None;

    while pos < input.len() {
        let start = pos;
        let rest = input
            .get(pos..)
            .ok_or(DecodeError::Truncated { offset: start })?;

        let eq_rel = memchr(rest, b'=').ok_or(DecodeError::GarbledField {
            offset: start,
            reason: "missing '=' in field",
        })?;
        let tag_bytes = rest.get(..eq_rel).unwrap_or(&[]);
        let tag = parse_u32(tag_bytes).ok_or(DecodeError::InvalidTag { offset: start })?;
        let val_start = pos + eq_rel + 1;

        let (value, next) = if let Some(len) = pending_data_len.take() {
            let val_end = val_start
                .checked_add(len)
                .ok_or(DecodeError::GarbledField {
                    offset: start,
                    reason: "data length overflow",
                })?;
            let v = input
                .get(val_start..val_end)
                .ok_or(DecodeError::Truncated { offset: val_start })?;
            match input.get(val_end) {
                Some(&b) if b == SOH => (v.to_vec(), val_end + 1),
                _ => {
                    return Err(DecodeError::GarbledField {
                        offset: val_end,
                        reason: "data field not terminated by SOH",
                    })
                }
            }
        } else {
            let after = input
                .get(val_start..)
                .ok_or(DecodeError::Truncated { offset: val_start })?;
            let soh_rel = memchr(after, SOH).ok_or(DecodeError::GarbledField {
                offset: val_start,
                reason: "field not terminated by SOH",
            })?;
            let v = after.get(..soh_rel).unwrap_or(&[]);
            (v.to_vec(), val_start + soh_rel + 1)
        };

        if pending_data_len.is_none() && data_field_for_length(tag).is_some() {
            if let Some(len) = parse_usize(&value) {
                pending_data_len = Some(len);
            }
        }

        tokens.push((tag, value, start));
        pos = next;
    }

    Ok(tokens)
}

fn memchr(haystack: &[u8], needle: u8) -> Option<usize> {
    haystack.iter().position(|&b| b == needle)
}

fn parse_u32(bytes: &[u8]) -> Option<u32> {
    core::str::from_utf8(bytes).ok()?.parse().ok()
}

fn parse_usize(bytes: &[u8]) -> Option<usize> {
    core::str::from_utf8(bytes).ok()?.parse().ok()
}
