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

// --- T039/T040 (US7) — Data/UtcDateOnly/UtcTimeOnly round-trips (FR-011) ---

#[test]
fn data_field_round_trips_raw_bytes_including_embedded_soh() {
    let cases: &[&[u8]] = &[
        b"hello",
        b"",
        b"with\x01embedded\x01SOH",
        &[0u8, 1, 2, 255, 254, 0],
    ];
    for raw in cases {
        let f = Field::bytes(96, raw); // RawData
        assert_eq!(f.as_bytes(), *raw);
        assert_eq!(f.value_bytes(), *raw);
    }
}

#[test]
fn data_field_bytes_and_value_bytes_agree() {
    let f = Field::bytes(212, b"xml-data-length-prefixed");
    assert_eq!(f.as_bytes(), f.value_bytes());
}

#[test]
fn utc_date_only_round_trips_the_fix_wire_format() {
    let cases = [
        (2012, time::Month::March, 31, "20120331"),
        (1999, time::Month::January, 1, "19990101"),
        (2026, time::Month::December, 25, "20261225"),
    ];
    for (year, month, day, wire) in cases {
        let date = time::Date::from_calendar_date(year, month, day).unwrap();
        let f = Field::utc_date_only(453, date);
        assert_eq!(f.as_str().unwrap(), wire);
        let parsed = f.as_utc_date_only().unwrap();
        assert_eq!(parsed, date);
    }
}

#[test]
fn utc_date_only_invalid_values_error_never_panic() {
    assert!(Field::string(453, "not-a-date").as_utc_date_only().is_err());
    assert!(Field::string(453, "20121331") // month 13
        .as_utc_date_only()
        .is_err());
    assert!(Field::string(453, "2012033") // too short
        .as_utc_date_only()
        .is_err());
    assert!(Field::string(453, "").as_utc_date_only().is_err());
}

#[test]
fn utc_time_only_round_trips_the_fix_wire_format_at_millisecond_precision() {
    let time = time::Time::from_hms_milli(10, 15, 30, 444).unwrap();
    let f = Field::utc_time_only(273, time);
    assert_eq!(f.as_str().unwrap(), "10:15:30.444");
    assert_eq!(f.as_utc_time_only().unwrap(), time);
}

#[test]
fn utc_time_only_accepts_no_fraction_and_truncates_picoseconds() {
    let no_frac = Field::string(273, "10:15:30").as_utc_time_only().unwrap();
    assert_eq!(no_frac.millisecond(), 0);

    let with_ps = Field::string(273, "10:15:30.123456789012")
        .as_utc_time_only()
        .unwrap();
    assert_eq!(with_ps.nanosecond(), 123_456_789);
}

#[test]
fn utc_time_only_invalid_values_error_never_panic() {
    assert!(Field::string(273, "not-a-time").as_utc_time_only().is_err());
    assert!(Field::string(273, "25:00:00") // hour 25
        .as_utc_time_only()
        .is_err());
    assert!(Field::string(273, "").as_utc_time_only().is_err());
}
