//! Vendor-published FIX dictionary XML → normalized `.fixdict` conversion (US3, feature 011,
//! FR-011 through FR-017).
//!
//! Build/tooling-time only (feature `dict-tooling`, gated on `quick-xml`; never compiled into the
//! runtime engine's default dependency graph — matches [`crate::orchestra`]'s own scoping). Parses
//! the nested XML shape a FIX counterparty commonly publishes its own dictionary in (e.g.
//! Binance's `spot-fix-oe.xml`/`spot-fix-md.xml`, independently licensed by Binance):
//!
//! ```text
//! <fix type="FIX" major="4" minor="4" [servicepack="N"]>
//!   <header><field name="..." required="Y|N"/>...</header>
//!   <trailer><field name="..." required="Y|N"/>...</trailer>
//!   <messages>
//!     <message name="..." msgtype="..." msgcat="...">
//!       <field name="..." required="Y|N"/>
//!       <component name="..." required="Y|N"/>
//!       <group name="..." required="Y|N">...(nested field/component/group)...</group>
//!     </message>
//!   </messages>
//!   <components>
//!     <component name="...">...(field/component/group)...</component>
//!   </components>
//!   <fields>
//!     <field number="N" name="..." type="...">
//!       <value enum="..." description=".../>
//!     </field>
//!   </fields>
//! </fix>
//! ```
//!
//! This module reproduces only the *published XML shape* (a common, independently-documented
//! dictionary format shared by several FIX counterparties) plus the public FIX protocol
//! specification; it does not read, translate, or embed any content from `thrdpty/quickfix`'s or
//! `thrdpty/quickfixj`'s own bundled `spec/FIX*.xml` files — the private data files this project
//! already removed a converter for (`qfj_xml`, superseded by [`crate::fix_repository`]). See
//! `specs/011-body-repeating-groups/contracts/vendor-xml-dictionary.md` for the full provenance
//! rationale (Constitution Principle III).
//!
//! Like [`crate::orchestra::convert`]/[`crate::fix_repository::convert`], this converter emits the
//! normalized `.fixdict` text grammar ([`crate::parse`]) rather than building a [`crate::DataDictionary`]
//! directly — one rendering path, shared by every source format.
//!
//! Unlike the vendor dialect's `<message>`/`<group>`/`<component>` member references (which name a
//! field/component by `name=`, resolved against the separate `<fields>` section), `.fixdict`'s own
//! grammar addresses members by tag number — this module's main job is that name-to-number
//! resolution, plus flattening each inline `<group>` into its own standalone `.fixdict` `group`
//! directive (referenced from its parent's member list by tag number, exactly like a plain field).
//!
//! Header/trailer support only flat `<field name=.../>` children, matching every real
//! dictionary this shape is used for — `.fixdict`'s own `header`/`trailer` directives are plain tag
//! lists with no group/component support either ([`crate::parser::parse`]).

use std::collections::{BTreeMap, BTreeSet};

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::model::FieldType;

/// An error converting vendor dictionary XML to the normalized `.fixdict` grammar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VendorXmlError {
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
    /// The root `<fix>` element is missing recognizable version attributes (`major`/`minor`), so
    /// no `version` directive can be synthesized (FR-016; research.md R6 — never guessed/omitted).
    #[error("the root <fix> element is missing recognizable version attributes (major/minor)")]
    MissingVersion,
    /// An attribute expected to be numeric did not parse as one.
    #[error("<{elem}> attribute {attr:?} is not a number: {value:?}")]
    NotANumber {
        /// The element name.
        elem: &'static str,
        /// The attribute name.
        attr: &'static str,
        /// The offending value.
        value: String,
    },
    /// A `<field type=...>` value did not match a known normalized [`FieldType`] token.
    #[error("field {name:?} has unknown type {type_name:?}")]
    UnknownType {
        /// The field's name.
        name: String,
        /// The offending type name.
        type_name: String,
    },
    /// A `<field name=.../>`/`<component name=.../>`/`<group name=.../>` reference named a field
    /// or component that was never defined in `<fields>`/`<components>`.
    #[error("unknown {kind} reference {name:?}")]
    UnknownReference {
        /// `"field"` or `"component"`.
        kind: &'static str,
        /// The referenced (missing) name.
        name: String,
    },
}

