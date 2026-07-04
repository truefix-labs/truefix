//! T069 (US7, feature 006) — the `EncodedLeg*` (and sibling) Len/Data field pairs missing from
//! `data_field_for_length` (B22/FR-033): embedded SOH bytes in their content must not corrupt
//! message framing, mirroring `signature_length.rs`'s established pattern for this class of bug.

use truefix_core::{Field, Message, decode, encode};

fn base_message() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "SENDER"));
    m.header.set(Field::string(56, "TARGET"));
    m.header.set(Field::string(52, "20240101-12:00:00"));
    m.body.set(Field::string(98, "0"));
    m.body.set(Field::int(108, 30));
    m
}

fn assert_len_data_pair_round_trips(len_tag: u32, data_tag: u32) {
    let data: &[u8] = b"leg\x01with\x01embedded\x01soh";
    let mut m = base_message();
    m.body.set(Field::int(len_tag, data.len() as i64));
    m.body.set(Field::bytes(data_tag, data));

    let bytes = encode(&m);
    let decoded = decode(&bytes).unwrap();

    assert_eq!(
        decoded.body.get(data_tag).unwrap().value_bytes(),
        data,
        "tag {data_tag} must decode using its length field {len_tag}'s byte count, not stop at \
         the first embedded SOH"
    );
    assert_eq!(
        encode(&decoded),
        bytes,
        "encode->decode->encode is byte-stable for tag {data_tag}"
    );
}

#[test]
fn encoded_leg_issuer_round_trips_with_embedded_soh() {
    assert_len_data_pair_round_trips(618, 619);
}

#[test]
fn encoded_leg_security_desc_round_trips_with_embedded_soh() {
    assert_len_data_pair_round_trips(621, 622);
}

#[test]
fn encoded_list_status_text_round_trips_with_embedded_soh() {
    assert_len_data_pair_round_trips(445, 446);
}

#[test]
fn encoded_symbol_round_trips_with_embedded_soh() {
    assert_len_data_pair_round_trips(1359, 1360);
}

#[test]
fn encoded_mkt_segm_desc_round_trips_with_embedded_soh() {
    assert_len_data_pair_round_trips(1397, 1398);
}

#[test]
fn encoded_security_list_desc_round_trips_with_embedded_soh() {
    assert_len_data_pair_round_trips(1468, 1469);
}
