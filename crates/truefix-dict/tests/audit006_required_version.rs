//! NEW-127: required header/trailer envelope-field validation (`check_required_envelope_fields`,
//! opt-in — see its doc for why default `false`).
//! NEW-145: the `BeginString` version-match check now falls back to `self.version` (the plain
//! `version FIX.M.N` directive every bundled dictionary declares) instead of being a no-op
//! whenever `version_meta` is absent.

use truefix_core::{Field, Message};
use truefix_dict::{ValidationOptions, load_fix44};

fn full_heartbeat(begin: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin));
    m.header.set(Field::int(9, 0));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.trailer.set(Field::string(10, "000"));
    m
}

/// As [`full_heartbeat`], but omitting BodyLength(9) -- for NEW-127's missing-header-field cases.
fn heartbeat_without_body_length(begin: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.trailer.set(Field::string(10, "000"));
    m
}

/// As [`full_heartbeat`], but omitting CheckSum(10) -- for NEW-127's missing-trailer-field case.
fn heartbeat_without_checksum(begin: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin));
    m.header.set(Field::int(9, 0));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

// --- NEW-127 ---

#[test]
fn audit006_required_envelope_fields_pass_when_all_present() {
    let d = load_fix44().unwrap();
    let m = full_heartbeat("FIX.4.4");
    let opts = ValidationOptions {
        check_required_envelope_fields: true,
        ..ValidationOptions::default()
    };
    assert!(d.validate(&m, &opts).is_ok());
}

#[test]
fn audit006_missing_body_length_is_rejected_when_envelope_check_enabled() {
    let d = load_fix44().unwrap();
    let m = heartbeat_without_body_length("FIX.4.4");
    let opts = ValidationOptions {
        check_required_envelope_fields: true,
        ..ValidationOptions::default()
    };
    let err = d.validate(&m, &opts).unwrap_err();
    assert_eq!(err.ref_tag, Some(9));
}

#[test]
fn audit006_missing_checksum_is_rejected_when_envelope_check_enabled() {
    let d = load_fix44().unwrap();
    let m = heartbeat_without_checksum("FIX.4.4");
    let opts = ValidationOptions {
        check_required_envelope_fields: true,
        ..ValidationOptions::default()
    };
    let err = d.validate(&m, &opts).unwrap_err();
    assert_eq!(err.ref_tag, Some(10));
}

#[test]
fn audit006_missing_body_length_is_not_rejected_by_default() {
    // check_required_envelope_fields defaults to false -- existing lenient behavior for messages
    // built directly (bypassing the codec envelope) is unaffected.
    let d = load_fix44().unwrap();
    let m = heartbeat_without_body_length("FIX.4.4");
    assert!(d.validate(&m, &ValidationOptions::default()).is_ok());
}

// --- NEW-145 ---

#[test]
fn audit006_begin_string_mismatch_is_rejected_via_version_fallback_with_no_version_meta() {
    let d = load_fix44().unwrap();
    assert!(
        d.version_meta().is_none(),
        "bundled dictionaries declare no version-meta directive"
    );
    let m = full_heartbeat("FIX.4.2"); // mismatches the FIX.4.4 dictionary's own `version` line
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert_eq!(err.ref_tag, Some(8));
}

#[test]
fn audit006_begin_string_match_is_accepted_via_version_fallback() {
    let d = load_fix44().unwrap();
    let m = full_heartbeat("FIX.4.4");
    assert!(d.validate(&m, &ValidationOptions::default()).is_ok());
}
