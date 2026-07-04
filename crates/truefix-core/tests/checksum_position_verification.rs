//! T107/T108 (US3, feature 007): `frame_length` confirms the bytes at the computed checksum
//! position actually resemble a `10=` checksum field before trusting `total` (BUG-46, FR-047) —
//! previously a declared `BodyLength` that was numerically valid (and within `MAX_BODY_LEN`) but
//! didn't match the message's real length would still make `frame_length` compute a `total`
//! pointing at the wrong offset, and the caller would drain the wrong number of bytes and
//! desynchronize the stream permanently, instead of failing this one frame and resynchronizing
//! (matching QFJ's `FIXMessageDecoder`, which verifies `10=???<SOH>` at the expected position).

use truefix_core::framing::frame_length;

/// Build a well-formed frame, then let the caller corrupt `BodyLength` afterward.
fn frame(body: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"8=FIX.4.4\x01");
    buf.extend_from_slice(format!("9={}\x01", body.len()).as_bytes());
    buf.extend_from_slice(body);
    buf.extend_from_slice(b"10=000\x01");
    buf
}

#[test]
fn a_correct_body_length_frames_normally() {
    let buf = frame(b"35=0\x0134=1\x0149=A\x0156=B\x0152=20240101-00:00:00\x01");
    assert!(matches!(frame_length(&buf), Ok(Some(_))));
}

#[test]
fn a_body_length_declaring_too_few_bytes_is_rejected_not_misframed() {
    // The real body is 30 bytes; declare 20 instead. `total` then lands mid-field, where the
    // bytes are not `10=` -- this must be rejected (triggering resync one layer up), not
    // accepted as a (silently wrong) frame boundary.
    let body = b"35=0\x0134=1\x0149=A\x0156=B\x0152=20240101-00:00:00\x01";
    let mut buf = Vec::new();
    buf.extend_from_slice(b"8=FIX.4.4\x01");
    buf.extend_from_slice(format!("9={}\x01", body.len() - 10).as_bytes()); // wrong, too small
    buf.extend_from_slice(body);
    buf.extend_from_slice(b"10=000\x01");
    assert!(
        frame_length(&buf).is_err(),
        "a BodyLength that doesn't land on a real `10=` field must be rejected"
    );
}

#[test]
fn a_body_length_declaring_too_many_bytes_waits_for_more_data_or_is_rejected() {
    // Declare more bytes than are actually present in the body (but the buffer as a whole still
    // has *enough total bytes* for `total`, landing partway into what would be trailing garbage
    // rather than a real `10=` field).
    let body = b"35=0\x0134=1\x0149=A\x0156=B\x0152=20240101-00:00:00\x01";
    let mut buf = Vec::new();
    buf.extend_from_slice(b"8=FIX.4.4\x01");
    buf.extend_from_slice(format!("9={}\x01", body.len() + 10).as_bytes()); // wrong, too large
    buf.extend_from_slice(body);
    buf.extend_from_slice(b"10=000\x01");
    buf.extend_from_slice(b"garbagepadding"); // enough trailing bytes that `total` is reachable
    assert!(
        frame_length(&buf).is_err(),
        "a BodyLength landing on non-`10=` bytes must be rejected, not misframed"
    );
}
