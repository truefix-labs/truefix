//! T064 (US9, feature 005) — `encode_with_order` emits body fields in a configured tag order,
//! byte-for-byte, with unlisted/UDF fields appended after (FR-027).

use truefix_core::{Field, Message, encode, encode_with_order};

fn msg() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    // Insertion order deliberately differs from the desired emission order below.
    m.body.set(Field::string(44, "150.25")); // Price
    m.body.set(Field::string(11, "ORD1")); // ClOrdID
    m.body.set(Field::string(9999, "UDF")); // not in field_order
    m.body.set(Field::string(55, "AAPL")); // Symbol
    m
}

#[test]
fn field_order_reorders_top_level_body_fields() {
    let order = [55u32, 11, 44]; // Symbol, ClOrdID, Price
    let bytes = encode_with_order(&msg(), Some(&order));
    let s = String::from_utf8(bytes).unwrap();
    let body_start = s.find("35=D\u{1}").unwrap() + 5;
    let body = &s[body_start..s.find("10=").unwrap()];
    // Listed fields first, in the configured order; the unlisted UDF (9999) appended after.
    assert_eq!(body, "55=AAPL\u{1}11=ORD1\u{1}44=150.25\u{1}9999=UDF\u{1}");
}

#[test]
fn no_field_order_matches_plain_encode_byte_for_byte() {
    assert_eq!(encode(&msg()), encode_with_order(&msg(), None));
}

#[test]
fn field_order_does_not_affect_a_repeating_groups_internal_field_order() {
    use truefix_core::Group;
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    let mut entry = Field::string(448, "PARTY_1");
    let mut group = Group::new(453);
    let mut fm = truefix_core::FieldMap::new();
    fm.add_field(std::mem::replace(&mut entry, Field::string(447, "I")));
    fm.add_field(entry);
    group.add_entry(fm);
    m.body.add_group(group);
    m.body.set(Field::string(11, "ORD1"));

    // field_order only reorders top-level body members (11 before/after the group 453) — the
    // group's own entry field order (448 then 447) is always preserved regardless.
    let order = [11u32, 453];
    let bytes = encode_with_order(&m, Some(&order));
    let s = String::from_utf8(bytes).unwrap();
    assert!(s.contains("11=ORD1\u{1}453=1\u{1}448=PARTY_1\u{1}447=I\u{1}"));
}
