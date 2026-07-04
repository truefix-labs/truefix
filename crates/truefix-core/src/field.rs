//! FIX fields and their typed value converters.

use rust_decimal::Decimal;
use time::OffsetDateTime;

use crate::error::FieldError;

/// A single FIX field: a tag number and its raw (wire) value bytes.
///
/// Raw bytes are preserved verbatim so re-encoding reproduces the original representation
/// (round-trip fidelity, FR-B9). Typed accessors convert on demand and never panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    tag: u32,
    value: Vec<u8>,
}

impl Field {
    /// Create a field from a tag and raw value bytes.
    pub fn new(tag: u32, value: impl Into<Vec<u8>>) -> Self {
        Self {
            tag,
            value: value.into(),
        }
    }

    /// Create a field from a tag and a string value.
    pub fn string(tag: u32, value: &str) -> Self {
        Self::new(tag, value.as_bytes().to_vec())
    }

    /// Create an integer-valued field.
    pub fn int(tag: u32, value: i64) -> Self {
        Self::new(tag, value.to_string().into_bytes())
    }

    /// Create a UTC-timestamp-valued field at millisecond precision
    /// (`YYYYMMDD-HH:MM:SS.sss`) — the FIX wire format, which does *not* match
    /// `OffsetDateTime`'s own `Display` output. Sessions needing other precisions format their
    /// own SendingTime; this constructor is the safe default for typed-message setters.
    pub fn utc_timestamp(tag: u32, value: OffsetDateTime) -> Self {
        Self::string(tag, &format_utc_timestamp_millis(value))
    }

    /// Create a `Data`-typed field from raw bytes (e.g. `RawData`/tag 96), preserving them
    /// verbatim — including embedded SOH — since Data fields are not text (FR-011). A thin,
    /// semantically-named wrapper over [`Self::new`]/[`Self::value_bytes`], distinguishing "this is
    /// a Data field" call sites from generic byte construction.
    pub fn bytes(tag: u32, value: &[u8]) -> Self {
        Self::new(tag, value.to_vec())
    }

    /// The value as raw bytes, with no UTF-8 assumption (FIX `Data` fields; FR-011). Equivalent to
    /// [`Self::value_bytes`], named for symmetry with [`Self::bytes`].
    pub fn as_bytes(&self) -> &[u8] {
        &self.value
    }

    /// Create a UTC-date-only-valued field (`YYYYMMDD`; FR-011).
    pub fn utc_date_only(tag: u32, value: time::Date) -> Self {
        Self::string(tag, &format_utc_date_only(value))
    }

    /// The value as a UTC date-only (`YYYYMMDD`; FR-011).
    pub fn as_utc_date_only(&self) -> Result<time::Date, FieldError> {
        let s = self.as_str()?;
        parse_utc_date_only(s).ok_or_else(|| FieldError::NotDateOnly {
            tag: self.tag,
            value: s.to_owned(),
        })
    }

    /// Create a UTC-time-only-valued field at millisecond precision (`HH:MM:SS.sss`; FR-011),
    /// matching [`Self::utc_timestamp`]'s precision convention.
    pub fn utc_time_only(tag: u32, value: time::Time) -> Self {
        Self::string(tag, &format_utc_time_only_millis(value))
    }

    /// The value as a UTC time-only (`HH:MM:SS[.fraction]`; FR-011). Accepts only 0/3/6/9
    /// fractional digits (BUG-48/FR-046, feature 007) and maps a leap second to `59` plus the
    /// maximum sub-second fraction (BUG-78), matching [`Self::as_utc_timestamp`]'s tolerance.
    pub fn as_utc_time_only(&self) -> Result<time::Time, FieldError> {
        let s = self.as_str()?;
        parse_utc_time_only(s).ok_or_else(|| FieldError::NotTimeOnly {
            tag: self.tag,
            value: s.to_owned(),
        })
    }

    /// The field's tag number.
    pub fn tag(&self) -> u32 {
        self.tag
    }

    /// The raw value bytes.
    pub fn value_bytes(&self) -> &[u8] {
        &self.value
    }

    /// The value as a UTF-8 string slice.
    pub fn as_str(&self) -> Result<&str, FieldError> {
        core::str::from_utf8(&self.value).map_err(|_| FieldError::NotUtf8 { tag: self.tag })
    }

    /// The value as an integer.
    pub fn as_int(&self) -> Result<i64, FieldError> {
        let s = self.as_str()?;
        s.trim().parse().map_err(|_| FieldError::NotInt {
            tag: self.tag,
            value: s.to_owned(),
        })
    }

    /// The value as a fixed-precision decimal (FIX Price/Qty/Amt).
    pub fn as_decimal(&self) -> Result<Decimal, FieldError> {
        let s = self.as_str()?;
        s.trim().parse().map_err(|_| FieldError::NotDecimal {
            tag: self.tag,
            value: s.to_owned(),
        })
    }

    /// The value as a single character (FIX char).
    pub fn as_char(&self) -> Result<char, FieldError> {
        let s = self.as_str()?;
        let mut chars = s.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Ok(c),
            _ => Err(FieldError::NotChar {
                tag: self.tag,
                value: s.to_owned(),
            }),
        }
    }

    /// The value as a boolean (`Y`/`N`).
    pub fn as_bool(&self) -> Result<bool, FieldError> {
        match self.value.as_slice() {
            b"Y" => Ok(true),
            b"N" => Ok(false),
            _ => Err(FieldError::NotBool {
                tag: self.tag,
                value: String::from_utf8_lossy(&self.value).into_owned(),
            }),
        }
    }

    /// The value as a UTC timestamp (FR-B7). Accepts only 0/3/6/9 fractional digits (BUG-48/
    /// FR-046, feature 007), matching QFJ; a leap second (`sec == 60`) maps to `59` plus the
    /// maximum sub-second fraction (BUG-78), matching QFJ's explicit handling.
    pub fn as_utc_timestamp(&self) -> Result<OffsetDateTime, FieldError> {
        let s = self.as_str()?;
        parse_utc_timestamp(s).ok_or_else(|| FieldError::NotTimestamp {
            tag: self.tag,
            value: s.to_owned(),
        })
    }
}

