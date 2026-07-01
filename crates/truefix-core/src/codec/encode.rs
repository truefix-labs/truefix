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
    render_members(&msg.body, &[], &mut middle);
    render_members(&msg.trailer, &[CHECK_SUM], &mut middle);

    let mut out = Vec::new();
    render_raw(BEGIN_STRING, &begin, &mut out);
    render_raw(BODY_LENGTH, middle.len().to_string().as_bytes(), &mut out);
    out.extend_from_slice(&middle);

    let checksum: u32 = out.iter().map(|&b| u32::from(b)).sum::<u32>() & 0xFF;
    render_raw(CHECK_SUM, format!("{checksum:03}").as_bytes(), &mut out);
    out
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
