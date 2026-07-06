//! T145 — `parse_fix_begin_string` distinguishes FIX 5.0/SP1/SP2 (NEW-45): a dictionary declaring
//! `version-meta ... sp=2` must not accept a message whose BeginString names a different (or
//! absent) service pack, mirroring `version_meta.rs`'s existing major/minor-mismatch coverage.

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

fn dict_sp2() -> String {
    format!("version FIX.5.0SP2\nversion-meta major=5 minor=0 sp=2\n{HEADER_TRAILER_FIELDS}")
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
fn fix50_service_packs_are_distinct_begin_strings() {
    let dictionary = parse(&dict_sp2()).unwrap();

    assert!(
        dictionary
            .validate(&heartbeat("FIX.5.0SP2"), &ValidationOptions::default())
            .is_ok()
    );
    for mismatched in ["FIX.5.0", "FIX.5.0SP1"] {
        let err = dictionary
            .validate(&heartbeat(mismatched), &ValidationOptions::default())
            .unwrap_err();
        assert_eq!(
            err.reason,
            RejectReason::ValueIsIncorrect,
            "{mismatched} must not match a FIX.5.0SP2 dictionary"
        );
        assert_eq!(err.ref_tag, Some(8));
    }
}
