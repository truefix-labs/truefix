//! SBE binary codec.

pub mod error;
pub mod flyweight;

use truefix_core::Message;
use truefix_core::field::Field;
use truefix_core::field_map::{FieldMap, MemberRef};
use truefix_core::group::Group;
use truefix_dict::sbe_schema::{
    SbeDataType, SbeFieldDef, SbeGroupDef, SbeSchemaSet, SbeVarDataDef,
};

use crate::BinaryCodec;
use flyweight::Flyweight;

pub use error::SbeCodecError;

const HEADER_LEN: usize = 6;

/// A schema-driven SBE codec.
#[derive(Debug)]
pub struct SbeCodec {
    schemas: SbeSchemaSet,
    session: String,
}

impl SbeCodec {
    /// Create an SBE codec for `schemas`.
    pub fn new(schemas: SbeSchemaSet) -> Self {
        Self {
            schemas,
            session: "unknown".to_owned(),
        }
    }

    /// Create an SBE codec with an observability session label.
    pub fn with_session(schemas: SbeSchemaSet, session: impl Into<String>) -> Self {
        Self {
            schemas,
            session: session.into(),
        }
    }
}

impl BinaryCodec for SbeCodec {
    type Error = SbeCodecError;

    fn encode(&self, message: &Message, template_id: u32) -> Result<Vec<u8>, Self::Error> {
        let schema = self
            .schemas
            .get(template_id)
            .ok_or(SbeCodecError::UnknownTemplateId { id: template_id })?;
        let mut out = Vec::new();
        push_u16(&mut out, template_id, 0)?;
        push_u16(&mut out, self.schemas.schema_id.unwrap_or(0), 0)?;
        push_u16(&mut out, schema.block_length, 0)?;
        let block_len = usize::try_from(schema.block_length).map_err(|_| {
            SbeCodecError::BlockLengthMismatch {
                declared: schema.block_length,
                actual: schema.block_length,
            }
        })?;
        reject_unknown_body_fields(message, schema)?;
        let fixed_start = out.len();
        out.resize(fixed_start + block_len, 0);

        for field in &schema.fields {
            if let Some(value) = message.body.get(field.tag) {
                write_field(&mut out, fixed_start, field, value.value_bytes())?;
            }
        }
        for group in &schema.groups {
            write_group(&mut out, message, group)?;
        }
        for var_data in &schema.var_data {
            write_var_data(&mut out, message, var_data)?;
        }
        Ok(out)
    }

    fn decode(&self, bytes: &[u8]) -> Result<(Message, u32), Self::Error> {
        let result = self.decode_inner(bytes);
        if let Err(error) = &result {
            error::report_decode_failure(&self.session, None, error);
        }
        result
    }
}

fn reject_unknown_body_fields(
    message: &Message,
    schema: &truefix_dict::sbe_schema::SbeMessageSchema,
) -> Result<(), SbeCodecError> {
    for member in message.body.members() {
        let tag = match member {
            MemberRef::Field(field) => field.tag(),
            MemberRef::Group { count_tag, .. } => count_tag,
        };
        let known = schema.fields.iter().any(|field| field.tag == tag)
            || schema.groups.iter().any(|group| group.tag == tag)
            || schema.var_data.iter().any(|data| data.tag == tag);
        if !known {
            return Err(SbeCodecError::UnsupportedValue {
                field: tag,
                reason: "field is not declared by SBE schema".to_owned(),
            });
        }
    }
    Ok(())
}

