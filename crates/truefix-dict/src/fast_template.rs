//! FAST template XML parsing.
//!
//! This module is available only with the `dict-tooling` feature, matching the existing XML
//! converter modules in this crate. It parses the FAST template subset needed by TrueFix's binary
//! codec: template IDs, scalar fields, field operators, nullability, and nested groups.

use std::collections::BTreeMap;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// FAST template identifier.
pub type TemplateId = u32;

/// A loaded set of FAST templates keyed by template id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastTemplateSet {
    templates: BTreeMap<TemplateId, FastTemplate>,
}

impl FastTemplateSet {
    /// Return the template for `id`.
    pub fn get(&self, id: TemplateId) -> Option<&FastTemplate> {
        self.templates.get(&id)
    }

    /// Iterate templates in stable template-id order.
    pub fn templates(&self) -> impl Iterator<Item = &FastTemplate> {
        self.templates.values()
    }
}

/// One FAST template definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastTemplate {
    /// Template id.
    pub id: TemplateId,
    /// Template name.
    pub name: String,
    /// Ordered field definitions.
    pub fields: Vec<FastFieldDef>,
}

/// One FAST field definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastFieldDef {
    /// FIX tag.
    pub tag: u32,
    /// Field name.
    pub name: String,
    /// FAST scalar type.
    pub data_type: FastDataType,
    /// FAST field operator.
    pub operator: FastOperator,
    /// Whether the field is nullable.
    pub nullable: bool,
    /// Repeating-group definition when this field is a group count field.
    pub group: Option<Box<FastGroupDef>>,
}

/// Supported FAST scalar types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastDataType {
    /// Unsigned 32-bit integer.
    UInt32,
    /// Signed 32-bit integer.
    Int32,
    /// Unsigned 64-bit integer.
    UInt64,
    /// Signed 64-bit integer.
    Int64,
    /// ASCII string.
    AsciiString,
    /// Unicode string.
    UnicodeString,
    /// Decimal value.
    Decimal,
}

/// Supported FAST operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastOperator {
    /// Copy operator.
    Copy,
    /// Increment operator.
    Increment,
    /// Delta operator.
    Delta,
    /// Default operator with optional value.
    Default { value: Option<String> },
    /// No operator.
    None,
}

/// FAST repeating-group definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastGroupDef {
    /// Group count field.
    pub count_field: FastFieldDef,
    /// Ordered group members.
    pub members: Vec<FastFieldDef>,
}

/// FAST template parse/load error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FastTemplateError {
    /// The XML itself was malformed.
    #[error("XML parse error at byte {pos}: {reason}")]
    Xml {
        /// Byte offset of the error.
        pos: usize,
        /// Underlying reason.
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
    /// A field declared an unknown operator.
    #[error("field {field:?} has unknown FAST operator {operator:?}")]
    UnknownOperator {
        /// Field name.
        field: String,
        /// Operator name.
        operator: String,
    },
    /// A field declared an unknown data type.
    #[error("field {field:?} has unknown FAST data type {type_name:?}")]
    UnknownDataType {
        /// Field name.
        field: String,
        /// Data type name.
        type_name: String,
    },
    /// Two templates declared the same id.
    #[error("duplicate FAST template id {id}")]
    DuplicateTemplateId {
        /// Duplicated template id.
        id: TemplateId,
    },
    /// A file could not be read.
    #[error("could not read FAST template file {path}: {reason}")]
    Io {
        /// File path.
        path: String,
        /// I/O reason.
        reason: String,
    },
}

struct TemplateFrame {
    id: TemplateId,
    name: String,
    fields: Vec<FastFieldDef>,
}

struct GroupFrame {
    count_field: FastFieldDef,
    members: Vec<FastFieldDef>,
}

/// Load FAST templates from a file.
pub fn load_fast_templates_from_file(path: &Path) -> Result<FastTemplateSet, FastTemplateError> {
    let xml = std::fs::read_to_string(path).map_err(|err| FastTemplateError::Io {
        path: path.display().to_string(),
        reason: err.to_string(),
    })?;
    parse_fast_templates(&xml)
}

/// Parse FAST templates from XML text.
pub fn parse_fast_templates(xml: &str) -> Result<FastTemplateSet, FastTemplateError> {
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
                return Err(FastTemplateError::Xml {
                    pos: reader.buffer_position() as usize,
                    reason: err.to_string(),
                });
            }
        }
    }

    Ok(FastTemplateSet {
        templates: parser.templates,
    })
}

#[derive(Default)]
struct Parser {
    templates: BTreeMap<TemplateId, FastTemplate>,
    current_template: Option<TemplateFrame>,
    groups: Vec<GroupFrame>,
}

