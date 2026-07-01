//! T021 (US3) — dictionary-driven repeating-group validation (FR-004/005).

use truefix_core::{Field, Message};
use truefix_dict::{load_fix44, ValidationOptions};

/// A FIX.4.4 NewOrderSingle whose body carries the given (flat, wire-ordered) group fields after the
/// required fields — i.e. exactly what a flat decode produces.
fn nos(group_fields: &[(u32, &str)]) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, 2));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(11, "O1"));
    m.body.set(Field::string(21, "1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::string(54, "1"));
    m.body.set(Field::string(60, "20240101-00:00:00"));
    m.body.set(Field::string(40, "2"));
    for (t, v) in group_fields {
        m.body.add_field(Field::string(*t, v));
    }
    m
}

fn code(m: &Message, opts: &ValidationOptions) -> Result<(), u32> {
    load_fix44()
        .unwrap()
        .validate(m, opts)
        .map_err(|e| e.reason.code())
}

#[test]
fn valid_group_is_accepted() {
    // NoPartyIDs=2, two well-formed entries (delimiter 448 first).
    let m = nos(&[
        (453, "2"),
        (448, "A"),
        (447, "1"),
        (452, "1"),
        (448, "B"),
        (447, "1"),
        (452, "2"),
    ]);
    assert_eq!(code(&m, &ValidationOptions::default()), Ok(()));
}

#[test]
fn wrong_count_is_rejected() {
    // NoPartyIDs declares 3 but only 2 entries are present → IncorrectNumInGroupCount (16).
    let m = nos(&[(453, "3"), (448, "A"), (452, "1"), (448, "B"), (452, "2")]);
    assert_eq!(code(&m, &ValidationOptions::default()), Err(16));
}

#[test]
fn zero_count_is_accepted() {
    let m = nos(&[(453, "0")]);
    assert_eq!(code(&m, &ValidationOptions::default()), Ok(()));
}

#[test]
fn missing_delimiter_is_rejected() {
    // Entry starts with 447 instead of the delimiter 448 → RepeatingGroupFieldsOutOfOrder (14).
    let m = nos(&[(453, "1"), (447, "1"), (448, "A")]);
    assert_eq!(code(&m, &ValidationOptions::default()), Err(14));
}

#[test]
fn out_of_order_members_rejected_only_when_toggle_on() {
    // Entry: delimiter 448, then 452 (idx 2), then 447 (idx 1) → out of order.
    let m = nos(&[(453, "1"), (448, "A"), (452, "1"), (447, "1")]);
    assert_eq!(code(&m, &ValidationOptions::default()), Err(14));

    let lax = ValidationOptions {
        validate_unordered_group_fields: false,
        ..Default::default()
    };
    assert_eq!(code(&m, &lax), Ok(()));
}

#[test]
fn nested_group_valid_is_accepted() {
    // NoPartyIDs=1 with a nested NoPartySubIDs=1.
    let m = nos(&[
        (453, "1"),
        (448, "A"),
        (447, "1"),
        (452, "1"),
        (802, "1"),
        (523, "S"),
        (803, "1"),
    ]);
    assert_eq!(code(&m, &ValidationOptions::default()), Ok(()));
}

#[test]
fn nested_group_missing_delimiter_is_rejected() {
    // Nested NoPartySubIDs=1 but its entry starts with 803 instead of the delimiter 523 (QFJ934).
    let m = nos(&[(453, "1"), (448, "A"), (802, "1"), (803, "1"), (523, "S")]);
    assert_eq!(code(&m, &ValidationOptions::default()), Err(14));
}
