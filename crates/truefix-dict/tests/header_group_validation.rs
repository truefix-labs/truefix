//! T056/T057 (US2, feature 009, `NEW-19`): `validate_groups` scanned only `message.body` for
//! repeating-group structure (count/delimiter/order) -- but the standard header can itself
//! declare a repeating group (`NoHops(627)` in every bundled dictionary: `header ... 627` /
//! `group 627 NoHops 628 628,629,630`). A header-level group's structure was never checked at
//! all, unlike the identical body-level case.

use truefix_core::{Field, Message};
use truefix_dict::{RejectReason, ValidationOptions, load_fix44};

fn nos() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(11, "ORD1"));
    m.body.set(Field::string(21, "1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::string(54, "1"));
    m.body.set(Field::string(60, "20240101-00:00:00"));
    m.body.set(Field::string(40, "2"));
    m.body.set(Field::string(38, "100"));
    m.body.set(Field::string(44, "150.25"));
    m.trailer.set(Field::string(10, "000"));
    m
}

#[test]
fn a_header_level_nohops_group_with_a_mismatched_count_is_rejected() {
    let mut m = nos();
    // Declares 2 hops (NoHops=2) but only supplies one HopCompID(628) entry.
    m.header.add_field(Field::int(627, 2));
    m.header.add_field(Field::string(628, "HOP1"));

    let err = load_fix44()
        .unwrap()
        .validate(&m, &ValidationOptions::default())
        .unwrap_err();
    assert_eq!(
        err.reason,
        RejectReason::IncorrectNumInGroupCount,
        "a header-level NoHops(627) group with a declared count that doesn't match its actual \
         entries must be rejected, same as an equivalent body-level group (NEW-19)"
    );
}

#[test]
fn a_header_level_nohops_group_with_a_matching_count_passes() {
    let mut m = nos();
    m.header.add_field(Field::int(627, 1));
    m.header.add_field(Field::string(628, "HOP1"));

    assert!(
        load_fix44()
            .unwrap()
            .validate(&m, &ValidationOptions::default())
            .is_ok()
    );
}
