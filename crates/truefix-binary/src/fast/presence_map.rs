//! FAST presence-map bit reader and writer.

use super::error::FastCodecError;

/// A decoded FAST presence map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceMap {
    bits: Vec<bool>,
}

impl PresenceMap {
    /// Create a presence map from explicit bits.
    pub fn from_bits(bits: Vec<bool>) -> Self {
        Self { bits }
    }

    /// Return bit `index`, defaulting to absent when the map is shorter.
    pub fn bit(&self, index: usize) -> bool {
        self.bits.get(index).copied().unwrap_or(false)
    }

    /// Encode this presence map using FAST stop-bit bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut current = 0u8;
        let mut used = 0usize;
        for bit in &self.bits {
            current <<= 1;
            if *bit {
                current |= 1;
            }
            used += 1;
            if used == 7 {
                out.push(current);
                current = 0;
                used = 0;
            }
        }
        if used != 0 {
            current <<= 7usize.saturating_sub(used);
            out.push(current);
        }
        if out.is_empty() {
            out.push(0x80);
        } else if let Some(last) = out.last_mut() {
            *last |= 0x80;
        }
        out
    }

    /// Decode a presence map from `bytes` starting at `offset`.
    pub fn decode(bytes: &[u8], offset: usize) -> Result<(Self, usize), FastCodecError> {
        let mut bits = Vec::new();
        let mut pos = offset;
        loop {
            let Some(byte) = bytes.get(pos).copied() else {
                return Err(FastCodecError::Truncated { offset: pos });
            };
            pos += 1;
            for shift in (0..7).rev() {
                bits.push(byte & (1 << shift) != 0);
            }
            if byte & 0x80 != 0 {
                return Ok((Self { bits }, pos));
            }
            if pos.saturating_sub(offset) > 16 {
                return Err(FastCodecError::InvalidStopBit { offset });
            }
        }
    }
}
