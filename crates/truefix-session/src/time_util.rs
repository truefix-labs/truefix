//! UTC SendingTime formatting.

use time::OffsetDateTime;

use crate::config::TimeStampPrecision;

/// Format the current UTC time at the given precision (FR-009).
pub(crate) fn now_utc_timestamp_prec(precision: TimeStampPrecision) -> String {
    format_utc_timestamp(OffsetDateTime::now_utc(), precision)
}

/// Format `dt` as `YYYYMMDD-HH:MM:SS[.frac]` with sub-second digits per `precision`.
fn format_utc_timestamp(dt: OffsetDateTime, precision: TimeStampPrecision) -> String {
    let base = format!(
        "{:04}{:02}{:02}-{:02}:{:02}:{:02}",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
    );
    let nanos = dt.nanosecond(); // 0..1_000_000_000
    match precision {
        TimeStampPrecision::Seconds => base,
        TimeStampPrecision::Milliseconds => format!("{base}.{:03}", nanos / 1_000_000),
        TimeStampPrecision::Microseconds => format!("{base}.{:06}", nanos / 1_000),
        TimeStampPrecision::Nanoseconds => format!("{base}.{nanos:09}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_has_expected_shape() {
        let s = now_utc_timestamp_prec(TimeStampPrecision::Milliseconds);
        // YYYYMMDD-HH:MM:SS.mmm => 21 chars
        assert_eq!(s.len(), 21, "{s}");
        assert_eq!(s.as_bytes().get(8), Some(&b'-'));
    }

    #[test]
    fn precision_controls_subsecond_digits() {
        let dt = OffsetDateTime::from_unix_timestamp_nanos(1_700_000_000_123_456_789).unwrap();
        assert_eq!(
            format_utc_timestamp(dt, TimeStampPrecision::Seconds).len(),
            17
        );
        assert!(format_utc_timestamp(dt, TimeStampPrecision::Milliseconds).ends_with(".123"));
        assert!(format_utc_timestamp(dt, TimeStampPrecision::Microseconds).ends_with(".123456"));
        assert!(format_utc_timestamp(dt, TimeStampPrecision::Nanoseconds).ends_with(".123456789"));
    }
}
