//! Extract complete FIX messages from a byte stream.

use truefix_core::DecodeError;

const SOH: u8 = 0x01;

/// If `buf` begins with at least one complete FIX message, return `Ok(Some(total_len))` (the
/// byte length of that message). Return `Ok(None)` if more bytes are needed, or `Err` if the
/// start of the buffer cannot be framed.
///
/// Uses BodyLength (tag 9) to locate the body, then accounts for the fixed 7-byte
/// `10=XXX<SOH>` CheckSum trailer.
pub(crate) fn frame_length(buf: &[u8]) -> Result<Option<usize>, DecodeError> {
    let Some(soh1) = find(buf, SOH) else {
        return Ok(None);
    };
    if buf.get(0..2) != Some(b"8=") {
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

    let body_start = soh1 + 1 + soh2_rel + 1;
    // CheckSum field "10=" + 3 digits + SOH is always 7 bytes.
    let total = body_start + body_len + 7;
    if buf.len() >= total {
        Ok(Some(total))
    } else {
        Ok(None)
    }
}

fn find(haystack: &[u8], needle: u8) -> Option<usize> {
    haystack.iter().position(|&b| b == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MSG: &[u8] = b"8=FIX.4.4\x019=21\x0135=A\x0195=7\x0196=rawdata\x0110=086\x01";

    #[test]
    fn frames_complete_message() {
        assert_eq!(frame_length(MSG), Ok(Some(MSG.len())));
    }

    #[test]
    fn needs_more_bytes() {
        assert_eq!(frame_length(&MSG[..10]), Ok(None));
        assert_eq!(frame_length(b"8=FIX.4.4"), Ok(None));
    }

    #[test]
    fn detects_two_messages_in_buffer() {
        let mut two = MSG.to_vec();
        two.extend_from_slice(MSG);
        assert_eq!(frame_length(&two), Ok(Some(MSG.len())));
    }
}
