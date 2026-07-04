//! T004 (US1, feature 005) — `Signature`(89)/`SignatureLength`(93) length-field mapping (BUG-02).
//!
//! `data_field_for_length` follows the `lengthTag = dataTag - 1` convention for every pair except
//! tag 89, whose length field is 93 (not 88) — QuickFIX/J's own documented exception
//! (`Message.java:949-952`). Before the fix, TrueFix's decoder had no entry for `93 => 89`, so a
//! `Signature` value containing an embedded SOH byte would be mis-tokenized (split at the first SOH
//! instead of consuming exactly `SignatureLength` bytes).

use truefix_core::{Field, Message, decode, encode};

#[test]
fn signature_with_embedded_soh_round_trips_using_signature_length() {
    let signature: &[u8] = b"sig\x01with\x01embedded\x01soh";

    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "SENDER"));
    m.header.set(Field::string(56, "TARGET"));
    m.header.set(Field::string(52, "20240101-12:00:00"));
    m.body.set(Field::string(98, "0"));
    m.body.set(Field::int(108, 30));
    m.trailer.set(Field::int(93, signature.len() as i64));
    m.trailer.set(Field::bytes(89, signature));

    let bytes = encode(&m);
    let decoded = decode(&bytes).unwrap();

    assert_eq!(
        decoded.trailer.get(89).unwrap().value_bytes(),
        signature,
        "Signature must decode using the SignatureLength byte count, not stop at the first SOH"
    );
    assert_eq!(
        encode(&decoded),
        bytes,
        "encode->decode->encode is byte-stable"
    );
}
