//! Dual-track codegen (Constitution Principle IV): generates, per dictionary version, a content
//! hash (the runtime asserts its parsed hash equals this, proving both tracks derive from one
//! source), MsgType constants, strongly-typed per-message structs (thin wrappers over
//! `truefix_core::Message`), field-value enums, repeating-group entry structs, and a
//! `crack_<version>` dispatcher for typed MessageCracker-style dispatch (FR-020/021/022).
//!
//! Typed structs wrap the same generic `Message`/`FieldMap` the runtime codec produces, so
//! encode/decode is always byte-identical with the generic path (FR-021) — there is no separate
//! wire representation to keep in sync.
//!
//! Shared, unmodified, between two build targets (US13, FR-018; no parallel implementation):
//! `build.rs` includes this file directly (`#[path = "src/codegen.rs"] mod codegen;` — a build
//! script can't depend on its own not-yet-built crate, so the *source* is shared instead) to
//! generate the crate's own bundled dictionaries, and the `truefix-dict` CLI's `generate-code`
//! subcommand calls the same [`generate`] function on an arbitrary `.fixdict` file.
//!
//! This module's error handling is intentionally `Result`-based (not `panic!`), unlike a typical
//! build script's own top-level `main()`: it is reachable from the CLI, a user-facing tool that
//! should report a clean error and exit non-zero on malformed input, not print a Rust panic
//! backtrace (Constitution Principle I). `build.rs`'s own `main()` still panics on error — that
//! remains the correct, idiomatic way for a build script to fail — but only at its own top level,
//! after calling into this module's `Result`-returning API.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// An error generating typed code from a normalized dictionary source. Distinct from
/// `parser::ParseError` (the runtime track's parser) — this is a separate, independently-evolving
/// parse pass over the same grammar, by design (dual-track: both tracks must independently derive
/// the same result, verified by the content hash, not share one parser implementation).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CodegenError {
    /// A `req:`/`opt:`/group-member-list token was neither a tag number nor `component:<Name>`.
    #[error("bad tag or component:<Name> token: {0:?}")]
    BadToken(String),
    /// A `component:<Name>` reference named a component that was never defined.
    #[error("unknown component {0:?}")]
    UnknownComponent(String),
    /// A component (directly or transitively) references itself.
    #[error("component {0:?} is part of a cycle")]
    ComponentCycle(String),
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// A field definition as read from the normalized dictionary source.
struct FieldDef {
    name: String,
    ty: String,
    /// `(raw value, optional label)` — label comes from an optional `Value=Label` token.
    values: Vec<(String, Option<String>)>,
}

/// A repeating-group definition.
#[allow(dead_code)] // `delimiter` is parsed for completeness; codegen doesn't need it directly.
struct GroupDef {
    name: String,
    delimiter: u32,
    members: Vec<u32>,
}

/// A message definition, in declaration order.
struct MessageDef {
    msg_type: String,
    name: String,
    required: Vec<u32>,
    optional: Vec<u32>,
    /// GAP-27/FR-031 (feature 006): mirrors `parser::MessageDef::field_order` (the runtime
    /// track's identical field, already correctly parsed and consumed by `Message::encode_with_
    /// order`) — codegen previously never parsed the `ordered` modifier at all, so a message
    /// declaring it had zero effect on the codegen-generated `encode()` path.
    field_order: Option<Vec<u32>>,
}

/// The whole parsed dictionary (fields/groups/messages only — codegen doesn't need
/// header/trailer classification, which the runtime `DataDictionary` already provides).
struct Dict {
    fields: BTreeMap<u32, FieldDef>,
    groups: BTreeMap<u32, GroupDef>,
    messages: Vec<MessageDef>,
}

/// A `req:`/`opt:`/group-member-list token before `component:<Name>` references are expanded
/// into their flat tag lists (mirrors `parser::RawMember` in the runtime track — codegen must
/// understand `component:` tokens too, or messages using them would silently lose members).
enum RawMember {
    Tag(u32),
    Component(String),
}

fn parse_member_list_raw(list: &str) -> Result<Vec<RawMember>, CodegenError> {
    list.split(',')
        .filter(|s| !s.is_empty())
        .map(|s| match s.strip_prefix("component:") {
            Some(name) => Ok(RawMember::Component(name.to_owned())),
            None => s
                .parse()
                .map(RawMember::Tag)
                .map_err(|_| CodegenError::BadToken(s.to_owned())),
        })
        .collect()
}

