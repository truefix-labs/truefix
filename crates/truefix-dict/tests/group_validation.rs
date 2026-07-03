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
    // Entry starts with 447 instead of the delimiter 448 → RepeatingGroupFieldsOutOfOrder (15).
    let m = nos(&[(453, "1"), (447, "1"), (448, "A")]);
    assert_eq!(code(&m, &ValidationOptions::default()), Err(15));
}

#[test]
fn out_of_order_members_rejected_only_when_toggle_on() {
    // Entry: delimiter 448, then 452 (idx 2), then 447 (idx 1) → out of order.
    let m = nos(&[(453, "1"), (448, "A"), (452, "1"), (447, "1")]);
    assert_eq!(code(&m, &ValidationOptions::default()), Err(15));

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
    assert_eq!(code(&m, &ValidationOptions::default()), Err(15));
}

// --- T061 (US9, feature 005): a group's `child` dictionary type/enum-checks fields *within* a
// group entry (GAP-24/FR-024) — previously silently skipped, since `FieldMap::fields()` (which
// the top-level field loop walks) never descends into repeating-group entries at all. ---

const CHILD_DICT_SRC: &str = "version FIX.4.4\n\
     field 8 BeginString STRING\n\
     field 9 BodyLength LENGTH\n\
     field 35 MsgType STRING\n\
     field 34 MsgSeqNum SEQNUM\n\
     field 49 SenderCompID STRING\n\
     field 56 TargetCompID STRING\n\
     field 52 SendingTime UTCTIMESTAMP\n\
     field 10 CheckSum STRING\n\
     field 453 NoPartyIDs NUMINGROUP\n\
     field 448 PartyID STRING\n\
     field 447 PartyIDSource CHAR D C\n\
     header 8 9 35 34 49 56 52\n\
     trailer 10\n\
     group 453 NoPartyIDs 448 448,447\n\
     message 0 Heartbeat req: opt:453\n";

fn heartbeat_with_group(group_fields: &[(u32, &str)]) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    for (t, v) in group_fields {
        m.body.add_field(Field::string(*t, v));
    }
    m
}

#[test]
fn a_group_definition_carries_a_child_dictionary() {
    let d = truefix_dict::parse(CHILD_DICT_SRC).unwrap();
    let gdef = d.group(453).unwrap();
    let child = gdef
        .child
        .as_ref()
        .expect("child dictionary should be built");
    assert!(child.field(448).is_some());
    assert!(child.field(447).is_some());
}

#[test]
fn a_valid_enum_value_within_a_group_entry_is_accepted() {
    let d = truefix_dict::parse(CHILD_DICT_SRC).unwrap();
    let m = heartbeat_with_group(&[(453, "1"), (448, "A"), (447, "D")]);
    assert!(d.validate(&m, &ValidationOptions::default()).is_ok());
}

#[test]
fn an_invalid_enum_value_within_a_group_entry_is_now_rejected() {
    let d = truefix_dict::parse(CHILD_DICT_SRC).unwrap();
    // PartyIDSource(447) = "X" is not in its {D, C} enum — this must now be caught, whereas
    // before GAP-24's fix, group-entry fields were never type/enum-checked at all.
    let m = heartbeat_with_group(&[(453, "1"), (448, "A"), (447, "X")]);
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.reason.code(), 5); // ValueIsIncorrect
    assert_eq!(err.ref_tag, Some(447));
}

#[test]
fn an_incorrectly_formatted_group_field_is_rejected() {
    // PartyIDSource(447) is CHAR (exactly one character); "XX" is malformed for the type,
    // independent of enum membership.
    let d = truefix_dict::parse(CHILD_DICT_SRC).unwrap();
    let m = heartbeat_with_group(&[(453, "1"), (448, "A"), (447, "XX")]);
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.reason.code(), 6); // IncorrectDataFormat
}

#[test]
fn group_field_checking_is_skipped_when_check_field_types_is_off() {
    let d = truefix_dict::parse(CHILD_DICT_SRC).unwrap();
    let m = heartbeat_with_group(&[(453, "1"), (448, "A"), (447, "X")]);
    let lax = ValidationOptions {
        check_field_types: false,
        ..ValidationOptions::default()
    };
    assert!(d.validate(&m, &lax).is_ok());
}
