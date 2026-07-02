//! FIX Orchestra XML → normalized `.fixdict` conversion (US9, FR-012).
//!
//! Build/tooling-time only (feature `dict-tooling`, gated on `quick-xml`; never compiled into the
//! runtime engine's default dependency graph — see `research.md` §4). Parses a representative
//! subset of the FIX Orchestra repository schema (FPL's open, machine-readable dictionary format —
//! `<fixr:repository>` containing `<fixr:fields>`, `<fixr:components>`, `<fixr:groups>`,
//! `<fixr:messages>`) and emits the same normalized-dictionary grammar every bundled `.fixdict`
//! already uses (`crate::parser`). No Orchestra XML content is vendored: this module reproduces the
//! FPL-published *schema shape* only, and the bundled `FIXLATEST.orchestra.xml` fixture that feeds
//! `dict-src/normalized/FIXLATEST.fixdict` is authored from that shape, exactly like the other 9
//! bundled dictionaries were hand-normalized from the public FIX specification (Principle III).
//!
//! Supported schema subset:
//! - `<fixr:field id="TAG" name="NAME" type="TYPE">` (children: `<fixr:code value="V" name="L"/>`
//!   for enumerated values)
//! - `<fixr:component id="ID" name="NAME">` (children: `<fixr:fieldRef id="TAG"/>`,
//!   `<fixr:groupRef id="COUNT_TAG"/>`, `<fixr:componentRef id="ID"/>`); components named
//!   `StandardHeader`/`StandardTrailer` become the dictionary's `header`/`trailer` directive
//!   instead of a `component` directive (their fields are implicit on every message, matching the
//!   convention every other bundled dictionary already follows)
//! - `<fixr:group id="COUNT_TAG" name="NAME">` (children: ordered `<fixr:fieldRef id="TAG"/>`; the
//!   first is the delimiter, matching `parser::parse`'s `group` grammar)
//! - `<fixr:message name="NAME" msgType="TYPE"><fixr:structure>` (children:
//!   `<fixr:fieldRef id="TAG" presence="required|optional"/>`, `<fixr:groupRef id="COUNT_TAG" .../>`,
//!   `<fixr:componentRef id="ID" presence="..."/>`; `presence` defaults to `optional`)
//!
//! Nested groups/components-within-groups are out of scope for this converter (the bundled
//! dictionaries do not use them either).

use std::collections::BTreeMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// An error converting Orchestra XML to the normalized `.fixdict` grammar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OrchestraError {
    /// The XML itself was malformed.
    #[error("XML parse error at byte {pos}: {reason}")]
    Xml {
        /// Byte offset of the error.
        pos: usize,
        /// The underlying reason.
        reason: String,
    },
    /// A required attribute was missing on an element.
    #[error("<{elem}> is missing required attribute {attr:?}")]
    MissingAttr {
        /// The element name.
        elem: &'static str,
        /// The missing attribute name.
        attr: &'static str,
    },
    /// A `<fixr:field>`'s `type` did not map to a known normalized [`crate::FieldType`] token.
    #[error("field id={id} has unknown Orchestra type {type_name:?}")]
    UnknownType {
        /// The field's tag (as text, since it may have failed to parse as a number first).
        id: String,
        /// The offending type name.
        type_name: String,
    },
    /// A `componentRef`/`groupRef` named a component/group id that was never defined.
    #[error("unknown {kind} id {id:?}")]
    UnknownRef {
        /// `"component"` or `"group"`.
        kind: &'static str,
        /// The referenced (missing) id.
        id: String,
    },
    /// An `id`/tag attribute did not parse as a number.
    #[error("<{elem}> attribute {attr:?} is not a number: {value:?}")]
    NotANumber {
        /// The element name.
        elem: &'static str,
        /// The attribute name.
        attr: &'static str,
        /// The offending value.
        value: String,
    },
}

struct RawField {
    tag: u32,
    name: String,
    type_name: String,
    values: Vec<(String, String)>, // (value, label)
}

enum Member {
    Field(u32),
    Component(String), // resolved component name
}

struct RawComponent {
    name: String,
    members: Vec<Member>,
}

struct RawGroup {
    count_tag: u32,
    name: String,
    member_tags: Vec<u32>,
}

struct RawMessage {
    msg_type: String,
    name: String,
    required: Vec<Member>,
    optional: Vec<Member>,
}

