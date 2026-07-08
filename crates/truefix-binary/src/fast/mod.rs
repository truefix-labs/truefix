//! FAST binary codec.

pub mod context;
pub mod error;
pub mod presence_map;
pub mod varint;

use std::cell::RefCell;
use std::collections::BTreeMap;

use truefix_core::Message;
use truefix_core::field::Field;
use truefix_core::field_map::MemberRef;
use truefix_dict::fast_template::{FastDataType, FastFieldDef, FastOperator, FastTemplateSet};

use crate::BinaryCodec;

pub use context::EncodingContext;
pub use error::FastCodecError;
use presence_map::PresenceMap;

/// A template-driven FAST codec.
#[derive(Debug)]
pub struct FastCodec {
    templates: FastTemplateSet,
    contexts: RefCell<BTreeMap<u32, EncodingContext>>,
    session: String,
}

impl FastCodec {
    /// Create a FAST codec for `templates`.
    pub fn new(templates: FastTemplateSet) -> Self {
        Self {
            templates,
            contexts: RefCell::new(BTreeMap::new()),
            session: "unknown".to_owned(),
        }
    }

    /// Create a FAST codec with an observability session label.
    pub fn with_session(templates: FastTemplateSet, session: impl Into<String>) -> Self {
        Self {
            templates,
            contexts: RefCell::new(BTreeMap::new()),
            session: session.into(),
        }
    }

    /// Reset one template's encoding context.
    pub fn reset_context(&self, template_id: u32) {
        let mut contexts = self.contexts.borrow_mut();
        let context = contexts.entry(template_id).or_default();
        context.reset(&self.session, template_id);
    }
}

impl BinaryCodec for FastCodec {
    type Error = FastCodecError;