impl Parser {
    fn handle_start_or_empty(
        &mut self,
        e: &quick_xml::events::BytesStart<'_>,
        is_empty: bool,
    ) -> Result<(), FastTemplateError> {
        match local_name(e).as_str() {
            "template" => {
                let id = attr_num(e, "template", "id")?;
                if self.templates.contains_key(&id) {
                    return Err(FastTemplateError::DuplicateTemplateId { id });
                }
                let name = attr(e, "name")?.unwrap_or_else(|| id.to_string());
                if is_empty {
                    self.insert_template(FastTemplate {
                        id,
                        name,
                        fields: Vec::new(),
                    })?;
                } else {
                    self.current_template = Some(TemplateFrame {
                        id,
                        name,
                        fields: Vec::new(),
                    });
                }
            }
            "group" => {
                let count_field = field_from_attrs(e, Some(FastDataType::UInt32))?;
                self.groups.push(GroupFrame {
                    count_field,
                    members: Vec::new(),
                });
                if is_empty {
                    self.finish_group()?;
                }
            }
            "field" => {
                let field = field_from_attrs(e, None)?;
                self.push_field(field);
            }
            "uInt32" | "int32" | "uInt64" | "int64" | "string" | "ascii" | "unicode"
            | "decimal" => {
                let elem = local_name(e);
                let ty = type_from_element(&elem)?;
                let field = field_from_attrs(e, Some(ty))?;
                self.push_field(field);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_end(&mut self, name: &str) -> Result<(), FastTemplateError> {
        match name {
            "group" => self.finish_group()?,
            "template" => {
                if let Some(template) = self.current_template.take() {
                    self.insert_template(FastTemplate {
                        id: template.id,
                        name: template.name,
                        fields: template.fields,
                    })?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn push_field(&mut self, field: FastFieldDef) {
        if let Some(group) = self.groups.last_mut() {
            group.members.push(field);
        } else if let Some(template) = self.current_template.as_mut() {
            template.fields.push(field);
        }
    }

    fn finish_group(&mut self) -> Result<(), FastTemplateError> {
        let Some(group) = self.groups.pop() else {
            return Ok(());
        };
        let field = FastFieldDef {
            group: Some(Box::new(FastGroupDef {
                count_field: group.count_field.clone(),
                members: group.members,
            })),
            ..group.count_field
        };
        self.push_field(field);
        Ok(())
    }

    fn insert_template(&mut self, template: FastTemplate) -> Result<(), FastTemplateError> {
        if self.templates.insert(template.id, template).is_some() {
            return Err(FastTemplateError::DuplicateTemplateId {
                id: self
                    .templates
                    .keys()
                    .next_back()
                    .copied()
                    .unwrap_or_default(),
            });
        }
        Ok(())
    }
}

fn field_from_attrs(
    e: &quick_xml::events::BytesStart<'_>,
    fallback_type: Option<FastDataType>,
) -> Result<FastFieldDef, FastTemplateError> {
    let name = attr(e, "name")?.ok_or(FastTemplateError::MissingAttr {
        elem: "field",
        attr: "name",
    })?;
    let tag = attr_num(e, "field", "id")?;
    let data_type = match attr(e, "type")? {
        Some(type_name) => {
            parse_data_type(&type_name).ok_or(FastTemplateError::UnknownDataType {
                field: name.clone(),
                type_name,
            })?
        }
        None => fallback_type.ok_or(FastTemplateError::MissingAttr {
            elem: "field",
            attr: "type",
        })?,
    };
    let operator = match attr(e, "operator")? {
        Some(op) => parse_operator(&name, &op, attr(e, "value")?)?,
        None => FastOperator::None,
    };
    let nullable = attr(e, "nullable")?
        .map(|value| matches!(value.as_str(), "true" | "1" | "Y" | "y"))
        .unwrap_or(false);

    Ok(FastFieldDef {
        tag,
        name,
        data_type,
        operator,
        nullable,
        group: None,
    })
}

fn parse_operator(
    field: &str,
    op: &str,
    value: Option<String>,
) -> Result<FastOperator, FastTemplateError> {
    Ok(match op.to_ascii_lowercase().as_str() {
        "copy" => FastOperator::Copy,
        "increment" => FastOperator::Increment,
        "delta" => FastOperator::Delta,
        "default" => FastOperator::Default { value },
        "none" => FastOperator::None,
        _ => {
            return Err(FastTemplateError::UnknownOperator {
                field: field.to_owned(),
                operator: op.to_owned(),
            });
        }
    })
}

fn parse_data_type(type_name: &str) -> Option<FastDataType> {
    Some(match type_name.to_ascii_lowercase().as_str() {
        "uint32" | "u32" | "uint" => FastDataType::UInt32,
        "int32" | "i32" | "int" => FastDataType::Int32,
        "uint64" | "u64" => FastDataType::UInt64,
        "int64" | "i64" | "long" => FastDataType::Int64,
        "asciistring" | "ascii" | "string" => FastDataType::AsciiString,
        "unicodestring" | "unicode" => FastDataType::UnicodeString,
        "decimal" => FastDataType::Decimal,
        _ => return None,
    })
}

fn type_from_element(elem: &str) -> Result<FastDataType, FastTemplateError> {
    parse_data_type(elem).ok_or(FastTemplateError::UnknownDataType {
        field: elem.to_owned(),
        type_name: elem.to_owned(),
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
) -> Result<Option<String>, FastTemplateError> {
    for a in e.attributes() {
        let a = a.map_err(|err| FastTemplateError::Xml {
            pos: 0,
            reason: err.to_string(),
        })?;
        if local_name_bytes(a.key.as_ref()) == key {
            let value = a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|err| FastTemplateError::Xml {
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
) -> Result<u32, FastTemplateError> {
    let value = attr(e, key)?.ok_or(FastTemplateError::MissingAttr { elem, attr: key })?;
    value.parse().map_err(|_| FastTemplateError::NotANumber {
        elem,
        attr: key,
        value,
    })
}
