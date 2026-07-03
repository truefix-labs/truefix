//! FIX Trading Community "Unified Repository" XML → normalized `.fixdict` conversion (US9, feature
//! 005, FR-031 — corrected).
//!
//! Build/tooling-time only (feature `dict-tooling`, gated on `quick-xml`; never compiled into the
//! runtime engine's default dependency graph — matches [`crate::orchestra`]'s own scoping).
//!
//! **Replaces the earlier `qfj_xml` module**, which mechanically converted `thrdpty/quickfix`'s
//! bundled `spec/FIX*.xml` files — QuickFIX's own *private data files*, distributed under the
//! restrictive "QuickFIX Software License" — directly into shipped `.fixdict` content. That is
//! exactly what Constitution Principle III forbids ("MUST NOT ... 复制其源代码、注释或私有数据文件").
//! This module instead converts the **official** FIX Trading Community "Unified Repository 2010
//! Edition" download (Apache-2.0-licensed; see `dict-src/fix-repository/PROVENANCE.md`), which
//! ships one already-per-version-filtered directory tree (`Fields.xml`/`Enums.xml`/
//! `Components.xml`/`Messages.xml`/`MsgContents.xml`) for each of `FIX.4.0` through `FIX.5.0SP2`
//! and `FIXT.1.1` — no additional version-filtering by the caller is required.
//!
//! Schema notes (a relational join, not a nested-XML tree like Orchestra or QuickFIX's own DTD
//! schema):
//! - `Fields.xml`: `<Field><Tag/><Name/><Type/>...</Field>` — the field catalog, keyed by `Tag`.
//! - `Enums.xml`: `<Enum><Tag/><Value/><SymbolicName/>...</Enum>` — per-tag enumerated values with
//!   a human-readable label (`SymbolicName`), feeding FR-030's value-label lookup.
//! - `Components.xml`: `<Component><ComponentID/><ComponentType/><Name/>...</Component>` — every
//!   message body, plain component, header, trailer, and repeating group is modeled uniformly as a
//!   "component" here, distinguished only by `ComponentType` (`Block`/`ImplicitBlock` = a plain
//!   component; `BlockRepeating`/`ImplicitBlockRepeating` = a repeating group). `StandardHeader`/
//!   `StandardTrailer` are themselves ordinary (non-repeating) components.
//! - `Messages.xml`: `<Message><ComponentID/><MsgType/><Name/>...</Message>` — a message's own
//!   `ComponentID` is the join key into `MsgContents.xml` for its body, exactly like a component's.
//! - `MsgContents.xml`: `<MsgContent><ComponentID/><TagText/><Indent/><Position/><Reqd/>...
//!   </MsgContent>` — the structure table: for a given `ComponentID`, the ordered (`Position`) list
//!   of members, each either a numeric field tag or another component's `Name` (resolved
//!   recursively). For a *repeating* component, exactly one entry sits at `Indent=0` (the count
//!   field) and the rest at `Indent=1` are the group's per-entry members — confirmed empirically
//!   (e.g. `Parties`, ComponentID 1012: position 1/indent 0/tag 453 = `NoPartyIDs`, positions 2-5/
//!   indent 1 = the group's own members). For a non-repeating component or a message body, all
//!   entries are direct members in `Position` order.

