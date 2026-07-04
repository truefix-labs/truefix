//! T020 (US3) — dictionary-driven structured group decode + byte-identical round-trip (FR-004/021).

use truefix_core::{Field, FieldMap, Group, Message, decode_with_groups};
use truefix_dict::load_fix44;

fn entry(fields: &[(u32, &str)]) -> FieldMap {
    let mut fm = FieldMap::new();
    for (t, v) in fields {
        fm.add_field(Field::string(*t, v));
    }
    fm
}

/// Build a FIX.4.4 NewOrderSingle carrying a NoPartyIDs group with a nested NoPartySubIDs.
fn nos_with_nested_group() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, 2));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    for (t, v) in [(11, "O1"), (21, "1"), (55, "AAPL"), (54, "1")] {
        m.body.set(Field::string(t, v));
    }
    m.body.set(Field::string(60, "20240101-00:00:00"));
    m.body.set(Field::string(40, "2"));

    let mut party = entry(&[(448, "A"), (447, "1"), (452, "1")]);
    let mut subs = Group::new(802);
    subs.add_entry(entry(&[(523, "S"), (803, "1")]));
    party.add_group(subs);

    let mut parties = Group::new(453);
    parties.add_entry(party);
    m.body.add_group(parties);
    m
}

#[test]
fn structured_decode_recovers_nested_groups() {
    let dict = load_fix44().unwrap();
    let bytes = nos_with_nested_group().encode();

    let decoded = decode_with_groups(&bytes, &dict).expect("decode");
    let parties = decoded.body.group(453).expect("NoPartyIDs present");
    assert_eq!(parties.len(), 1);
    let sub = parties[0].group(802).expect("nested NoPartySubIDs present");
    assert_eq!(sub.len(), 1);
    assert_eq!(sub[0].get(523).unwrap().as_str().unwrap(), "S");
}

#[test]
fn structured_decode_round_trips_byte_identically() {
    let dict = load_fix44().unwrap();
    let bytes = nos_with_nested_group().encode();
    let decoded = decode_with_groups(&bytes, &dict).expect("decode");
    assert_eq!(
        decoded.encode(),
        bytes,
        "structured decode must round-trip byte-identically"
    );
}
