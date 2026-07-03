//! QuickFIX-family DTD-based XML dictionary → normalized `.fixdict` conversion (US9, feature 005,
//! FR-031).
//!
//! Build/tooling-time only (feature `dict-tooling`, gated on `quick-xml`; never compiled into the
//! runtime engine's default dependency graph — matches [`crate::orchestra`]'s own scoping).
//! Converts the classic QuickFIX DTD-based dictionary XML schema (`<fix><fields/><header/>
//! <trailer/><messages/><components/></fix>`, used by both the `quickfix` C++ engine's bundled
//! specs and QuickFIX/J's own `dictionary/*.xml` — the same schema shape) into the same
//! normalized-dictionary grammar every bundled `.fixdict` already uses (`crate::parser`). No
//! QuickFIX/J source is copied: this module reproduces the published *schema shape* only, reading
//! field/message/group/component names, tags, types, and associations — data facts, not source
//! code (Constitution Principle III, matching this project's established XML-conversion precedent
//! in [`crate::orchestra`]).
//!
//! Schema notes (distinct from FIX Orchestra's flatter shape):
//! - `<fields>` defines every field once (`<field number="TAG" name="NAME" type="TYPE">`, with
//!   `<value enum="V" description="LABEL"/>` children for enumerated values) — always appears
//!   last in the document, so this module makes two passes: fields first, then structure.
//! - `<header>`/`<trailer>`/`<message>`/`<component>` bodies are *recursively* nested: a `<field
//!   name="NAME" required="Y|N"/>` (self-closing) is a *reference* by name (resolved against the
//!   fields pass); a `<component name="NAME" required="Y|N"/>` (self-closing) is a reference to a
//!   component defined elsewhere in `<components>`; a `<group name="NAME" required="Y|N">...
//!   </group>` (never self-closing) defines a repeating group *inline*, wherever it's used —
//!   there is no separate top-level `<groups>` section the way Orchestra has one.
//! - A group's own body has no required/optional distinction in QuickFIX's schema either
//!   (`required` only appears on the group's own opening tag, not on its member fields) — matches
//!   the normalized `.fixdict` grammar's own `group` directive, which is a flat member list.