/// Resolve `name`'s component member list into flat tags, expanding nested `component:`
/// references (recursively, with cycle detection matching the runtime parser).
fn resolve_component(
    name: &str,
    raw: &BTreeMap<String, Vec<RawMember>>,
    resolved: &mut BTreeMap<String, Vec<u32>>,
    resolving: &mut std::collections::BTreeSet<String>,
) -> Result<Vec<u32>, CodegenError> {
    if let Some(members) = resolved.get(name) {
        return Ok(members.clone());
    }
    if !resolving.insert(name.to_owned()) {
        return Err(CodegenError::ComponentCycle(name.to_owned()));
    }
    let raw_members = raw
        .get(name)
        .ok_or_else(|| CodegenError::UnknownComponent(name.to_owned()))?;
    let mut members = Vec::new();
    for m in raw_members {
        match m {
            RawMember::Tag(t) => members.push(*t),
            RawMember::Component(n) => {
                members.extend(resolve_component(n, raw, resolved, resolving)?)
            }
        }
    }
    resolving.remove(name);
    resolved.insert(name.to_owned(), members.clone());
    Ok(members)
}

fn expand_members(
    raw: &[RawMember],
    components: &BTreeMap<String, Vec<u32>>,
) -> Result<Vec<u32>, CodegenError> {
    let mut out = Vec::new();
    for m in raw {
        match m {
            RawMember::Tag(t) => out.push(*t),
            RawMember::Component(name) => out.extend(
                components
                    .get(name)
                    .ok_or_else(|| CodegenError::UnknownComponent(name.to_owned()))?
                    .iter()
                    .copied(),
            ),
        }
    }
    Ok(out)
}

/// `(msg_type, name, required, optional, ordered)` — one raw, not-yet-component-expanded message
/// declaration. `ordered` mirrors the `parser::MessageDef::field_order` modifier (GAP-27/FR-031,
/// feature 006).
type RawMessage = (String, String, Vec<RawMember>, Vec<RawMember>, bool);

fn parse_dict(text: &str) -> Result<Dict, CodegenError> {
    let mut fields = BTreeMap::new();
    let mut groups_raw: BTreeMap<u32, (String, u32, Vec<RawMember>)> = BTreeMap::new();
    let mut messages_raw: Vec<RawMessage> = Vec::new();
    let mut components_raw: BTreeMap<String, Vec<RawMember>> = BTreeMap::new();

    for raw in text.lines() {
        let line = match raw.find('#') {
            Some(i) => raw.split_at(i).0,
            None => raw,
        }
        .trim();
        if line.is_empty() {
            continue;
        }
        let mut tokens = line.split_whitespace();
        match tokens.next() {
            Some("field") => {
                let Some(tag) = tokens.next().and_then(|t| t.parse::<u32>().ok()) else {
                    continue;
                };
                let Some(name) = tokens.next() else { continue };
                let Some(ty) = tokens.next() else { continue };
                // BUG-74/FR-050 (feature 007): strip a leading `open` modifier the same way
                // `parser.rs`'s runtime track already does — previously codegen ingested it as a
                // literal (unlabeled) enum value instead, a dual-track divergence (Constitution
                // Principle IV). Codegen doesn't need `open_enum` itself (it doesn't emit
                // enum-membership-checking code — that's a `validate.rs`/runtime concern), so this
                // is purely about not corrupting the emitted value list with a stray `"open"`
                // variant.
                let mut remaining: Vec<&str> = tokens.collect();
                if remaining.first() == Some(&"open") {
                    remaining.remove(0);
                }
                let values = remaining
                    .into_iter()
                    .map(|tok| match tok.split_once('=') {
                        Some((v, l)) => (v.to_owned(), Some(l.to_owned())),
                        None => (tok.to_owned(), None),
                    })
                    .collect();
                fields.insert(
                    tag,
                    FieldDef {
                        name: name.to_owned(),
                        ty: ty.to_owned(),
                        values,
                    },
                );
            }
            Some("group") => {
                let Some(count_tag) = tokens.next().and_then(|t| t.parse::<u32>().ok()) else {
                    continue;
                };
                let Some(name) = tokens.next() else { continue };
                let Some(delimiter) = tokens.next().and_then(|t| t.parse::<u32>().ok()) else {
                    continue;
                };
                let members = match tokens.next() {
                    Some(list) => parse_member_list_raw(list)?,
                    None => Vec::new(),
                };
                groups_raw.insert(count_tag, (name.to_owned(), delimiter, members));
            }
            Some("component") => {
                let Some(name) = tokens.next() else { continue };
                let members = match tokens.next() {
                    Some(list) => parse_member_list_raw(list)?,
                    None => Vec::new(),
                };
                components_raw.insert(name.to_owned(), members);
            }
            Some("message") => {
                let Some(msg_type) = tokens.next() else {
                    continue;
                };
                let Some(name) = tokens.next() else { continue };
                let mut required = Vec::new();
                let mut optional = Vec::new();
                let mut ordered = false;
                for tok in tokens {
                    if tok == "ordered" {
                        ordered = true;
                    } else if let Some(list) = tok.strip_prefix("req:") {
                        required = parse_member_list_raw(list)?;
                    } else if let Some(list) = tok.strip_prefix("opt:") {
                        optional = parse_member_list_raw(list)?;
                    }
                }
                messages_raw.push((
                    msg_type.to_owned(),
                    name.to_owned(),
                    required,
                    optional,
                    ordered,
                ));
            }
            _ => {}
        }
    }

    let mut components: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for name in components_raw.keys() {
        if !components.contains_key(name) {
            let mut resolving = std::collections::BTreeSet::new();
            resolve_component(name, &components_raw, &mut components, &mut resolving)?;
        }
    }

    let groups = groups_raw
        .into_iter()
        .map(|(count_tag, (name, delimiter, raw_members))| {
            Ok((
                count_tag,
                GroupDef {
                    name,
                    delimiter,
                    members: expand_members(&raw_members, &components)?,
                },
            ))
        })
        .collect::<Result<_, CodegenError>>()?;

    let messages = messages_raw
        .into_iter()
        .map(|(msg_type, name, req_raw, opt_raw, ordered)| {
            let required = expand_members(&req_raw, &components)?;
            let optional = expand_members(&opt_raw, &components)?;
            let field_order =
                ordered.then(|| required.iter().chain(optional.iter()).copied().collect());
            Ok(MessageDef {
                msg_type,
                name,
                required,
                optional,
                field_order,
            })
        })
        .collect::<Result<_, CodegenError>>()?;

    Ok(Dict {
        fields,
        groups,
        messages,
    })
}

