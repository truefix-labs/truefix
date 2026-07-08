//! Safe zero-copy SBE buffer view.

use super::error::SbeCodecError;

/// Borrowed SBE buffer reader.
#[derive(Debug, Clone, Copy)]
pub struct Flyweight<'a> {
    buf: &'a [u8],
}

impl<'a> Flyweight<'a> {
    /// Create a flyweight over `buf`.
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    /// Return the buffer length.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Read a borrowed byte slice.
    pub fn slice(&self, offset: usize, len: usize) -> Result<&'a [u8], SbeCodecError> {
        let end = offset
            .checked_add(len)
            .ok_or(SbeCodecError::Truncated { offset })?;
        self.buf
            .get(offset..end)
            .ok_or(SbeCodecError::Truncated { offset: end })
    }

    /// Read a u8.
    pub fn u8(&self, offset: usize) -> Result<u8, SbeCodecError> {
        self.buf
            .get(offset)
            .copied()
            .ok_or(SbeCodecError::Truncated { offset })
    }

    /// Read a little-endian u16.
    pub fn u16_le(&self, offset: usize) -> Result<u16, SbeCodecError> {
        let bytes: [u8; 2] = self
            .slice(offset, 2)?
            .try_into()
            .map_err(|_| SbeCodecError::Truncated { offset })?;
        Ok(u16::from_le_bytes(bytes))
    }

    /// Read a little-endian u32.
    pub fn u32_le(&self, offset: usize) -> Result<u32, SbeCodecError> {
        let bytes: [u8; 4] = self
            .slice(offset, 4)?
            .try_into()
            .map_err(|_| SbeCodecError::Truncated { offset })?;
        Ok(u32::from_le_bytes(bytes))
    }

    /// Read a little-endian i32.
    pub fn i32_le(&self, offset: usize) -> Result<i32, SbeCodecError> {
        let bytes: [u8; 4] = self
            .slice(offset, 4)?
            .try_into()
            .map_err(|_| SbeCodecError::Truncated { offset })?;
        Ok(i32::from_le_bytes(bytes))
    }

    /// Read a little-endian u64.
    pub fn u64_le(&self, offset: usize) -> Result<u64, SbeCodecError> {
        let bytes: [u8; 8] = self
            .slice(offset, 8)?
            .try_into()
            .map_err(|_| SbeCodecError::Truncated { offset })?;
        Ok(u64::from_le_bytes(bytes))
    }

    /// Read a little-endian i64.
    pub fn i64_le(&self, offset: usize) -> Result<i64, SbeCodecError> {
        let bytes: [u8; 8] = self
            .slice(offset, 8)?
            .try_into()
            .map_err(|_| SbeCodecError::Truncated { offset })?;
        Ok(i64::from_le_bytes(bytes))
    }
}
