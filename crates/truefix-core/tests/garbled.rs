//! T011 — negative codec tests: typed errors, never panic (FR-B8, SC-005).

use truefix_core::{DecodeError, decode};

#[test]
fn empty_input() {
    assert_eq!(decode(b""), Err(DecodeError::Empty));
}

#[test]
fn missing_begin_string() {
    assert_eq!(decode(b"1=2\x01"), Err(DecodeError::MissingBeginString));
}

#[test]
fn invalid_tag() {
    assert!(matches!(
        decode(b"8=FIX.4.2\x01XY=1\x01"),
        Err(DecodeError::InvalidTag { .. })
    ));
}

#[test]
fn checksum_mismatch() {
    let bad = b"8=FIX.4.4\x019=21\x0135=A\x0195=7\x0196=rawdata\x0110=000\x01";
    assert!(matches!(
        decode(bad),
        Err(DecodeError::ChecksumMismatch { .. })
    ));
}

#[test]
fn body_length_mismatch() {
    let bad = b"8=FIX.4.4\x019=20\x0135=A\x0195=7\x0196=rawdata\x0110=086\x01";
    assert!(matches!(
        decode(bad),
        Err(DecodeError::BodyLengthMismatch { .. })
    ));
}

#[test]
fn truncated_field_without_soh() {
    assert!(decode(b"8=FIX.4.2\x0135=A").is_err());
}

#[test]
fn missing_separator() {
    assert!(matches!(
        decode(b"8FIX\x01"),
        Err(DecodeError::GarbledField { .. })
    ));
}

#[test]
fn never_panics_on_arbitrary_bytes() {
    for b in 0u8..=255 {
        let _ = decode(&[b]);
        let _ = decode(&[8, b'=', b, 0x01]);
        let _ = decode(&[8, b'=', b'F', 0x01, b, b'=', b'1', 0x01]);
    }
}
