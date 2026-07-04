//! T091/T092 (US3, feature 007): a multi-character `CHAR` field on a FIX.4.0/4.1 message is
//! accepted, matching QFJ (which skips `CharConverter`'s single-character validation entirely for
//! those two versions, treating `CHAR` as a plain string) — BUG-62/FR-035. TrueFix previously
//! called `field.as_char()` unconditionally for every version, rejecting a real, QFJ-accepted
//! multi-character value on FIX.4.0/4.1.
//!
//! Uses two real fields from the bundled dictionaries that demonstrate the exact version-sensitive
//! quirk: `Account`(1) is declared `CHAR` in `FIX40.fixdict`/`FIX41.fixdict` but `STRING` in
//! `FIX44.fixdict` (so it can't show the "later versions still enforce" half); `CxlType`(125) is
//! `CHAR` with no enum restriction in `FIX44.fixdict`, used to confirm the strict check still
//! applies from FIX.4.2 onward.

use truefix_core::{Field, Message};
use truefix_dict::{ValidationOptions, load_fix40, load_fix41, load_fix44};

fn heartbeat_with_extra_field(begin: &str, tag: u32, value: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(tag, value));
    m
}

fn lax_opts() -> ValidationOptions {
    // Only the field-type/format check is under test here; bypass "does this field belong to
    // Heartbeat" (it doesn't, on either version -- irrelevant to what BUG-62 is about).
    ValidationOptions {
        allow_unknown_msg_fields: true,
        ..ValidationOptions::default()
    }
}

#[test]
fn a_multi_character_char_field_is_accepted_on_fix40() {
    let d = load_fix40().unwrap();
    let m = heartbeat_with_extra_field("FIX.4.0", 1, "ACCT123"); // Account: CHAR in FIX.4.0
    assert!(
        d.validate(&m, &lax_opts()).is_ok(),
        "a multi-character CHAR field must be accepted on FIX.4.0, matching QFJ"
    );
}

#[test]
fn a_multi_character_char_field_is_accepted_on_fix41() {
    let d = load_fix41().unwrap();
    let m = heartbeat_with_extra_field("FIX.4.1", 1, "ACCT123"); // Account: CHAR in FIX.4.1
    assert!(
        d.validate(&m, &lax_opts()).is_ok(),
        "a multi-character CHAR field must be accepted on FIX.4.1, matching QFJ"
    );
}

#[test]
fn a_single_character_value_is_still_accepted_on_fix40() {
    let d = load_fix40().unwrap();
    let m = heartbeat_with_extra_field("FIX.4.0", 1, "A");
    assert!(d.validate(&m, &lax_opts()).is_ok());
}

#[test]
fn a_multi_character_char_field_is_still_rejected_from_fix44_onward() {
    let d = load_fix44().unwrap();
    // CxlType(125): CHAR, no enum restriction, in FIX.4.4 -- the strict per-character check must
    // still apply on this later version, unlike FIX.4.0/4.1.
    let m = heartbeat_with_extra_field("FIX.4.4", 125, "AB");
    let err = d.validate(&m, &lax_opts()).unwrap_err();
    assert_eq!(err.reason.code(), 6); // IncorrectDataFormat
    assert_eq!(err.ref_tag, Some(125));
}

#[test]
fn a_single_character_value_is_accepted_on_fix44() {
    let d = load_fix44().unwrap();
    let m = heartbeat_with_extra_field("FIX.4.4", 125, "A");
    assert!(d.validate(&m, &lax_opts()).is_ok());
}
