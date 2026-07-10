use sha1::{Digest, Sha1};

use crate::error::{FutuError, FutuResult};

pub const HEADER_FLAG: [u8; 2] = *b"FT";
pub const FRAME_HEADER_LEN: usize = 44;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    pub proto_id: u32,
    pub proto_fmt: u8,
    pub proto_ver: u8,
    pub serial_no: u32,
    pub body_len: u32,
    pub body_sha1: [u8; 20],
}

pub fn encode_frame(header: &FrameHeader, ciphertext: &[u8]) -> [u8; FRAME_HEADER_LEN] {
    let mut frame = [0u8; FRAME_HEADER_LEN];
    let mut header = header.clone();
    header.body_len = ciphertext.len() as u32;

    frame[0..2].copy_from_slice(&HEADER_FLAG);
    frame[2..6].copy_from_slice(&header.proto_id.to_le_bytes());
    frame[6] = header.proto_fmt;
    frame[7] = header.proto_ver;
    frame[8..12].copy_from_slice(&header.serial_no.to_le_bytes());
    frame[12..16].copy_from_slice(&header.body_len.to_le_bytes());
    frame[16..36].copy_from_slice(&header.body_sha1);
    frame
}

pub fn decode_header(buf: &[u8; FRAME_HEADER_LEN]) -> FutuResult<FrameHeader> {
    if buf[0..2] != HEADER_FLAG {
        return Err(FutuError::BadMagic([buf[0], buf[1]]));
    }
    let mut body_sha1 = [0u8; 20];
    body_sha1.copy_from_slice(&buf[16..36]);
    Ok(FrameHeader {
        proto_id: u32::from_le_bytes(buf[2..6].try_into().unwrap_or([0; 4])),
        proto_fmt: buf[6],
        proto_ver: buf[7],
        serial_no: u32::from_le_bytes(buf[8..12].try_into().unwrap_or([0; 4])),
        body_len: u32::from_le_bytes(buf[12..16].try_into().unwrap_or([0; 4])),
        body_sha1,
    })
}

pub fn body_sha1(plaintext: &[u8]) -> [u8; 20] {
    let digest = Sha1::digest(plaintext);
    let mut out = [0u8; 20];
    out.copy_from_slice(&digest);
    out
}

pub fn verify_sha1(header: &FrameHeader, plaintext: &[u8]) -> bool {
    header.body_sha1 == body_sha1(plaintext)
}
