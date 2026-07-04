//! Decode wire bytes into a [`Message`], verifying BodyLength (tag 9) and CheckSum (tag 10).
//!
//! Decoding produces flat ordered fields (no dictionary-driven group parsing at this layer).
//! Binary length-prefixed data fields (e.g. RawDataLength/RawData) are handled so embedded
//! SOH bytes do not corrupt parsing. No path panics.

use crate::error::DecodeError;
use crate::field::Field;
use crate::field_map::FieldMap;
use crate::group::{Group, GroupSpec};
use crate::message::Message;
use crate::tags::{
    data_field_for_length, is_header, is_trailer, BEGIN_STRING, BODY_LENGTH, CHECK_SUM, MSG_TYPE,
    SOH,
};

/// A tokenized field: tag, raw value bytes, and the byte offset where the field began.
type Token = (u32, Vec<u8>, usize);

/// Wire section a tag statically classifies into (header < body < trailer). Used only to detect
/// out-of-order sectioning (`ValidateFieldsOutOfOrder`; FR-006) — it does not affect where the
/// field actually lands (that's still governed by `is_header`/`is_trailer` at each call site).
fn section_of(tag: u32) -> u8 {
    if is_trailer(tag) {
        2
    } else if is_header(tag) {
        0
    } else {
        1
    }
}

