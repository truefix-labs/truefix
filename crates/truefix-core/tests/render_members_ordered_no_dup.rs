//! FR-047 (US3, feature 009, `NEW-80`): a tag appearing twice in a dictionary's
//! `MessageDef::field_order` list (the `ordered` modifier's per-message field order) must still be
//! emitted only once by `encode_with_order`/`render_members_ordered` -- not once per occurrence in
//! `order`.

use truefix_core::{Field, Message, encode_with_order};

fn msg() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.body.set(Field::string(11, "ORD1")); // ClOrdID
    m.body.set(Field::string(44, "150.25")); // Price
    m
}

#[test]
fn a_tag_repeated_in_field_order_is_emitted_only_once() {
    // 11 appears twice in `order` (as if the dictionary's field_order list itself had a
    // duplicate entry); the encoded body must still contain exactly one `11=...`.
    let order = [11u32, 11, 44];
    let bytes = encode_with_order(&msg(), Some(&order));
    let s = String::from_utf8(bytes).unwrap();
    let body_start = s.find("35=D\u{1}").unwrap() + 5;
    let body = &s[body_start..s.find("10=").unwrap()];

    assert_eq!(
        body.matches("11=ORD1\u{1}").count(),
        1,
        "tag 11 must be emitted exactly once even though it appears twice in field_order, got \
         body: {body:?}"
    );
    assert_eq!(body, "11=ORD1\u{1}44=150.25\u{1}");
}
