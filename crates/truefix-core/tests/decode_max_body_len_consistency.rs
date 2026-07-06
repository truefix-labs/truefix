use truefix_core::{DecodeError, MAX_BODY_LEN, decode};

/// T168/T169 (feature 009, NEW-04): `decode` (called directly, bypassing `frame_length`'s own
/// `MAX_BODY_LEN` guard entirely) must reject a declared BodyLength beyond `MAX_BODY_LEN` too,
/// not just when reached through the transport read loop.
#[test]
fn decode_rejects_a_declared_body_length_beyond_max_body_len_even_bypassing_frame_length() {
    let declared = MAX_BODY_LEN + 1;
    let raw = format!("8=FIX.4.4\x019={declared}\x0135=0\x0110=000\x01");
    let err =
        decode(raw.as_bytes()).expect_err("a BodyLength beyond MAX_BODY_LEN must be rejected");
    match err {
        DecodeError::BodyLengthTooLarge { declared: got, max } => {
            assert_eq!(got, declared);
            assert_eq!(max, MAX_BODY_LEN);
        }
        other => panic!("expected BodyLengthTooLarge, got {other:?}"),
    }
}

/// Control: an ordinary, well-formed message with a BodyLength well under the cap still decodes.
#[test]
fn decode_still_accepts_a_normal_body_length() {
    let mut m = truefix_core::Message::new();
    m.header.set(truefix_core::Field::string(8, "FIX.4.4"));
    m.header.set(truefix_core::Field::string(35, "0"));
    let bytes = m.encode();
    decode(&bytes).expect("a normal, well-formed message should still decode");
}
