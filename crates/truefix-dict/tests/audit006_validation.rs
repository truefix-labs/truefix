//! Shared fixtures for audit 006 dictionary validation tests.
//!
//! NEW-146: section-order violations are rejected via `validate_fields_out_of_order` (already
//! flipped to default `true` at the `.cfg` layer by NEW-104; this validates the same policy
//! directly against `DataDictionary::validate`).
//! NEW-147: nested repeating group members are validated using the nested group's own definition.

use truefix_core::{Field, Message};
use truefix_dict::{ValidationOptions, load_fix44};

fn header_only_message(begin: &str, msg_type: &str) -> Message {
    let mut msg = Message::new();
    msg.header.set(Field::string(8, begin));
    msg.header.set(Field::string(35, msg_type));
    msg
}

#[test]
fn audit006_validation_fixture_builds_header_message() {
    let msg = header_only_message("FIX.4.4", "D");
    assert_eq!(msg.header.get(8).unwrap().as_str().unwrap(), "FIX.4.4");
    assert_eq!(msg.header.get(35).unwrap().as_str().unwrap(), "D");
}

/// A wire message where a header tag reappears after a body field, computing BodyLength/CheckSum
/// correctly so only the section-order violation is at issue -- no tag repeats (PossDupFlag(43)
/// is a header tag distinct from every other header tag already used earlier in this fixture, so
/// it introduces the violation without also tripping `TagAppearsMoreThanOnce`). Constructed as
/// raw wire bytes (not via `Message::new()`/`.set()`) because `Message::fields_out_of_order` is
/// only ever set by the decoder's own wire-position tracking.
fn message_with_header_tag_reappearing_in_body() -> Vec<u8> {
    // BodyLength(9) covers everything from just after `9=<len>\x01` to just before `10=`; only
    // BeginString/BodyLength are outside it, so 35/49/56/34/52/112/43 are all counted here
    // (matching real wire framing) even though 49/56/34/52/43 are declared header fields in the
    // dictionary -- that's exactly the section-order violation this fixture is for. MsgType `0`
    // (Heartbeat, optional TestReqID(112)) is used so the "check disabled" case has no unmet
    // required body fields to also trip over.
    let body =
        b"35=0\x0149=SENDER\x0156=TARGET\x0134=1\x0152=20240101-00:00:00\x01112=TEST\x0143=N\x01";
    let mut msg = Vec::new();
    msg.extend_from_slice(b"8=FIX.4.4\x01");
    msg.extend_from_slice(format!("9={}\x01", body.len()).as_bytes());
    msg.extend_from_slice(body);
    let sum: u32 = msg.iter().map(|&b| u32::from(b)).sum::<u32>() & 0xFF;
    msg.extend_from_slice(format!("10={sum:03}\x01").as_bytes());
    msg
}

// --- NEW-146 ---

#[test]
fn audit006_section_order_violation_is_rejected_when_enabled() {
    let dict = load_fix44().unwrap();
    let wire = message_with_header_tag_reappearing_in_body();
    let msg = Message::decode(&wire).expect("decodes despite the section-order violation");
    assert!(msg.fields_out_of_order());

    let opts = ValidationOptions {
        validate_fields_out_of_order: true,
        ..ValidationOptions::default()
    };
    let err = dict.validate(&msg, &opts).unwrap_err();
    assert_eq!(err.reason.code(), 14); // TagOutOfRequiredOrder
}

#[test]
fn audit006_section_order_violation_is_accepted_when_disabled() {
    let dict = load_fix44().unwrap();
    let wire = message_with_header_tag_reappearing_in_body();
    let msg = Message::decode(&wire).unwrap();

    let opts = ValidationOptions {
        validate_fields_out_of_order: false,
        ..ValidationOptions::default()
    };
    assert!(dict.validate(&msg, &opts).is_ok());
}

// --- NEW-147: nested repeating group members are validated with the nested group's own
// definition, not the parent's ---

const NESTED_GROUP_DICT_SRC: &str = "version FIX.4.4\n\
     field 8 BeginString STRING\n\
     field 9 BodyLength LENGTH\n\
     field 35 MsgType STRING\n\
     field 34 MsgSeqNum SEQNUM\n\
     field 49 SenderCompID STRING\n\
     field 56 TargetCompID STRING\n\
     field 52 SendingTime UTCTIMESTAMP\n\
     field 10 CheckSum STRING\n\
     field 78 NoAllocs NUMINGROUP\n\
     field 79 AllocAccount STRING\n\
     field 539 NoNestedPartyIDs NUMINGROUP\n\
     field 524 NestedPartyID STRING\n\
     field 525 NestedPartyIDSource CHAR D C\n\
     header 8 9 35 34 49 56 52\n\
     trailer 10\n\
     group 539 NoNestedPartyIDs 524 524,525\n\
     group 78 NoAllocs 79 79,539\n\
     message 0 Heartbeat req: opt:78\n";

fn heartbeat_with_nested_group(fields: &[(u32, &str)]) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    for (t, v) in fields {
        m.body.add_field(Field::string(*t, v));
    }
    m
}

#[test]
fn audit006_nested_group_with_valid_members_is_accepted() {
    let d = truefix_dict::parse(NESTED_GROUP_DICT_SRC).unwrap();
    let m = heartbeat_with_nested_group(&[
        (78, "1"),
        (79, "ACCT1"),
        (539, "1"),
        (524, "PARTY1"),
        (525, "D"),
    ]);
    assert!(d.validate(&m, &ValidationOptions::default()).is_ok());
}

#[test]
fn audit006_nested_group_members_enum_value_is_checked_against_the_nested_definition() {
    let d = truefix_dict::parse(NESTED_GROUP_DICT_SRC).unwrap();
    // NestedPartyIDSource(525) = "X" is not in its {D, C} enum -- must be rejected using 525's
    // own field definition, not silently accepted via the parent (NoAllocs) group's scope.
    let m = heartbeat_with_nested_group(&[
        (78, "1"),
        (79, "ACCT1"),
        (539, "1"),
        (524, "PARTY1"),
        (525, "X"),
    ]);
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.reason.code(), 5); // ValueIsIncorrect
    assert_eq!(err.ref_tag, Some(525));
}

#[test]
fn audit006_nested_group_entry_count_mismatch_is_rejected() {
    let d = truefix_dict::parse(NESTED_GROUP_DICT_SRC).unwrap();
    let m = heartbeat_with_nested_group(&[
        (78, "1"),
        (79, "ACCT1"),
        (539, "2"), // declares 2 nested entries but only provides 1
        (524, "PARTY1"),
        (525, "D"),
    ]);
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.ref_tag, Some(539));
}
