//! T105/T106 (US3, feature 007): timestamp/time-only parsing rejects a fractional-seconds digit
//! count other than 0/3/6/9 (BUG-48, FR-046), matching QFJ; and a leap second (`ss=60`) maps to 59
//! seconds plus the maximum sub-second fraction rather than being rejected outright (BUG-78,
//! FR-046), also matching QFJ's explicit `s == 60` handling. Previously any digit count >= 1 was
//! accepted (truncated to nanoseconds), and `sec == 60` was rejected unconditionally by
//! `time::Time::from_hms_nano`.

use truefix_core::Field;

fn timestamp(frac: &str) -> Field {
    Field::string(52, &format!("20240101-12:00:00{frac}"))
}

fn time_only(frac: &str) -> Field {
    Field::string(273, &format!("12:00:00{frac}"))
}

#[test]
fn timestamp_rejects_every_non_0_3_6_9_digit_count() {
    for digits in 1..=11 {
        if matches!(digits, 3 | 6 | 9) {
            continue;
        }
        let frac = format!(".{}", "1".repeat(digits));
        assert!(
            timestamp(&frac).as_utc_timestamp().is_err(),
            "{digits}-digit fraction must be rejected"
        );
    }
}

#[test]
fn timestamp_accepts_exactly_0_3_6_9_digit_counts() {
    assert!(Field::string(52, "20240101-12:00:00")
        .as_utc_timestamp()
        .is_ok());
    for digits in [3, 6, 9] {
        let frac = format!(".{}", "1".repeat(digits));
        assert!(
            timestamp(&frac).as_utc_timestamp().is_ok(),
            "{digits}-digit fraction must be accepted"
        );
    }
}

#[test]
fn time_only_rejects_every_non_0_3_6_9_digit_count() {
    for digits in 1..=11 {
        if matches!(digits, 3 | 6 | 9) {
            continue;
        }
        let frac = format!(".{}", "1".repeat(digits));
        assert!(
            time_only(&frac).as_utc_time_only().is_err(),
            "{digits}-digit fraction must be rejected"
        );
    }
}

#[test]
fn time_only_accepts_exactly_0_3_6_9_digit_counts() {
    assert!(Field::string(273, "12:00:00").as_utc_time_only().is_ok());
    for digits in [3, 6, 9] {
        let frac = format!(".{}", "1".repeat(digits));
        assert!(
            time_only(&frac).as_utc_time_only().is_ok(),
            "{digits}-digit fraction must be accepted"
        );
    }
}

#[test]
fn timestamp_leap_second_maps_to_59_and_max_fraction() {
    let t = Field::string(52, "20240101-23:59:60")
        .as_utc_timestamp()
        .unwrap();
    assert_eq!(t.hour(), 23);
    assert_eq!(t.minute(), 59);
    assert_eq!(t.second(), 59);
    assert_eq!(t.nanosecond(), 999_999_999);
}

#[test]
fn timestamp_leap_second_with_a_fraction_still_maps_to_max_fraction() {
    // Even an explicit sub-second fraction on a leap second is superseded by the maximum
    // fraction mapping -- QFJ's own leap-second handling doesn't attempt to combine the two.
    let t = Field::string(52, "20240101-23:59:60.500")
        .as_utc_timestamp()
        .unwrap();
    assert_eq!(t.second(), 59);
    assert_eq!(t.nanosecond(), 999_999_999);
}

#[test]
fn time_only_leap_second_maps_to_59_and_max_fraction() {
    let t = Field::string(273, "23:59:60").as_utc_time_only().unwrap();
    assert_eq!(t.second(), 59);
    assert_eq!(t.nanosecond(), 999_999_999);
}

#[test]
fn a_second_value_beyond_60_is_still_rejected() {
    assert!(Field::string(52, "20240101-12:00:61")
        .as_utc_timestamp()
        .is_err());
    assert!(Field::string(273, "12:00:61").as_utc_time_only().is_err());
}