/// Convert a FIX PascalCase field/message name (e.g. `ClOrdID`, `NoPartyIDs`) to snake_case,
/// treating runs of uppercase letters as acronyms and keeping a trailing bare `s` (plural) glued
/// to a 2-letter acronym (`NoPartyIDs` -> `no_party_ids`, not `no_party_i_ds`).
fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev_upper = chars.get(i - 1).is_some_and(|p| p.is_ascii_uppercase());
            if !prev_upper {
                out.push('_');
            } else {
                // End of a >=2-letter acronym run, if followed by a lowercase letter: the
                // trailing letters normally start a new word, *unless* the run up to here is
                // exactly 2 letters and what follows is a bare plural `s` (e.g. `IDs`).
                let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
                if next_lower {
                    let run_len_here = {
                        let mut n = 1;
                        let mut j = i;
                        while j > 0 && chars.get(j - 1).is_some_and(|c| c.is_ascii_uppercase()) {
                            n += 1;
                            j -= 1;
                        }
                        n
                    };
                    let is_bare_plural_s = chars.get(i + 1) == Some(&'s')
                        && chars.get(i + 2).is_none_or(|n| !n.is_alphabetic());
                    if !(run_len_here == 2 && is_bare_plural_s) {
                        out.push('_');
                    }
                }
            }
        }
        out.push(c.to_ascii_lowercase());
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, 'f');
    }
    // US9 (feature 005): real QFJ field names include `Yield` (tag 236) — confirmed empirically
    // as the one actual keyword collision across every bundled dictionary source; the rest below
    // are added defensively (harmless if never hit — no real field name currently matches them).
    match out.as_str() {
        "type" | "ref" | "self" | "move" | "in" | "fn" | "struct" | "enum" | "match" | "for"
        | "yield" | "loop" | "impl" | "as" | "break" | "continue" | "static" | "const"
        | "trait" | "use" | "mod" | "pub" | "where" | "dyn" | "let" | "if" | "else" | "while"
        | "return" | "true" | "false" | "async" | "await" | "unsafe" | "extern" | "super"
        | "crate" | "box" | "try" => {
            out.push('_');
        }
        _ => {}
    }
    out
}

/// Rust type + `Field` accessor/constructor names for a normalized field type.
struct TypeMapping {
    /// Rust type returned by the getter (borrowed forms use `'_`).
    rust_ty: &'static str,
    /// `Field` method that converts the raw value (e.g. `as_str`).
    getter: &'static str,
    /// Whether the getter returns a borrowed `&str` (needs `.map(str::to_owned)` avoided; kept
    /// simple by returning owned/Copy types except for STRING-like fields).
    borrowed_str: bool,
}

