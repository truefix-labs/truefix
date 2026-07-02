//! T018 (US3) — `Message::fields_out_of_order` wire-sectioning detection (FR-006).
//!
//! `decode()` classifies each tag into header/body/trailer by static tag identity (not wire
//! position), so cross-section interleaving is only observable *during* decode; this flag is how
//! that observation survives past decode for `truefix-dict::validate()` to act on.

use truefix_core::decode;

/// Build a well-formed frame from `rest` (every field after BeginString/BodyLength, ending just
/// before CheckSum), computing BodyLength and CheckSum correctly regardless of `rest`'s order —
/// so the *only* thing under test is field ordering, never framing/checksum validity.
fn raw_ordered(rest: &[(u32, &str)]) -> Vec<u8> {
    let body: String = rest.iter().map(|(t, v)| format!("{t}={v}\x01")).collect();
    let prefix = format!("8=FIX.4.4\x019={}\x01", body.len());
    let pre_checksum = format!("{prefix}{body}");
    let checksum: u32 = pre_checksum.bytes().map(u32::from).sum::<u32>() & 0xFF;
    format!("{pre_checksum}10={checksum:03}\x01").into_bytes()
}

#[test]
fn well_formed_message_is_not_out_of_order() {
    let raw = raw_ordered(&[
        (35, "D"),
        (49, "CLIENT"),
        (56, "SERVER"),
        (34, "1"),
        (52, "20240101-00:00:00"),
        (55, "AAPL"),
    ]);
    let msg = decode(&raw).expect("well-formed frame should decode");
    assert!(!msg.fields_out_of_order());
}

#[test]
fn third_field_not_msg_type_is_out_of_order() {
    // MsgType(35) must be the third field on the wire; here SenderCompID(49) usurps that slot.
    let raw = raw_ordered(&[
        (49, "CLIENT"),
        (35, "D"),
        (56, "SERVER"),
        (34, "1"),
        (52, "20240101-00:00:00"),
    ]);
    let msg = decode(&raw).expect("still a well-formed frame (framing doesn't care about tag 3)");
    assert!(msg.fields_out_of_order());
}

#[test]
fn body_field_before_header_is_complete_is_out_of_order() {
    // A body field (Symbol, 55) interleaved before a header field (SenderCompID, 49).
    let raw = raw_ordered(&[
        (35, "D"),
        (55, "AAPL"),   // body — sets max_section_seen to "body"
        (49, "CLIENT"), // header — arrives after body already started
        (56, "SERVER"),
        (34, "1"),
        (52, "20240101-00:00:00"),
    ]);
    let msg = decode(&raw).expect("well-formed frame");
    assert!(msg.fields_out_of_order());
}

#[test]
fn header_and_body_fields_ordered_differently_is_out_of_order() {
    // 15_HeaderAndBodyFieldsOrderedDifferently — header/body fields interleaved throughout,
    // rather than cleanly sectioned.
    let raw = raw_ordered(&[
        (35, "D"),
        (49, "CLIENT"),
        (55, "AAPL"),   // body field
        (56, "SERVER"), // header field, after a body field already appeared
        (34, "1"),
        (52, "20240101-00:00:00"),
    ]);
    let msg = decode(&raw).expect("well-formed frame");
    assert!(msg.fields_out_of_order());
}

#[test]
fn manually_constructed_message_defaults_to_not_out_of_order() {
    // A Message built in code (not decoded) has no wire to be out of order on.
    assert!(!truefix_core::Message::new().fields_out_of_order());
}
