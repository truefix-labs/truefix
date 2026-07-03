//! T044 — validation toggles (one case per toggle).

use truefix_core::{Field, Message};
use truefix_dict::{load_fix44, DataDictionary, RejectReason, ValidationOptions};

fn nos() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, 2));
    m.header.set(Field::string(49, "A"));
    m.header.set(Field::string(56, "B"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(11, "ORD1")); // ClOrdID
    m.body.set(Field::string(21, "1")); // HandlInst
    m.body.set(Field::string(55, "AAPL")); // Symbol
    m.body.set(Field::string(54, "1")); // Side
    m.body.set(Field::string(60, "20240101-00:00:00")); // TransactTime
    m.body.set(Field::string(40, "2")); // OrdType
    m.body.set(Field::string(38, "100")); // OrderQty
    m.body.set(Field::string(44, "150.25")); // Price
    m.trailer.set(Field::string(10, "000"));
    m
}

fn dict() -> DataDictionary {
    load_fix44().unwrap()
}

#[test]
fn valid_message_passes() {
    assert!(dict()
        .validate(&nos(), &ValidationOptions::default())
        .is_ok());
}

#[test]
fn required_field_missing() {
    let mut m = nos();
    m.body = {
        // rebuild body without Side(54) — a directly-required NewOrderSingle field. Symbol(55) is
        // no longer usable for this: it's a member of the `Instrument` component, which
        // NewOrderSingle references as optional (US9, feature 005, FR-031/GAP-24 — a component's
        // own individual fields are never unconditionally required in the real QFJ schema).
        let mut b = truefix_core::FieldMap::new();
        for f in nos().body.fields() {
            if f.tag() != 54 {
                b.set(Field::new(f.tag(), f.value_bytes().to_vec()));
            }
        }
        b
    };
    let err = dict()
        .validate(&m, &ValidationOptions::default())
        .unwrap_err();
    assert_eq!(err.reason, RejectReason::RequiredTagMissing);
}

#[test]
fn unknown_tag_rejected_unless_allowed() {
    let mut m = nos();
    m.body.set(Field::string(999, "x")); // not in dictionary, below the UDF range
    let opts = ValidationOptions::default();
    let err = dict().validate(&m, &opts).unwrap_err();
    assert_eq!(err.reason, RejectReason::InvalidTagNumber);

    let lenient = ValidationOptions {
        allow_unknown_msg_fields: true,
        ..ValidationOptions::default()
    };
    assert!(dict().validate(&m, &lenient).is_ok());
}

#[test]
fn user_defined_field_skipped_unless_validated() {
    let mut m = nos();
    m.body.set(Field::string(6001, "custom")); // UDF (>= 5000)
                                               // default: UDFs skipped -> ok
    assert!(dict().validate(&m, &ValidationOptions::default()).is_ok());
    // validate UDFs -> unknown tag rejected
    let strict = ValidationOptions {
        validate_user_defined_fields: true,
        ..ValidationOptions::default()
    };
    assert!(dict().validate(&m, &strict).is_err());
}

#[test]
fn bad_enum_value_is_incorrect() {
    let mut m = nos();
    m.body.set(Field::string(54, "Z")); // not a real Side value
    let err = dict()
        .validate(&m, &ValidationOptions::default())
        .unwrap_err();
    assert_eq!(err.reason, RejectReason::ValueIsIncorrect);
}

#[test]
fn bad_format_is_incorrect_data_format() {
    let mut m = nos();
    m.body.set(Field::string(38, "abc")); // OrderQty must be decimal
    let err = dict()
        .validate(&m, &ValidationOptions::default())
        .unwrap_err();
    assert_eq!(err.reason, RejectReason::IncorrectDataFormat);
}

#[test]
fn tag_not_defined_for_message_type() {
    let mut m = nos();
    m.body.set(Field::int(7, 1)); // BeginSeqNo is defined but not part of NewOrderSingle
    let err = dict()
        .validate(&m, &ValidationOptions::default())
        .unwrap_err();
    assert_eq!(err.reason, RejectReason::TagNotDefinedForMessageType);
}

#[test]
fn empty_value_rejected_when_checked() {
    let mut m = nos();
    m.body.set(Field::string(44, "")); // empty Price
    let err = dict()
        .validate(&m, &ValidationOptions::default())
        .unwrap_err();
    assert_eq!(err.reason, RejectReason::TagSpecifiedWithoutValue);
}
