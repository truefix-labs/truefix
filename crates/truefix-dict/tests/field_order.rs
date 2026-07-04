//! T018 (US3) — `ValidationOptions::validate_fields_out_of_order` gates `validate()`'s reaction to
//! `Message::fields_out_of_order` (set by `truefix-core::decode` when header/body/trailer wire
//! sectioning is violated). FR-006.

use truefix_core::decode;
use truefix_dict::{ValidationOptions, load_fix44};

/// A well-formed FIX.4.4 NewOrderSingle, with `rest` controlling the field order after
/// BeginString/BodyLength (which `decode` requires to always be first two). BodyLength/CheckSum
/// are computed correctly regardless of `rest`'s order.
fn raw_new_order_single(rest: &[(u32, &str)]) -> Vec<u8> {
    let body: String = rest.iter().map(|(t, v)| format!("{t}={v}\x01")).collect();
    let prefix = format!("8=FIX.4.4\x019={}\x01", body.len());
    let pre_checksum = format!("{prefix}{body}");
    let checksum: u32 = pre_checksum.bytes().map(u32::from).sum::<u32>() & 0xFF;
    format!("{pre_checksum}10={checksum:03}\x01").into_bytes()
}

const IN_ORDER: &[(u32, &str)] = &[
    (35, "D"),
    (49, "CLIENT"),
    (56, "SERVER"),
    (34, "2"),
    (52, "20240101-00:00:00"),
    (11, "O1"),
    (21, "1"),
    (55, "AAPL"),
    (54, "1"),
    (60, "20240101-00:00:00"),
    (40, "2"),
];

const OUT_OF_ORDER: &[(u32, &str)] = &[
    (35, "D"),
    (55, "AAPL"), // body field arrives before the header section (49/56/...) is done
    (49, "CLIENT"),
    (56, "SERVER"),
    (34, "2"),
    (52, "20240101-00:00:00"),
    (11, "O1"),
    (21, "1"),
    (54, "1"),
    (60, "20240101-00:00:00"),
    (40, "2"),
];

#[test]
fn out_of_order_fields_rejected_when_toggle_enabled() {
    let msg = decode(&raw_new_order_single(OUT_OF_ORDER)).unwrap();
    assert!(msg.fields_out_of_order());
    let opts = ValidationOptions {
        validate_fields_out_of_order: true,
        ..ValidationOptions::default()
    };
    let err = load_fix44().unwrap().validate(&msg, &opts).unwrap_err();
    assert_eq!(err.reason.code(), 14); // TagOutOfRequiredOrder
}

#[test]
fn out_of_order_fields_accepted_when_toggle_disabled_default() {
    let msg = decode(&raw_new_order_single(OUT_OF_ORDER)).unwrap();
    assert!(msg.fields_out_of_order());
    // Default ValidationOptions has validate_fields_out_of_order = false.
    assert!(
        load_fix44()
            .unwrap()
            .validate(&msg, &ValidationOptions::default())
            .is_ok()
    );
}

#[test]
fn in_order_message_passes_regardless_of_toggle() {
    let msg = decode(&raw_new_order_single(IN_ORDER)).unwrap();
    assert!(!msg.fields_out_of_order());
    let opts = ValidationOptions {
        validate_fields_out_of_order: true,
        ..ValidationOptions::default()
    };
    assert!(load_fix44().unwrap().validate(&msg, &opts).is_ok());
}
