//! T045 — two rejection layers: session-level (dictionary/validation) vs business-level.

use truefix_core::{Field, Message};
use truefix_dict::{RejectReason, ValidationOptions, load_fix44};

fn base(msg_type: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, 2));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.trailer.set(Field::string(10, "000"));
    m
}

#[test]
fn dictionary_failure_is_session_level() {
    let d = load_fix44().unwrap();
    let mut m = base("D");
    m.body.set(Field::string(11, "ORD1"));
    m.body.set(Field::string(21, "1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::string(54, "Z")); // bad enum -> validation failure
    m.body.set(Field::string(60, "20240101-00:00:00"));
    m.body.set(Field::string(40, "2"));
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert!(!err.business, "field validation failure is session-level");
    assert_eq!(err.reason, RejectReason::ValueIsIncorrect);
}

#[test]
fn unknown_msg_type_is_business_level() {
    let d = load_fix44().unwrap();
    let m = base("ZZ"); // structurally valid but unknown message type
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert!(
        err.business,
        "unsupported MsgType is a business-level reject"
    );
    assert_eq!(err.reason, RejectReason::InvalidMsgType);
}

#[test]
fn missing_msg_type_is_session_level() {
    let d = load_fix44().unwrap();
    let mut m = base("0");
    m.header = {
        // drop MsgType(35)
        let mut h = truefix_core::FieldMap::new();
        for f in base("0").header.fields() {
            if f.tag() != 35 {
                h.set(Field::new(f.tag(), f.value_bytes().to_vec()));
            }
        }
        h
    };
    let err = d.validate(&m, &ValidationOptions::default()).unwrap_err();
    assert!(!err.business);
    assert_eq!(err.reason, RejectReason::InvalidMsgType);
}
