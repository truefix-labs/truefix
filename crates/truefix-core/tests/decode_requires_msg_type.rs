//! T111/T112 (US3, feature 007): a decoded message with no `MsgType` (tag 35) field anywhere is
//! rejected (BUG-80, FR-049) — previously `tokenize_validated` only required BeginString(8)/
//! BodyLength(9)/CheckSum(10), so a message missing MsgType entirely (e.g. `8=FIX.4.4\x019=0\x01
//! 10=086\x01`) was accepted. Checked by *presence*, not *position*: a MsgType present but not at
//! its normal third-field slot is a separate, already-handled concern (`fields_out_of_order`) and
//! must still decode successfully (regression guard).

use truefix_core::decode;

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

/// Build a well-formed frame from `middle` (everything between `9=<len>\x01` and the checksum
/// trailer), computing BodyLength/CheckSum correctly.
fn frame(middle: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"8=FIX.4.4\x01");
    out.extend_from_slice(format!("9={}\x01", middle.len()).as_bytes());
    out.extend_from_slice(middle);
    let cs = checksum(&out);
    out.extend_from_slice(format!("10={cs:03}\x01").as_bytes());
    out
}

#[test]
fn a_message_with_no_msg_type_anywhere_is_rejected() {
    let raw = frame(b"49=CLIENT\x0156=SERVER\x0134=1\x0152=20240101-00:00:00\x01");
    assert!(
        decode(&raw).is_err(),
        "a message with no MsgType(35) field at all must be rejected"
    );
}

#[test]
fn a_well_formed_message_with_msg_type_still_decodes() {
    let raw = frame(b"35=0\x0149=CLIENT\x0156=SERVER\x0134=1\x0152=20240101-00:00:00\x01");
    assert!(decode(&raw).is_ok());
}

#[test]
fn a_msg_type_present_but_out_of_its_normal_position_still_decodes() {
    // MsgType(35) present, but not the third field (SenderCompID usurps that slot) -- this is a
    // pre-existing, separate `fields_out_of_order` concern, not grounds for outright rejection.
    let raw = frame(b"49=CLIENT\x0135=0\x0156=SERVER\x0134=1\x0152=20240101-00:00:00\x01");
    let msg = decode(&raw).expect("MsgType present (just out of position) must still decode");
    assert!(msg.fields_out_of_order());
}
