//! T058/T059 (US2, feature 009, `NEW-20`): `MonthYear`/`LocalMktDate` previously fell into
//! `FieldType::value_ok`'s `_ => true` catch-all, letting a garbled value silently pass (same
//! class of gap `BUG-12`/feature 006 already fixed for `UtcTimeOnly`/`UtcDate`).

use truefix_core::Field;
use truefix_dict::FieldType;

fn field(value: &str) -> Field {
    Field::string(1, value)
}

#[test]
fn local_mkt_date_accepts_a_valid_yyyymmdd() {
    assert!(FieldType::LocalMktDate.value_ok(&field("20240115"), false));
}

#[test]
fn local_mkt_date_rejects_garbled_values() {
    assert!(!FieldType::LocalMktDate.value_ok(&field("not-a-date"), false));
    assert!(!FieldType::LocalMktDate.value_ok(&field("2024011"), false));
    assert!(!FieldType::LocalMktDate.value_ok(&field("20241315"), false)); // month 13
}

#[test]
fn month_year_accepts_bare_yyyymm() {
    assert!(FieldType::MonthYear.value_ok(&field("202401"), false));
}

#[test]
fn month_year_accepts_yyyymm_plus_day() {
    assert!(FieldType::MonthYear.value_ok(&field("20240115"), false));
}

#[test]
fn month_year_accepts_yyyymm_plus_week_descriptor() {
    assert!(FieldType::MonthYear.value_ok(&field("202401w1"), false));
    assert!(FieldType::MonthYear.value_ok(&field("202401w5"), false));
}

#[test]
fn month_year_rejects_garbled_values() {
    assert!(!FieldType::MonthYear.value_ok(&field("not-a-date"), false));
    assert!(!FieldType::MonthYear.value_ok(&field("20241301"), false)); // month 13
    assert!(!FieldType::MonthYear.value_ok(&field("202401w9"), false)); // no week 9
    assert!(!FieldType::MonthYear.value_ok(&field("2024011"), false)); // wrong length
}
