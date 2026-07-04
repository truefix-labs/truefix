//! T030 (US4) — repeated-tag detection outside repeating groups (FR-007; SessionRejectReason=13).

use truefix_core::{Field, Message};
use truefix_dict::{ValidationOptions, load_fix44};

fn nos_with_extra(extra: &[(u32, &str)]) -> Message {
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
    for (t, v) in extra {
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
fn duplicate_top_level_tag_is_rejected() {
    // ClOrdID(11) appears twice, outside any repeating group.
    let m = nos_with_extra(&[(11, "O1-DUP")]);
    assert_eq!(code(&m, &ValidationOptions::default()), Err(13));
}

#[test]
fn duplicate_top_level_tag_accepted_when_toggle_off() {
    let m = nos_with_extra(&[(11, "O1-DUP")]);
    let lax = ValidationOptions {
        check_repeated_tags: false,
        ..Default::default()
    };
    // Field::set on decode isn't used here (add_field allows literal duplicates); with the check
    // off, validation does not reject on the repeat (other checks still apply).
    assert_eq!(code(&m, &lax), Ok(()));
}

#[test]
fn group_member_tags_may_legitimately_repeat() {
    // NoPartyIDs=2 with two entries — 448/447/452 each appear twice, which is expected.
    let mut m = nos_with_extra(&[]);
    for (t, v) in [
        (453u32, "2"),
        (448, "A"),
        (447, "1"),
        (452, "1"),
        (448, "B"),
        (447, "1"),
        (452, "2"),
    ] {
        m.body.add_field(Field::string(t, v));
    }
    assert_eq!(code(&m, &ValidationOptions::default()), Ok(()));
}
