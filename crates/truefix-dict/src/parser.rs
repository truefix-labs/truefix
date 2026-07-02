//! Parser for the normalized dictionary format.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::hash::fnv1a;
use crate::model::{ComponentDef, DataDictionary, FieldDef, FieldType, GroupDef, MessageDef};

/// An error parsing a normalized dictionary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    /// No `version` directive was found.
    #[error("missing `version` directive")]
    MissingVersion,
    /// A line could not be parsed.
    #[error("line {line}: {reason}")]
    Malformed {
        /// 1-based line number.
        line: usize,
        /// Reason.
        reason: &'static str,
    },
    /// An unknown field type token.
    #[error("line {line}: unknown field type {token:?}")]
    UnknownType {
        /// 1-based line number.
        line: usize,
        /// The offending token.
        token: String,
    },
    /// A `component:<Name>` token names a component that was never defined.
    #[error("unknown component {name:?}")]
    UnknownComponent {
        /// The referenced (missing) component name.
        name: String,
    },
    /// A component (directly or transitively) references itself.
    #[error("component {name:?} is part of a cycle")]
    ComponentCycle {
        /// The component name where the cycle was detected.
        name: String,
    },
}

/// One entry of a `message`/`group`/`component` member list before component references are
/// resolved into flat tag lists.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RawMember {
    /// A plain field tag (or, if it is itself a group's count_tag, a nested group).
    Tag(u32),
    /// A `component:<Name>` reference, resolved after the whole document is parsed.
    Component(String),
}

/// Parse a normalized dictionary document.
pub fn parse(input: &str) -> Result<DataDictionary, ParseError> {
    let mut version: Option<String> = None;
    let mut fields: BTreeMap<u32, FieldDef> = BTreeMap::new();
    let mut field_by_name: BTreeMap<String, u32> = BTreeMap::new();
    let mut header: BTreeSet<u32> = BTreeSet::new();
    let mut trailer: BTreeSet<u32> = BTreeSet::new();

    // Raw (pre-component-resolution) forms, keyed the same way the final maps will be.
    let mut messages_raw: BTreeMap<String, (String, Vec<RawMember>, Vec<RawMember>)> =
        BTreeMap::new();
    let mut groups_raw: BTreeMap<u32, (u32, Vec<RawMember>)> = BTreeMap::new();
    let mut components_raw: BTreeMap<String, Vec<RawMember>> = BTreeMap::new();

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let mut tokens = line.split_whitespace();
        let directive = tokens.next().unwrap_or("");
        match directive {
            "version" => {
                let v = tokens.next().ok_or(ParseError::Malformed {
                    line: line_no,
                    reason: "version requires a value",
                })?;
                version = Some(v.to_owned());
            }
            "field" => {
                let tag = parse_tag(tokens.next(), line_no)?;
                let name = tokens.next().ok_or(ParseError::Malformed {
                    line: line_no,
                    reason: "field requires a name",
                })?;
                let type_token = tokens.next().ok_or(ParseError::Malformed {
                    line: line_no,
                    reason: "field requires a type",
                })?;
                let field_type =
                    FieldType::parse(type_token).ok_or_else(|| ParseError::UnknownType {
                        line: line_no,
                        token: type_token.to_owned(),
                    })?;
                // Enum tokens may carry an optional `=Label` suffix for codegen's benefit (e.g.
                // `1=Buy`); runtime validation only compares the raw wire value, so it is stripped.
                let values: Vec<String> = tokens
                    .map(|t| t.split('=').next().unwrap_or(t).to_owned())
                    .collect();
                fields.insert(
                    tag,
                    FieldDef {
                        tag,
                        name: name.to_owned(),
                        field_type,
                        values,
                    },
                );
                field_by_name.insert(name.to_owned(), tag);
            }
            "header" => {
                for t in tokens {
                    header.insert(parse_tag(Some(t), line_no)?);
                }
            }
            "trailer" => {
                for t in tokens {
                    trailer.insert(parse_tag(Some(t), line_no)?);
                }
            }
            "message" => {
                let msg_type = tokens.next().ok_or(ParseError::Malformed {
                    line: line_no,
                    reason: "message requires a MsgType",
                })?;
                let name = tokens.next().ok_or(ParseError::Malformed {
                    line: line_no,
                    reason: "message requires a name",
                })?;
                let mut required = Vec::new();
                let mut optional = Vec::new();
                for token in tokens {
                    if let Some(list) = token.strip_prefix("req:") {
                        required = parse_member_list(list, line_no)?;
                    } else if let Some(list) = token.strip_prefix("opt:") {
                        optional = parse_member_list(list, line_no)?;
                    }
                }
                messages_raw.insert(msg_type.to_owned(), (name.to_owned(), required, optional));
            }
            "group" => {
                let count_tag = parse_tag(tokens.next(), line_no)?;
                let _name = tokens.next().ok_or(ParseError::Malformed {
                    line: line_no,
                    reason: "group requires a name",
                })?;
                let delimiter = parse_tag(tokens.next(), line_no)?;
                let members_token = tokens.next().ok_or(ParseError::Malformed {
                    line: line_no,
                    reason: "group requires a member list",
                })?;
                let members = parse_member_list(members_token, line_no)?;
                groups_raw.insert(count_tag, (delimiter, members));
            }
            "component" => {
                let name = tokens.next().ok_or(ParseError::Malformed {
                    line: line_no,
                    reason: "component requires a name",
                })?;
                let members_token = tokens.next().ok_or(ParseError::Malformed {
                    line: line_no,
                    reason: "component requires a member list",
                })?;
                let members = parse_member_list(members_token, line_no)?;
                components_raw.insert(name.to_owned(), members);
            }
            _ => {
                return Err(ParseError::Malformed {
                    line: line_no,
                    reason: "unknown directive",
                })
            }
        }
    }

    let version = version.ok_or(ParseError::MissingVersion)?;

    // Resolve components first (they may reference each other, but not cyclically), then expand
    // component references in messages'/groups' member lists into their flat tag lists — from this
    // point on, decode/validate never need to know components exist (FR-009).
    let components = resolve_all_components(&components_raw)?;

    let mut groups: BTreeMap<u32, GroupDef> = BTreeMap::new();
    for (count_tag, (delimiter, raw_members)) in groups_raw {
        let members = expand_members(&raw_members, &components)?;
        groups.insert(
            count_tag,
            GroupDef {
                count_tag,
                delimiter,
                members,
            },
        );
    }

    let mut messages: BTreeMap<String, MessageDef> = BTreeMap::new();
    for (msg_type, (name, req_raw, opt_raw)) in messages_raw {
        let required = expand_members(&req_raw, &components)?;
        let optional = expand_members(&opt_raw, &components)?;
        messages.insert(
            msg_type.clone(),
            MessageDef {
                msg_type,
                name,
                required,
                optional,
                member_tags: BTreeSet::new(),
            },
        );
    }

    // Expand each message's repeating-group members (transitively) so the validator accepts group
    // member fields as belonging to the message type.
    for mdef in messages.values_mut() {
        let mut member_tags = BTreeSet::new();
        let seed: Vec<u32> = mdef
            .required
            .iter()
            .chain(mdef.optional.iter())
            .copied()
            .collect();
        for tag in seed {
            collect_group_members(&groups, tag, &mut member_tags);
        }
        mdef.member_tags = member_tags;
    }

    Ok(DataDictionary {
        version,
        fields,
        field_by_name,
        messages,
        header,
        trailer,
        groups,
        components,
        hash: fnv1a(input.as_bytes()),
    })
}

