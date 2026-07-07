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

/// T012 (US1, feature 011, FR-005/FR-006): `present()`'s group-aware fix. A dictionary that marks
/// a group's own count tag as message-level-required must recognize the group as present once
/// it's `Member::Group`-structured — before this fix, `FieldMap::get()`/`contains()` (which only
/// match `Member::Field`) made a genuinely-present structured group invisible to this check,
/// spuriously rejecting it as a missing required field.
const GROUP_REQUIRED_AT_MESSAGE_LEVEL_DICT_SRC: &str = "version FIX.4.4\n\
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
     header 8 9 35 34 49 56 52\n\
     trailer 10\n\
     group 453 NoPartyIDs 448 448\n\
     message 0 Heartbeat req:453\n";

struct PartyIdsSpec;
impl truefix_core::GroupSpec for PartyIdsSpec {
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])> {
        (count_tag == 453).then_some((448, [448u32].as_slice()))
    }
}

#[test]
fn a_message_level_required_group_that_is_present_and_structured_is_not_reported_missing() {
    let d = truefix_dict::parse(GROUP_REQUIRED_AT_MESSAGE_LEVEL_DICT_SRC).unwrap();
    let body = b"35=0\x0134=1\x0149=A\x0156=B\x0152=20240101-00:00:00\x01453=1\x01448=P1\x01";
    let mut wire = Vec::new();
    wire.extend_from_slice(b"8=FIX.4.4\x01");
    wire.extend_from_slice(format!("9={}\x01", body.len()).as_bytes());
    wire.extend_from_slice(body);
    let sum: u32 = wire.iter().map(|&b| u32::from(b)).sum::<u32>() & 0xFF;
    wire.extend_from_slice(format!("10={sum:03}\x01").as_bytes());

    let decoded = truefix_core::decode_with_groups(&wire, &PartyIdsSpec).expect("decode");
    assert!(
        decoded.body.group(453).is_some(),
        "precondition: the group must actually be Member::Group-structured"
    );
    assert!(
        d.validate(&decoded, &ValidationOptions::default()).is_ok(),
        "a present, structured group required at the message level must not be reported missing"
    );
}

#[test]
fn a_message_level_required_group_that_is_genuinely_absent_is_still_rejected() {
    let d = truefix_dict::parse(GROUP_REQUIRED_AT_MESSAGE_LEVEL_DICT_SRC).unwrap();
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));

    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.reason, RejectReason::RequiredTagMissing);
    assert_eq!(err.ref_tag, Some(453));
}