/// One raw `<field number= name= type=>` definition from the `<fields>` section, before name
/// resolution.
struct RawFieldXml {
    number: u32,
    name: String,
    type_name: String,
    values: Vec<(String, String)>, // (enum value, description)
}

/// One member reference inside a `<message>`/`<component>`/`<group>`, before name resolution.
enum MemberKind {
    Field(String),
    Component(String),
    Group(RawGroup),
}

struct MemberEntry {
    kind: MemberKind,
    required: bool,
}

/// An inline `<group name=... required=...>` definition — `name` is the group's own count-field
/// name (e.g. `"NoPartyIDs"`), resolved to a tag number (and thus a `.fixdict` `group` directive)
/// once every `<fields>` definition is known.
struct RawGroup {
    name: String,
    members: Vec<MemberEntry>,
}

/// An in-progress nested context on the member-reference stack (a `<message>`, a `<component>`
/// definition, or a `<group>`) — these can nest arbitrarily (a group inside a group inside a
/// message), so a stack (rather than a handful of `Option<...>` fields) is required.
enum Frame {
    Message {
        msg_type: String,
        name: String,
        members: Vec<MemberEntry>,
    },
    ComponentDef {
        name: String,
        members: Vec<MemberEntry>,
    },
    Group {
        name: String,
        required: bool,
        members: Vec<MemberEntry>,
    },
}

/// In-progress `<field number= name= type=>` (in the `<fields>` section): `(number, name,
/// type_name, enum values)`, finalized into a [`RawFieldXml`] once its matching `End` is seen.
type FieldInProgress = (u32, String, String, Vec<(String, String)>);

#[derive(Default)]
struct Parser {
    fix_type: String,
    major: Option<u8>,
    minor: Option<u8>,
    servicepack: Option<u8>,

    header: Vec<String>,
    trailer: Vec<String>,
    in_header: bool,
    in_trailer: bool,

    raw_fields: Vec<RawFieldXml>,
    in_fields_section: bool,
    current_field: Option<FieldInProgress>,

    raw_components: Vec<(String, Vec<MemberEntry>)>,
    raw_messages: Vec<(String, String, Vec<MemberEntry>)>,

    stack: Vec<Frame>,
}

