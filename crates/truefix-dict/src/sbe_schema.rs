//! SBE schema XML parsing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// SBE template identifier.
pub type TemplateId = u32;

/// A loaded set of SBE message schemas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbeSchemaSet {
    /// Optional schema id.
    pub schema_id: Option<u32>,
    messages: BTreeMap<TemplateId, SbeMessageSchema>,
}

impl SbeSchemaSet {
    /// Return the message schema for `template_id`.
    pub fn get(&self, template_id: TemplateId) -> Option<&SbeMessageSchema> {
        self.messages.get(&template_id)
    }

    /// Iterate message schemas in stable template-id order.
    pub fn messages(&self) -> impl Iterator<Item = &SbeMessageSchema> {
        self.messages.values()
    }
}

/// One SBE message schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbeMessageSchema {
    /// Template id.
    pub template_id: TemplateId,
    /// Message name.
    pub name: String,
    /// Fixed block length.
    pub block_length: u32,
    /// Fixed fields.
    pub fields: Vec<SbeFieldDef>,
    /// Repeating groups.
    pub groups: Vec<SbeGroupDef>,
    /// Variable-length data fields.
    pub var_data: Vec<SbeVarDataDef>,
}

/// One fixed SBE field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbeFieldDef {
    /// FIX tag used when converting to/from `Message`.
    pub tag: u32,
    /// Field name.
    pub name: String,
    /// Byte offset from the message block start.
    pub offset: u32,
    /// Primitive type.
    pub data_type: SbeDataType,
    /// Byte order.
    pub byte_order: ByteOrder,
}

/// One SBE repeating group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbeGroupDef {
    /// FIX count tag.
    pub tag: u32,
    /// Group name.
    pub name: String,
    /// Per-entry block length.
    pub block_length: u32,
    /// Dimension header offset.
    pub dimension_offset: u32,
    /// Group fields.
    pub fields: Vec<SbeFieldDef>,
    /// Nested groups.
    pub nested_groups: Vec<SbeGroupDef>,
    /// Group varData fields.
    pub var_data: Vec<SbeVarDataDef>,
}

/// One SBE variable-length data field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbeVarDataDef {
    /// FIX tag.
    pub tag: u32,
    /// Field name.
    pub name: String,
    /// Size in bytes of the length prefix.
    pub length_field_size: u8,
}

/// Supported SBE primitive types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbeDataType {
    /// Unsigned 8-bit integer.
    UInt8,
    /// Unsigned 16-bit integer.
    UInt16,
    /// Unsigned 32-bit integer.
    UInt32,
    /// Signed 32-bit integer.
    Int32,
    /// Unsigned 64-bit integer.
    UInt64,
    /// Signed 64-bit integer.
    Int64,
    /// Single byte character.
    Char,
}

impl SbeDataType {
    /// Width in bytes.
    pub fn width(self) -> u32 {
        match self {
            Self::UInt8 | Self::Char => 1,
            Self::UInt16 => 2,
            Self::UInt32 | Self::Int32 => 4,
            Self::UInt64 | Self::Int64 => 8,
        }
    }
}

/// SBE byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    /// Little endian.
    LittleEndian,
    /// Big endian.
    BigEndian,
}

/// SBE schema parse/load error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SbeSchemaError {
    /// XML parse error.
    #[error("XML parse error at byte {pos}: {reason}")]
    Xml {
        /// Byte offset.
        pos: usize,
        /// Reason.
        reason: String,
    },
    /// A required attribute was missing.
    #[error("<{elem}> is missing required attribute {attr:?}")]
    MissingAttr {
        /// Element name.
        elem: &'static str,
        /// Attribute name.
        attr: &'static str,
    },
    /// An attribute expected to be numeric was not numeric.
    #[error("<{elem}> attribute {attr:?} is not a number: {value:?}")]
    NotANumber {
        /// Element name.
        elem: &'static str,
        /// Attribute name.
        attr: &'static str,
        /// Offending value.
        value: String,
    },
    /// Unknown data type.
    #[error("field {field:?} has unknown SBE data type {type_name:?}")]
    UnknownDataType {
        /// Field name.
        field: String,
        /// Type name.
        type_name: String,
    },
    /// Duplicate template id.
    #[error("duplicate SBE template id {id}")]
    DuplicateTemplateId {
        /// Duplicated id.
        id: TemplateId,
    },
    /// Overlapping fixed-field offsets.
    #[error("field {field:?} overlaps at byte offset {offset}")]
    OverlappingOffset {
        /// Field name.
        field: String,
        /// Overlapping offset.
        offset: u32,
    },
    /// File read failed.
    #[error("could not read SBE schema file {path}: {reason}")]
    Io {
        /// File path.
        path: String,
        /// I/O reason.
        reason: String,
    },
}

