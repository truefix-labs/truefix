//! T059/T060 (US9, feature 005) — format validation for the 11 new `FieldType` variants
//! (FR-022) and the open-enum sentinel (FR-023).

use truefix_core::Field;
use truefix_dict::FieldType;

fn field(value: &str) -> Field {
    Field::string(1, value)
}

#[test]
fn price_offset_accepts_decimals_and_rejects_non_numeric() {
    assert!(FieldType::PriceOffset.value_ok(&field("-0.05")));
    assert!(FieldType::PriceOffset.value_ok(&field("1.5")));
    assert!(!FieldType::PriceOffset.value_ok(&field("abc")));
}

#[test]
fn local_mkt_date_and_utc_date_are_format_accepted() {
    // LocalMktDate is accepted as-is at this layer (no format check); UtcDate IS now format-
    // checked (BUG-12/FR-030, feature 006) -- this asserts the positive case still passes.
    assert!(FieldType::LocalMktDate.value_ok(&field("20240101")));
    assert!(FieldType::UtcDate.value_ok(&field("20240101")));
}

// --- T065 (US7, feature 006): UtcTimeOnly/UtcDate format validation (BUG-12/FR-030) ---

#[test]
fn utc_time_only_rejects_a_garbled_value() {
    assert!(FieldType::UtcTimeOnly.value_ok(&field("12:00:00")));
    assert!(!FieldType::UtcTimeOnly.value_ok(&field("not-a-time")));
}

#[test]
fn utc_date_rejects_a_garbled_value() {
    assert!(FieldType::UtcDate.value_ok(&field("20240101")));
    assert!(!FieldType::UtcDate.value_ok(&field("not-a-date")));
}

#[test]
fn day_of_month_accepts_integers_and_rejects_non_numeric() {
    assert!(FieldType::DayOfMonth.value_ok(&field("15")));
    assert!(!FieldType::DayOfMonth.value_ok(&field("thirty")));
}

#[test]
fn time_accepts_utc_timestamp_shaped_values() {
    assert!(FieldType::Time.value_ok(&field("20240101-12:00:00")));
    assert!(!FieldType::Time.value_ok(&field("not-a-time")));
}

#[test]
fn currency_accepts_three_uppercase_letters_only() {
    assert!(FieldType::Currency.value_ok(&field("USD")));
    assert!(!FieldType::Currency.value_ok(&field("usd"))); // lowercase rejected
    assert!(!FieldType::Currency.value_ok(&field("US"))); // too short
    assert!(!FieldType::Currency.value_ok(&field("USDX"))); // too long
}

#[test]
fn country_accepts_two_uppercase_letters_only() {
    assert!(FieldType::Country.value_ok(&field("US")));
    assert!(!FieldType::Country.value_ok(&field("USA")));
    assert!(!FieldType::Country.value_ok(&field("us")));
}

#[test]
fn exchange_accepts_up_to_four_alphanumeric_characters() {
    assert!(FieldType::Exchange.value_ok(&field("N")));
    assert!(FieldType::Exchange.value_ok(&field("XNYS")));
    assert!(!FieldType::Exchange.value_ok(&field("TOOLONG")));
    assert!(!FieldType::Exchange.value_ok(&field("")));
}

#[test]
fn multiple_char_value_accepts_space_separated_single_characters() {
    assert!(FieldType::MultipleCharValue.value_ok(&field("A B C")));
    assert!(FieldType::MultipleCharValue.value_ok(&field("A")));
    assert!(!FieldType::MultipleCharValue.value_ok(&field("AB C")));
}

#[test]
fn multiple_value_string_and_multiple_string_value_are_accepted_at_the_type_layer() {
    // Per-token enum-membership checking is FieldDef::allows's job (see open_enum_and_multi_value
    // below), not FieldType::value_ok's — this layer only checks the *format*, which is free-form.
    assert!(FieldType::MultipleValueString.value_ok(&field("A B C")));
    assert!(FieldType::MultipleStringValue.value_ok(&field("A B C")));
}

#[test]
fn field_type_parse_recognizes_all_11_new_type_tokens() {
    for (token, expected) in [
        ("PRICEOFFSET", FieldType::PriceOffset),
        ("LOCALMKTDATE", FieldType::LocalMktDate),
        ("DAYOFMONTH", FieldType::DayOfMonth),
        ("UTCDATE", FieldType::UtcDate),
        ("TIME", FieldType::Time),
        ("CURRENCY", FieldType::Currency),
        ("EXCHANGE", FieldType::Exchange),
        ("MULTIPLEVALUESTRING", FieldType::MultipleValueString),
        ("MULTIPLESTRINGVALUE", FieldType::MultipleStringValue),
        ("MULTIPLECHARVALUE", FieldType::MultipleCharValue),
        ("COUNTRY", FieldType::Country),
    ] {
        assert_eq!(FieldType::parse(token), Some(expected), "for token {token}");
    }
}

// --- T060 (US9, feature 005): open-enum acceptance (FR-023) ---

const DICT_SRC: &str = "version FIX.4.4\n\
     field 1 ClosedEnum STRING A B C\n\
     field 2 OpenEnum STRING open A B C\n\
     field 3 OpenMultiValue MULTIPLEVALUESTRING open A B\n\
     field 4 ClosedMultiValue MULTIPLEVALUESTRING A B\n";

#[test]
fn a_closed_enum_rejects_an_out_of_list_value() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let f = d.field(1).unwrap();
    assert!(f.allows("A"));
    assert!(!f.allows("Z"));
}

#[test]
fn an_open_enum_accepts_an_out_of_list_value() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let f = d.field(2).unwrap();
    assert!(f.open_enum);
    assert!(f.allows("A"));
    assert!(f.allows("Z")); // out-of-list, but the field is open
}

#[test]
fn an_open_multiple_value_string_accepts_any_token() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let f = d.field(3).unwrap();
    assert!(f.allows("A B")); // both in-list
    assert!(f.allows("A Z")); // Z out-of-list, but the field is open
}

#[test]
fn a_closed_multiple_value_string_checks_every_token() {
    let d = truefix_dict::parse(DICT_SRC).unwrap();
    let f = d.field(4).unwrap();
    assert!(f.allows("A B"));
    assert!(f.allows("A"));
    assert!(!f.allows("A Z")); // Z out-of-list and the field is closed
}

// --- T066 (US9, feature 005): value -> label lookup (FR-030) ---

#[test]
fn a_labeled_enum_value_round_trips_its_label() {
    let d = truefix_dict::parse("version FIX.4.4\nfield 54 Side CHAR 1=Buy 2=Sell\n").unwrap();
    let side = d.field(54).unwrap();
    assert_eq!(side.label("1"), Some("Buy"));
    assert_eq!(side.label("2"), Some("Sell"));
    assert!(side.allows("1")); // membership checking still uses the raw value, not the label
}

#[test]
fn an_unlabeled_enum_value_has_no_label_but_is_still_allowed() {
    let d = truefix_dict::parse("version FIX.4.4\nfield 54 Side CHAR 1=Buy 2\n").unwrap();
    let side = d.field(54).unwrap();
    assert_eq!(side.label("2"), None);
    assert!(side.allows("2"));
}