use std::collections::BTreeMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// An error converting QuickFIX-schema XML to the normalized `.fixdict` grammar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QfjXmlError {
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
    /// A `<field>`'s `type` did not map to a known normalized [`crate::FieldType`] token.
    #[error("field {name:?} has unknown QuickFIX type {type_name:?}")]
    UnknownType {
        /// The field's name.
        name: String,
        /// The offending type name.
        type_name: String,
    },
    /// A `<field required=.../>`/`<group name=.../>` reference named a field never defined in
    /// `<fields>`.
    #[error("unknown field name {name:?}")]
    UnknownFieldName {
        /// The referenced (missing) field name.
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Member {
    Field(u32),
    Component(String),
}

struct RawField {
    tag: u32,
    type_name: String,
    values: Vec<(String, String)>,
}

struct RawGroup {
    count_tag: u32,
    delimiter: u32,
    members: Vec<Member>,
}

struct RawComponent {
    name: String,
    members: Vec<Member>,
}

struct RawMessage {
    msg_type: String,
    name: String,
    required: Vec<Member>,
    optional: Vec<Member>,
}

/// A frame of the recursive header/trailer/message/component/group body being accumulated.
enum Frame {
    Header {
        members: Vec<Member>,
    },
    Trailer {
        members: Vec<Member>,
    },
    Message {
        msg_type: String,
        name: String,
        required: Vec<Member>,
        optional: Vec<Member>,
    },
    Component {
        name: String,
        members: Vec<Member>,
    },
    Group {
        count_tag: u32,
        required: bool,
        members: Vec<Member>,
    },
}

impl Frame {
    /// Add a member to this frame's own body. `required` is ignored for `Header`/`Trailer`/
    /// `Component`/`Group` (the normalized grammar has no required/optional distinction there);
    /// only `Message` actually splits on it.
    fn push_member(&mut self, m: Member, required: bool) {
        match self {
            Frame::Header { members }
            | Frame::Trailer { members }
            | Frame::Component { members, .. }
            | Frame::Group { members, .. } => members.push(m),
            Frame::Message {
                required: req,
                optional: opt,
                ..
            } => {
                if required {
                    req.push(m);
                } else {
                    opt.push(m);
                }
            }
        }
    }
}

/// Pass 1: collect every `<fields>`-declared field definition, keyed by name (QuickFIX's schema
/// references fields by name everywhere outside `<fields>` itself).
fn parse_fields(xml: &str) -> Result<BTreeMap<String, RawField>, QfjXmlError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut in_fields = false;
    let mut fields: BTreeMap<String, RawField> = BTreeMap::new();
    let mut current: Option<(String, RawField)> = None;

    loop {
        let event = reader.read_event().map_err(|e| QfjXmlError::Xml {
            pos: reader.buffer_position() as usize,
            reason: e.to_string(),
        })?;
        match event {
            Event::Eof => break,
            Event::Start(e) if local_name(&e) == "fields" => in_fields = true,
            Event::End(e) if local_name_bytes(e.name().as_ref()) == "fields" => in_fields = false,
            Event::Start(e) if in_fields && local_name(&e) == "field" => {
                let tag = attr_num(&e, "field", "number")?;
                let name = attr(&e, "name")?.ok_or(QfjXmlError::MissingAttr {
                    elem: "field",
                    attr: "name",
                })?;
                let type_name = attr(&e, "type")?.ok_or(QfjXmlError::MissingAttr {
                    elem: "field",
                    attr: "type",
                })?;
                current = Some((
                    name,
                    RawField {
                        tag,
                        type_name,
                        values: Vec::new(),
                    },
                ));
            }
            Event::Empty(e) if in_fields && local_name(&e) == "field" => {
                let tag = attr_num(&e, "field", "number")?;
                let name = attr(&e, "name")?.ok_or(QfjXmlError::MissingAttr {
                    elem: "field",
                    attr: "name",
                })?;
                let type_name = attr(&e, "type")?.ok_or(QfjXmlError::MissingAttr {
                    elem: "field",
                    attr: "type",
                })?;
                fields.insert(
                    name,
                    RawField {
                        tag,
                        type_name,
                        values: Vec::new(),
                    },
                );
            }
            Event::Empty(e) if in_fields && local_name(&e) == "value" && current.is_some() => {
                let value = attr(&e, "enum")?.ok_or(QfjXmlError::MissingAttr {
                    elem: "value",
                    attr: "enum",
                })?;
                let label = attr(&e, "description")?.unwrap_or_default();
                if let Some((_, f)) = current.as_mut() {
                    f.values.push((value, label));
                }
            }
            Event::End(e) if in_fields && local_name_bytes(e.name().as_ref()) == "field" => {
                if let Some((name, f)) = current.take() {
                    fields.insert(name, f);
                }
            }
            _ => {}
        }
    }

    Ok(fields)
}

/// Pass 2: walk the whole document again, resolving every field/group reference by name against
/// `field_by_name`, and recursively accumulating header/trailer/message/component/group bodies
/// via an explicit stack (QuickFIX's schema nests groups/components arbitrarily deep, unlike
/// Orchestra's flatter one-level shape).
#[allow(clippy::type_complexity)]
fn parse_structure(
    xml: &str,
    field_by_name: &BTreeMap<String, RawField>,
) -> Result<
    (
        Vec<Member>,
        Vec<Member>,
        Vec<RawGroup>,
        Vec<RawComponent>,
        Vec<RawMessage>,
    ),
    QfjXmlError,
> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut stack: Vec<Frame> = Vec::new();
    let mut header: Vec<Member> = Vec::new();
    let mut trailer: Vec<Member> = Vec::new();
    let mut groups: Vec<RawGroup> = Vec::new();
    let mut components: Vec<RawComponent> = Vec::new();
    let mut messages: Vec<RawMessage> = Vec::new();
    let mut in_fields = false;

    let resolve = |name: &str| -> Result<u32, QfjXmlError> {
        field_by_name
            .get(name)
            .map(|f| f.tag)
            .ok_or_else(|| QfjXmlError::UnknownFieldName {
                name: name.to_owned(),
            })
    };

    loop {
        let event = reader.read_event().map_err(|e| QfjXmlError::Xml {
            pos: reader.buffer_position() as usize,
            reason: e.to_string(),
        })?;
        match event {
            Event::Eof => break,
            Event::Start(e) => match local_name(&e).as_str() {
                "fields" => in_fields = true,
                "header" if !in_fields => stack.push(Frame::Header {
                    members: Vec::new(),
                }),
                "trailer" if !in_fields => stack.push(Frame::Trailer {
                    members: Vec::new(),
                }),
                "message" if !in_fields => {
                    let msg_type = attr(&e, "msgtype")?.ok_or(QfjXmlError::MissingAttr {
                        elem: "message",
                        attr: "msgtype",
                    })?;
                    let name = attr(&e, "name")?.ok_or(QfjXmlError::MissingAttr {
                        elem: "message",
                        attr: "name",
                    })?;
                    stack.push(Frame::Message {
                        msg_type,
                        name,
                        required: Vec::new(),
                        optional: Vec::new(),
                    });
                }
                "component" if !in_fields => {
                    let name = attr(&e, "name")?.ok_or(QfjXmlError::MissingAttr {
                        elem: "component",
                        attr: "name",
                    })?;
                    stack.push(Frame::Component {
                        name,
                        members: Vec::new(),
                    });
                }
                "group" if !in_fields => {
                    let name = attr(&e, "name")?.ok_or(QfjXmlError::MissingAttr {
                        elem: "group",
                        attr: "name",
                    })?;
                    let count_tag = resolve(&name)?;
                    let required = attr(&e, "required")?.as_deref() == Some("Y");
                    stack.push(Frame::Group {
                        count_tag,
                        required,
                        members: Vec::new(),
                    });
                }
                _ => {}
            },
            Event::Empty(e) if !in_fields => match local_name(&e).as_str() {
                "field" => {
                    let name = attr(&e, "name")?.ok_or(QfjXmlError::MissingAttr {
                        elem: "field",
                        attr: "name",
                    })?;
                    let tag = resolve(&name)?;
                    let required = attr(&e, "required")?.as_deref() == Some("Y");
                    if let Some(top) = stack.last_mut() {
                        top.push_member(Member::Field(tag), required);
                    }
                }
                "component" => {
                    let name = attr(&e, "name")?.ok_or(QfjXmlError::MissingAttr {
                        elem: "component",
                        attr: "name",
                    })?;
                    // A component reference's own `required` attribute means "the component as a
                    // whole is expected" (e.g. "some instrument identification"), not "every one
                    // of its constituent fields is individually required" — confirmed empirically
                    // against FIX44.xml: zero fields *within* any <component> definition are
                    // themselves marked required='Y', even for components referenced as
                    // required='Y' (e.g. Instrument's own Symbol/SecurityID/... are all 'N').
                    // TrueFix's flat per-tag required/optional model has no way to express
                    // "at least one of these fields" component-level constraints, so every
                    // component reference is treated as optional here — the fields still count as
                    // valid/belonging to the message (via `member_tags`, unaffected by req/opt),
                    // just never individually mandatory.
                    if let Some(top) = stack.last_mut() {
                        top.push_member(Member::Component(name), false);
                    }
                }
                _ => {}
            },
            Event::End(e) => {
                let name = local_name_bytes(e.name().as_ref());
                match name.as_str() {
                    "fields" => in_fields = false,
                    "header" if !in_fields => {
                        if let Some(Frame::Header { members }) = stack.pop() {
                            header = members;
                        }
                    }
                    "trailer" if !in_fields => {
                        if let Some(Frame::Trailer { members }) = stack.pop() {
                            trailer = members;
                        }
                    }
                    "message" if !in_fields => {
                        if let Some(Frame::Message {
                            msg_type,
                            name,
                            required,
                            optional,
                        }) = stack.pop()
                        {
                            messages.push(RawMessage {
                                msg_type,
                                name,
                                required,
                                optional,
                            });
                        }
                    }
                    "component" if !in_fields => {
                        if let Some(Frame::Component { name, members }) = stack.pop() {
                            components.push(RawComponent {
                                name: name.clone(),
                                members,
                            });
                            // A <components>-section top-level definition isn't itself a
                            // reference — only push onto a parent if this component closed
                            // while genuinely nested inside another frame (a component
                            // referencing another component's *definition* inline never
                            // happens in this schema; definitions only ever sit directly under
                            // <components>, so there is intentionally no parent push here).
                        }
                    }
                    "group" if !in_fields => {
                        if let Some(Frame::Group {
                            count_tag,
                            required,
                            members,
                        }) = stack.pop()
                        {
                            let delimiter = members
                                .iter()
                                .find_map(|m| match m {
                                    Member::Field(t) => Some(*t),
                                    Member::Component(_) => None,
                                })
                                .unwrap_or(count_tag);
                            groups.push(RawGroup {
                                count_tag,
                                delimiter,
                                members,
                            });
                            if let Some(top) = stack.last_mut() {
                                top.push_member(Member::Field(count_tag), required);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok((header, trailer, groups, components, messages))
}

/// Recursively flatten `members` into a plain tag list, inlining any `Member::Component`
/// reference's own (transitively-flattened) members — used only for `header`/`trailer` emission,
/// since the normalized `.fixdict` grammar's `header`/`trailer` directives are plain tag lists
/// with no `component:` token support (unlike `message`/`group`/`component` bodies, which the
/// parser already expands `component:` references within).
fn flatten_members(members: &[Member], components: &BTreeMap<String, Vec<Member>>) -> Vec<u32> {
    let mut out = Vec::new();
    for m in members {
        match m {
            Member::Field(t) => out.push(*t),
            Member::Component(name) => {
                if let Some(sub) = components.get(name) {
                    out.extend(flatten_members(sub, components));
                }
            }
        }
    }
    out
}

/// Map a QuickFIX schema `type` name to a normalized `.fixdict` type token (see
/// [`crate::FieldType::parse`] for the accepted set). Several QuickFIX/J-only type spellings
/// (introduced for FIX 5.0+) have no dedicated normalized variant and reuse the closest
/// format-equivalent one, matching how `crate::FieldType::value_ok` itself already treats several
/// of the 11 new US9 variants as format-aliases of pre-existing ones.
fn map_type(type_name: &str) -> Option<&'static str> {
    Some(match type_name.to_ascii_uppercase().as_str() {
        "INT" => "INT",
        "LENGTH" => "LENGTH",
        "SEQNUM" => "SEQNUM",
        "NUMINGROUP" => "NUMINGROUP",
        "FLOAT" => "FLOAT",
        "PRICE" => "PRICE",
        "QTY" | "QUANTITY" => "QTY",
        "AMT" => "AMT",
        "PERCENTAGE" => "PERCENTAGE",
        "CHAR" => "CHAR",
        "BOOLEAN" => "BOOLEAN",
        "STRING" | "LANGUAGE" | "TZTIMEZONE" | "XMLDATA" => "STRING",
        "DATA" => "DATA",
        "UTCTIMESTAMP" | "TZTIMESTAMP" => "UTCTIMESTAMP",
        "UTCTIMEONLY" | "TZTIMEONLY" => "UTCTIMEONLY",
        "UTCDATEONLY" | "DATE" => "UTCDATEONLY",
        "MONTHYEAR" => "MONTHYEAR",
        "PRICEOFFSET" => "PRICEOFFSET",
        "LOCALMKTDATE" => "LOCALMKTDATE",
        "DAYOFMONTH" => "DAYOFMONTH",
        "UTCDATE" => "UTCDATE",
        "TIME" => "TIME",
        "CURRENCY" => "CURRENCY",
        "EXCHANGE" => "EXCHANGE",
        "MULTIPLEVALUESTRING" => "MULTIPLEVALUESTRING",
        "MULTIPLESTRINGVALUE" => "MULTIPLESTRINGVALUE",
        "MULTIPLECHARVALUE" => "MULTIPLECHARVALUE",
        "COUNTRY" => "COUNTRY",
        _ => return None,
    })
}

/// Convert a QuickFIX-schema dictionary XML document into the normalized `.fixdict` text grammar.
/// `version` is the `.fixdict` `version` directive's value (e.g. `"FIX.4.4"`) — QuickFIX's XML
/// carries major/minor/servicepack as separate attributes on `<fix>` rather than one version
/// string, so the caller supplies the already-formatted target string directly.
pub fn convert(xml: &str, version: &str) -> Result<String, QfjXmlError> {
    let fields_by_name = parse_fields(xml)?;
    let (header, trailer, groups, components, messages) = parse_structure(xml, &fields_by_name)?;

    let component_members: BTreeMap<String, Vec<Member>> = components
        .iter()
        .map(|c| (c.name.clone(), c.members.iter().map(clone_member).collect()))
        .collect();

    let mut out = String::new();
    out.push_str(
        "# Generated by truefix-dict's QuickFIX-XML conversion tool (US9, feature 005,\n\
         # FR-031) — do not edit by hand; regenerate from the source XML instead.\n",
    );
    out.push_str(&format!("version {version}\n\n"));

    // Sort by tag for stable, readable output (BTreeMap iteration is already tag-ordered since
    // fields_by_name is keyed by name — re-key by tag for emission).
    let mut by_tag: BTreeMap<u32, (&str, &RawField)> = BTreeMap::new();
    for (name, f) in &fields_by_name {
        by_tag.insert(f.tag, (name.as_str(), f));
    }
    for (tag, (name, f)) in &by_tag {
        let type_token = map_type(&f.type_name).ok_or_else(|| QfjXmlError::UnknownType {
            name: (*name).to_owned(),
            type_name: f.type_name.clone(),
        })?;
        out.push_str(&format!("field {tag} {name} {type_token}"));
        for (value, label) in &f.values {
            if label.is_empty() {
                out.push_str(&format!(" {value}"));
            } else {
                out.push_str(&format!(" {value}={label}"));
            }
        }
        out.push('\n');
    }
    out.push('\n');

    let header_tags = flatten_members(&header, &component_members);
    let trailer_tags = flatten_members(&trailer, &component_members);
    if !header_tags.is_empty() {
        let tags: Vec<String> = header_tags.iter().map(u32::to_string).collect();
        out.push_str(&format!("header {}\n", tags.join(" ")));
    }
    if !trailer_tags.is_empty() {
        let tags: Vec<String> = trailer_tags.iter().map(u32::to_string).collect();
        out.push_str(&format!("trailer {}\n", tags.join(" ")));
    }
    if !header_tags.is_empty() || !trailer_tags.is_empty() {
        out.push('\n');
    }

    for g in &groups {
        let name = by_tag
            .get(&g.count_tag)
            .map(|(n, _)| *n)
            .unwrap_or("Unknown");
        let members: Vec<String> = g.members.iter().map(member_token).collect();
        out.push_str(&format!(
            "group {} {} {} {}\n",
            g.count_tag,
            name,
            g.delimiter,
            members.join(",")
        ));
    }
    if !groups.is_empty() {
        out.push('\n');
    }

    for c in &components {
        let tokens: Vec<String> = c.members.iter().map(member_token).collect();
        out.push_str(&format!("component {} {}\n", c.name, tokens.join(",")));
    }
    if !components.is_empty() {
        out.push('\n');
    }

    for m in &messages {
        let req: Vec<String> = m.required.iter().map(member_token).collect();
        let opt: Vec<String> = m.optional.iter().map(member_token).collect();
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

fn clone_member(m: &Member) -> Member {
    match m {
        Member::Field(t) => Member::Field(*t),
        Member::Component(n) => Member::Component(n.clone()),
    }
}

fn member_token(m: &Member) -> String {
    match m {
        Member::Field(t) => t.to_string(),
        Member::Component(n) => format!("component:{n}"),
    }
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

fn attr(e: &quick_xml::events::BytesStart<'_>, key: &str) -> Result<Option<String>, QfjXmlError> {
    for a in e.attributes() {
        let a = a.map_err(|err| QfjXmlError::Xml {
            pos: 0,
            reason: err.to_string(),
        })?;
        if local_name_bytes(a.key.as_ref()) == key {
            let value = a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|err| QfjXmlError::Xml {
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
) -> Result<u32, QfjXmlError> {
    let value = attr(e, key)?.ok_or(QfjXmlError::MissingAttr { elem, attr: key })?;
    value
        .parse()
        .map_err(|_| QfjXmlError::MissingAttr { elem, attr: key })
}
