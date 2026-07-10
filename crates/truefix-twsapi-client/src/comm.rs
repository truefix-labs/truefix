use crate::constants::{INFINITY_STR, UNSET_DOUBLE, UNSET_INTEGER};
use crate::error::{TwsApiError, TwsApiResult};

/// A parsed length-prefixed frame and the remaining bytes after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<'a> {
    /// Payload length from the 4-byte big-endian prefix.
    pub size: usize,
    /// Complete frame payload.
    pub payload: &'a [u8],
    /// Bytes following the complete frame.
    pub rest: &'a [u8],
}

/// Builds the enhanced-handshake message body, including the 4-byte length prefix.
pub fn make_initial_msg(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

/// Builds the full `API\0` enhanced handshake.
pub fn make_client_handshake(
    min_client_ver: i32,
    max_client_ver: i32,
    options: Option<&str>,
) -> Vec<u8> {
    let mut version = format!("v{min_client_ver}..{max_client_ver}");
    if let Some(options) = options.filter(|value| !value.is_empty()) {
        version.push(' ');
        version.push_str(options);
    }

    let mut out = b"API\0".to_vec();
    out.extend_from_slice(&make_initial_msg(&version));
    out
}

/// Builds a length-prefixed protobuf payload with a raw 4-byte message id.
pub fn make_msg_proto(msg_id: i32, protobuf_data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + protobuf_data.len());
    payload.extend_from_slice(&msg_id.to_be_bytes());
    payload.extend_from_slice(protobuf_data);

    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    out
}

/// Builds a length-prefixed field-based payload.
pub fn make_msg(msg_id: i32, use_raw_int_msg_id: bool, text: &str) -> TwsApiResult<Vec<u8>> {
    let mut payload = Vec::new();
    if use_raw_int_msg_id {
        payload.extend_from_slice(&msg_id.to_be_bytes());
        payload.extend_from_slice(text.as_bytes());
    } else {
        payload.extend_from_slice(make_field(msg_id)?.as_bytes());
        payload.extend_from_slice(text.as_bytes());
    }

    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Encodes a NUL-terminated field.
pub fn make_field<T>(value: T) -> TwsApiResult<String>
where
    T: TwsField,
{
    let text = value.to_tws_field()?;
    ensure_ascii_printable(&text)?;
    Ok(format!("{text}\0"))
}

/// Encodes a NUL-terminated field, mapping unset integer/double sentinels to an empty value.
pub fn make_field_handle_empty<T>(value: T) -> TwsApiResult<String>
where
    T: TwsNullableField,
{
    let text = value.to_tws_nullable_field()?;
    ensure_ascii_printable(&text)?;
    Ok(format!("{text}\0"))
}

/// Reads one complete frame from `buf`.
pub fn read_msg(buf: &[u8]) -> TwsApiResult<Option<Frame<'_>>> {
    if buf.len() < 4 {
        return Ok(None);
    }

    let (prefix, data) = buf.split_at(4);
    let prefix: [u8; 4] = prefix
        .try_into()
        .map_err(|_| TwsApiError::IncompleteFrame {
            needed: 4,
            available: buf.len(),
        })?;
    let size = u32::from_be_bytes(prefix);
    let size = usize::try_from(size).map_err(|_| TwsApiError::FrameTooLarge(size))?;
    let needed = 4 + size;
    if buf.len() < needed {
        return Ok(None);
    }

    Ok(Some(Frame {
        size,
        payload: data.get(..size).ok_or(TwsApiError::IncompleteFrame {
            needed,
            available: buf.len(),
        })?,
        rest: data.get(size..).ok_or(TwsApiError::IncompleteFrame {
            needed,
            available: buf.len(),
        })?,
    }))
}

/// Splits a payload into NUL-terminated fields, dropping the final empty segment like Python.
pub fn read_fields(buf: &[u8]) -> Vec<&[u8]> {
    let mut fields = buf.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    fields
}

/// TWS field serialization used by [`make_field`].
pub trait TwsField {
    /// Converts a value into its TWS field string.
    fn to_tws_field(self) -> TwsApiResult<String>;
}

/// TWS field serialization used by [`make_field_handle_empty`].
pub trait TwsNullableField {
    /// Converts a value into its TWS field string, with unset sentinels mapped to empty.
    fn to_tws_nullable_field(self) -> TwsApiResult<String>;
}

impl TwsField for &str {
    fn to_tws_field(self) -> TwsApiResult<String> {
        Ok(self.to_owned())
    }
}

impl TwsField for &String {
    fn to_tws_field(self) -> TwsApiResult<String> {
        Ok(self.clone())
    }
}

impl TwsField for String {
    fn to_tws_field(self) -> TwsApiResult<String> {
        Ok(self)
    }
}

impl TwsField for bool {
    fn to_tws_field(self) -> TwsApiResult<String> {
        Ok(i32::from(self).to_string())
    }
}

impl TwsField for i32 {
    fn to_tws_field(self) -> TwsApiResult<String> {
        Ok(self.to_string())
    }
}

impl TwsField for i64 {
    fn to_tws_field(self) -> TwsApiResult<String> {
        Ok(self.to_string())
    }
}

impl TwsField for usize {
    fn to_tws_field(self) -> TwsApiResult<String> {
        Ok(self.to_string())
    }
}

impl TwsField for f64 {
    fn to_tws_field(self) -> TwsApiResult<String> {
        if self.is_infinite() && self.is_sign_positive() {
            return Ok(INFINITY_STR.to_owned());
        }
        Ok(self.to_string())
    }
}

impl TwsNullableField for i32 {
    fn to_tws_nullable_field(self) -> TwsApiResult<String> {
        if self == UNSET_INTEGER {
            return Ok(String::new());
        }
        self.to_tws_field()
    }
}

impl TwsNullableField for f64 {
    fn to_tws_nullable_field(self) -> TwsApiResult<String> {
        if self == UNSET_DOUBLE {
            return Ok(String::new());
        }
        self.to_tws_field()
    }
}

impl<T> TwsNullableField for Option<T>
where
    T: TwsField,
{
    fn to_tws_nullable_field(self) -> TwsApiResult<String> {
        match self {
            Some(value) => value.to_tws_field(),
            None => Err(TwsApiError::MissingField),
        }
    }
}

fn ensure_ascii_printable(value: &str) -> TwsApiResult<()> {
    if value
        .bytes()
        .all(|byte| byte == b'\t' || (0x20..=0x7e).contains(&byte))
    {
        return Ok(());
    }
    Err(TwsApiError::NonPrintableAscii(value.to_owned()))
}