use std::collections::BTreeMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// An error converting FIX Repository XML to the normalized `.fixdict` grammar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FixRepositoryError {
    /// The XML itself was malformed.
    #[error("XML parse error in {file} at byte {pos}: {reason}")]
    Xml {
        /// Which of the 5 input files.
        file: &'static str,
        /// Byte offset of the error.
        pos: usize,
        /// The underlying reason.
        reason: String,
    },
    /// A required element/attribute was missing.
    #[error("{file}: <{elem}> is missing required child/attribute {what:?}")]
    Missing {
        /// Which of the 5 input files.
        file: &'static str,
        /// The element name.
        elem: &'static str,
        /// The missing piece.
        what: &'static str,
    },
    /// A `Field`'s `Type` did not map to a known normalized [`crate::FieldType`] token.
    #[error("field {name:?} (tag {tag}) has unknown Repository type {type_name:?}")]
    UnknownType {
        /// The field's tag.
        tag: u32,
        /// The field's name.
        name: String,
        /// The offending type name.
        type_name: String,
    },
    /// A `MsgContents` entry's `TagText` named a component never defined in `Components.xml`, and
    /// wasn't parseable as a plain numeric field tag either.
    #[error("unresolvable TagText {tag_text:?} (component id {component_id})")]
    UnresolvedTagText {
        /// The owning ComponentID.
        component_id: u32,
        /// The unresolved reference text.
        tag_text: String,
    },
    /// Component/group reference nesting exceeded the depth guard (16) — almost certainly a cyclic
    /// reference in malformed input, not a real FIX structure.
    #[error("component reference nesting exceeded depth 16 (component id {component_id})")]
    TooDeep {
        /// The ComponentID where the guard tripped.
        component_id: u32,
    },
    /// `flatten_members`' own nesting guard (T086/GAP-50/BUG-18, feature 006), tripped after
    /// `resolve_entries` has already turned each `Member::Component` into a name rather than an
    /// id — so this names the component by name, not id, unlike [`Self::TooDeep`] above.
    #[error("member flattening nesting exceeded depth 16 (component name {component_name:?})")]
    FlattenTooDeep {
        /// The component name where the guard tripped.
        component_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Member {
    Field(u32),
    Component(String),
}

struct RawField {
    name: String,
    type_name: String,
}

struct RawComponent {
    name: String,
    repeating: bool,
}

struct MsgContentEntry {
    tag_text: String,
    indent: u32,
    /// `Position` is usually a plain integer, but the vendored data also uses fractional values
    /// (e.g. `1.1`, `1.2`) to insert a member between two integer positions without renumbering
    /// the rest — `f64` preserves that relative order; the value itself is never emitted.
    position: f64,
    required: bool,
}

struct RawGroup {
    count_tag: u32,
    name: String,
    delimiter: u32,
    members: Vec<Member>,
}

struct RawComponentBody {
    name: String,
    members: Vec<Member>,
}

struct RawMessage {
    msg_type: String,
    name: String,
    required: Vec<Member>,
    optional: Vec<Member>,
}

/// The 5 input documents for one FIX version, already read into memory by the caller (typically
/// from `dict-src/fix-repository/<Version>/*.xml`).
pub struct RepositorySource<'a> {
    /// `Fields.xml` contents.
    pub fields_xml: &'a str,
    /// `Enums.xml` contents.
    pub enums_xml: &'a str,
    /// `Components.xml` contents.
    pub components_xml: &'a str,
    /// `Messages.xml` contents.
    pub messages_xml: &'a str,
    /// `MsgContents.xml` contents.
    pub msg_contents_xml: &'a str,
}

fn parse_fields(xml: &str) -> Result<BTreeMap<u32, RawField>, FixRepositoryError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut fields = BTreeMap::new();
    let mut in_field = false;
    let (mut tag, mut name, mut type_name) = (None::<u32>, None::<String>, None::<String>);
    let mut current_tag_name = String::new();

    loop {
        let event = reader.read_event().map_err(|e| FixRepositoryError::Xml {
            file: "Fields.xml",
            pos: reader.buffer_position() as usize,
            reason: e.to_string(),
        })?;
        match event {
            Event::Eof => break,
            Event::Start(e) if local_name(&e) == "Field" => {
                in_field = true;
                tag = None;
                name = None;
                type_name = None;
            }
            Event::Start(e) if in_field => current_tag_name = local_name(&e),
            Event::Text(t) if in_field => {
                let text = t.decode().unwrap_or_default().into_owned();
                match current_tag_name.as_str() {
                    "Tag" => tag = text.trim().parse().ok(),
                    "Name" => name = Some(text),
                    "Type" => type_name = Some(text),
                    _ => {}
                }
            }
            Event::End(e) if local_name_bytes(e.name().as_ref()) == "Field" => {
                in_field = false;
                if let (Some(t), Some(n), Some(ty)) = (tag, name.take(), type_name.take()) {
                    fields.insert(
                        t,
                        RawField {
                            name: n,
                            type_name: ty,
                        },
                    );
                }
            }
            _ => {}
        }
    }
    Ok(fields)
}