/// Format `dt` as a FIX UTCTimestamp `YYYYMMDD-HH:MM:SS.sss` at millisecond precision.
fn format_utc_timestamp_millis(dt: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}-{:02}:{:02}:{:02}.{:03}",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        dt.millisecond()
    )
}

/// Parse `YYYYMMDD-HH:MM:SS[.fraction]` as UTC. Returns `None` on any malformation.
fn parse_utc_timestamp(s: &str) -> Option<OffsetDateTime> {
    let (date, time_part) = s.split_once('-')?;
    if date.len() != 8 {
        return None;
    }
    let year: i32 = date.get(0..4)?.parse().ok()?;
    let month: u8 = date.get(4..6)?.parse().ok()?;
    let day: u8 = date.get(6..8)?.parse().ok()?;

    let (hms, frac) = match time_part.split_once('.') {
        Some((h, f)) => (h, Some(f)),
        None => (time_part, None),
    };
    let mut parts = hms.split(':');
    let hour: u8 = parts.next()?.parse().ok()?;
    let min: u8 = parts.next()?.parse().ok()?;
    let sec: u8 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }

    // BUG-48/FR-046 (feature 007): only 0 (no fraction)/3/6/9 fractional digits are accepted,
    // matching QFJ -- previously any digit count >= 1 was accepted (e.g. a single-digit
    // `.1`), which QFJ rejects. TrueFix's own `TimeStampPrecision` only goes up to nanoseconds (9
    // digits) in the first place, so 12 (picoseconds) isn't representable regardless.
    let nanos: u32 = match frac {
        None => 0,
        Some(f) if matches!(f.len(), 3 | 6 | 9) && f.bytes().all(|b| b.is_ascii_digit()) => {
            let mut buf = [b'0'; 9];
            for (slot, digit) in buf.iter_mut().zip(f.bytes()) {
                *slot = digit;
            }
            core::str::from_utf8(&buf).ok()?.parse().ok()?
        }
        Some(_) => return None,
    };

    let month = time::Month::try_from(month).ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    // BUG-78/FR-046 (feature 007): a leap second (`sec == 60`) maps to `59` seconds plus the
    // maximum sub-second fraction, matching QFJ's explicit `s == 60` handling -- `time::Time::
    // from_hms_nano` otherwise rejects `sec == 60` outright, unlike QFJ which tolerates it.
    let (sec, nanos) = if sec == 60 {
        (59, 999_999_999)
    } else {
        (sec, nanos)
    };
    let time = time::Time::from_hms_nano(hour, min, sec, nanos).ok()?;
    Some(time::PrimitiveDateTime::new(date, time).assume_utc())
}

/// Format `date` as a FIX UtcDateOnly `YYYYMMDD`.
fn format_utc_date_only(date: time::Date) -> String {
    format!(
        "{:04}{:02}{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// Parse `YYYYMMDD` as a UTC date-only. Returns `None` on any malformation.
fn parse_utc_date_only(s: &str) -> Option<time::Date> {
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: i32 = s.get(0..4)?.parse().ok()?;
    let month: u8 = s.get(4..6)?.parse().ok()?;
    let day: u8 = s.get(6..8)?.parse().ok()?;
    let month = time::Month::try_from(month).ok()?;
    time::Date::from_calendar_date(year, month, day).ok()
}

/// Format `t` as a FIX UtcTimeOnly `HH:MM:SS.sss` at millisecond precision.
fn format_utc_time_only_millis(t: time::Time) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        t.hour(),
        t.minute(),
        t.second(),
        t.millisecond()
    )
}

/// Parse `HH:MM:SS[.fraction]` as a UTC time-only. Returns `None` on any malformation. Accepts
/// only 0/3/6/9 fractional digits, and maps a leap second (`sec == 60`) to `59` plus the maximum
/// sub-second fraction (matching `parse_utc_timestamp`'s BUG-48/BUG-78 tolerance, FR-046).
fn parse_utc_time_only(s: &str) -> Option<time::Time> {
    let (hms, frac) = match s.split_once('.') {
        Some((h, f)) => (h, Some(f)),
        None => (s, None),
    };
    let mut parts = hms.split(':');
    let hour: u8 = parts.next()?.parse().ok()?;
    let min: u8 = parts.next()?.parse().ok()?;
    let sec: u8 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }

    let nanos: u32 = match frac {
        None => 0,
        Some(f) if matches!(f.len(), 3 | 6 | 9) && f.bytes().all(|b| b.is_ascii_digit()) => {
            let mut buf = [b'0'; 9];
            for (slot, digit) in buf.iter_mut().zip(f.bytes()) {
                *slot = digit;
            }
            core::str::from_utf8(&buf).ok()?.parse().ok()?
        }
        Some(_) => return None,
    };

    let (sec, nanos) = if sec == 60 {
        (59, 999_999_999)
    } else {
        (sec, nanos)
    };
    time::Time::from_hms_nano(hour, min, sec, nanos).ok()
}
