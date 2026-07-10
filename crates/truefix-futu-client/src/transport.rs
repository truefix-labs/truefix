use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::codec::crypto::EncAlgo;
use crate::codec::frame::{
    FRAME_HEADER_LEN, FrameHeader, body_sha1, decode_header, encode_frame, verify_sha1,
};
use crate::error::{FutuError, FutuResult};

#[derive(Debug)]
pub struct FramedTransport {
    stream: TcpStream,
    enc: EncAlgo,
}

#[derive(Debug)]
pub struct FrameReader {
    reader: OwnedReadHalf,
    enc: EncAlgo,
}

#[derive(Debug)]
pub struct FrameWriter {
    writer: OwnedWriteHalf,
    enc: EncAlgo,
}

impl FramedTransport {
    pub async fn connect(host: &str, port: u16) -> FutuResult<Self> {
        let stream = TcpStream::connect((host, port)).await?;
        Ok(Self {
            stream,
            enc: EncAlgo::None,
        })
    }

    pub async fn send(&mut self, proto_id: u32, serial_no: u32, body: &[u8]) -> FutuResult<()> {
        send_frame(&mut self.stream, &self.enc, proto_id, serial_no, body).await
    }

    pub async fn send_custom(
        &mut self,
        proto_id: u32,
        serial_no: u32,
        plaintext: &[u8],
        ciphertext: &[u8],
    ) -> FutuResult<()> {
        send_custom_frame(&mut self.stream, proto_id, serial_no, plaintext, ciphertext).await
    }

    pub async fn recv(&mut self) -> FutuResult<(FrameHeader, Bytes)> {
        recv_frame(&mut self.stream, &self.enc).await
    }

    pub async fn recv_raw(&mut self) -> FutuResult<(FrameHeader, Bytes)> {
        recv_raw_frame(&mut self.stream).await
    }

    pub fn set_enc(&mut self, enc: EncAlgo) {
        self.enc = enc;
    }

    pub fn split(self) -> (FrameReader, FrameWriter) {
        let (reader, writer) = self.stream.into_split();
        (
            FrameReader {
                reader,
                enc: self.enc.clone(),
            },
            FrameWriter {
                writer,
                enc: self.enc,
            },
        )
    }
}

impl FrameReader {
    pub async fn recv(&mut self) -> FutuResult<(FrameHeader, Bytes)> {
        recv_frame(&mut self.reader, &self.enc).await
    }
}

impl FrameWriter {
    pub async fn send(&mut self, proto_id: u32, serial_no: u32, body: &[u8]) -> FutuResult<()> {
        send_frame(&mut self.writer, &self.enc, proto_id, serial_no, body).await
    }
}

async fn send_frame<W>(
    writer: &mut W,
    enc: &EncAlgo,
    proto_id: u32,
    serial_no: u32,
    body: &[u8],
) -> FutuResult<()>
where
    W: AsyncWrite + Unpin,
{
    let ciphertext = enc.encrypt(body)?;
    let header = FrameHeader {
        proto_id,
        proto_fmt: 0,
        proto_ver: 0,
        serial_no,
        body_len: ciphertext.len() as u32,
        body_sha1: body_sha1(body),
    };
    let frame = encode_frame(&header, &ciphertext);
    writer.write_all(&frame).await?;
    writer.write_all(&ciphertext).await?;
    Ok(())
}

async fn send_custom_frame<W>(
    writer: &mut W,
    proto_id: u32,
    serial_no: u32,
    plaintext: &[u8],
    ciphertext: &[u8],
) -> FutuResult<()>
where
    W: AsyncWrite + Unpin,
{
    let header = FrameHeader {
        proto_id,
        proto_fmt: 0,
        proto_ver: 0,
        serial_no,
        body_len: ciphertext.len() as u32,
        body_sha1: body_sha1(plaintext),
    };
    let frame = encode_frame(&header, ciphertext);
    writer.write_all(&frame).await?;
    writer.write_all(ciphertext).await?;
    Ok(())
}

async fn recv_frame<R>(reader: &mut R, enc: &EncAlgo) -> FutuResult<(FrameHeader, Bytes)>
where
    R: AsyncRead + Unpin,
{
    let mut header_buf = [0u8; FRAME_HEADER_LEN];
    reader.read_exact(&mut header_buf).await?;
    let header = decode_header(&header_buf)?;
    let body_len = usize::try_from(header.body_len)
        .map_err(|_| FutuError::Crypto("frame body length overflow".into()))?;
    let mut ciphertext = vec![0u8; body_len];
    reader.read_exact(&mut ciphertext).await?;
    let plaintext = enc.decrypt(&ciphertext)?;
    if !verify_sha1(&header, &plaintext) {
        return Err(FutuError::Sha1Mismatch);
    }
    Ok((header, Bytes::from(plaintext)))
}

async fn recv_raw_frame<R>(reader: &mut R) -> FutuResult<(FrameHeader, Bytes)>
where
    R: AsyncRead + Unpin,
{
    let mut header_buf = [0u8; FRAME_HEADER_LEN];
    reader.read_exact(&mut header_buf).await?;
    let header = decode_header(&header_buf)?;
    let body_len = usize::try_from(header.body_len)
        .map_err(|_| FutuError::Crypto("frame body length overflow".into()))?;
    let mut ciphertext = vec![0u8; body_len];
    reader.read_exact(&mut ciphertext).await?;
    Ok((header, Bytes::from(ciphertext)))
}
