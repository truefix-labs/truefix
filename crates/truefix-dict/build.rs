//! Build-time codegen (dual-track, Constitution Principle IV).
//!
//! Reads the same normalized dictionary sources the runtime loads and generates, per version:
//! a content hash (the runtime asserts its parsed hash equals this, proving both tracks derive
//! from one source), MsgType constants (`<version>_msgs`, unchanged from earlier stages), and —
//! for feature 002/US6 — strongly-typed per-message structs (thin wrappers over
//! `truefix_core::Message`), field-value enums, repeating-group entry structs, and a
//! `crack_<version>` dispatcher for typed MessageCracker-style dispatch (FR-020/021/022).
//!
//! Typed structs wrap the same generic `Message`/`FieldMap` the runtime codec produces, so
//! encode/decode is always byte-identical with the generic path (FR-021) — there is no separate
//! wire representation to keep in sync.

use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

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
}

/// The whole parsed dictionary (fields/groups/messages only — codegen doesn't need
/// header/trailer classification, which the runtime `DataDictionary` already provides).
struct Dict {
    fields: BTreeMap<u32, FieldDef>,
    groups: BTreeMap<u32, GroupDef>,
    messages: Vec<MessageDef>,
}

fn parse_dict(text: &str) -> Dict {
    let mut fields = BTreeMap::new();
    let mut groups = BTreeMap::new();
    let mut messages = Vec::new();

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
                let values = tokens
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
                let members: Vec<u32> = tokens
                    .next()
                    .map(|list| list.split(',').filter_map(|s| s.parse().ok()).collect())
                    .unwrap_or_default();
                groups.insert(
                    count_tag,
                    GroupDef {
                        name: name.to_owned(),
                        delimiter,
                        members,
                    },
                );
            }
            Some("message") => {
                let Some(msg_type) = tokens.next() else {
                    continue;
                };
                let Some(name) = tokens.next() else { continue };
                let mut required = Vec::new();
                let mut optional = Vec::new();
                for tok in tokens {
                    if let Some(list) = tok.strip_prefix("req:") {
                        required = list.split(',').filter_map(|s| s.parse().ok()).collect();
                    } else if let Some(list) = tok.strip_prefix("opt:") {
                        optional = list.split(',').filter_map(|s| s.parse().ok()).collect();
                    }
                }
                messages.push(MessageDef {
                    msg_type: msg_type.to_owned(),
                    name: name.to_owned(),
                    required,
                    optional,
                });
            }
            _ => {}
        }
    }

    Dict {
        fields,
        groups,
        messages,
    }
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
                        while j > 0 && chars[j - 1].is_ascii_uppercase() {
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
    match out.as_str() {
        "type" | "ref" | "self" | "move" | "in" | "fn" | "struct" | "enum" | "match" | "for" => {
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
        "FLOAT" | "PRICE" | "QTY" | "AMT" | "PERCENTAGE" => TypeMapping {
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
        "UTCTIMESTAMP" => TypeMapping {
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

/// Emit a field-value enum (e.g. `pub enum Side { Buy, Sell, ... }`) if `field` declares labeled
/// enum values; returns the enum's Rust name if one was emitted.
fn emit_field_enum(code: &mut String, field: &FieldDef) -> Option<String> {
    let labeled: Vec<(&str, &str)> = field
        .values
        .iter()
        .filter_map(|(v, l)| l.as_deref().map(|l| (v.as_str(), l)))
        .collect();
    if labeled.is_empty() {
        return None;
    }
    let enum_name = field.name.clone();
    let _ = writeln!(code, "/// Enumerated values for {}.", field.name);
    let _ = writeln!(code, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
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
    let _ = writeln!(code, "impl From<truefix_core::FieldMap> for {struct_name} {{ fn from(m: truefix_core::FieldMap) -> Self {{ Self(m) }} }}");
}

fn emit_version(code: &mut String, name: &str, bytes: &[u8]) {
    let hash = fnv1a(bytes);
    let _ = writeln!(code, "/// Content hash of the {name} dictionary source.");
    let _ = writeln!(code, "pub const {name}_DICT_HASH: u64 = {hash};");

    let text = String::from_utf8_lossy(bytes);
    let dict = parse_dict(&text);

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
    let mut enum_names: BTreeMap<u32, String> = BTreeMap::new();
    let mut used_tags: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    for m in &dict.messages {
        for &t in m.required.iter().chain(m.optional.iter()) {
            used_tags.insert(t);
        }
    }
    for &tag in &used_tags {
        if let Some(field) = dict.fields.get(&tag) {
            if let Some(enum_name) = emit_field_enum(code, field) {
                enum_names.insert(tag, enum_name);
            }
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
        let _ = writeln!(
            code,
            "    pub fn encode(&self) -> Vec<u8> {{ self.0.encode() }}"
        );
        let _ = writeln!(code, "}}");
        let _ = writeln!(
            code,
            "impl Default for {struct_name} {{ fn default() -> Self {{ Self::new() }} }}"
        );
        let _ = writeln!(
            code,
            "impl From<Message> for {struct_name} {{ fn from(m: Message) -> Self {{ Self(m) }} }}"
        );
        let _ = writeln!(code, "impl From<{struct_name}> for Message {{ fn from(t: {struct_name}) -> Self {{ t.0 }} }}");
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
        _ => "",
    }
}

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let mut code = String::new();
    code.push_str("// @generated by build.rs from dict-src/normalized — do not edit.\n");

    for (name, file) in [
        ("FIX40", "FIX40.fixdict"),
        ("FIX41", "FIX41.fixdict"),
        ("FIX42", "FIX42.fixdict"),
        ("FIX43", "FIX43.fixdict"),
        ("FIX44", "FIX44.fixdict"),
        ("FIXT11", "FIXT11.fixdict"),
        ("FIX50", "FIX50.fixdict"),
        ("FIX50SP1", "FIX50SP1.fixdict"),
        ("FIX50SP2", "FIX50SP2.fixdict"),
    ] {
        let path = Path::new(&manifest).join("dict-src/normalized").join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        emit_version(&mut code, name, &bytes);
    }

    let dest = Path::new(&out_dir).join("generated.rs");
    fs::write(&dest, code).unwrap_or_else(|e| panic!("write generated.rs: {e}"));
}
