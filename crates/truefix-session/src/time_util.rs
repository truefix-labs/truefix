//! UTC SendingTime formatting.

use time::OffsetDateTime;

/// Format the current UTC time as a FIX UTCTimestamp `YYYYMMDD-HH:MM:SS.sss` (millisecond
/// precision). Never panics.
pub(crate) fn now_utc_timestamp() -> String {
    format_utc_timestamp(OffsetDateTime::now_utc())
}

fn format_utc_timestamp(dt: OffsetDateTime) -> String {
    let ms = dt.millisecond();
    format!(
        "{:04}{:02}{:02}-{:02}:{:02}:{:02}.{:03}",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        ms
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_has_expected_shape() {
        let s = now_utc_timestamp();
        // YYYYMMDD-HH:MM:SS.mmm => 21 chars
        assert_eq!(s.len(), 21, "{s}");
        assert_eq!(s.as_bytes().get(8), Some(&b'-'));
    }
}