/// Load SBE schemas from a file.
pub fn load_sbe_schemas_from_file(path: &Path) -> Result<SbeSchemaSet, SbeSchemaError> {
    let xml = std::fs::read_to_string(path).map_err(|err| SbeSchemaError::Io {
        path: path.display().to_string(),
        reason: err.to_string(),
    })?;
    parse_sbe_schemas(&xml)
}

/// Parse SBE schemas from XML text.
pub fn parse_sbe_schemas(xml: &str) -> Result<SbeSchemaSet, SbeSchemaError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut parser = Parser::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => parser.handle_start_or_empty(&e, false)?,
            Ok(Event::Empty(e)) => parser.handle_start_or_empty(&e, true)?,
            Ok(Event::End(e)) => parser.handle_end(&local_name_bytes(e.local_name().as_ref()))?,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                return Err(SbeSchemaError::Xml {
                    pos: reader.buffer_position() as usize,
                    reason: err.to_string(),
                });
            }
        }
    }

    Ok(SbeSchemaSet {
        schema_id: parser.schema_id,
        messages: parser.messages,
    })
}

#[derive(Default)]
struct Parser {
    schema_id: Option<u32>,
    messages: BTreeMap<TemplateId, SbeMessageSchema>,
    current_message: Option<SbeMessageSchema>,
    groups: Vec<SbeGroupDef>,
}

