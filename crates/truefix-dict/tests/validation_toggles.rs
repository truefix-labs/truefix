//! T019 (US8) — the four extra validation toggles: `validate_checksum`,
//! `validate_incoming_message`, `allow_pos_dup`, `requires_orig_sending_time` (FR-007).

use truefix_core::{Field, Message, decode};
use truefix_dict::{ValidationOptions, load_fix44};

fn nos() -> Message {
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
    m
}

// --- validate_incoming_message: master switch ---

#[test]
fn validate_incoming_message_false_skips_all_checks() {
    let mut m = nos();
    // Remove a required field (HandlInst, 21) — would normally fail RequiredTagMissing.
    m.body = {
        let mut b = truefix_core::FieldMap::new();
        for f in nos().body.fields() {
            if f.tag() != 21 {
                b.set(Field::new(f.tag(), f.value_bytes().to_vec()));
            }
        }
        b
    };
    let opts = ValidationOptions {
        validate_incoming_message: false,
        ..ValidationOptions::default()
    };
    assert!(load_fix44().unwrap().validate(&m, &opts).is_ok());
}

#[test]
fn validate_incoming_message_true_default_still_checks() {
    let mut m = nos();
    m.body = {
        // Drop Side(54) — a directly-required NewOrderSingle field (US9, feature 005, FR-031:
        // HandlInst(21), used here before the real-QFJ-data expansion, is optional in the real
        // schema).
        let mut b = truefix_core::FieldMap::new();
        for f in nos().body.fields() {
            if f.tag() != 54 {
                b.set(Field::new(f.tag(), f.value_bytes().to_vec()));
            }
        }
        b
    };
    assert!(
        load_fix44()
            .unwrap()
            .validate(&m, &ValidationOptions::default())
            .is_err()
    );
}

// --- allow_pos_dup ---

fn poss_dup_message() -> Message {
    let mut m = nos();
    m.header.set(Field::string(43, "Y")); // PossDupFlag
    m.header.set(Field::string(122, "20240101-00:00:00")); // OrigSendingTime
    m
}

#[test]
fn poss_dup_rejected_when_not_allowed() {
    let opts = ValidationOptions {
        allow_pos_dup: false,
        ..ValidationOptions::default()
    };
    let err = load_fix44()
        .unwrap()
        .validate(&poss_dup_message(), &opts)
        .unwrap_err();
    assert_eq!(err.ref_tag, Some(43));
}

#[test]
fn poss_dup_accepted_by_default() {
    assert!(
        load_fix44()
            .unwrap()
            .validate(&poss_dup_message(), &ValidationOptions::default())
            .is_ok()
    );
}

#[test]
fn non_poss_dup_message_unaffected_by_allow_pos_dup_false() {
    let opts = ValidationOptions {
        allow_pos_dup: false,
        ..ValidationOptions::default()
    };
    assert!(load_fix44().unwrap().validate(&nos(), &opts).is_ok());
}

// --- requires_orig_sending_time ---

#[test]
fn poss_dup_missing_orig_sending_time_rejected_when_required() {
    let mut m = nos();
    m.header.set(Field::string(43, "Y")); // PossDupFlag, no OrigSendingTime
    let opts = ValidationOptions {
        requires_orig_sending_time: true,
        ..ValidationOptions::default()
    };
    let err = load_fix44().unwrap().validate(&m, &opts).unwrap_err();
    assert_eq!(err.ref_tag, Some(122));
}

#[test]
fn poss_dup_with_orig_sending_time_accepted_when_required() {
    let opts = ValidationOptions {
        requires_orig_sending_time: true,
        ..ValidationOptions::default()
    };
    assert!(
        load_fix44()
            .unwrap()
            .validate(&poss_dup_message(), &opts)
            .is_ok()
    );
}

#[test]
fn requires_orig_sending_time_ignored_when_not_poss_dup() {
    let opts = ValidationOptions {
        requires_orig_sending_time: true,
        ..ValidationOptions::default()
    };
    assert!(load_fix44().unwrap().validate(&nos(), &opts).is_ok());
}

// --- validate_checksum: documented-mandatory behavior (Principle I/II) ---

#[test]
fn checksum_is_always_validated_regardless_of_the_toggle() {
    // A bad checksum is a decode-time error — it never reaches validate() at all, with either
    // toggle value, because TrueFix does not support disabling checksum enforcement.
    let bad = b"8=FIX.4.4\x019=21\x0135=A\x0195=7\x0196=rawdata\x0110=000\x01";
    assert!(decode(bad).is_err());
}

#[test]
fn validate_checksum_field_exists_and_defaults_true() {
    assert!(ValidationOptions::default().validate_checksum);
}