    fn encode(&self, message: &Message, template_id: u32) -> Result<Vec<u8>, Self::Error> {
        let template = self
            .templates
            .get(template_id)
            .ok_or(FastCodecError::UnknownTemplateId { id: template_id })?;
        let mut out = varint::encode_u64(u64::from(template_id));
        let mut bits = Vec::new();
        let mut field_bytes = Vec::new();
        let mut contexts = self.contexts.borrow_mut();
        let context = contexts.entry(template_id).or_default();
        reject_unknown_body_fields(message, &template.fields)?;

        for field_def in &template.fields {
            let value = message
                .body
                .get(field_def.tag)
                .or_else(|| message.header.get(field_def.tag))
                .or_else(|| message.trailer.get(field_def.tag));
            let present = value.is_some();
            bits.push(present);
            if let Some(field) = value {
                encode_field_value(field_def, field.value_bytes(), &mut field_bytes)?;
                context.set_previous(field_def.tag, field.value_bytes().to_vec());
            }
        }

        out.extend(PresenceMap::from_bits(bits).encode());
        out.extend(field_bytes);
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
    fields: &[FastFieldDef],
) -> Result<(), FastCodecError> {
    for member in message.body.members() {
        let tag = match member {
            MemberRef::Field(field) => field.tag(),
            MemberRef::Group { count_tag, .. } => count_tag,
        };
        if !fields.iter().any(|field| field.tag == tag) {
            return Err(FastCodecError::UnsupportedValue {
                field: tag,
                reason: "field is not declared by FAST template".to_owned(),
            });
        }
    }
    Ok(())
}

impl FastCodec {
    fn decode_inner(&self, bytes: &[u8]) -> Result<(Message, u32), FastCodecError> {
        let (template_id, offset) = varint::decode_u64(bytes, 0)?;
        let template_id =
            u32::try_from(template_id).map_err(|_| FastCodecError::InvalidStopBit { offset: 0 })?;
        let Some(template) = self.templates.get(template_id) else {
            error::report_unknown_template(&self.session, template_id);
            return Err(FastCodecError::UnknownTemplateId { id: template_id });
        };
        let (pmap, mut offset) = PresenceMap::decode(bytes, offset)?;
        let mut message = Message::new();
        let mut contexts = self.contexts.borrow_mut();
        let context = contexts.entry(template_id).or_default();

        for (index, field_def) in template.fields.iter().enumerate() {
            let present = pmap.bit(index);
            let decoded = if present {
                let (value, next) = decode_field_value(field_def, bytes, offset)?;
                offset = next;
                context.set_previous(field_def.tag, value.clone());
                Some(value)
            } else {
                recover_omitted_value(field_def, context)?
            };
            if let Some(value) = decoded {
                message.body.set(Field::new(field_def.tag, value));
            }
        }

        Ok((message, template_id))
    }
}

fn encode_field_value(
    field_def: &FastFieldDef,
    value: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), FastCodecError> {
    match field_def.data_type {
        FastDataType::UInt32 | FastDataType::UInt64 => {
            let text =
                core::str::from_utf8(value).map_err(|_| FastCodecError::UnsupportedValue {
                    field: field_def.tag,
                    reason: "integer field is not UTF-8 text".to_owned(),
                })?;
            let parsed = text
                .parse::<u64>()
                .map_err(|err| FastCodecError::UnsupportedValue {
                    field: field_def.tag,
                    reason: err.to_string(),
                })?;
            out.extend(varint::encode_u64(parsed));
        }
        FastDataType::Int32 | FastDataType::Int64 => {
            let text =
                core::str::from_utf8(value).map_err(|_| FastCodecError::UnsupportedValue {
                    field: field_def.tag,
                    reason: "integer field is not UTF-8 text".to_owned(),
                })?;
            let parsed = text
                .parse::<i64>()
                .map_err(|err| FastCodecError::UnsupportedValue {
                    field: field_def.tag,
                    reason: err.to_string(),
                })?;
            out.extend(varint::encode_i64(parsed));
        }
        FastDataType::AsciiString | FastDataType::UnicodeString | FastDataType::Decimal => {
            out.extend(varint::encode_u64(value.len() as u64));
            out.extend(value);
        }
    }
    Ok(())
}

fn decode_field_value(
    field_def: &FastFieldDef,
    bytes: &[u8],
    offset: usize,
) -> Result<(Vec<u8>, usize), FastCodecError> {
    match field_def.data_type {
        FastDataType::UInt32 | FastDataType::UInt64 => {
            let (value, next) = varint::decode_u64(bytes, offset)?;
            Ok((value.to_string().into_bytes(), next))
        }
        FastDataType::Int32 | FastDataType::Int64 => {
            let (value, next) = varint::decode_i64(bytes, offset)?;
            Ok((value.to_string().into_bytes(), next))
        }
        FastDataType::AsciiString | FastDataType::UnicodeString | FastDataType::Decimal => {
            let (len, start) = varint::decode_u64(bytes, offset)?;
            let len =
                usize::try_from(len).map_err(|_| FastCodecError::InvalidStopBit { offset })?;
            let end = start
                .checked_add(len)
                .ok_or(FastCodecError::Truncated { offset: start })?;
            let Some(slice) = bytes.get(start..end) else {
                return Err(FastCodecError::Truncated { offset: end });
            };
            Ok((slice.to_vec(), end))
        }
    }
}

fn recover_omitted_value(
    field_def: &FastFieldDef,
    context: &EncodingContext,
) -> Result<Option<Vec<u8>>, FastCodecError> {
    match &field_def.operator {
        FastOperator::Copy => context
            .previous(field_def.tag)
            .map(|value| Some(value.to_vec()))
            .ok_or(FastCodecError::PresenceMapMismatch {
                field: field_def.tag,
                offset: 0,
            }),
        FastOperator::Default { value } => Ok(value.as_ref().map(|text| text.as_bytes().to_vec())),
        FastOperator::None if field_def.nullable => Ok(None),
        FastOperator::None => Ok(None),
        FastOperator::Increment | FastOperator::Delta => context
            .previous(field_def.tag)
            .map(|value| Some(value.to_vec()))
            .ok_or(FastCodecError::PresenceMapMismatch {
                field: field_def.tag,
                offset: 0,
            }),
    }
}
