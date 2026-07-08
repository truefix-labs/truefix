//! FAST stop-bit variable-length integer primitives.

use super::error::FastCodecError;

/// Encode an unsigned integer using FAST stop-bit encoding.
pub fn encode_u64(mut value: u64) -> Vec<u8> {
    let mut parts = Vec::new();
    parts.push((value & 0x7f) as u8);
    value >>= 7;
    while value != 0 {
        parts.push((value & 0x7f) as u8);
        value >>= 7;
    }

    let mut out = Vec::with_capacity(parts.len());
    let mut iter = parts.iter().rev().peekable();
    while let Some(part) = iter.next() {
        let mut byte = *part;
        if iter.peek().is_none() {
            byte |= 0x80;
        }
        out.push(byte);
    }
    out
}

/// Decode an unsigned stop-bit integer from `bytes` starting at `offset`.
pub fn decode_u64(bytes: &[u8], offset: usize) -> Result<(u64, usize), FastCodecError> {
    let mut value = 0u64;
    let mut read = 0usize;
    let mut pos = offset;
    loop {
        let Some(byte) = bytes.get(pos).copied() else {
            return Err(FastCodecError::Truncated { offset: pos });
        };
        if read >= 10 {
            return Err(FastCodecError::InvalidStopBit { offset });
        }
        value = value
            .checked_shl(7)
            .ok_or(FastCodecError::InvalidStopBit { offset })?
            | u64::from(byte & 0x7f);
        read += 1;
        pos += 1;
        if byte & 0x80 != 0 {
            return Ok((value, pos));
        }
    }
}

/// Encode a signed integer using zig-zag plus FAST stop-bit encoding.
pub fn encode_i64(value: i64) -> Vec<u8> {
    let zigzag = ((value << 1) ^ (value >> 63)) as u64;
    encode_u64(zigzag)
}

/// Decode a signed integer using zig-zag plus FAST stop-bit encoding.
pub fn decode_i64(bytes: &[u8], offset: usize) -> Result<(i64, usize), FastCodecError> {
    let (raw, next) = decode_u64(bytes, offset)?;
    let value = ((raw >> 1) as i64) ^ -((raw & 1) as i64);
    Ok((value, next))
}