impl SbeCodec {
    fn decode_inner(&self, bytes: &[u8]) -> Result<(Message, u32), SbeCodecError> {
        let fly = Flyweight::new(bytes);
        let template_id = u32::from(fly.u16_le(0)?);
        let _schema_id = fly.u16_le(2)?;
        let declared_block_length = u32::from(fly.u16_le(4)?);
        let Some(schema) = self.schemas.get(template_id) else {
            error::report_unknown_template(&self.session, template_id);
            return Err(SbeCodecError::UnknownTemplateId { id: template_id });
        };
        if declared_block_length != schema.block_length {
            return Err(SbeCodecError::BlockLengthMismatch {
                declared: declared_block_length,
                actual: schema.block_length,
            });
        }
        let block_len = usize::try_from(schema.block_length)
            .map_err(|_| SbeCodecError::Truncated { offset: HEADER_LEN })?;
        fly.slice(HEADER_LEN, block_len)?;

        let mut message = Message::new();
        for field in &schema.fields {
            let offset = HEADER_LEN
                .checked_add(
                    usize::try_from(field.offset)
                        .map_err(|_| SbeCodecError::Truncated { offset: HEADER_LEN })?,
                )
                .ok_or(SbeCodecError::Truncated { offset: HEADER_LEN })?;
            let value = read_field(&fly, offset, field)?;
            message.body.set(Field::new(field.tag, value));
        }

        let mut cursor = HEADER_LEN + block_len;
        for group in &schema.groups {
            cursor = read_group(&fly, cursor, group, &mut message)?;
        }
        for var_data in &schema.var_data {
            cursor = read_var_data(&fly, cursor, var_data, &mut message)?;
        }
        Ok((message, template_id))
    }
}

fn read_field(
    fly: &Flyweight<'_>,
    offset: usize,
    field: &SbeFieldDef,
) -> Result<Vec<u8>, SbeCodecError> {
    Ok(match field.data_type {
        SbeDataType::UInt8 => fly.u8(offset)?.to_string().into_bytes(),
        SbeDataType::UInt16 => fly.u16_le(offset)?.to_string().into_bytes(),
        SbeDataType::UInt32 => fly.u32_le(offset)?.to_string().into_bytes(),
        SbeDataType::Int32 => fly.i32_le(offset)?.to_string().into_bytes(),
        SbeDataType::UInt64 => fly.u64_le(offset)?.to_string().into_bytes(),
        SbeDataType::Int64 => fly.i64_le(offset)?.to_string().into_bytes(),
        SbeDataType::Char => vec![fly.u8(offset)?],
    })
}

fn read_group(
    fly: &Flyweight<'_>,
    cursor: usize,
    group: &SbeGroupDef,
    message: &mut Message,
) -> Result<usize, SbeCodecError> {
    let block_length = usize::from(fly.u16_le(cursor)?);
    let count = usize::from(fly.u16_le(cursor + 2)?);
    let mut next = cursor + 4;
    let mut decoded = Group::new(group.tag);
    for _ in 0..count {
        fly.slice(next, block_length)?;
        let mut entry = FieldMap::new();
        for field in &group.fields {
            let field_offset = next
                .checked_add(
                    usize::try_from(field.offset)
                        .map_err(|_| SbeCodecError::Truncated { offset: next })?,
                )
                .ok_or(SbeCodecError::Truncated { offset: next })?;
            let value = read_field(fly, field_offset, field)?;
            entry.set(Field::new(field.tag, value));
        }
        decoded.add_entry(entry);
        next += block_length;
    }
    message.body.add_group(decoded);
    Ok(next)
}

fn read_var_data(
    fly: &Flyweight<'_>,
    cursor: usize,
    var_data: &SbeVarDataDef,
    message: &mut Message,
) -> Result<usize, SbeCodecError> {
    let len = usize::from(fly.u8(cursor)?);
    let start = cursor + usize::from(var_data.length_field_size);
    let available = fly.len().saturating_sub(start);
    if available < len {
        return Err(SbeCodecError::VarDataLengthMismatch {
            field: var_data.name.clone(),
            declared: len as u32,
            actual: available as u32,
        });
    }
    let value = fly.slice(start, len)?.to_vec();
    message.body.set(Field::bytes(var_data.tag, &value));
    Ok(start + len)
}