fn parse_enums(xml: &str) -> Result<BTreeMap<(u32, String), String>, FixRepositoryError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut enums = BTreeMap::new();
    let mut in_enum = false;
    let (mut tag, mut value, mut symbolic) = (None::<u32>, None::<String>, None::<String>);
    let mut current_tag_name = String::new();

    loop {
        let event = reader.read_event().map_err(|e| FixRepositoryError::Xml {
            file: "Enums.xml",
            pos: reader.buffer_position() as usize,
            reason: e.to_string(),
        })?;
        match event {
            Event::Eof => break,
            Event::Start(e) if local_name(&e) == "Enum" => {
                in_enum = true;
                tag = None;
                value = None;
                symbolic = None;
            }
            Event::Start(e) if in_enum => current_tag_name = local_name(&e),
            Event::Text(t) if in_enum => {
                let text = t.decode().unwrap_or_default().into_owned();
                match current_tag_name.as_str() {
                    "Tag" => tag = text.trim().parse().ok(),
                    "Value" => value = Some(text),
                    "SymbolicName" => symbolic = Some(text),
                    _ => {}
                }
            }
            Event::End(e) if local_name_bytes(e.name().as_ref()) == "Enum" => {
                in_enum = false;
                if let (Some(t), Some(v)) = (tag, value.take()) {
                    enums.insert((t, v), symbolic.take().unwrap_or_default());
                }
            }
            _ => {}
        }
    }
    Ok(enums)
}

/// Components keyed by ID, plus a name-to-ID lookup.
type ParsedComponents = (BTreeMap<u32, RawComponent>, BTreeMap<String, u32>);

fn parse_components(xml: &str) -> Result<ParsedComponents, FixRepositoryError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut by_id = BTreeMap::new();
    let mut id_by_name = BTreeMap::new();
    let mut in_component = false;
    let (mut id, mut name, mut ctype) = (None::<u32>, None::<String>, None::<String>);
    let mut current_tag_name = String::new();

    loop {
        let event = reader.read_event().map_err(|e| FixRepositoryError::Xml {
            file: "Components.xml",
            pos: reader.buffer_position() as usize,
            reason: e.to_string(),
        })?;
        match event {
            Event::Eof => break,
            Event::Start(e) if local_name(&e) == "Component" => {
                in_component = true;
                id = None;
                name = None;
                ctype = None;
            }
            Event::Start(e) if in_component => current_tag_name = local_name(&e),
            Event::Text(t) if in_component => {
                let text = t.decode().unwrap_or_default().into_owned();
                match current_tag_name.as_str() {
                    "ComponentID" => id = text.trim().parse().ok(),
                    "Name" => name = Some(text),
                    "ComponentType" => ctype = Some(text),
                    _ => {}
                }
            }
            Event::End(e) if local_name_bytes(e.name().as_ref()) == "Component" => {
                in_component = false;
                if let (Some(i), Some(n), Some(t)) = (id, name.take(), ctype.take()) {
                    id_by_name.insert(n.clone(), i);
                    by_id.insert(
                        i,
                        RawComponent {
                            name: n,
                            repeating: t.contains("Repeating"),
                        },
                    );
                }
            }
            _ => {}
        }
    }
    Ok((by_id, id_by_name))
}

