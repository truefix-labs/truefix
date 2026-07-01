//! Parser for the normalized dictionary format.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::hash::fnv1a;
use crate::model::{DataDictionary, FieldDef, FieldType, GroupDef, MessageDef};

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
}

/// Parse a normalized dictionary document.
pub fn parse(input: &str) -> Result<DataDictionary, ParseError> {
    let mut version: Option<String> = None;
    let mut fields: BTreeMap<u32, FieldDef> = BTreeMap::new();
    let mut field_by_name: BTreeMap<String, u32> = BTreeMap::new();
    let mut messages: BTreeMap<String, MessageDef> = BTreeMap::new();
    let mut header: BTreeSet<u32> = BTreeSet::new();
    let mut trailer: BTreeSet<u32> = BTreeSet::new();
    let mut groups: BTreeMap<u32, GroupDef> = BTreeMap::new();

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
                let values: Vec<String> = tokens.map(str::to_owned).collect();
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
                        required = parse_tag_list(list, line_no)?;
                    } else if let Some(list) = token.strip_prefix("opt:") {
                        optional = parse_tag_list(list, line_no)?;
                    }
                }
                messages.insert(
                    msg_type.to_owned(),
                    MessageDef {
                        msg_type: msg_type.to_owned(),
                        name: name.to_owned(),
                        required,
                        optional,
                        member_tags: BTreeSet::new(),
                    },
                );
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
                let members = parse_tag_list(members_token, line_no)?;
                groups.insert(
                    count_tag,
                    GroupDef {
                        count_tag,
                        delimiter,
                        members,
                    },
                );
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
        hash: fnv1a(input.as_bytes()),
    })
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

fn parse_tag_list(list: &str, line: usize) -> Result<Vec<u32>, ParseError> {
    list.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse().map_err(|_| ParseError::Malformed {
                line,
                reason: "expected a comma-separated tag list",
            })
        })
        .collect()
}
