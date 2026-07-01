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

    /// The value as a UTC timestamp. Accepts up to picosecond fractional digits and
    /// truncates to nanosecond resolution (FR-B7).
    pub fn as_utc_timestamp(&self) -> Result<OffsetDateTime, FieldError> {
        let s = self.as_str()?;
        parse_utc_timestamp(s).ok_or_else(|| FieldError::NotTimestamp {
            tag: self.tag,
            value: s.to_owned(),
        })
    }
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

    // Accept >= 1 fractional digits up to picosecond; truncate to nanosecond (first 9 digits).
    let nanos: u32 = match frac {
        None => 0,
        Some(f) => {
            if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let mut buf = [b'0'; 9];
            for (slot, digit) in buf.iter_mut().zip(f.bytes()) {
                *slot = digit;
            }
            core::str::from_utf8(&buf).ok()?.parse().ok()?
        }
    };

    let month = time::Month::try_from(month).ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    let time = time::Time::from_hms_nano(hour, min, sec, nanos).ok()?;
    Some(time::PrimitiveDateTime::new(date, time).assume_utc())
}
