//! T089/T090 (US3, feature 007): a repeating group's own required fields are recursively
//! validated within each entry (BUG-54), and a message whose body groups are already
//! `Member::Group`-structured (via `decode_with_groups` called with a `GroupSpec` that covers body
//! groups, not just the production transport path's header/trailer-only scope) has its entries'
//! required-field and type/enum checks applied too (BUG-55) — previously `validate_groups`'s flat
//! wire-order walk saw nothing at all for a structured group, since `FieldMap::fields()` skips
//! `Member::Group` members entirely.

use truefix_core::{Field, FieldMap, Group, Message};
use truefix_dict::ValidationOptions;

// A group (453 NoPartyIDs, delimiter 448) whose entries also require 447 (PartyIDSource, an enum
// of {D, C}) — declared via the new `req:` group-line syntax (BUG-54/FR-034).
const DICT_SRC: &str = "version FIX.4.4\n\
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
     field 452 PartyRole INT\n\
     header 8 9 35 34 49 56 52\n\
     trailer 10\n\
     group 453 NoPartyIDs 448 448,447,452 req:448,447\n\
     message 0 Heartbeat req: opt:453\n";

fn heartbeat_with_flat_group(group_fields: &[(u32, &str)]) -> Message {
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

fn entry(fields: &[(u32, &str)]) -> FieldMap {
    let mut fm = FieldMap::new();
    for (t, v) in fields {
        fm.add_field(Field::string(*t, v));
    }
    fm
}

fn heartbeat_with_structured_group(entries: Vec<FieldMap>) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    let mut group = Group::new(453);
    for e in entries {
        group.add_entry(e);
    }
    m.body.add_group(group);
    m
}

// --- BUG-54: required field within a (flat, wire-order) group entry ---

#[test]
fn a_flat_group_entry_missing_its_required_member_is_rejected() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    // Entry has the delimiter (448) but not the required 447.
    let m = heartbeat_with_flat_group(&[(453, "1"), (448, "A"), (452, "1")]);
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.reason.code(), 1); // RequiredTagMissing
    assert_eq!(err.ref_tag, Some(447));
}

#[test]
fn a_flat_group_entry_with_its_required_member_present_is_accepted() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let m = heartbeat_with_flat_group(&[(453, "1"), (448, "A"), (447, "D"), (452, "1")]);
    assert!(d.validate(&m, &ValidationOptions::default()).is_ok());
}

#[test]
fn group_required_check_is_skipped_when_check_required_fields_is_off() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let m = heartbeat_with_flat_group(&[(453, "1"), (448, "A"), (452, "1")]);
    let lax = ValidationOptions {
        check_required_fields: false,
        ..ValidationOptions::default()
    };
    assert!(d.validate(&m, &lax).is_ok());
}

// --- BUG-55: a `Member::Group`-structured message's entries are validated too ---

#[test]
fn a_structured_group_entry_missing_its_required_member_is_rejected() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let m = heartbeat_with_structured_group(vec![entry(&[(448, "A"), (452, "1")])]);
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.reason.code(), 1); // RequiredTagMissing
    assert_eq!(err.ref_tag, Some(447));
}

#[test]
fn a_structured_group_entry_with_its_required_member_present_is_accepted() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let m = heartbeat_with_structured_group(vec![entry(&[(448, "A"), (447, "D"), (452, "1")])]);
    assert!(d.validate(&m, &ValidationOptions::default()).is_ok());
}

#[test]
fn a_structured_group_entrys_invalid_enum_value_is_rejected() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    // PartyIDSource(447) = "X" is not in its {D, C} enum.
    let m = heartbeat_with_structured_group(vec![entry(&[(448, "A"), (447, "X")])]);
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.reason.code(), 5); // ValueIsIncorrect
    assert_eq!(err.ref_tag, Some(447));
}

#[test]
fn a_structured_group_entrys_malformed_field_is_rejected() {
    // PartyIDSource(447) is CHAR (exactly one character); "XX" is malformed for the type.
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let m = heartbeat_with_structured_group(vec![entry(&[(448, "A"), (447, "XX")])]);
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.reason.code(), 6); // IncorrectDataFormat
}

#[test]
fn a_fully_valid_structured_group_with_multiple_entries_is_accepted() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let m = heartbeat_with_structured_group(vec![
        entry(&[(448, "A"), (447, "D"), (452, "1")]),
        entry(&[(448, "B"), (447, "C"), (452, "2")]),
    ]);
    assert!(d.validate(&m, &ValidationOptions::default()).is_ok());
}