impl Parser {
    fn handle_start_or_empty(
        &mut self,
        e: &quick_xml::events::BytesStart<'_>,
        is_empty: bool,
    ) -> Result<(), SbeSchemaError> {
        match local_name(e).as_str() {
            "messageSchema" => self.schema_id = attr_opt_num(e, "messageSchema", "id")?,
            "message" => {
                let template_id = attr_num(e, "message", "id")?;
                if self.messages.contains_key(&template_id) {
                    return Err(SbeSchemaError::DuplicateTemplateId { id: template_id });
                }
                let name = attr(e, "name")?.ok_or(SbeSchemaError::MissingAttr {
                    elem: "message",
                    attr: "name",
                })?;
                let block_length = attr_num(e, "message", "blockLength")?;
                let message = SbeMessageSchema {
                    template_id,
                    name,
                    block_length,
                    fields: Vec::new(),
                    groups: Vec::new(),
                    var_data: Vec::new(),
                };
                if is_empty {
                    self.insert_message(message)?;
                } else {
                    self.current_message = Some(message);
                }
            }
            "field" => {
                let field = field_from_attrs(e)?;
                self.push_field(field);
            }
            "group" => {
                let name = attr(e, "name")?.ok_or(SbeSchemaError::MissingAttr {
                    elem: "group",
                    attr: "name",
                })?;
                let block_length = attr_num(e, "group", "blockLength")?;
                let dimension_offset = attr_opt_num(e, "group", "dimensionOffset")?.unwrap_or(0);
                self.groups.push(SbeGroupDef {
                    tag: attr_num(e, "group", "id")?,
                    name,
                    block_length,
                    dimension_offset,
                    fields: Vec::new(),
                    nested_groups: Vec::new(),
                    var_data: Vec::new(),
                });
                if is_empty {
                    self.finish_group();
                }
            }
            "data" | "varData" => {
                let data = SbeVarDataDef {
                    tag: attr_num(e, "data", "id")?,
                    name: attr(e, "name")?.ok_or(SbeSchemaError::MissingAttr {
                        elem: "data",
                        attr: "name",
                    })?,
                    length_field_size: attr_opt_num(e, "data", "lengthFieldSize")?
                        .unwrap_or(1)
                        .try_into()
                        .map_err(|_| SbeSchemaError::NotANumber {
                            elem: "data",
                            attr: "lengthFieldSize",
                            value: "too large".to_owned(),
                        })?,
                };
                self.push_var_data(data);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_end(&mut self, name: &str) -> Result<(), SbeSchemaError> {
        match name {
            "group" => self.finish_group(),
            "message" => {
                if let Some(message) = self.current_message.take() {
                    validate_offsets(&message.fields)?;
                    self.insert_message(message)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn push_field(&mut self, field: SbeFieldDef) {
        if let Some(group) = self.groups.last_mut() {
            group.fields.push(field);
        } else if let Some(message) = self.current_message.as_mut() {
            message.fields.push(field);
        }
    }

    fn push_var_data(&mut self, data: SbeVarDataDef) {
        if let Some(group) = self.groups.last_mut() {
            group.var_data.push(data);
        } else if let Some(message) = self.current_message.as_mut() {
            message.var_data.push(data);
        }
    }

    fn finish_group(&mut self) {
        let Some(group) = self.groups.pop() else {
            return;
        };
        if let Some(parent) = self.groups.last_mut() {
            parent.nested_groups.push(group);
        } else if let Some(message) = self.current_message.as_mut() {
            message.groups.push(group);
        }
    }

    fn insert_message(&mut self, message: SbeMessageSchema) -> Result<(), SbeSchemaError> {
        let id = message.template_id;
        if self.messages.insert(id, message).is_some() {
            return Err(SbeSchemaError::DuplicateTemplateId { id });
        }
        Ok(())
    }
}

fn validate_offsets(fields: &[SbeFieldDef]) -> Result<(), SbeSchemaError> {
    let mut used = BTreeSet::new();
    for field in fields {
        let end = field.offset.saturating_add(field.data_type.width());
        for offset in field.offset..end {
            if !used.insert(offset) {
                return Err(SbeSchemaError::OverlappingOffset {
                    field: field.name.clone(),
                    offset,
                });
            }
        }
    }
    Ok(())
}

fn field_from_attrs(e: &quick_xml::events::BytesStart<'_>) -> Result<SbeFieldDef, SbeSchemaError> {
    let name = attr(e, "name")?.ok_or(SbeSchemaError::MissingAttr {
        elem: "field",
        attr: "name",
    })?;
    let type_name = attr(e, "type")?.ok_or(SbeSchemaError::MissingAttr {
        elem: "field",
        attr: "type",
    })?;
    let data_type = parse_data_type(&type_name).ok_or(SbeSchemaError::UnknownDataType {
        field: name.clone(),
        type_name,
    })?;
    let byte_order = match attr(e, "byteOrder")?.as_deref() {
        Some("bigEndian") | Some("big") => ByteOrder::BigEndian,
        _ => ByteOrder::LittleEndian,
    };
    Ok(SbeFieldDef {
        tag: attr_num(e, "field", "id")?,
        name,
        offset: attr_num(e, "field", "offset")?,
        data_type,
        byte_order,
    })
}

fn parse_data_type(type_name: &str) -> Option<SbeDataType> {
    Some(match type_name.to_ascii_lowercase().as_str() {
        "uint8" => SbeDataType::UInt8,
        "uint16" => SbeDataType::UInt16,
        "uint32" => SbeDataType::UInt32,
        "int32" => SbeDataType::Int32,
        "uint64" => SbeDataType::UInt64,
        "int64" => SbeDataType::Int64,
        "char" => SbeDataType::Char,
        _ => return None,
    })
}

fn local_name(e: &quick_xml::events::BytesStart<'_>) -> String {
    local_name_bytes(e.local_name().as_ref())
}

fn local_name_bytes(bytes: &[u8]) -> String {
    let name = std::str::from_utf8(bytes).unwrap_or("");
    match name.rsplit_once(':') {
        Some((_, local)) => local.to_owned(),
        None => name.to_owned(),
    }
}

fn attr(
    e: &quick_xml::events::BytesStart<'_>,
    key: &str,
) -> Result<Option<String>, SbeSchemaError> {
    for a in e.attributes() {
        let a = a.map_err(|err| SbeSchemaError::Xml {
            pos: 0,
            reason: err.to_string(),
        })?;
        if local_name_bytes(a.key.as_ref()) == key {
            let value = a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|err| SbeSchemaError::Xml {
                    pos: 0,
                    reason: err.to_string(),
                })?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

fn attr_num(
    e: &quick_xml::events::BytesStart<'_>,
    elem: &'static str,
    key: &'static str,
) -> Result<u32, SbeSchemaError> {
    let value = attr(e, key)?.ok_or(SbeSchemaError::MissingAttr { elem, attr: key })?;
    value.parse().map_err(|_| SbeSchemaError::NotANumber {
        elem,
        attr: key,
        value,
    })
}

fn attr_opt_num(
    e: &quick_xml::events::BytesStart<'_>,
    elem: &'static str,
    key: &'static str,
) -> Result<Option<u32>, SbeSchemaError> {
    attr(e, key)?
        .map(|value| {
            value.parse().map_err(|_| SbeSchemaError::NotANumber {
                elem,
                attr: key,
                value,
            })
        })
        .transpose()
}