/// Parses `Messages.xml` into one [`RawMessageMeta`] per `<Message>` element (its `ComponentID`,
/// `MsgType`, and `Name`). Unlike [`parse_components`], this does **not** register anything into
/// `id_by_name`/`by_id` — those two catalogs are built exclusively from `Components.xml`
/// (T087/GAP-51, feature 006: this doc comment previously claimed a synthetic `id_by_name`/`by_id`
/// registration for each message's `ComponentID` that the implementation never performed).
/// `resolve_entries`' `&components_by_id[&ref_id]` index (which this might look like it protects)
/// is safe regardless: `ref_id` there is always obtained via `components_by_name.get(...)` first,
/// and `parse_components` only ever inserts into `id_by_name`/`by_id` together, in the same match
/// arm — so any name lookup that succeeds guarantees the paired id lookup succeeds too.
fn parse_messages(xml: &str) -> Result<Vec<RawMessageMeta>, FixRepositoryError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut messages = Vec::new();
    let mut in_message = false;
    let (mut id, mut msg_type, mut name) = (None::<u32>, None::<String>, None::<String>);
    let mut current_tag_name = String::new();

    loop {
        let event = reader.read_event().map_err(|e| FixRepositoryError::Xml {
            file: "Messages.xml",
            pos: reader.buffer_position() as usize,
            reason: e.to_string(),
        })?;
        match event {
            Event::Eof => break,
            Event::Start(e) if local_name(&e) == "Message" => {
                in_message = true;
                id = None;
                msg_type = None;
                name = None;
            }
            Event::Start(e) if in_message => current_tag_name = local_name(&e),
            Event::Text(t) if in_message => {
                let text = t.decode().unwrap_or_default().into_owned();
                match current_tag_name.as_str() {
                    "ComponentID" => id = text.trim().parse().ok(),
                    "MsgType" => msg_type = Some(text),
                    "Name" => name = Some(text),
                    _ => {}
                }
            }
            Event::End(e) if local_name_bytes(e.name().as_ref()) == "Message" => {
                in_message = false;
                if let (Some(i), Some(mt), Some(n)) = (id, msg_type.take(), name.take()) {
                    messages.push(RawMessageMeta {
                        component_id: i,
                        msg_type: mt,
                        name: n,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(messages)
}

struct RawMessageMeta {
    component_id: u32,
    msg_type: String,
    name: String,
}

fn parse_msg_contents(
    xml: &str,
) -> Result<BTreeMap<u32, Vec<MsgContentEntry>>, FixRepositoryError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut by_component: BTreeMap<u32, Vec<MsgContentEntry>> = BTreeMap::new();
    let mut in_entry = false;
    let (mut component_id, mut tag_text, mut indent, mut position, mut reqd) = (
        None::<u32>,
        None::<String>,
        None::<u32>,
        None::<f64>,
        None::<u32>,
    );
    let mut current_tag_name = String::new();

    loop {
        let event = reader.read_event().map_err(|e| FixRepositoryError::Xml {
            file: "MsgContents.xml",
            pos: reader.buffer_position() as usize,
            reason: e.to_string(),
        })?;
        match event {
            Event::Eof => break,
            Event::Start(e) if local_name(&e) == "MsgContent" => {
                in_entry = true;
                component_id = None;
                tag_text = None;
                indent = None;
                position = None;
                reqd = None;
            }
            Event::Start(e) if in_entry => current_tag_name = local_name(&e),
            Event::Text(t) if in_entry => {
                let text = t.decode().unwrap_or_default().into_owned();
                match current_tag_name.as_str() {
                    "ComponentID" => component_id = text.trim().parse().ok(),
                    "TagText" => tag_text = Some(text),
                    "Indent" => indent = text.trim().parse().ok(),
                    "Position" => position = text.trim().parse().ok(),
                    "Reqd" => reqd = text.trim().parse().ok(),
                    _ => {}
                }
            }
            Event::End(e) if local_name_bytes(e.name().as_ref()) == "MsgContent" => {
                in_entry = false;
                if let (Some(cid), Some(tt), Some(pos)) = (component_id, tag_text.take(), position)
                {
                    by_component.entry(cid).or_default().push(MsgContentEntry {
                        tag_text: tt,
                        indent: indent.unwrap_or(0),
                        position: pos,
                        required: reqd.unwrap_or(0) != 0,
                    });
                }
            }
            _ => {}
        }
    }
    for entries in by_component.values_mut() {
        entries.sort_by(|a, b| a.position.total_cmp(&b.position));
    }
    Ok(by_component)
}

/// Recursively resolve one `ComponentID`'s ordered `MsgContents` entries into the flat `Member`
/// list every group/component definition needs — resolving nested component/group references by
/// name along the way. `header_trailer_only` entries (`StandardHeader`/`StandardTrailer`) are
/// filtered out by the caller when building a message body, not here.
#[allow(clippy::too_many_arguments)]
fn resolve_entries(
    component_id: u32,
    msg_contents: &BTreeMap<u32, Vec<MsgContentEntry>>,
    fields_by_tag: &BTreeMap<u32, RawField>,
    components_by_id: &BTreeMap<u32, RawComponent>,
    components_by_name: &BTreeMap<String, u32>,
    depth: u32,
) -> Result<Vec<(Member, bool)>, FixRepositoryError> {
    if depth > 16 {
        return Err(FixRepositoryError::TooDeep { component_id });
    }
    let Some(entries) = msg_contents.get(&component_id) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries {
        if let Ok(tag) = entry.tag_text.parse::<u32>() {
            if fields_by_tag.contains_key(&tag) {
                out.push((Member::Field(tag), entry.required));
            }
            // A numeric `TagText` whose tag isn't in this version's `Fields.xml` names a field
            // that genuinely isn't part of this version's wire format (the vendored per-version
            // Repository slice retroactively lists some later-version fields against
            // earlier-version components — e.g. FIX.4.3's `CommissionData` component references
            // field 479, `CommCurrency`, which the Repository itself only defines starting
            // FIX.4.4). Silently omitting it here is correct, not a workaround: it matches what
            // that version's actual wire format supports.
            continue;
        }
        let Some(&ref_id) = components_by_name.get(&entry.tag_text) else {
            return Err(FixRepositoryError::UnresolvedTagText {
                component_id,
                tag_text: entry.tag_text.clone(),
            });
        };
        let referenced = &components_by_id[&ref_id];
        if referenced.repeating {
            // A nested/message-level reference to a repeating component contributes only its own
            // count field's tag number to the parent's member list (the parent never needs the
            // group's internal member list — that's resolved separately when the group itself is
            // emitted as a top-level `group` directive). `required` is respected here (unlike
            // component references below), matching QuickFIX-schema-derived precedent. A `None`
            // count tag means this version's data never actually defines the group's own body
            // (see `group_count_tag`) — such a reference is dropped, for the same "not really
            // part of this version" reason as the unresolved-numeric-tag case above.
            let Some(count_tag) = group_count_tag(ref_id, msg_contents, fields_by_tag) else {
                continue;
            };
            out.push((Member::Field(count_tag), entry.required));
        } else {
            // A plain component reference's own `Reqd` flag means "the component as a whole is
            // expected," not "every one of its constituent fields is individually required" —
            // same finding as the earlier QuickFIX-XML converter (verified there against FIX44.xml;
            // holds here too since it's the identical real-world FIX semantic, just a different
            // XML encoding of the same fact). TrueFix's flat per-tag required/optional model can't
            // express "at least one of N," so every component reference is optional regardless of
            // its own `Reqd` value.
            out.push((Member::Component(referenced.name.clone()), false));
        }
    }
    Ok(out)
}

/// The `NoXXX` count-field tag for a repeating component: its own `MsgContents` entries begin with
/// exactly one `Indent=0` entry naming that field. `None` means this version's vendored data never
/// actually defines the group's body (the `Components.xml` catalog entry exists — often inherited
/// from a later version's schema — but `MsgContents.xml` has no matching entries at all, e.g.
/// FIXT.1.1's `MsgTypeGrp`) — the caller drops the reference rather than treating that as an error.
fn group_count_tag(
    component_id: u32,
    msg_contents: &BTreeMap<u32, Vec<MsgContentEntry>>,
    fields_by_tag: &BTreeMap<u32, RawField>,
) -> Option<u32> {
    let entries = msg_contents
        .get(&component_id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    entries
        .iter()
        .find(|e| e.indent == 0)
        .and_then(|e| e.tag_text.parse::<u32>().ok())
        .filter(|t| fields_by_tag.contains_key(t))
}

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

/// Flatten `members` to a plain tag list, inlining nested components. `depth` starts at 0 from
/// the public call sites and increments on each `Member::Component` recursion; guarded at 16
/// (T086/GAP-50/BUG-18, feature 006 — matching sibling `resolve_entries`' own guard) to error
/// instead of recursing unboundedly on a cyclic component reference in malformed input.
fn flatten_members(
    members: &[Member],
    components: &BTreeMap<String, Vec<Member>>,
    depth: u32,
) -> Result<Vec<u32>, FixRepositoryError> {
    let mut out = Vec::new();
    for m in members {
        match m {
            Member::Field(t) => out.push(*t),
            Member::Component(name) => {
                if let Some(sub) = components.get(name) {
                    if depth > 16 {
                        return Err(FixRepositoryError::FlattenTooDeep {
                            component_name: name.clone(),
                        });
                    }
                    out.extend(flatten_members(sub, components, depth + 1)?);
                }
            }
        }
    }
    Ok(out)
}

fn member_token(m: &Member) -> String {
    match m {
        Member::Field(t) => t.to_string(),
        Member::Component(n) => format!("component:{n}"),
    }
}

/// Convert one FIX version's Unified Repository XML documents into the normalized `.fixdict` text
/// grammar. `version` is the `.fixdict` `version` directive's value (e.g. `"FIX.4.4"`).
pub fn convert(src: &RepositorySource<'_>, version: &str) -> Result<String, FixRepositoryError> {
    let fields_by_tag = parse_fields(src.fields_xml)?;
    let enums = parse_enums(src.enums_xml)?;
    let (components_by_id, components_by_name) = parse_components(src.components_xml)?;
    let messages_meta = parse_messages(src.messages_xml)?;
    let msg_contents = parse_msg_contents(src.msg_contents_xml)?;

    // Resolve every component/group's own member list once, keyed by name (used both for
    // top-level `component`/`group` directive emission and for header/trailer flattening).
    let mut groups: Vec<RawGroup> = Vec::new();
    let mut plain_components: Vec<RawComponentBody> = Vec::new();
    for (&id, comp) in &components_by_id {
        let resolved = resolve_entries(
            id,
            &msg_contents,
            &fields_by_tag,
            &components_by_id,
            &components_by_name,
            0,
        )?;
        if comp.repeating {
            // See `group_count_tag`'s doc comment: a repeating component this version's data
            // never actually defines a body for is simply not emitted as a `group` directive.
            let Some(count_tag) = group_count_tag(id, &msg_contents, &fields_by_tag) else {
                continue;
            };
            let members: Vec<Member> = resolved.into_iter().map(|(m, _)| m).collect();
            let delimiter = members
                .iter()
                .find_map(|m| match m {
                    Member::Field(t) => Some(*t),
                    Member::Component(_) => None,
                })
                .unwrap_or(count_tag);
            groups.push(RawGroup {
                count_tag,
                name: comp.name.clone(),
                delimiter,
                members,
            });
        } else {
            let members: Vec<Member> = resolved.into_iter().map(|(m, _)| m).collect();
            plain_components.push(RawComponentBody {
                name: comp.name.clone(),
                members,
            });
        }
    }
    groups.sort_by_key(|g| g.count_tag);
    plain_components.sort_by(|a, b| a.name.cmp(&b.name));

    let component_member_map: BTreeMap<String, Vec<Member>> = plain_components
        .iter()
        .map(|c| (c.name.clone(), c.members.clone()))
        .collect();

    let header_id =
        *components_by_name
            .get("StandardHeader")
            .ok_or(FixRepositoryError::Missing {
                file: "Components.xml",
                elem: "Component",
                what: "StandardHeader",
            })?;
    let trailer_id =
        *components_by_name
            .get("StandardTrailer")
            .ok_or(FixRepositoryError::Missing {
                file: "Components.xml",
                elem: "Component",
                what: "StandardTrailer",
            })?;
    let header_members: Vec<Member> = resolve_entries(
        header_id,
        &msg_contents,
        &fields_by_tag,
        &components_by_id,
        &components_by_name,
        0,
    )?
    .into_iter()
    .map(|(m, _)| m)
    .collect();
    let trailer_members: Vec<Member> = resolve_entries(
        trailer_id,
        &msg_contents,
        &fields_by_tag,
        &components_by_id,
        &components_by_name,
        0,
    )?
    .into_iter()
    .map(|(m, _)| m)
    .collect();
    let header_tags = flatten_members(&header_members, &component_member_map, 0)?;
    let trailer_tags = flatten_members(&trailer_members, &component_member_map, 0)?;

    let mut messages: Vec<RawMessage> = Vec::new();
    for meta in &messages_meta {
        let resolved = resolve_entries(
            meta.component_id,
            &msg_contents,
            &fields_by_tag,
            &components_by_id,
            &components_by_name,
            0,
        )?;
        let mut required = Vec::new();
        let mut optional = Vec::new();
        for (m, req) in resolved {
            if matches!(&m, Member::Component(n) if n == "StandardHeader" || n == "StandardTrailer")
            {
                continue;
            }
            if req {
                required.push(m);
            } else {
                optional.push(m);
            }
        }
        messages.push(RawMessage {
            msg_type: meta.msg_type.clone(),
            name: meta.name.clone(),
            required,
            optional,
        });
    }

    // Fixpoint-prune components that end up with zero members once the "not really part of this
    // version" omissions above are applied (e.g. FIX.4.3's `InstrumentLeg`, whose only members are
    // leg fields the Repository doesn't introduce until later versions) — the normalized `.fixdict`
    // grammar rejects an empty `component` directive, and such a component isn't part of this
    // version's wire format either, so every group/component/message that references it drops the
    // reference the same way (which can itself cascade into a *different* component going empty,
    // hence the fixpoint loop rather than a single pass).
    loop {
        let empty_names: std::collections::BTreeSet<String> = plain_components
            .iter()
            .filter(|c| c.members.is_empty())
            .map(|c| c.name.clone())
            .collect();
        if empty_names.is_empty() {
            break;
        }
        let drop_empty_refs = |members: &mut Vec<Member>| {
            members.retain(|m| !matches!(m, Member::Component(n) if empty_names.contains(n)));
        };
        for c in plain_components.iter_mut() {
            drop_empty_refs(&mut c.members);
        }
        for g in groups.iter_mut() {
            drop_empty_refs(&mut g.members);
        }
        for m in messages.iter_mut() {
            drop_empty_refs(&mut m.required);
            drop_empty_refs(&mut m.optional);
        }
        plain_components.retain(|c| !empty_names.contains(&c.name));
    }
    messages.sort_by(|a, b| a.name.cmp(&b.name));

    let mut out = String::new();
    out.push_str(
        "# Generated by truefix-dict's FIX Repository conversion tool (US9, feature 005,\n\
         # FR-031) — do not edit by hand; regenerate from the source XML instead.\n",
    );
    out.push_str(&format!("version {version}\n\n"));

    for (tag, f) in &fields_by_tag {
        let type_token = map_type(&f.type_name).ok_or_else(|| FixRepositoryError::UnknownType {
            tag: *tag,
            name: f.name.clone(),
            type_name: f.type_name.clone(),
        })?;
        out.push_str(&format!("field {tag} {} {type_token}", f.name));
        for ((etag, value), label) in &enums {
            if etag != tag {
                continue;
            }
            if label.is_empty() {
                out.push_str(&format!(" {value}"));
            } else {
                out.push_str(&format!(" {value}={label}"));
            }
        }
        out.push('\n');
    }
    out.push('\n');

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
        let members: Vec<String> = g.members.iter().map(member_token).collect();
        out.push_str(&format!(
            "group {} {} {} {}\n",
            g.count_tag,
            g.name,
            g.delimiter,
            members.join(",")
        ));
    }
    if !groups.is_empty() {
        out.push('\n');
    }

    for c in &plain_components {
        let tokens: Vec<String> = c.members.iter().map(member_token).collect();
        out.push_str(&format!("component {} {}\n", c.name, tokens.join(",")));
    }
    if !plain_components.is_empty() {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// T086 (US10, feature 006): `flatten_members` had no recursion-depth guard at all (unlike its
    /// sibling `resolve_entries`, which already bounds at depth 16) — a self-referencing component
    /// chain in malformed input would recurse unboundedly instead of failing cleanly.
    #[test]
    fn a_self_referencing_component_chain_fails_instead_of_recursing_unboundedly() {
        let mut components: BTreeMap<String, Vec<Member>> = BTreeMap::new();
        // "A" contains "A" -- a direct self-reference.
        components.insert("A".to_owned(), vec![Member::Component("A".to_owned())]);
        let members = vec![Member::Component("A".to_owned())];

        let result = flatten_members(&members, &components, 0);
        match result {
            Err(FixRepositoryError::FlattenTooDeep { component_name }) => {
                assert_eq!(component_name, "A");
            }
            other => panic!("expected FlattenTooDeep, got {other:?}"),
        }
    }

    #[test]
    fn a_normal_non_cyclic_component_chain_still_flattens_correctly() {
        let mut components: BTreeMap<String, Vec<Member>> = BTreeMap::new();
        components.insert("Inner".to_owned(), vec![Member::Field(2)]);
        components.insert(
            "Outer".to_owned(),
            vec![Member::Field(1), Member::Component("Inner".to_owned())],
        );
        let members = vec![Member::Component("Outer".to_owned())];

        let tags = flatten_members(&members, &components, 0).unwrap();
        assert_eq!(tags, vec![1, 2]);
    }
}