fn write_field(
    out: &mut [u8],
    base: usize,
    field: &SbeFieldDef,
    value: &[u8],
) -> Result<(), SbeCodecError> {
    let offset = base
        .checked_add(usize::try_from(field.offset).map_err(|_| {
            SbeCodecError::UnsupportedValue {
                field: field.tag,
                reason: "offset too large".to_owned(),
            }
        })?)
        .ok_or(SbeCodecError::UnsupportedValue {
            field: field.tag,
            reason: "offset overflow".to_owned(),
        })?;
    match field.data_type {
        SbeDataType::UInt8 | SbeDataType::Char => {
            let byte = value
                .first()
                .copied()
                .ok_or(SbeCodecError::UnsupportedValue {
                    field: field.tag,
                    reason: "empty one-byte field".to_owned(),
                })?;
            let slot = out
                .get_mut(offset)
                .ok_or(SbeCodecError::Truncated { offset })?;
            *slot = byte;
        }
        SbeDataType::UInt16 => {
            write_fixed(out, offset, &parse_u64(field, value)?.to_le_bytes(), 2)?
        }
        SbeDataType::UInt32 => {
            write_fixed(out, offset, &parse_u64(field, value)?.to_le_bytes(), 4)?
        }
        SbeDataType::Int32 => write_fixed(out, offset, &parse_i64(field, value)?.to_le_bytes(), 4)?,
        SbeDataType::UInt64 => {
            write_fixed(out, offset, &parse_u64(field, value)?.to_le_bytes(), 8)?
        }
        SbeDataType::Int64 => write_fixed(out, offset, &parse_i64(field, value)?.to_le_bytes(), 8)?,
    }
    Ok(())
}

fn write_fixed(
    out: &mut [u8],
    offset: usize,
    bytes: &[u8],
    len: usize,
) -> Result<(), SbeCodecError> {
    let dst = out
        .get_mut(offset..offset + len)
        .ok_or(SbeCodecError::Truncated { offset })?;
    let src = bytes
        .get(..len)
        .ok_or(SbeCodecError::Truncated { offset })?;
    dst.copy_from_slice(src);
    Ok(())
}

fn write_group(
    out: &mut Vec<u8>,
    message: &Message,
    group: &SbeGroupDef,
) -> Result<(), SbeCodecError> {
    let entries = message.body.group(group.tag).unwrap_or(&[]);
    push_u16(out, group.block_length, group.tag)?;
    push_u16(out, entries.len() as u32, group.tag)?;
    let block_length =
        usize::try_from(group.block_length).map_err(|_| SbeCodecError::UnsupportedValue {
            field: group.tag,
            reason: "group block length too large".to_owned(),
        })?;
    for entry in entries {
        let base = out.len();
        out.resize(base + block_length, 0);
        for field in &group.fields {
            if let Some(value) = entry.get(field.tag) {
                write_field(out, base, field, value.value_bytes())?;
            }
        }
    }
    Ok(())
}

fn write_var_data(
    out: &mut Vec<u8>,
    message: &Message,
    var_data: &SbeVarDataDef,
) -> Result<(), SbeCodecError> {
    let Some(field) = message.body.get(var_data.tag) else {
        out.push(0);
        return Ok(());
    };
    let len = field.value_bytes().len();
    if len > u8::MAX as usize {
        return Err(SbeCodecError::UnsupportedValue {
            field: var_data.tag,
            reason: "varData too long for one-byte length".to_owned(),
        });
    }
    out.push(len as u8);
    out.extend(field.value_bytes());
    Ok(())
}

fn parse_u64(field: &SbeFieldDef, value: &[u8]) -> Result<u64, SbeCodecError> {
    let text = core::str::from_utf8(value).map_err(|_| SbeCodecError::UnsupportedValue {
        field: field.tag,
        reason: "not UTF-8 text".to_owned(),
    })?;
    text.parse().map_err(
        |err: std::num::ParseIntError| SbeCodecError::UnsupportedValue {
            field: field.tag,
            reason: err.to_string(),
        },
    )
}

fn parse_i64(field: &SbeFieldDef, value: &[u8]) -> Result<i64, SbeCodecError> {
    let text = core::str::from_utf8(value).map_err(|_| SbeCodecError::UnsupportedValue {
        field: field.tag,
        reason: "not UTF-8 text".to_owned(),
    })?;
    text.parse().map_err(
        |err: std::num::ParseIntError| SbeCodecError::UnsupportedValue {
            field: field.tag,
            reason: err.to_string(),
        },
    )
}

fn push_u16(out: &mut Vec<u8>, value: u32, field: u32) -> Result<(), SbeCodecError> {
    let value = u16::try_from(value).map_err(|_| SbeCodecError::UnsupportedValue {
        field,
        reason: "value does not fit u16".to_owned(),
    })?;
    out.extend(value.to_le_bytes());
    Ok(())
}
