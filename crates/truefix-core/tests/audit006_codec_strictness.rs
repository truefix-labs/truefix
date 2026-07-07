//! Shared fixtures for audit 006 codec strictness tests.
//!
//! NEW-101: `Field::as_int`/`as_decimal` reject whitespace-padded values.
//! NEW-107: `frame_length` verifies the SOH byte after the checksum trailer.
//! NEW-155: `Message::decode` requires the checksum to be exactly three ASCII digits.

use truefix_core::{Field, Message, frame_length};

fn with_soh(fields: &[&str]) -> Vec<u8> {
    let mut out = fields.join("\x01").into_bytes();
    out.push(1);
    out
}

#[test]
fn audit006_codec_fixture_joins_fields_with_soh() {
    assert_eq!(
        with_soh(&["8=FIX.4.4", "10=000"]),
        b"8=FIX.4.4\x0110=000\x01"
    );
}

// --- NEW-101 ---

#[test]
fn audit006_as_int_rejects_whitespace_padded_value() {
    let f = Field::string(34, " 123 ");
    assert!(f.as_int().is_err());
}

#[test]
fn audit006_as_int_accepts_unpadded_value() {
    let f = Field::string(34, "123");
    assert_eq!(f.as_int().unwrap(), 123);
}

#[test]
fn audit006_as_decimal_rejects_whitespace_padded_value() {
    let f = Field::string(44, " 1.23 ");
    assert!(f.as_decimal().is_err());
}

#[test]
fn audit006_as_decimal_accepts_unpadded_value() {
    let f = Field::string(44, "1.23");
    assert!(f.as_decimal().is_ok());
}

// --- NEW-107 ---

fn valid_message() -> Vec<u8> {
    // 8=FIX.4.4|9=5|35=0|10=NNN|  (body length covers just "35=0", 5 bytes)
    let body = b"35=0\x01";
    let mut msg = Vec::new();
    msg.extend_from_slice(b"8=FIX.4.4\x01");
    msg.extend_from_slice(format!("9={}\x01", body.len()).as_bytes());
    msg.extend_from_slice(body);
    let sum: u32 = msg.iter().map(|&b| u32::from(b)).sum::<u32>() & 0xFF;
    msg.extend_from_slice(format!("10={sum:03}\x01").as_bytes());
    msg
}

#[test]
fn audit006_frame_length_accepts_a_well_formed_trailer() {
    let msg = valid_message();
    assert_eq!(frame_length(&msg).unwrap(), Some(msg.len()));
}

#[test]
fn audit006_frame_length_rejects_a_trailer_missing_the_final_soh() {
    let mut msg = valid_message();
    let last = msg.len() - 1;
    msg[last] = b'X'; // checksum trailer's terminating SOH replaced with a non-SOH byte
    assert!(frame_length(&msg).is_err());
}

// --- NEW-155 ---

#[test]
fn audit006_decode_rejects_a_checksum_that_is_not_three_digits() {
    let body = b"35=0\x01";
    let mut msg = Vec::new();
    msg.extend_from_slice(b"8=FIX.4.4\x01");
    msg.extend_from_slice(format!("9={}\x01", body.len()).as_bytes());
    msg.extend_from_slice(body);
    // A checksum that "parses as an integer" but isn't exactly three digits.
    msg.extend_from_slice(b"10=7\x01");
    assert!(Message::decode(&msg).is_err());
}

#[test]
fn audit006_decode_accepts_a_three_digit_checksum() {
    let msg = valid_message();
    assert!(Message::decode(&msg).is_ok());
}
