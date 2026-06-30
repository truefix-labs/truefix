//! T072 — per-version codec round-trip + BodyLength/CheckSum.
//!
//! The codec is version-agnostic (FIX framing is identical across versions), so the same
//! encode/decode handles every BeginString. FIX 5.0/5.0SP1/5.0SP2 ride on the FIXT.1.1 wire
//! BeginString; the application version is carried by ApplVerID, not BeginString.

use truefix_core::{decode, encode, Field, Message};

const WIRE_BEGIN_STRINGS: &[&str] = &[
    "FIX.4.0", "FIX.4.1", "FIX.4.2", "FIX.4.3", "FIX.4.4", "FIXT.1.1",
];

fn logon(begin: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "SENDER"));
    m.header.set(Field::string(56, "TARGET"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

#[test]
fn every_version_roundtrips_byte_identical() {
    for begin in WIRE_BEGIN_STRINGS {
        let m = logon(begin);
        let bytes = encode(&m);
        let decoded = decode(&bytes).unwrap();
        assert_eq!(encode(&decoded), bytes, "round-trip for {begin}");
        assert_eq!(decoded.begin_string(), Some(*begin));
        // BodyLength and CheckSum present and well-formed
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.starts_with(&format!("8={begin}\u{1}9=")));
        assert!(s.ends_with("\u{1}")); // terminated
        assert!(s.contains("\u{1}10="));
    }
}

#[test]
fn checksum_is_three_digits_each_version() {
    for begin in WIRE_BEGIN_STRINGS {
        let bytes = encode(&logon(begin));
        let s = String::from_utf8(bytes).unwrap();
        let cs = s.rsplit("\u{1}10=").next().unwrap_or("");
        // "NNN\u{1}"
        assert_eq!(cs.len(), 4, "checksum field for {begin}: {cs:?}");
        assert!(cs[..3].chars().all(|c| c.is_ascii_digit()));
    }
}