/// Resolve every raw component's member list into a flat, fully-expanded `ComponentDef`, detecting
/// cycles (a component that directly or transitively references itself).
fn resolve_all_components(
    raw: &BTreeMap<String, Vec<RawMember>>,
) -> Result<BTreeMap<String, ComponentDef>, ParseError> {
    let mut resolved: BTreeMap<String, ComponentDef> = BTreeMap::new();
    for name in raw.keys() {
        if !resolved.contains_key(name) {
            let mut resolving = BTreeSet::new();
            resolve_component(name, raw, &mut resolved, &mut resolving)?;
        }
    }
    Ok(resolved)
}

fn resolve_component(
    name: &str,
    raw: &BTreeMap<String, Vec<RawMember>>,
    resolved: &mut BTreeMap<String, ComponentDef>,
    resolving: &mut BTreeSet<String>,
) -> Result<Vec<u32>, ParseError> {
    if let Some(def) = resolved.get(name) {
        return Ok(def.members.clone());
    }
    if !resolving.insert(name.to_owned()) {
        return Err(ParseError::ComponentCycle {
            name: name.to_owned(),
        });
    }
    let raw_members = raw.get(name).ok_or_else(|| ParseError::UnknownComponent {
        name: name.to_owned(),
    })?;
    let mut members = Vec::new();
    for m in raw_members {
        match m {
            RawMember::Tag(t) => members.push(*t),
            RawMember::Component(n) => {
                let expanded = resolve_component(n, raw, resolved, resolving)?;
                members.extend(expanded);
            }
        }
    }
    resolving.remove(name);
    resolved.insert(
        name.to_owned(),
        ComponentDef {
            name: name.to_owned(),
            members: members.clone(),
        },
    );
    Ok(members)
}

/// Expand a message's/group's raw member list into a flat tag list, splicing in each referenced
/// component's already-resolved members in place.
fn expand_members(
    raw: &[RawMember],
    components: &BTreeMap<String, ComponentDef>,
) -> Result<Vec<u32>, ParseError> {
    let mut out = Vec::new();
    for m in raw {
        match m {
            RawMember::Tag(t) => out.push(*t),
            RawMember::Component(name) => {
                let def = components
                    .get(name)
                    .ok_or_else(|| ParseError::UnknownComponent { name: name.clone() })?;
                out.extend(def.members.iter().copied());
            }
        }
    }
    Ok(out)
}

/// Add `count_tag`'s group members (and nested groups' members, transitively) to `out`.
fn collect_group_members(
    groups: &BTreeMap<u32, GroupDef>,
    count_tag: u32,
    out: &mut BTreeSet<u32>,
) {
    if let Some(group) = groups.get(&count_tag) {
        for &m in &group.members {
            if out.insert(m) && groups.contains_key(&m) {
                collect_group_members(groups, m, out);
            }
        }
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(idx) => line.get(..idx).unwrap_or(""),
        None => line,
    }
}

fn parse_tag(token: Option<&str>, line: usize) -> Result<u32, ParseError> {
    token
        .and_then(|t| t.parse().ok())
        .ok_or(ParseError::Malformed {
            line,
            reason: "expected a numeric tag",
        })
}

/// A comma-separated list of tag numbers and/or `component:<Name>` references.
fn parse_member_list(list: &str, line: usize) -> Result<Vec<RawMember>, ParseError> {
    list.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if let Some(name) = s.strip_prefix("component:") {
                Ok(RawMember::Component(name.to_owned()))
            } else {
                s.parse()
                    .map(RawMember::Tag)
                    .map_err(|_| ParseError::Malformed {
                        line,
                        reason: "expected a tag number or component:<Name>",
                    })
            }
        })
        .collect()
}