/// In-progress `<fixr:field>`: `(tag, name, type_name, enum values)`.
type FieldInProgress = (u32, String, String, Vec<(String, String)>);
/// In-progress `<fixr:component>`: `(id, name, members)`.
type ComponentInProgress = (u32, String, Vec<Member>);
/// In-progress `<fixr:group>`: `(count_tag, name, member tags)`.
type GroupInProgress = (u32, String, Vec<u32>);
/// In-progress `<fixr:message>`: `(msg_type, name, required members, optional members)`.
type MessageInProgress = (String, String, Vec<Member>, Vec<Member>);

/// Accumulates parse state across the single-pass XML walk. Held as a struct (rather than a
/// closure over many locals) so `handle_start_or_empty`/`handle_end` can each take `&mut self`.
#[derive(Default)]
struct Parser {
    version: String,
    fields: Vec<RawField>,
    component_id_to_name: BTreeMap<u32, String>,
    components: Vec<RawComponent>,
    header: Vec<u32>,
    trailer: Vec<u32>,
    groups: Vec<RawGroup>,
    messages: Vec<RawMessage>,

    // Innermost-element context; these never nest into each other beyond one level.
    current_field: Option<FieldInProgress>,
    current_component: Option<ComponentInProgress>,
    current_group: Option<GroupInProgress>,
    current_message: Option<MessageInProgress>,
    in_structure: bool,
}