fn type_mapping(ty: &str) -> TypeMapping {
    match ty {
        "INT" | "LENGTH" | "SEQNUM" | "NUMINGROUP" => TypeMapping {
            rust_ty: "i64",
            getter: "as_int",
            borrowed_str: false,
        },
        // BUG-72/FR-050 (feature 007): PRICEOFFSET shares Price/Qty/Amt/Percentage's decimal wire
        // format (already true on the runtime track, `model.rs`'s `FieldType::value_ok`) — codegen
        // previously fell through to the generic `&str` catch-all below, giving it a stringly
        // accessor instead of a typed decimal one.
        "FLOAT" | "PRICE" | "QTY" | "AMT" | "PERCENTAGE" | "PRICEOFFSET" => TypeMapping {
            rust_ty: "rust_decimal::Decimal",
            getter: "as_decimal",
            borrowed_str: false,
        },
        "CHAR" => TypeMapping {
            rust_ty: "char",
            getter: "as_char",
            borrowed_str: false,
        },
        "BOOLEAN" => TypeMapping {
            rust_ty: "bool",
            getter: "as_bool",
            borrowed_str: false,
        },
        // BUG-72/FR-050 (feature 007): TIME shares UTCTIMESTAMP's wire format and accessor
        // (`model.rs`'s `FieldType::value_ok` groups `UtcTimestamp | Time` identically) —
        // previously fell through to `&str`.
        "UTCTIMESTAMP" | "TIME" => TypeMapping {
            rust_ty: "time::OffsetDateTime",
            getter: "as_utc_timestamp",
            borrowed_str: false,
        },
        // STRING, DATA, UTCTIMEONLY, UTCDATEONLY, MONTHYEAR, and anything unrecognized: treat as
        // a raw string (safe default; still round-trips byte-exactly).
        _ => TypeMapping {
            rust_ty: "&str",
            getter: "as_str",
            borrowed_str: true,
        },
    }
}

/// Rust keywords (2018+ strict and reserved) a sanitized enum-variant label must not collide with
/// (BUG-73/FR-050, feature 007) — checked case-insensitively, since an enum variant is
/// conventionally PascalCase (`Type`, `Self`) even though most actual Rust keywords are lowercase;
/// being case-insensitive here is a deliberately cautious superset, not just the exact keywords a
/// literal Rust parser would reject.
const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];

/// Sanitize a QuickFIX enum-value label into a valid Rust identifier (US9, feature 005, FR-031):
/// real QFJ-schema labels are always `[A-Za-z0-9_]+` (confirmed empirically against every bundled
/// XML source — no punctuation), but some start with a digit (e.g. `"3A3"`, `"42"`, `"106H106J"`),
/// which no Rust identifier may do. Prefixing with `V` when that happens is the minimal fix — the
/// hand-picked labels this codegen shipped with before US9's real-QFJ-data expansion never hit
/// this case, since they were all conventional words like `"Buy"`/`"Sell"`.
///
/// BUG-73/FR-050 (feature 007): a label exactly matching a Rust keyword (`"Type"`, `"Self"`,
/// `"Match"`, etc.) previously produced uncompilable generated code — no real bundled QFJ schema
/// label actually hits this (they're all `SCREAMING_SNAKE_CASE`, e.g. `PER_UNIT`), but a custom
/// dictionary source could. Reuses the same `V`-prefix strategy as the leading-digit case above.
fn sanitize_variant(label: &str) -> String {
    let needs_prefix = label.starts_with(|c: char| c.is_ascii_digit())
        || RUST_KEYWORDS
            .iter()
            .any(|kw| kw.eq_ignore_ascii_case(label));
    if needs_prefix {
        format!("V{label}")
    } else {
        label.to_owned()
    }
}

