//! T009 — table-driven field type / converter tests.

use truefix_core::Field;

#[test]
fn typed_conversions_succeed() {
    let cases_int = [(38u32, "100", 100i64), (34, "1", 1), (34, "-5", -5)];
    for (tag, raw, want) in cases_int {
        assert_eq!(Field::string(tag, raw).as_int().unwrap(), want, "tag {tag}");
    }

    assert_eq!(
        Field::string(44, "150.25")
            .as_decimal()
            .unwrap()
            .to_string(),
        "150.25"
    );
    assert_eq!(Field::string(35, "D").as_char().unwrap(), 'D');
    assert!(Field::string(43, "Y").as_bool().unwrap());
    assert!(!Field::string(43, "N").as_bool().unwrap());
    assert_eq!(Field::string(55, "AAPL").as_str().unwrap(), "AAPL");
    assert_eq!(Field::int(38, 100).value_bytes(), b"100");
}

#[test]
fn bad_conversions_error_never_panic() {
    assert!(Field::string(38, "x").as_int().is_err());
    assert!(Field::string(44, "x").as_decimal().is_err());
    assert!(Field::string(35, "AB").as_char().is_err());
    assert!(Field::string(35, "").as_char().is_err());
    assert!(Field::string(43, "Maybe").as_bool().is_err());
}

#[test]
fn utc_timestamp_nanosecond_and_picosecond_truncation() {
    let with_ms = Field::string(52, "20120331-10:15:30.444")
        .as_utc_timestamp()
        .unwrap();
    assert_eq!(with_ms.year(), 2012);
    assert_eq!(with_ms.nanosecond(), 444_000_000);

    // Picosecond input accepted, stored truncated to nanosecond (FR-B7).
    let with_ps = Field::string(52, "20120331-10:15:30.123456789012")
        .as_utc_timestamp()
        .unwrap();
    assert_eq!(with_ps.nanosecond(), 123_456_789);

    let no_frac = Field::string(52, "20120331-10:15:30")
        .as_utc_timestamp()
        .unwrap();
    assert_eq!(no_frac.nanosecond(), 0);
}

#[test]
fn invalid_timestamp_errors() {
    assert!(Field::string(52, "not-a-time").as_utc_timestamp().is_err());
    assert!(Field::string(52, "20121331-10:15:30") // month 13
        .as_utc_timestamp()
        .is_err());
    assert!(Field::string(52, "20120331-25:00:00") // hour 25
        .as_utc_timestamp()
        .is_err());
}
