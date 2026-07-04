//! T037 (US1, feature 007): `frame_length` rejects a declared `BodyLength=0` as malformed
//! (BUG-100/FR-014) — a zero-length body previously framed and decoded as a valid (if empty)
//! message, unlike QuickFIX/J (QFJ-903) and QuickFIX/Go, which both reject it.

use truefix_core::{frame_length, DecodeError};

#[test]
fn a_declared_body_length_of_zero_is_rejected() {
    let buf = b"8=FIX.4.4\x019=0\x0110=000\x01";
    let err = frame_length(buf).expect_err("BodyLength=0 must be rejected, not framed");
    assert!(
        matches!(err, DecodeError::ZeroBodyLength),
        "expected ZeroBodyLength, got {err:?}"
    );
}

#[test]
fn a_declared_positive_body_length_still_frames_normally() {
    let buf = b"8=FIX.4.4\x019=5\x0135=0\x0110=000\x01";
    let result = frame_length(buf).expect("a positive BodyLength must still frame");
    assert_eq!(result, Some(buf.len()));
}