/// Emit a field-value enum (e.g. `pub enum Side { Buy, Sell, ... }`) if `field` declares labeled
/// enum values; returns the enum's Rust name if one was emitted. `message_names` disambiguates
/// the rare real-QFJ-schema case (US9, feature 005, FR-031) where a field and a message share the
/// same human-readable name (e.g. FIX 5.0SP2's field 965 `SecurityStatus` and message `f`
/// `SecurityStatus`) — the enum gets a `Value`-suffixed name instead of colliding with the
/// generated message struct of the same name.
fn emit_field_enum(
    code: &mut String,
    field: &FieldDef,
    message_names: &std::collections::BTreeSet<String>,
) -> Option<String> {
    let labeled: Vec<(&str, String)> = field
        .values
        .iter()
        .filter_map(|(v, l)| l.as_deref().map(|l| (v.as_str(), sanitize_variant(l))))
        .collect();
    if labeled.is_empty() {
        return None;
    }
    let enum_name = if message_names.contains(&field.name) {
        format!("{}Value", field.name)
    } else {
        field.name.clone()
    };
    let _ = writeln!(code, "/// Enumerated values for {}.", field.name);
    let _ = writeln!(code, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    // US9 (feature 005): real QFJ enum labels are SCREAMING_SNAKE_CASE (e.g. `PER_UNIT`), not
    // the UpperCamelCase this project's own hand-picked labels (`Buy`/`Sell`) happened to already
    // be — emitted verbatim (Constitution Principle III: reproduce the source's own naming, not
    // re-derive a "nicer" one) rather than silently reformatted, so this allow is scoped to
    // exactly the generated variants that are affected.
    let _ = writeln!(code, "#[allow(non_camel_case_types)]");
    let _ = writeln!(code, "pub enum {enum_name} {{");
    for (_, label) in &labeled {
        let _ = writeln!(code, "    /// Wire value handled in `as_str`/`parse`.");
        let _ = writeln!(code, "    {label},");
    }
    let _ = writeln!(code, "}}");
    let _ = writeln!(code, "impl {enum_name} {{");
    let _ = writeln!(code, "    /// The raw FIX wire value.");
    let _ = writeln!(code, "    pub fn as_str(self) -> &'static str {{");
    let _ = writeln!(code, "        match self {{");
    for (value, label) in &labeled {
        let _ = writeln!(code, "            Self::{label} => {value:?},");
    }
    let _ = writeln!(code, "        }}");
    let _ = writeln!(code, "    }}");
    let _ = writeln!(
        code,
        "    /// Parse a raw FIX wire value into its enum variant."
    );
    let _ = writeln!(code, "    pub fn parse(s: &str) -> Option<Self> {{");
    let _ = writeln!(code, "        match s {{");
    for (value, label) in &labeled {
        let _ = writeln!(code, "            {value:?} => Some(Self::{label}),");
    }
    let _ = writeln!(code, "            _ => None,");
    let _ = writeln!(code, "        }}");
    let _ = writeln!(code, "    }}");
    let _ = writeln!(code, "}}");
    Some(enum_name)
}

/// Emit typed get/set accessors for `tag` on a struct whose inner field-map is reached via
/// `access` (e.g. `self.0.body`).
fn emit_field_accessors(
    code: &mut String,
    dict: &Dict,
    tag: u32,
    access: &str,
    enum_names: &BTreeMap<u32, String>,
) {
    let Some(field) = dict.fields.get(&tag) else {
        return;
    };
    let ident = snake_case(&field.name);
    if let Some(enum_name) = enum_names.get(&tag) {
        let _ = writeln!(
            code,
            "    /// {} ({tag}), as its enumerated type.",
            field.name
        );
        let _ = writeln!(
            code,
            "    pub fn {ident}(&self) -> Option<{enum_name}> {{ self.{access}.get({tag}).and_then(|f| f.as_str().ok()).and_then({enum_name}::parse) }}"
        );
        let _ = writeln!(
            code,
            "    /// Set {} ({tag}) from its enumerated type.",
            field.name
        );
        let _ = writeln!(
            code,
            "    pub fn set_{ident}(&mut self, v: {enum_name}) -> &mut Self {{ self.{access}.set(truefix_core::Field::string({tag}, v.as_str())); self }}"
        );
        return;
    }
    let mapping = type_mapping(&field.ty);
    let _ = writeln!(code, "    /// {} ({tag}).", field.name);
    if mapping.borrowed_str {
        let _ = writeln!(
            code,
            "    pub fn {ident}(&self) -> Option<&str> {{ self.{access}.get({tag}).and_then(|f| f.{}().ok()) }}",
            mapping.getter
        );
        let _ = writeln!(code, "    /// Set {} ({tag}).", field.name);
        let _ = writeln!(
            code,
            "    pub fn set_{ident}(&mut self, v: &str) -> &mut Self {{ self.{access}.set(truefix_core::Field::string({tag}, v)); self }}"
        );
    } else {
        let _ = writeln!(
            code,
            "    pub fn {ident}(&self) -> Option<{}> {{ self.{access}.get({tag}).and_then(|f| f.{}().ok()) }}",
            mapping.rust_ty, mapping.getter
        );
        let _ = writeln!(code, "    /// Set {} ({tag}).", field.name);
        // `OffsetDateTime`'s own `Display` is not the FIX wire format, so UTCTIMESTAMP fields use
        // the dedicated `Field::utc_timestamp` constructor instead of `.to_string()`.
        let ctor = if field.ty == "UTCTIMESTAMP" {
            format!("truefix_core::Field::utc_timestamp({tag}, v)")
        } else {
            format!("truefix_core::Field::string({tag}, &v.to_string())")
        };
        let _ = writeln!(
            code,
            "    pub fn set_{ident}(&mut self, v: {}) -> &mut Self {{ self.{access}.set({ctor}); self }}",
            mapping.rust_ty
        );
    }
}

/// Emit a typed entry struct for a repeating group (and recursively for any nested groups its
/// members reference), wrapping a `FieldMap`.
fn emit_group_structs(
    code: &mut String,
    dict: &Dict,
    count_tag: u32,
    enum_names: &BTreeMap<u32, String>,
    emitted: &mut std::collections::BTreeSet<u32>,
) {
    if !emitted.insert(count_tag) {
        return;
    }
    let Some(group) = dict.groups.get(&count_tag) else {
        return;
    };
    // Emit nested groups first so this struct can reference them.
    for &member in &group.members {
        if dict.groups.contains_key(&member) {
            emit_group_structs(code, dict, member, enum_names, emitted);
        }
    }
    let struct_name = format!("{}Entry", group.name);
    let _ = writeln!(code, "/// One entry of the {} repeating group.", group.name);
    let _ = writeln!(code, "#[derive(Debug, Clone, Default)]");
    let _ = writeln!(
        code,
        "pub struct {struct_name}(pub truefix_core::FieldMap);"
    );
    let _ = writeln!(code, "impl {struct_name} {{");
    let _ = writeln!(code, "    /// A new, empty entry.");
    let _ = writeln!(code, "    pub fn new() -> Self {{ Self::default() }}");
    for &member in &group.members {
        if let Some(nested) = dict.groups.get(&member) {
            let nested_struct = format!("{}Entry", nested.name);
            let ident = snake_case(&nested.name);
            let _ = writeln!(code, "    /// Nested {} group entries.", nested.name);
            let _ = writeln!(
                code,
                "    pub fn {ident}(&self) -> Vec<{nested_struct}> {{ self.0.group({member}).map(|es| es.iter().cloned().map({nested_struct}).collect()).unwrap_or_default() }}"
            );
            let _ = writeln!(
                code,
                "    /// Set the nested {} group entries.",
                nested.name
            );
            let _ = writeln!(
                code,
                "    pub fn set_{ident}(&mut self, entries: Vec<{nested_struct}>) -> &mut Self {{ let mut g = truefix_core::Group::new({member}); for e in entries {{ g.add_entry(e.0); }} self.0.add_group(g); self }}"
            );
        } else {
            emit_field_accessors(code, dict, member, "0", enum_names);
        }
    }
    let _ = writeln!(code, "}}");
    let _ = writeln!(
        code,
        "impl From<truefix_core::FieldMap> for {struct_name} {{ fn from(m: truefix_core::FieldMap) -> Self {{ Self(m) }} }}"
    );
}

/// Generate the code for one dictionary version and append it to `code` (used to concatenate
/// multiple versions into one `generated.rs`, as `build.rs` does).
pub fn emit_version(code: &mut String, name: &str, bytes: &[u8]) -> Result<(), CodegenError> {
    let hash = fnv1a(bytes);
    let _ = writeln!(code, "/// Content hash of the {name} dictionary source.");
    let _ = writeln!(code, "pub const {name}_DICT_HASH: u64 = {hash};");

    let text = String::from_utf8_lossy(bytes);
    let dict = parse_dict(&text)?;

    // --- MsgType constants (unchanged from earlier stages; dual_track.rs depends on this). ---
    let msgs_module = format!("{}_msgs", name.to_lowercase());
    let _ = writeln!(
        code,
        "/// Generated MsgType constants for the {name} dictionary."
    );
    let _ = writeln!(code, "pub mod {msgs_module} {{");
    for m in &dict.messages {
        let ident = m.name.to_uppercase();
        let _ = writeln!(code, "    /// MsgType for {}.", m.name);
        let _ = writeln!(code, "    pub const {ident}: &str = {:?};", m.msg_type);
    }
    let _ = writeln!(code, "}}");

    // --- Typed messages, field enums, group structs, and a MessageCracker dispatcher (US6). ---
    let module = name.to_lowercase();
    let _ = writeln!(
        code,
        "/// Strongly-typed per-message structs, field-value enums, group entries, and a\n\
         /// `crack_{module}` dispatcher for the {name} dictionary (FR-020/021/022)."
    );
    let _ = writeln!(code, "pub mod {module} {{");
    let _ = writeln!(code, "    #![allow(clippy::needless_lifetimes)]");
    let _ = writeln!(code, "    use truefix_core::Message;");

    // Field-value enums: one per field that carries labeled enum values, referenced by any message.
    let message_names: std::collections::BTreeSet<String> =
        dict.messages.iter().map(|m| m.name.clone()).collect();
    let mut enum_names: BTreeMap<u32, String> = BTreeMap::new();
    let mut used_tags: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    for m in &dict.messages {
        for &t in m.required.iter().chain(m.optional.iter()) {
            used_tags.insert(t);
        }
    }
    for &tag in &used_tags {
        if let Some(field) = dict.fields.get(&tag)
            && let Some(enum_name) = emit_field_enum(code, field, &message_names)
        {
            enum_names.insert(tag, enum_name);
        }
    }

    // Repeating-group entry structs referenced by any message.
    let mut emitted_groups = std::collections::BTreeSet::new();
    for &tag in &used_tags {
        if dict.groups.contains_key(&tag) {
            emit_group_structs(code, &dict, tag, &enum_names, &mut emitted_groups);
        }
    }

    // Per-message typed structs.
    for m in &dict.messages {
        let struct_name = m.name.clone();
        let _ = writeln!(
            code,
            "/// Typed {name} {} (MsgType={:?}).",
            m.name, m.msg_type
        );
        let _ = writeln!(code, "#[derive(Debug, Clone)]");
        let _ = writeln!(code, "pub struct {struct_name}(pub Message);");
        let _ = writeln!(code, "impl {struct_name} {{");
        let _ = writeln!(code, "    /// A new {} with MsgType stamped.", m.name);
        let _ = writeln!(code, "    pub fn new() -> Self {{");
        let _ = writeln!(code, "        let mut m = Message::new();");
        let _ = writeln!(
            code,
            "        m.header.set(truefix_core::Field::string(35, {:?}));",
            m.msg_type
        );
        let _ = writeln!(code, "        Self(m)");
        let _ = writeln!(code, "    }}");
        for &tag in m.required.iter().chain(m.optional.iter()) {
            if let Some(group) = dict.groups.get(&tag) {
                let entry_struct = format!("{}Entry", group.name);
                let ident = snake_case(&group.name);
                let _ = writeln!(code, "    /// {} group entries.", group.name);
                let _ = writeln!(
                    code,
                    "    pub fn {ident}(&self) -> Vec<{entry_struct}> {{ self.0.body.group({tag}).map(|es| es.iter().cloned().map({entry_struct}).collect()).unwrap_or_default() }}"
                );
                let _ = writeln!(code, "    /// Set the {} group entries.", group.name);
                let _ = writeln!(
                    code,
                    "    pub fn set_{ident}(&mut self, entries: Vec<{entry_struct}>) -> &mut Self {{ let mut g = truefix_core::Group::new({tag}); for e in entries {{ g.add_entry(e.0); }} self.0.body.add_group(g); self }}"
                );
            } else {
                emit_field_accessors(code, &dict, tag, "0.body", &enum_names);
            }
        }
        let _ = writeln!(
            code,
            "    /// Encode to wire bytes (byte-identical with the generic codec path)."
        );
        // GAP-27/FR-031 (feature 006): a message declaring a custom `fieldOrder` in its
        // dictionary now actually has that order reach the wire from the real (codegen-generated)
        // encode path — previously `encode_with_order` was correct but never invoked from here,
        // so declaring `ordered` in a dictionary had zero effect on any message TrueFix encoded.
        match &m.field_order {
            Some(order) if !order.is_empty() => {
                let order_ident = format!("{}_FIELD_ORDER", struct_name.to_uppercase());
                let order_list = order
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    code,
                    "    pub fn encode(&self) -> Vec<u8> {{ truefix_core::encode_with_order(&self.0, Some(&{order_ident})) }}"
                );
                let _ = writeln!(code, "}}");
                let _ = writeln!(
                    code,
                    "const {order_ident}: [u32; {}] = [{order_list}];",
                    order.len()
                );
            }
            _ => {
                let _ = writeln!(
                    code,
                    "    pub fn encode(&self) -> Vec<u8> {{ self.0.encode() }}"
                );
                let _ = writeln!(code, "}}");
            }
        }
        let _ = writeln!(
            code,
            "impl Default for {struct_name} {{ fn default() -> Self {{ Self::new() }} }}"
        );
        let _ = writeln!(
            code,
            "impl From<Message> for {struct_name} {{ fn from(m: Message) -> Self {{ Self(m) }} }}"
        );
        let _ = writeln!(
            code,
            "impl From<{struct_name}> for Message {{ fn from(t: {struct_name}) -> Self {{ t.0 }} }}"
        );
    }

    // The per-version typed handler trait + dispatcher (MessageCracker-style; FR-022).
    let handler_trait = format!("{}MessageHandler", name);
    let _ = writeln!(
        code,
        "/// A typed per-message handler for {name}; default methods are no-ops."
    );
    let _ = writeln!(code, "#[allow(unused_variables)]");
    let _ = writeln!(code, "pub trait {handler_trait} {{");
    for m in &dict.messages {
        let method = format!("on_{}", snake_case(&m.name));
        let _ = writeln!(code, "    /// Called for an inbound {} ({name}).", m.name);
        let _ = writeln!(code, "    fn {method}(&mut self, msg: &{}) {{}}", m.name);
    }
    let _ = writeln!(code, "}}");
    let _ = writeln!(
        code,
        "/// Dispatch `message` to the {handler_trait} method matching its MsgType, if `message`'s\n\
         /// BeginString is {:?}. Returns whether a handler method was invoked (FR-022).",
        version_begin_string(name)
    );
    let _ = writeln!(
        code,
        "pub fn crack_{module}(message: &Message, handler: &mut impl {handler_trait}) -> bool {{"
    );
    let _ = writeln!(
        code,
        "    if message.begin_string() != Some({:?}) {{ return false; }}",
        version_begin_string(name)
    );
    // NEW-06 (feature 009): FIX 5.0/SP1/SP2 all carry BeginString=FIXT.1.1 on the wire and are
    // distinguished only by ApplVerID(1128) -- without this extra guard, a FIX 5.0SP2 message
    // would match `crack_fix50` (and every other FIX-5.x `crack_*`) since they all share the same
    // BeginString check above.
    if let Some(appl_ver_id) = version_appl_ver_id(name) {
        let _ = writeln!(
            code,
            "    if message.header.get(1128).and_then(|f| f.as_str().ok()) != Some({appl_ver_id:?}) {{ return false; }}"
        );
    }
    let _ = writeln!(code, "    match message.msg_type() {{");
    for m in &dict.messages {
        let method = format!("on_{}", snake_case(&m.name));
        let _ = writeln!(
            code,
            "        Some({:?}) => {{ handler.{method}(&{}(message.clone())); true }}",
            m.msg_type, m.name
        );
    }
    let _ = writeln!(code, "        _ => false,");
    let _ = writeln!(code, "    }}");
    let _ = writeln!(code, "}}");

    let _ = writeln!(code, "}}"); // close module
    Ok(())
}

