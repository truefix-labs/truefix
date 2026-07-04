//! T010 — round-trip + byte-exact BodyLength/CheckSum.
//!
//! `V_RAW` is a known-good wire vector (FIX-spec-determined data, BodyLength 21, CheckSum 086)
//! used as an external anchor. The QuickFIX/J cross-vectors (T099) are a later supplement.

use truefix_core::{Field, Message, decode, encode};

const V_RAW: &[u8] = b"8=FIX.4.4\x019=21\x0135=A\x0195=7\x0196=rawdata\x0110=086\x01";

#[test]
fn decodes_known_rawdata_vector() {
    let m = decode(V_RAW).unwrap();
    assert_eq!(m.begin_string(), Some("FIX.4.4"));
    assert_eq!(m.msg_type(), Some("A"));
    // length-prefixed binary data field preserved exactly
    assert_eq!(m.body.get(96).unwrap().value_bytes(), b"rawdata");
}

#[test]
fn rawdata_vector_roundtrips_byte_identical() {
    let m = decode(V_RAW).unwrap();
    assert_eq!(encode(&m), V_RAW);
}

#[test]
fn bodylength_and_checksum_recomputed_correctly() {
    let m = decode(V_RAW).unwrap();
    let s = String::from_utf8(encode(&m)).unwrap();
    assert!(s.contains("9=21\u{1}"), "{s:?}");
    assert!(s.ends_with("10=086\u{1}"), "{s:?}");
}

#[test]
fn programmatic_newordersingle_roundtrips() {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.2"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "SENDER"));
    m.header.set(Field::string(56, "TARGET"));
    m.header.set(Field::string(52, "20240101-12:00:00"));
    m.body.set(Field::string(11, "ORDER1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::int(54, 1));
    m.body.set(Field::int(38, 100));
    m.body.set(Field::string(40, "2"));
    m.body.set(Field::string(44, "150.25"));

    let bytes = encode(&m);
    let decoded = decode(&bytes).unwrap();

    // encode->decode->encode is byte-stable
    assert_eq!(encode(&decoded), bytes);
    // typed access survives the round-trip
    assert_eq!(decoded.body.get(55).unwrap().as_str().unwrap(), "AAPL");
    assert_eq!(
        decoded
            .body
            .get(44)
            .unwrap()
            .as_decimal()
            .unwrap()
            .to_string(),
        "150.25"
    );
    // header was classified correctly
    assert_eq!(decoded.header.get(49).unwrap().as_str().unwrap(), "SENDER");
}