impl Parser {
    fn handle_start_or_empty(
        &mut self,
        e: &quick_xml::events::BytesStart<'_>,
        is_empty: bool,
    ) -> Result<(), VendorXmlError> {
        match local_name(e).as_str() {
            "fix" => {
                self.fix_type = attr(e, "type")?.unwrap_or_default();
                self.major = attr_opt_num(e, "fix", "major")?;
                self.minor = attr_opt_num(e, "fix", "minor")?;
                self.servicepack = attr_opt_num(e, "fix", "servicepack")?;
            }
            "header" => self.in_header = true,
            "trailer" => self.in_trailer = true,
            "fields" => self.in_fields_section = true,
            "messages" | "components" => {}
            "message" => {
                let msg_type = attr(e, "msgtype")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "message",
                    attr: "msgtype",
                })?;
                let name = attr(e, "name")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "message",
                    attr: "name",
                })?;
                self.stack.push(Frame::Message {
                    msg_type,
                    name,
                    members: Vec::new(),
                });
            }
            "component" if self.in_fields_section => {
                // Never reached in practice (components don't live in <fields>), but keeps the
                // match exhaustive/defensive rather than silently misclassifying.
            }
            "component" if !is_empty => {
                // A `<component name=...>...</component>` with children is a *definition* — only
                // meaningful directly under `<components>`, which is why definitions are always
                // finalized into `raw_components` on `End` regardless of stack depth (see
                // `handle_end`), rather than requiring a dedicated "am I under <components>" flag.
                let name = attr(e, "name")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "component",
                    attr: "name",
                })?;
                self.stack.push(Frame::ComponentDef {
                    name,
                    members: Vec::new(),
                });
            }
            "component" => {
                // Self-closing `<component name=.../>` at a message/group/component-def site is a
                // *reference*.
                let name = attr(e, "name")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "component",
                    attr: "name",
                })?;
                let required = attr(e, "required")?.as_deref() == Some("Y");
                self.push_member(MemberEntry {
                    kind: MemberKind::Component(name),
                    required,
                });
            }
            "group" => {
                let name = attr(e, "name")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "group",
                    attr: "name",
                })?;
                let required = attr(e, "required")?.as_deref() == Some("Y");
                if is_empty {
                    // A group with zero declared members.
                    self.push_member(MemberEntry {
                        kind: MemberKind::Group(RawGroup {
                            name,
                            members: Vec::new(),
                        }),
                        required,
                    });
                } else {
                    self.stack.push(Frame::Group {
                        name,
                        required,
                        members: Vec::new(),
                    });
                }
            }
            "field" if self.in_fields_section => {
                let number = attr_num(e, "field", "number")?;
                let name = attr(e, "name")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "field",
                    attr: "name",
                })?;
                let type_name = attr(e, "type")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "field",
                    attr: "type",
                })?;
                if is_empty {
                    self.raw_fields.push(RawFieldXml {
                        number,
                        name,
                        type_name,
                        values: Vec::new(),
                    });
                } else {
                    self.current_field = Some((number, name, type_name, Vec::new()));
                }
            }
            "field" if self.in_header || self.in_trailer => {
                let name = attr(e, "name")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "field",
                    attr: "name",
                })?;
                if self.in_header {
                    self.header.push(name);
                } else {
                    self.trailer.push(name);
                }
            }
            "field" => {
                let name = attr(e, "name")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "field",
                    attr: "name",
                })?;
                let required = attr(e, "required")?.as_deref() == Some("Y");
                self.push_member(MemberEntry {
                    kind: MemberKind::Field(name),
                    required,
                });
            }
            "value" if self.current_field.is_some() => {
                let value = attr(e, "enum")?.ok_or(VendorXmlError::MissingAttr {
                    elem: "value",
                    attr: "enum",
                })?;
                let description = attr(e, "description")?.unwrap_or_default();
                if let Some((_, _, _, values)) = self.current_field.as_mut() {
                    values.push((value, description));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Push a resolved member reference onto whichever frame is topmost — the enclosing
    /// `<message>`, `<component>` definition, or `<group>` (a top-level reference outside any of
    /// those, e.g. malformed input, is defensively dropped rather than panicking).
    fn push_member(&mut self, entry: MemberEntry) {
        match self.stack.last_mut() {
            Some(Frame::Message { members, .. })
            | Some(Frame::ComponentDef { members, .. })
            | Some(Frame::Group { members, .. }) => members.push(entry),
            None => {}
        }
    }

    fn handle_end(&mut self, local: &str) {
        match local {
            "header" => self.in_header = false,
            "trailer" => self.in_trailer = false,
            "fields" => self.in_fields_section = false,
            "field" => {
                if let Some((number, name, type_name, values)) = self.current_field.take() {
                    self.raw_fields.push(RawFieldXml {
                        number,
                        name,
                        type_name,
                        values,
                    });
                }
            }
            "message" => {
                if let Some(Frame::Message {
                    msg_type,
                    name,
                    members,
                }) = self.stack.pop()
                {
                    self.raw_messages.push((msg_type, name, members));
                }
            }
            "component" => {
                if matches!(self.stack.last(), Some(Frame::ComponentDef { .. }))
                    && let Some(Frame::ComponentDef { name, members }) = self.stack.pop()
                {
                    self.raw_components.push((name, members));
                }
            }
            "group" => {
                if matches!(self.stack.last(), Some(Frame::Group { .. }))
                    && let Some(Frame::Group {
                        name,
                        required,
                        members,
                    }) = self.stack.pop()
                {
                    self.push_member(MemberEntry {
                        kind: MemberKind::Group(RawGroup { name, members }),
                        required,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Convert vendor dictionary XML into the normalized `.fixdict` text grammar.
pub fn convert(xml: &str) -> Result<String, VendorXmlError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut parser = Parser::default();
    loop {
        let event = reader.read_event().map_err(|e| VendorXmlError::Xml {
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

    render(parser)
}

fn render(parser: Parser) -> Result<String, VendorXmlError> {
    let major = parser.major.ok_or(VendorXmlError::MissingVersion)?;
    let minor = parser.minor.ok_or(VendorXmlError::MissingVersion)?;
    // `servicepack="0"` is the (unsuffixed) baseline, matching FIX's own SPn convention — only a
    // nonzero service pack changes the emitted `.fixdict` version directive.
    let version = match parser.servicepack {
        Some(sp) if sp > 0 => format!("FIX.{major}.{minor}SP{sp}"),
        _ => format!("FIX.{major}.{minor}"),
    };

    let field_by_name: BTreeMap<String, u32> = parser
        .raw_fields
        .iter()
        .map(|f| (f.name.clone(), f.number))
        .collect();
    let component_names: BTreeSet<String> = parser
        .raw_components
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    let components_by_name: BTreeMap<&str, &Vec<MemberEntry>> = parser
        .raw_components
        .iter()
        .map(|(name, members)| (name.as_str(), members))
        .collect();

    let mut out = String::new();
    out.push_str(
        "# Generated by truefix-dict's vendor-XML conversion tool (US3, feature 011) — do not\n",
    );
    out.push_str("# edit by hand; regenerate from the vendor XML source file instead.\n");
    out.push_str(&format!("version {version}\n\n"));

    for f in &parser.raw_fields {
        let type_token = FieldType::parse(&f.type_name)
            .map(|_| f.type_name.as_str())
            .ok_or_else(|| VendorXmlError::UnknownType {
                name: f.name.clone(),
                type_name: f.type_name.clone(),
            })?;
        out.push_str(&format!("field {} {} {}", f.number, f.name, type_token));
        for (value, description) in &f.values {
            if description.is_empty() {
                out.push_str(&format!(" {value}"));
            } else {
                out.push_str(&format!(" {value}={description}"));
            }
        }
        out.push('\n');
    }
    out.push('\n');

    let header_tags = resolve_field_names(&parser.header, &field_by_name)?;
    let trailer_tags = resolve_field_names(&parser.trailer, &field_by_name)?;
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

    // Nested `<group>` definitions are flattened into standalone `.fixdict` `group` lines here,
    // emitted once per distinct count tag (first occurrence wins if the same group name is
    // referenced with differing member lists across messages — the same non-strict-duplicate
    // tolerance `crate::parser::parse` itself already has for `group` directives).
    let mut groups_out: Vec<String> = Vec::new();
    let mut emitted_groups: BTreeSet<u32> = BTreeSet::new();

    let mut components_out: Vec<String> = Vec::new();
    for (name, members) in &parser.raw_components {
        let tokens = resolve_members(
            members,
            &field_by_name,
            &component_names,
            &components_by_name,
            &mut groups_out,
            &mut emitted_groups,
        )?;
        components_out.push(format!("component {} {}\n", name, tokens.join(",")));
    }

    let mut messages_out: Vec<String> = Vec::new();
    for (msg_type, name, members) in &parser.raw_messages {
        let required: Vec<&MemberEntry> = members.iter().filter(|m| m.required).collect();
        let optional: Vec<&MemberEntry> = members.iter().filter(|m| !m.required).collect();
        let req_entries: Vec<MemberEntry> = required
            .into_iter()
            .map(|m| MemberEntry {
                kind: clone_kind(&m.kind),
                required: true,
            })
            .collect();
        let opt_entries: Vec<MemberEntry> = optional
            .into_iter()
            .map(|m| MemberEntry {
                kind: clone_kind(&m.kind),
                required: false,
            })
            .collect();
        let req_tokens = resolve_members(
            &req_entries,
            &field_by_name,
            &component_names,
            &components_by_name,
            &mut groups_out,
            &mut emitted_groups,
        )?;
        let opt_tokens = resolve_members(
            &opt_entries,
            &field_by_name,
            &component_names,
            &components_by_name,
            &mut groups_out,
            &mut emitted_groups,
        )?;
        let mut line = format!("message {msg_type} {name}");
        if !req_tokens.is_empty() {
            line.push_str(&format!(" req:{}", req_tokens.join(",")));
        }
        if !opt_tokens.is_empty() {
            line.push_str(&format!(" opt:{}", opt_tokens.join(",")));
        }
        line.push('\n');
        messages_out.push(line);
    }

    if !groups_out.is_empty() {
        for line in &groups_out {
            out.push_str(line);
        }
        out.push('\n');
    }
    if !components_out.is_empty() {
        for line in &components_out {
            out.push_str(line);
        }
        out.push('\n');
    }
    for line in &messages_out {
        out.push_str(line);
    }

    Ok(out)
}

/// Deep-clone a [`MemberKind`] (no `Clone` derive on purpose — `Group` owns a `Vec<MemberEntry>`
/// that would otherwise make an accidental shallow-copy-and-mutate bug easy; this helper makes the
/// one legitimate need, splitting a member list into a required/optional partition without
/// consuming the original, explicit).
fn clone_kind(kind: &MemberKind) -> MemberKind {
    match kind {
        MemberKind::Field(name) => MemberKind::Field(name.clone()),
        MemberKind::Component(name) => MemberKind::Component(name.clone()),
        MemberKind::Group(g) => MemberKind::Group(RawGroup {
            name: g.name.clone(),
            members: g
                .members
                .iter()
                .map(|m| MemberEntry {
                    kind: clone_kind(&m.kind),
                    required: m.required,
                })
                .collect(),
        }),
    }
}

fn resolve_field_names(
    names: &[String],
    field_by_name: &BTreeMap<String, u32>,
) -> Result<Vec<u32>, VendorXmlError> {
    names
        .iter()
        .map(|name| {
            field_by_name
                .get(name)
                .copied()
                .ok_or_else(|| VendorXmlError::UnknownReference {
                    kind: "field",
                    name: name.clone(),
                })
        })
        .collect()
}

/// Resolve an ordered member list into `.fixdict` member-list tokens (bare tag numbers or
/// `component:Name` references), flattening any inline `<group>` entries into standalone `group`
/// lines appended to `groups_out` (each emitted at most once, tracked via `emitted_groups`).
fn resolve_members(
    entries: &[MemberEntry],
    field_by_name: &BTreeMap<String, u32>,
    component_names: &BTreeSet<String>,
    components_by_name: &BTreeMap<&str, &Vec<MemberEntry>>,
    groups_out: &mut Vec<String>,
    emitted_groups: &mut BTreeSet<u32>,
) -> Result<Vec<String>, VendorXmlError> {
    let mut tokens = Vec::with_capacity(entries.len());
    for entry in entries {
        let token = match &entry.kind {
            MemberKind::Field(name) => {
                let tag =
                    field_by_name
                        .get(name)
                        .ok_or_else(|| VendorXmlError::UnknownReference {
                            kind: "field",
                            name: name.clone(),
                        })?;
                tag.to_string()
            }
            MemberKind::Component(name) => {
                if !component_names.contains(name) {
                    return Err(VendorXmlError::UnknownReference {
                        kind: "component",
                        name: name.clone(),
                    });
                }
                format!("component:{name}")
            }
            MemberKind::Group(group) => {
                let count_tag = field_by_name.get(&group.name).copied().ok_or_else(|| {
                    VendorXmlError::UnknownReference {
                        kind: "field",
                        name: group.name.clone(),
                    }
                })?;
                if emitted_groups.insert(count_tag) {
                    let member_tokens = resolve_members(
                        &group.members,
                        field_by_name,
                        component_names,
                        components_by_name,
                        groups_out,
                        emitted_groups,
                    )?;
                    let required_entries: Vec<MemberEntry> = group
                        .members
                        .iter()
                        .filter(|m| m.required)
                        .map(|m| MemberEntry {
                            kind: clone_kind(&m.kind),
                            required: true,
                        })
                        .collect();
                    let required_tokens = resolve_members(
                        &required_entries,
                        field_by_name,
                        component_names,
                        components_by_name,
                        groups_out,
                        emitted_groups,
                    )?;
                    // The delimiter is whichever tag is emitted *first* on the wire for this
                    // group's entries — not necessarily `member_tokens.first()` verbatim, since a
                    // `<component>` reference (unlike a plain field or a nested group, both of
                    // which already resolve to a bare tag number) renders as a `component:Name`
                    // token here. Recursing into that component's own first member (arbitrarily
                    // deep, e.g. a component whose first member is itself another component) finds
                    // the real first field tag; falling back to `count_tag` (the group's own count
                    // tag, which by construction never repeats inside an entry) would silently
                    // produce a group definition that matches zero entries on decode.
                    let delimiter =
                        first_member_tag(&group.members, field_by_name, components_by_name)?
                            .unwrap_or(count_tag);
                    let mut line = format!(
                        "group {} {} {} {}",
                        count_tag,
                        group.name,
                        delimiter,
                        member_tokens.join(",")
                    );
                    if !required_tokens.is_empty() {
                        line.push_str(&format!(" req:{}", required_tokens.join(",")));
                    }
                    line.push('\n');
                    groups_out.push(line);
                }
                count_tag.to_string()
            }
        };
        tokens.push(token);
    }
    Ok(tokens)
}

/// The tag of the first field this member list would emit on the wire, resolving through
/// `<component>` references recursively (a component's first member may itself be a component). A
/// nested `<group>` as the first member resolves to its own count tag, matching the token
/// `resolve_members` itself emits for a `Group` entry. `Ok(None)` only for an empty member list
/// (a degenerate, otherwise-unreachable dictionary shape).
fn first_member_tag(
    members: &[MemberEntry],
    field_by_name: &BTreeMap<String, u32>,
    components_by_name: &BTreeMap<&str, &Vec<MemberEntry>>,
) -> Result<Option<u32>, VendorXmlError> {
    let Some(first) = members.first() else {
        return Ok(None);
    };
    match &first.kind {
        MemberKind::Field(name) => field_by_name.get(name).copied().map(Some).ok_or_else(|| {
            VendorXmlError::UnknownReference {
                kind: "field",
                name: name.clone(),
            }
        }),
        MemberKind::Group(group) => field_by_name
            .get(&group.name)
            .copied()
            .map(Some)
            .ok_or_else(|| VendorXmlError::UnknownReference {
                kind: "field",
                name: group.name.clone(),
            }),
        MemberKind::Component(name) => {
            let inner = components_by_name.get(name.as_str()).ok_or_else(|| {
                VendorXmlError::UnknownReference {
                    kind: "component",
                    name: name.clone(),
                }
            })?;
            first_member_tag(inner, field_by_name, components_by_name)
        }
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

fn attr(
    e: &quick_xml::events::BytesStart<'_>,
    key: &str,
) -> Result<Option<String>, VendorXmlError> {
    for a in e.attributes() {
        let a = a.map_err(|err| VendorXmlError::Xml {
            pos: 0,
            reason: err.to_string(),
        })?;
        if local_name_bytes(a.key.as_ref()) == key {
            let value = a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|err| VendorXmlError::Xml {
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
) -> Result<u32, VendorXmlError> {
    let value = attr(e, key)?.ok_or(VendorXmlError::MissingAttr { elem, attr: key })?;
    value.parse().map_err(|_| VendorXmlError::NotANumber {
        elem,
        attr: key,
        value,
    })
}

fn attr_opt_num(
    e: &quick_xml::events::BytesStart<'_>,
    elem: &'static str,
    key: &'static str,
) -> Result<Option<u8>, VendorXmlError> {
    match attr(e, key)? {
        Some(value) if !value.is_empty() => {
            value
                .parse()
                .map(Some)
                .map_err(|_| VendorXmlError::NotANumber {
                    elem,
                    attr: key,
                    value,
                })
        }
        _ => Ok(None),
    }
}