/// NEW-06 (feature 009): the `ApplVerID(1128)` value distinguishing FIX 5.0/SP1/SP2 -- all three
/// carry the same `BeginString=FIXT.1.1` (see `version_begin_string` below), so a `crack_*`
/// dispatcher for one of these three versions must also check this value; every other version is
/// unambiguously identified by `BeginString` alone (`None`).
fn version_appl_ver_id(name: &str) -> Option<&'static str> {
    match name {
        "FIX50" => Some("7"),
        "FIX50SP1" => Some("8"),
        "FIX50SP2" => Some("9"),
        _ => None,
    }
}

/// The BeginString a version's messages carry (FIXT.1.1-transported FIX 5.0.x app messages still
/// carry `FIXT.1.1` on the wire; this only affects `crack_*`'s guard, not accessor generation).
fn version_begin_string(name: &str) -> &'static str {
    match name {
        "FIX40" => "FIX.4.0",
        "FIX41" => "FIX.4.1",
        "FIX42" => "FIX.4.2",
        "FIX43" => "FIX.4.3",
        "FIX44" => "FIX.4.4",
        "FIXT11" => "FIXT.1.1",
        "FIX50" | "FIX50SP1" | "FIX50SP2" => "FIXT.1.1",
        "FIXLATEST" => "FIX.Latest",
        _ => "",
    }
}

/// Generate the full typed-code module for one dictionary version, from its normalized source
/// bytes and a module `name` (e.g. `"FIX44"`). This is what the `truefix-dict` CLI's
/// `generate-code` subcommand calls; `build.rs` calls [`emit_version`] directly (repeatedly, to
/// concatenate all bundled versions into one file) instead — unused from `build.rs`'s own,
/// separately-compiled copy of this file, hence the allow.
#[allow(dead_code)]
pub fn generate(name: &str, bytes: &[u8]) -> Result<String, CodegenError> {
    let mut code = String::new();
    emit_version(&mut code, name, bytes)?;
    Ok(code)
}
