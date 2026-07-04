//! Encode a [`Message`] to wire bytes, computing BodyLength (tag 9) and CheckSum (tag 10).

use crate::field_map::{FieldMap, Member};
use crate::message::Message;
use crate::tags::{BEGIN_STRING, BODY_LENGTH, CHECK_SUM, MSG_TYPE, SOH};

/// Encode `msg` to SOH-delimited wire bytes.
///
/// Emits the canonical order `8, 9, 35, <rest of header>, <body>, <trailer except 10>, 10`.
/// BodyLength is the byte count of everything after the `9=..<SOH>` up to and including the
/// `<SOH>` before `10=`; CheckSum is the sum of all preceding bytes modulo 256.
pub fn encode(msg: &Message) -> Vec<u8> {
    encode_with_order(msg, None)
}

/// As [`encode`], but when `field_order` is `Some`, the message body's top-level fields are
/// emitted in that tag order instead of insertion order (US9, feature 005, FR-027) — fields
/// present in `field_order` come first (in that order), then any body field not listed in it
/// (e.g. a UDF), in its original insertion-relative order, matching QFJ's own
/// `FieldOrderComparator` "unspecified fields last" semantics. Repeating-group entries are
/// unaffected (each entry's own field order is unconditionally preserved) — `field_order` only
/// reorders the message body's direct top-level members.
pub fn encode_with_order(msg: &Message, field_order: Option<&[u32]>) -> Vec<u8> {
    let begin = msg
        .header
        .get(BEGIN_STRING)
        .map(|f| f.value_bytes().to_vec())
        .unwrap_or_default();
    let msg_type = msg
        .header
        .get(MSG_TYPE)
        .map(|f| f.value_bytes().to_vec())
        .unwrap_or_default();

    // Everything counted by BodyLength: MsgType, the rest of the header, the body, and the
    // trailer (excluding CheckSum).
    let mut middle = Vec::new();
    render_raw(MSG_TYPE, &msg_type, &mut middle);
    render_members(
        &msg.header,
        &[BEGIN_STRING, BODY_LENGTH, MSG_TYPE],
        &mut middle,
    );
    match field_order {
        Some(order) => render_members_ordered(&msg.body, order, &mut middle),
        None => render_members(&msg.body, &[], &mut middle),
    }
    render_members(&msg.trailer, &[CHECK_SUM], &mut middle);

    let mut out = Vec::new();
    render_raw(BEGIN_STRING, &begin, &mut out);
    render_raw(BODY_LENGTH, middle.len().to_string().as_bytes(), &mut out);
    out.extend_from_slice(&middle);

    // BUG-24/FR-032 (feature 007): a `u64` accumulator, not `u32` — `Iterator::sum::<u32>()` panics
    // on overflow in debug builds (violating the crate's "no path panics" invariant) once the
    // summed bytes exceed ~16.8M in a way whose sum surpasses `u32::MAX` (reachable: `encode()` has
    // no `MAX_BODY_LEN`-style cap of its own, since that limit is enforced only on the *decode*
    // path in `frame_length`). `u64` can't realistically overflow this sum (would need billions of
    // bytes), and `& 0xFF` gives the identical mod-256 result either width, since 2^32 and 2^64 are
    // both multiples of 256.
    let checksum: u32 = (out.iter().map(|&b| u64::from(b)).sum::<u64>() & 0xFF) as u32;
    render_raw(CHECK_SUM, format!("{checksum:03}").as_bytes(), &mut out);
    out
}

/// Render `map`'s top-level members ordered by `order` (fields listed in `order` first, in that
/// order; then any remaining top-level member — a field not in `order`, or a group — in its
/// original relative position among the remaining members). Each group entry's own internal
/// field order is untouched.
fn render_members_ordered(map: &FieldMap, order: &[u32], out: &mut Vec<u8>) {
    let members: &[Member] = map.members();
    let tag_of = |m: &Member| match m {
        Member::Field(f) => Some(f.tag()),
        Member::Group { count_tag, .. } => Some(*count_tag),
    };
    for &wanted in order {
        for member in members {
            if tag_of(member) == Some(wanted) {
                render_one_member(member, out);
            }
        }
    }
    for member in members {
        if !order.contains(&tag_of(member).unwrap_or(0)) {
            render_one_member(member, out);
        }
    }
}

fn render_one_member(member: &Member, out: &mut Vec<u8>) {
    match member {
        Member::Field(f) => render_raw(f.tag(), f.value_bytes(), out),
        Member::Group { count_tag, entries } => {
            render_raw(*count_tag, entries.len().to_string().as_bytes(), out);
            for entry in entries {
                render_members(entry, &[], out);
            }
        }
    }
}

fn render_members(map: &FieldMap, skip: &[u32], out: &mut Vec<u8>) {
    for member in map.members() {
        match member {
            Member::Field(f) => {
                if !skip.contains(&f.tag()) {
                    render_raw(f.tag(), f.value_bytes(), out);
                }
            }
            Member::Group { count_tag, entries } => {
                render_raw(*count_tag, entries.len().to_string().as_bytes(), out);
                for entry in entries {
                    render_members(entry, &[], out);
                }
            }
        }
    }
}

fn render_raw(tag: u32, value: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(tag.to_string().as_bytes());
    out.push(b'=');
    out.extend_from_slice(value);
    out.push(SOH);
}