/// Decode `input` into a [`Message`] with flat fields (no dictionary-driven group structure).
pub fn decode(input: &[u8]) -> Result<Message, DecodeError> {
    let fields = tokenize_validated(input)?;
    let mut msg = Message::new();
    let mut max_section_seen = 0u8;
    for (i, (tag, value, _)) in fields.iter().enumerate() {
        let (tag, value) = (*tag, value.clone());
        let section = section_of(tag);
        if section < max_section_seen {
            msg.fields_out_of_order = true;
        } else {
            max_section_seen = section;
        }
        if i == 2 && tag != MSG_TYPE {
            msg.fields_out_of_order = true;
        }
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

/// Decode `input` into a [`Message`] with repeating groups structured per `spec` (FR-004, and
/// header/trailer groups per US9, feature 005, FR-026). Header, body, and trailer are each
/// decoded through the same group-aware machinery (`decode_section_with_groups`) — a section with
/// no declared groups (today, header/trailer always; body when `spec` has none for it either)
/// decodes exactly as flat fields, since `spec.group_of(tag)` simply never matches. Structure is
/// best-effort/greedy — count/order validation is a separate dictionary concern (see
/// `truefix-dict`).
pub fn decode_with_groups(input: &[u8], spec: &dyn GroupSpec) -> Result<Message, DecodeError> {
    let fields = tokenize_validated(input)?;
    let mut msg = Message::new();
    let mut header: Vec<Token> = Vec::new();
    let mut body: Vec<Token> = Vec::new();
    let mut trailer: Vec<Token> = Vec::new();
    // GAP-26/FR-032 (feature 006): mirror `decode()`'s `fields_out_of_order`/
    // `ValidateFieldsOutOfOrder` tracking, which this function never performed at all — a
    // pre-existing gap in this otherwise-correct primitive, only surfaced once a production path
    // actually started calling it (`crates/truefix-transport`'s `classify_buffered`).
    let mut max_section_seen = 0u8;
    for (i, tok) in fields.iter().enumerate() {
        let tag = tok.0;
        let section = section_of(tag);
        if section < max_section_seen {
            msg.fields_out_of_order = true;
        } else {
            max_section_seen = section;
        }
        if i == 2 && tag != MSG_TYPE {
            msg.fields_out_of_order = true;
        }
    }
    for tok in fields {
        let tag = tok.0;
        if is_trailer(tag) {
            trailer.push(tok);
        } else if is_header(tag) {
            header.push(tok);
        } else {
            body.push(tok);
        }
    }
    decode_section_with_groups(&header, spec, &mut msg.header);
    decode_section_with_groups(&body, spec, &mut msg.body);
    decode_section_with_groups(&trailer, spec, &mut msg.trailer);
    Ok(msg)
}

/// Decode one wire section's (header/body/trailer) tokens into `out`, consuming a delimiter-led
/// repeating group wherever `spec.group_of` matches a token's tag (nested groups recurse via
/// `build_group`); every other token becomes a plain field.
fn decode_section_with_groups(tokens: &[Token], spec: &dyn GroupSpec, out: &mut FieldMap) {
    let mut pos = 0usize;
    while let Some(tok) = tokens.get(pos) {
        let tag = tok.0;
        if let Some((delimiter, members)) = spec.group_of(tag) {
            let group = build_group(tokens, &mut pos, spec, tag, delimiter, members);
            out.add_group(group);
        } else {
            out.add_field(Field::new(tag, tok.1.clone()));
            pos += 1;
        }
    }
}

/// Consume a repeating group starting at the count token, returning the structured [`Group`].
fn build_group(
    tokens: &[Token],
    pos: &mut usize,
    spec: &dyn GroupSpec,
    count_tag: u32,
    delimiter: u32,
    members: &[u32],
) -> Group {
    *pos += 1; // consume the NoXxx count field
    let mut group = Group::new(count_tag);
    while let Some(tok) = tokens.get(*pos) {
        if tok.0 != delimiter {
            break; // no more entries
        }
        let mut entry = FieldMap::new();
        entry.add_field(Field::new(delimiter, tok.1.clone()));
        *pos += 1;
        while let Some(t) = tokens.get(*pos) {
            let tag = t.0;
            if tag == delimiter || !members.contains(&tag) {
                break;
            }
            if let Some((d2, m2)) = spec.group_of(tag) {
                let sub = build_group(tokens, pos, spec, tag, d2, m2);
                entry.add_group(sub);
            } else {
                entry.add_field(Field::new(tag, t.1.clone()));
                *pos += 1;
            }
        }
        group.add_entry(entry);
    }
    group
}

/// Tokenize `input` and verify BeginString/BodyLength/CheckSum.
fn tokenize_validated(input: &[u8]) -> Result<Vec<Token>, DecodeError> {
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

    // BUG-80/FR-049 (feature 007): MsgType (tag 35) must be present somewhere in the message --
    // previously unchecked entirely, so a message missing it (e.g. a near-empty frame containing
    // only BeginString/BodyLength/CheckSum) was accepted. Checked by presence, not position: a
    // MsgType present but out of its normal third-field position is a separate, already-handled
    // concern (`fields_out_of_order`/`ValidateFieldsOutOfOrder`, FR-006) -- not a basis for
    // rejecting outright here.
    if !fields.iter().any(|f| f.0 == MSG_TYPE) {
        return Err(DecodeError::MissingMsgType);
    }

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
    // BUG-24/FR-032 (feature 007): same `u64`-accumulator fix as `encode.rs`'s mirror-image
    // checksum sum — see its comment for the overflow/panic rationale. `frame_length`'s
    // `MAX_BODY_LEN` cap makes this unreachable via the normal transport path, but `Message::decode`
    // is also a public API callable directly with arbitrary bytes, bypassing that cap entirely.
    let computed: u32 = (pre.iter().map(|&b| u64::from(b)).sum::<u64>() & 0xFF) as u32;
    if computed != declared_cs {
        return Err(DecodeError::ChecksumMismatch {
            declared: declared_cs,
            computed,
        });
    }

    Ok(fields)
}

/// Split `input` into `tag=value<SOH>` tokens, honoring length-prefixed binary data fields.
fn tokenize(input: &[u8]) -> Result<Vec<Token>, DecodeError> {
    let mut tokens = Vec::new();
    let mut pos = 0usize;
    // When the previous field was a length field (e.g. RawDataLength/95), this holds its declared
    // byte length together with the one tag (e.g. RawData/96) it's actually allowed to apply to
    // (BUG-38/FR-020, feature 007) — a length only ever governs its own documented data-tag
    // partner, never whatever tag happens to appear next. Applying it unconditionally to any
    // following tag let a phantom length silently swallow an embedded SOH byte and an entire
    // subsequent field into the wrong tag's "value", with the swallowed tag vanishing from the
    // decoded message and no error raised at all.
    let mut pending_data: Option<(u32, usize)> = None;

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

        let data_len = match pending_data.take() {
            Some((expected_tag, len)) if expected_tag == tag => Some(len),
            Some(_) => {
                // BUG-38/FR-020: the length field's documented partner never showed up -- the tag
                // that actually followed is something else entirely, a malformed/adversarial
                // message.
                return Err(DecodeError::GarbledField {
                    offset: start,
                    reason: "data field does not match its declared length field's partner tag",
                });
            }
            None => None,
        };

        let (value, next) = if let Some(len) = data_len {
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

        if let Some(expected_tag) = data_field_for_length(tag) {
            // BUG-49/FR-020 (feature 007): a non-numeric declared length is itself malformed --
            // silently leaving `pending_data` unset (as before) let the following data field be
            // misparsed as an ordinary SOH-delimited string field with no error at all.
            let len = parse_usize(&value).ok_or(DecodeError::GarbledField {
                offset: start,
                reason: "data-length field value is not a valid non-negative integer",
            })?;
            pending_data = Some((expected_tag, len));
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
