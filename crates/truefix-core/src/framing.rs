//! Extract complete FIX messages from a byte stream.

use crate::error::DecodeError;
use crate::tags::SOH;

/// BUG-13/FR-024 (feature 006): a sane maximum declared `BodyLength` (tag 9). No legitimate FIX
/// message (including large repeating-group or `RawData`-bearing ones) approaches this; a
/// connection declaring more is closed instead of growing an unbounded read buffer while waiting
/// for that many bytes to arrive — a straightforward memory-exhaustion DoS vector otherwise
/// reachable pre- or post-Logon on any open acceptor port.
pub const MAX_BODY_LEN: usize = 16 * 1024 * 1024;

/// If `buf` begins with at least one complete FIX message, return `Ok(Some(total_len))` (the byte
/// length of that message). Return `Ok(None)` if more bytes are needed, or `Err` if the start of
/// the buffer cannot be framed (including a declared `BodyLength` beyond [`MAX_BODY_LEN`]).
///
/// Uses BodyLength (tag 9) to locate the body, then accounts for the fixed 7-byte
/// `10=XXX<SOH>` CheckSum trailer.
pub fn frame_length(buf: &[u8]) -> Result<Option<usize>, DecodeError> {
    let Some(soh1) = find(buf, SOH) else {
        return Ok(None);
    };
    if buf.get(0..2) != Some(b"8=") {
        return Err(DecodeError::MissingBeginString);
    }
    // BUG-79/FR-048 (feature 007): beyond the leading `8=`, the BeginString *value* itself
    // (between `8=` and the first SOH) must match the `FIX.\d.\d`/`FIXT.\d.\d` shape QFJ
    // requires — previously any bytes at all following `8=` were accepted as a plausible frame
    // start, so non-FIX data (or a garbled `BeginString`) would still be framed.
    let begin_string_value = buf.get(2..soh1).unwrap_or(&[]);
    if !looks_like_fix_begin_string(begin_string_value) {
        return Err(DecodeError::MissingBeginString);
    }

    let after8 = buf.get(soh1 + 1..).unwrap_or(&[]);
    let Some(soh2_rel) = find(after8, SOH) else {
        return Ok(None);
    };
    let bl_field = after8.get(..soh2_rel).unwrap_or(&[]);
    if bl_field.get(0..2) != Some(b"9=") {
        return Err(DecodeError::InvalidBodyLength);
    }
    let bl_val = bl_field.get(2..).unwrap_or(&[]);
    let body_len: usize = core::str::from_utf8(bl_val)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or(DecodeError::InvalidBodyLength)?;
    // BUG-100/FR-014 (feature 007): a declared BodyLength of 0 is malformed -- every real FIX
    // message has a non-empty body (at minimum MsgType), matching QuickFIX/J (QFJ-903) and
    // QuickFIX/Go, which both reject it rather than framing an empty-body message.
    if body_len == 0 {
        return Err(DecodeError::ZeroBodyLength);
    }
    if body_len > MAX_BODY_LEN {
        return Err(DecodeError::BodyLengthTooLarge {
            declared: body_len,
            max: MAX_BODY_LEN,
        });
    }

    // BUG-23/FR-032 (feature 007): `checked_add` throughout, not bare `+` — `body_len` is already
    // capped by `MAX_BODY_LEN` above, and `soh1`/`soh2_rel` are bounded by `buf.len()`, so this
    // can't overflow in any reachable scenario today; but a bare `+` would silently wrap (in
    // release builds) if that ever stopped being true, drained the wrong number of bytes, and
    // desynchronized the stream for a remote, unauthenticated peer — defense in depth, not just a
    // theoretical concern, per the original audit finding.
    let overflow = || DecodeError::BodyLengthTooLarge {
        declared: body_len,
        max: MAX_BODY_LEN,
    };
    let body_start = soh1
        .checked_add(1)
        .and_then(|v| v.checked_add(soh2_rel))
        .and_then(|v| v.checked_add(1))
        .ok_or_else(overflow)?;
    let total = body_start
        .checked_add(body_len)
        .and_then(|v| v.checked_add(7))
        .ok_or_else(overflow)?;
    if buf.len() >= total {
        // BUG-46/FR-047 (feature 007): confirm the bytes at the computed checksum position
        // actually look like `10=` before trusting `total` — a wrong `BodyLength` (still numeric,
        // still within `MAX_BODY_LEN`, but not matching the message's real length) would otherwise
        // make `total` point at the wrong offset entirely; the caller would drain the wrong number
        // of bytes and desynchronize the stream for good, rather than failing this one frame and
        // resynchronizing (the existing malformed-frame recovery this `Err` return already
        // triggers one layer up, in `truefix-transport`'s `classify_buffered`) — matching QFJ's
        // `FIXMessageDecoder`, which verifies `10=???<SOH>` at the expected position.
        if buf.get(total - 7..total - 4) != Some(b"10=") {
            return Err(DecodeError::InvalidBodyLength);
        }
        // NEW-107 (audit 006): also confirm the byte immediately after the three checksum digits
        // is SOH, completing the `10=\d{3}\x01` trailer shape QFJ's `FIXMessageDecoder` verifies
        // in full -- the `10=` prefix check above alone would still accept a frame whose
        // `BodyLength` and checksum digits happen to be numeric/well-positioned but whose trailing
        // byte isn't SOH, which `tokenize_validated` would then misparse as the start of the next
        // field, desynchronizing the stream.
        if buf.get(total - 1) != Some(&SOH) {
            return Err(DecodeError::InvalidBodyLength);
        }
        Ok(Some(total))
    } else {
        Ok(None)
    }
}

/// T177/T178 (feature 009, NEW-35/36): a SIMD-accelerated search, replacing a hand-rolled
/// `.iter().position()` byte-by-byte scan in this decode/framing hot path.
fn find(haystack: &[u8], needle: u8) -> Option<usize> {
    memchr::memchr(needle, haystack)
}

/// Whether `value` (the BeginString field's value, e.g. `b"FIX.4.4"`) matches the
/// `FIX.<digit>.<digit>` / `FIXT.<digit>.<digit>` shape (BUG-79/FR-048, feature 007). Only the
/// first 3 characters after the `FIX.`/`FIXT.` prefix are checked (matching QFJ's own pattern);
/// any trailing suffix (e.g. `SP2`) is accepted as-is.
fn looks_like_fix_begin_string(value: &[u8]) -> bool {
    let Ok(s) = core::str::from_utf8(value) else {
        return false;
    };
    let Some(rest) = s.strip_prefix("FIXT.").or_else(|| s.strip_prefix("FIX.")) else {
        return false;
    };
    let bytes = rest.as_bytes();
    matches!(bytes.get(0..3), Some([d1, b'.', d2]) if d1.is_ascii_digit() && d2.is_ascii_digit())
}