impl Parser {
    fn handle_start_or_empty(
        &mut self,
        e: &quick_xml::events::BytesStart<'_>,
        is_empty: bool,
    ) -> Result<(), OrchestraError> {
        match local_name(e).as_str() {
            "repository" => {
                self.version = attr(e, "version")?.unwrap_or_default();
            }
            "field" => {
                let tag = attr_num(e, "field", "id")?;
                let name = attr(e, "name")?.ok_or(OrchestraError::MissingAttr {
                    elem: "field",
                    attr: "name",
                })?;
                let type_name = attr(e, "type")?.ok_or(OrchestraError::MissingAttr {
                    elem: "field",
                    attr: "type",
                })?;
                if is_empty {
                    self.fields.push(RawField {
                        tag,
                        name,
                        type_name,
                        values: Vec::new(),
                    });
                } else {
                    self.current_field = Some((tag, name, type_name, Vec::new()));
                }
            }
            "code" if self.current_field.is_some() => {
                let value = attr(e, "value")?.ok_or(OrchestraError::MissingAttr {
                    elem: "code",
                    attr: "value",
                })?;
                let name = attr(e, "name")?.ok_or(OrchestraError::MissingAttr {
                    elem: "code",
                    attr: "name",
                })?;
                if let Some((_, _, _, values)) = self.current_field.as_mut() {
                    values.push((value, name));
                }
            }
            "component" => {
                let id = attr_num(e, "component", "id")?;
                let name = attr(e, "name")?.ok_or(OrchestraError::MissingAttr {
                    elem: "component",
                    attr: "name",
                })?;
                self.component_id_to_name.insert(id, name.clone());
                if is_empty {
                    self.components.push(RawComponent {
                        name,
                        members: Vec::new(),
                    });
                } else {
                    self.current_component = Some((id, name, Vec::new()));
                }
            }
            "group" => {
                let count_tag = attr_num(e, "group", "id")?;
                let name = attr(e, "name")?.ok_or(OrchestraError::MissingAttr {
                    elem: "group",
                    attr: "name",
                })?;
                if is_empty {
                    self.groups.push(RawGroup {
                        count_tag,
                        name,
                        member_tags: Vec::new(),
                    });
                } else {
                    self.current_group = Some((count_tag, name, Vec::new()));
                }
            }
            "message" => {
                let msg_type = attr(e, "msgType")?.ok_or(OrchestraError::MissingAttr {
                    elem: "message",
                    attr: "msgType",
                })?;
                let name = attr(e, "name")?.ok_or(OrchestraError::MissingAttr {
                    elem: "message",
                    attr: "name",
                })?;
                self.current_message = Some((msg_type, name, Vec::new(), Vec::new()));
            }
            "structure" => self.in_structure = true,
            "fieldRef" if self.current_component.is_some() => {
                let tag = attr_num(e, "fieldRef", "id")?;
                if let Some((_, _, members)) = self.current_component.as_mut() {
                    members.push(Member::Field(tag));
                }
            }
            "groupRef" if self.current_component.is_some() => {
                let tag = attr_num(e, "groupRef", "id")?;
                if let Some((_, _, members)) = self.current_component.as_mut() {
                    members.push(Member::Field(tag));
                }
            }
            "componentRef" if self.current_component.is_some() => {
                let id = attr_num(e, "componentRef", "id")?;
                let name = self.resolve_component_name(id)?;
                if let Some((_, _, members)) = self.current_component.as_mut() {
                    members.push(Member::Component(name));
                }
            }
            "fieldRef" if self.current_group.is_some() => {
                let tag = attr_num(e, "fieldRef", "id")?;
                if let Some((_, _, members)) = self.current_group.as_mut() {
                    members.push(tag);
                }
            }
            "fieldRef" if self.in_structure && self.current_message.is_some() => {
                let tag = attr_num(e, "fieldRef", "id")?;
                let required = attr(e, "presence")?.as_deref() == Some("required");
                self.push_message_member(Member::Field(tag), required);
            }
            "groupRef" if self.in_structure && self.current_message.is_some() => {
                let tag = attr_num(e, "groupRef", "id")?;
                let required = attr(e, "presence")?.as_deref() == Some("required");
                self.push_message_member(Member::Field(tag), required);
            }
            "componentRef" if self.in_structure && self.current_message.is_some() => {
                let id = attr_num(e, "componentRef", "id")?;
                let name = self.resolve_component_name(id)?;
                if name == "StandardHeader" || name == "StandardTrailer" {
                    // Implicit on every message; not part of req/opt member lists.
                } else {
                    let required = attr(e, "presence")?.as_deref() == Some("required");
                    self.push_message_member(Member::Component(name), required);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve_component_name(&self, id: u32) -> Result<String, OrchestraError> {
        self.component_id_to_name
            .get(&id)
            .cloned()
            .ok_or(OrchestraError::UnknownRef {
                kind: "component",
                id: id.to_string(),
            })
    }

    fn push_message_member(&mut self, member: Member, required: bool) {
        if let Some((_, _, req, opt)) = self.current_message.as_mut() {
            if required {
                req.push(member);
            } else {
                opt.push(member);
            }
        }
    }

    fn handle_end(&mut self, local: &str) {
        match local {
            "field" => {
                if let Some((tag, name, type_name, values)) = self.current_field.take() {
                    self.fields.push(RawField {
                        tag,
                        name,
                        type_name,
                        values,
                    });
                }
            }
            "component" => {
                if let Some((_id, name, members)) = self.current_component.take() {
                    if name == "StandardHeader" {
                        self.header = members
                            .into_iter()
                            .filter_map(|m| match m {
                                Member::Field(t) => Some(t),
                                Member::Component(_) => None,
                            })
                            .collect();
                    } else if name == "StandardTrailer" {
                        self.trailer = members
                            .into_iter()
                            .filter_map(|m| match m {
                                Member::Field(t) => Some(t),
                                Member::Component(_) => None,
                            })
                            .collect();
                    } else {
                        self.components.push(RawComponent { name, members });
                    }
                }
            }
            "group" => {
                if let Some((count_tag, name, member_tags)) = self.current_group.take() {
                    self.groups.push(RawGroup {
                        count_tag,
                        name,
                        member_tags,
                    });
                }
            }
            "structure" => self.in_structure = false,
            "message" => {
                if let Some((msg_type, name, required, optional)) = self.current_message.take() {
                    self.messages.push(RawMessage {
                        msg_type,
                        name,
                        required,
                        optional,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Convert Orchestra repository XML into the normalized `.fixdict` text grammar.
pub fn convert(xml: &str) -> Result<String, OrchestraError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut parser = Parser::default();

    loop {
        let event = reader.read_event().map_err(|e| OrchestraError::Xml {
            pos: reader.buffer_position() as usize,
            reason: e.to_string(),
        })?;
        match event {
            Event::Eof => break,
            Event::Start(e) => parser.handle_start_or_empty(&e, false)?,
            Event::Empty(e) => parser.handle_start_or_empty(&e, true)?,
            Event::End(e) => parser.handle_end(&local_name_bytes(e.name().as_ref())),
            _ => {}
        }
    }

    render(
        &parser.version,
        &parser.fields,
        &parser.header,
        &parser.trailer,
        &parser.groups,
        &parser.components,
        &parser.messages,
    )
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
) -> Result<Option<String>, OrchestraError> {
    for a in e.attributes() {
        let a = a.map_err(|err| OrchestraError::Xml {
            pos: 0,
            reason: err.to_string(),
        })?;
        if local_name_bytes(a.key.as_ref()) == key {
            let value = a.unescape_value().map_err(|err| OrchestraError::Xml {
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
) -> Result<u32, OrchestraError> {
    let value = attr(e, key)?.ok_or(OrchestraError::MissingAttr { elem, attr: key })?;
    value.parse().map_err(|_| OrchestraError::NotANumber {
        elem,
        attr: key,
        value,
    })
}

/// Map an Orchestra `type` name to a normalized `.fixdict` type token (see
/// [`crate::FieldType::parse`] for the accepted set).
fn map_type(type_name: &str) -> Option<&'static str> {
    Some(match type_name.to_ascii_lowercase().as_str() {
        "int" => "INT",
        "length" => "LENGTH",
        "seqnum" => "SEQNUM",
        "numingroup" => "NUMINGROUP",
        "float" => "FLOAT",
        "price" => "PRICE",
        "qty" | "quantity" => "QTY",
        "amt" | "amount" => "AMT",
        "percentage" => "PERCENTAGE",
        "char" => "CHAR",
        "boolean" => "BOOLEAN",
        "string" => "STRING",
        "data" => "DATA",
        "utctimestamp" => "UTCTIMESTAMP",
        "utctimeonly" => "UTCTIMEONLY",
        "utcdateonly" | "localmktdate" => "UTCDATEONLY",
        "monthyear" => "MONTHYEAR",
        _ => return None,
    })
}

fn render(
    version: &str,
    fields: &[RawField],
    header: &[u32],
    trailer: &[u32],
    groups: &[RawGroup],
    components: &[RawComponent],
    messages: &[RawMessage],
) -> Result<String, OrchestraError> {
    let mut out = String::new();
    out.push_str(
        "# Generated by truefix-dict's Orchestra conversion tool (US9) — do not edit by\n",
    );
    out.push_str("# hand; regenerate from the Orchestra source fixture instead.\n");
    out.push_str(&format!("version {version}\n\n"));

    for f in fields {
        let type_token = map_type(&f.type_name).ok_or_else(|| OrchestraError::UnknownType {
            id: f.tag.to_string(),
            type_name: f.type_name.clone(),
        })?;
        out.push_str(&format!("field {} {} {}", f.tag, f.name, type_token));
        for (value, label) in &f.values {
            out.push_str(&format!(" {value}={label}"));
        }
        out.push('\n');
    }
    out.push('\n');

    if !header.is_empty() {
        let tags: Vec<String> = header.iter().map(u32::to_string).collect();
        out.push_str(&format!("header {}\n", tags.join(" ")));
    }
    if !trailer.is_empty() {
        let tags: Vec<String> = trailer.iter().map(u32::to_string).collect();
        out.push_str(&format!("trailer {}\n", tags.join(" ")));
    }
    if !header.is_empty() || !trailer.is_empty() {
        out.push('\n');
    }

    for g in groups {
        let delimiter = g.member_tags.first().copied().unwrap_or(g.count_tag);
        let members: Vec<String> = g.member_tags.iter().map(u32::to_string).collect();
        out.push_str(&format!(
            "group {} {} {} {}\n",
            g.count_tag,
            g.name,
            delimiter,
            members.join(",")
        ));
    }
    if !groups.is_empty() {
        out.push('\n');
    }

    for c in components {
        let tokens: Vec<String> = c
            .members
            .iter()
            .map(|m| match m {
                Member::Field(t) => t.to_string(),
                Member::Component(n) => format!("component:{n}"),
            })
            .collect();
        out.push_str(&format!("component {} {}\n", c.name, tokens.join(",")));
    }
    if !components.is_empty() {
        out.push('\n');
    }

    for m in messages {
        let req: Vec<String> = m
            .required
            .iter()
            .map(|mem| match mem {
                Member::Field(t) => t.to_string(),
                Member::Component(n) => format!("component:{n}"),
            })
            .collect();
        let opt: Vec<String> = m
            .optional
            .iter()
            .map(|mem| match mem {
                Member::Field(t) => t.to_string(),
                Member::Component(n) => format!("component:{n}"),
            })
            .collect();
        out.push_str(&format!("message {} {}", m.msg_type, m.name));
        if !req.is_empty() {
            out.push_str(&format!(" req:{}", req.join(",")));
        }
        if !opt.is_empty() {
            out.push_str(&format!(" opt:{}", opt.join(",")));
        }
        out.push('\n');
    }

    Ok(out)
}
