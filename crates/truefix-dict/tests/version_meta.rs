//! T065 (US9, feature 005) — dictionary version metadata + BeginString match validation
//! (FR-028/FR-029), including the no-op-when-absent Edge Case.

use truefix_core::{Field, Message};
use truefix_dict::{RejectReason, ValidationOptions, parse};

const HEADER_TRAILER_FIELDS: &str = "field 8 BeginString STRING\n\
     field 9 BodyLength LENGTH\n\
     field 35 MsgType STRING\n\
     field 34 MsgSeqNum SEQNUM\n\
     field 49 SenderCompID STRING\n\
     field 56 TargetCompID STRING\n\
     field 52 SendingTime UTCTIMESTAMP\n\
     field 10 CheckSum STRING\n\
     header 8 9 35 34 49 56 52\n\
     trailer 10\n\
     message 0 Heartbeat req: opt:\n";

fn dict_with_meta() -> String {
    format!("version FIX.4.4\nversion-meta major=4 minor=4\n{HEADER_TRAILER_FIELDS}")
}

fn dict_without_meta() -> String {
    format!("version FIX.4.4\n{HEADER_TRAILER_FIELDS}")
}

fn heartbeat(begin_string: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin_string));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

#[test]
fn version_meta_is_parsed_from_the_directive() {
    let d = parse(&dict_with_meta()).unwrap();
    let vm = d.version_meta().expect("version-meta should be present");
    assert_eq!(vm.major, 4);
    assert_eq!(vm.minor, 4);
    assert_eq!(vm.service_pack, None);
    assert_eq!(vm.extension_pack, None);
}

#[test]
fn version_meta_with_service_and_extension_pack() {
    let d = parse("version FIX.5.0SP2\nversion-meta major=5 minor=0 sp=2 ep=1\n").unwrap();
    let vm = d.version_meta().unwrap();
    assert_eq!(vm.major, 5);
    assert_eq!(vm.minor, 0);
    assert_eq!(vm.service_pack, Some(2));
    assert_eq!(vm.extension_pack, Some(1));
}

#[test]
fn a_matching_begin_string_passes() {
    let d = parse(&dict_with_meta()).unwrap();
    assert!(
        d.validate(&heartbeat("FIX.4.4"), &ValidationOptions::default())
            .is_ok()
    );
}

#[test]
fn a_mismatched_begin_string_is_rejected() {
    let d = parse(&dict_with_meta()).unwrap();
    let err = d
        .validate(&heartbeat("FIX.4.2"), &ValidationOptions::default())
        .unwrap_err();
    assert_eq!(err.reason, RejectReason::ValueIsIncorrect);
    assert_eq!(err.ref_tag, Some(8));
}

#[test]
fn no_version_meta_falls_back_to_the_plain_version_directive_and_still_rejects_a_mismatch() {
    // NEW-145 (audit 006): previously a no-op whenever `version_meta` was absent -- true for
    // every bundled dictionary, since none declares a `version-meta` directive, making the check
    // always skipped in practice (this test used to assert exactly that no-op). It now falls back
    // to parsing `self.version` (the plain `version FIX.M.N` directive `dict_without_meta` does
    // declare, here `FIX.4.4`), so a genuinely mismatched BeginString is still rejected.
    let d = parse(&dict_without_meta()).unwrap();
    assert!(d.version_meta().is_none());
    let err = d
        .validate(&heartbeat("FIX.9.9"), &ValidationOptions::default())
        .unwrap_err();
    assert_eq!(err.reason, RejectReason::ValueIsIncorrect);
    assert_eq!(err.ref_tag, Some(8));
}

#[test]
fn no_version_meta_fallback_still_accepts_a_matching_begin_string() {
    let d = parse(&dict_without_meta()).unwrap();
    assert!(d.version_meta().is_none());
    assert!(
        d.validate(&heartbeat("FIX.4.4"), &ValidationOptions::default())
            .is_ok()
    );
}

#[test]
fn a_non_fix_dot_shaped_begin_string_is_also_a_no_op() {
    // FIXT.1.1 (FIX 5.0+ transport) resolves its version via ApplVerID, a separate mechanism —
    // this check simply doesn't apply to it, not a false-positive mismatch.
    let d = parse(&dict_with_meta()).unwrap();
    assert!(
        d.validate(&heartbeat("FIXT.1.1"), &ValidationOptions::default())
            .is_ok()
    );
}
