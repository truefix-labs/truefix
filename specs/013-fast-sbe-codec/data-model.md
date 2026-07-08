# Data Model: FAST/SBE Binary Protocol Codec

Entities are grouped by crate. Per research.md Decision 1, this feature does **not** add a new
intermediate-representation type — `truefix_core::Message`/`FieldMap`/`Group`/`Field` (all existing,
unchanged) serve that role directly. Everything below is genuinely new.

## `truefix-dict` (new modules `fast_template.rs`, `sbe_schema.rs`, both `dict-tooling`-gated)

### `FastTemplateSet` (new, public)

```rust
pub struct FastTemplateSet {
    templates: BTreeMap<TemplateId, FastTemplate>,
}

pub type TemplateId = u32;

pub struct FastTemplate {
    pub id: TemplateId,
    pub name: String,
    pub fields: Vec<FastFieldDef>,
}

pub struct FastFieldDef {
    pub tag: u32,
    pub name: String,
    pub data_type: FastDataType,       // e.g. UInt32, Int64, AsciiString, Decimal, ...
    pub operator: FastOperator,
    pub nullable: bool,
    pub group: Option<Box<FastGroupDef>>, // nested repeating group, recursive
}

pub enum FastOperator {
    Copy,
    Increment,
    Delta,
    Default { value: Option<FastFieldValue> },
    None,
}

pub struct FastGroupDef {
    pub count_field: FastFieldDef,       // the NumInGroup-equivalent field
    pub members: Vec<FastFieldDef>,
}
```

**Relationships**: A `FastTemplateSet` is loaded once per session (FR-012: a *set*, not a single
template) via `load_fast_templates_from_file` (one XML file MAY declare multiple `<template>`
elements, each becoming one `FastTemplate` keyed by its `id`). `FastGroupDef` nests recursively,
mirroring FAST's own nested-group support.

**Validation rules** (from FR-001/FR-012 and Edge Cases):
- Every `FastTemplate.id` MUST be unique within a `FastTemplateSet`; a duplicate ID is a
  `FastTemplateError` at parse time, not silently overwritten.
- A field declaring an operator/type this feature doesn't implement MUST fail parsing with a named
  `FastTemplateError` variant (not fail later at encode/decode time) — see Edge Cases in spec.md.

### `FastTemplateError` (new, public, `thiserror`)

Shape mirrors `orchestra::OrchestraError` (research.md Decision 5): `Xml { pos, reason }`,
`MissingAttr { elem, attr }`, `UnknownOperator { field, operator }`, `UnknownDataType { field,
type_name }`, `DuplicateTemplateId { id }`.

### `SbeSchemaSet` (new, public)

```rust
pub struct SbeSchemaSet {
    schema_id: Option<u32>,
    messages: BTreeMap<TemplateId, SbeMessageSchema>,
}

pub struct SbeMessageSchema {
    pub template_id: TemplateId,
    pub name: String,
    pub block_length: u32,
    pub fields: Vec<SbeFieldDef>,        // fixed-offset fields, in declared order
    pub groups: Vec<SbeGroupDef>,
    pub var_data: Vec<SbeVarDataDef>,
}

pub struct SbeFieldDef {
    pub name: String,
    pub offset: u32,
    pub data_type: SbeDataType,          // e.g. UInt32, Int64, Decimal(precision), Char, ...
    pub byte_order: ByteOrder,           // LittleEndian (SBE default) / BigEndian
}

pub struct SbeGroupDef {
    pub name: String,
    pub block_length: u32,
    pub dimension_offset: u32,           // offset of the blockLength+numGroups header
    pub fields: Vec<SbeFieldDef>,
    pub nested_groups: Vec<SbeGroupDef>, // recursive
    pub var_data: Vec<SbeVarDataDef>,
}

pub struct SbeVarDataDef {
    pub name: String,
    pub length_field_size: u8,           // typically 1, 2, or 4 bytes
}
```

**Relationships**: A `SbeSchemaSet` is loaded once per session (FR-012), one XML schema file
declaring multiple `<message>` elements, each keyed by its `templateId` (and the schema's own
optional `schemaId`, per Edge Cases' "`templateId`/`schemaId`" phrasing).

**Validation rules** (from FR-004/FR-012 and Edge Cases):
- Every `SbeMessageSchema.template_id` MUST be unique within a `SbeSchemaSet`.
- Fixed-offset fields' declared offsets MUST be internally consistent (no declared overlap) —
  checked at parse time, not deferred to decode time.

### `SbeSchemaError` (new, public, `thiserror`)

Same shape family as `FastTemplateError`: `Xml { pos, reason }`, `MissingAttr`, `UnknownDataType`,
`DuplicateTemplateId`, `OverlappingOffset { field, offset }`.

## `truefix-binary` (new crate)

### `BinaryCodec` (new, public trait)

```rust
pub trait BinaryCodec {
    type Error;
    fn encode(&self, message: &Message, template_id: TemplateId) -> Result<Vec<u8>, Self::Error>;
    fn decode(&self, bytes: &[u8]) -> Result<(Message, TemplateId), Self::Error>;
}
```

Implemented by `FastCodec` and `SbeCodec`. `Message` here is `truefix_core::Message`, unchanged
(research.md Decision 1) — there is no `BinaryCodec`-specific message type.

### `FastCodec` / `EncodingContext` (new, public)

```rust
pub struct FastCodec {
    templates: FastTemplateSet,
    context: RefCell<BTreeMap<TemplateId, EncodingContext>>, // one context per loaded template
}

pub struct EncodingContext {
    previous_values: BTreeMap<u32, FastFieldValue>, // last transmitted value per field tag,
                                                      // for copy/delta/increment recovery
}
```

**Relationships**: `FastCodec` owns one `EncodingContext` per `TemplateId` in its `FastTemplateSet`
(FR-012 — contexts are independent per template, so switching template ID mid-session, e.g. across
message types, does not corrupt an unrelated template's PMap state). Lifecycle: created empty when
the codec is constructed for a session; reset (cleared) when the underlying connection is
re-established after a disconnect (spec Edge Cases) — this reset MUST emit the FR-013 structured
tracing event.

**Validation/error rules** (FR-002/FR-003/FR-009/FR-012):
- Decoding a presence-map bit for a field with no prior context value (first message on a fresh/
  reset context) and an operator other than `Default`/`None` MUST produce a typed error, not a
  panic or an arbitrary default.
- A `template_id` absent from `templates` MUST produce `FastCodecError::UnknownTemplateId`.

### `SbeCodec` / `Flyweight` (new, public/internal)

```rust
pub struct SbeCodec {
    schemas: SbeSchemaSet,
}

// internal: zero-copy read view, built on safe slice + byte-order operations (no raw pointers,
// per unsafe_code = "forbid")
struct Flyweight<'a> {
    buf: &'a [u8],
    schema: &'a SbeMessageSchema,
}
```

`SbeCodec` is stateless across messages (SBE has no PMap/context concept — every message is
self-contained), unlike `FastCodec`.

**Validation/error rules** (FR-005/FR-009/FR-012):
- A `templateId` absent from `schemas` MUST produce `SbeCodecError::UnknownTemplateId`.
- A declared `blockLength`/`numGroups`/VarData length that would read past the end of `buf` MUST
  produce a typed error before any read is attempted (bounds-checked, not caught via panic).

### `FastCodecError` / `SbeCodecError` (new, public, `thiserror`)

Per research.md Decision 5: `Truncated { offset }`, `InvalidStopBit { offset }`,
`PresenceMapMismatch { field, offset }`, `UnknownTemplateId { id }` (FAST); `Truncated { offset }`,
`BlockLengthMismatch { declared, actual }`, `UnknownTemplateId { id }`, `VarDataLengthMismatch
{ field, declared, actual }` (SBE).

### `ir` module (new, public functions, no new type — research.md Decision 1)

```rust
pub fn decode_into_message(
    codec: &impl BinaryCodec,
    bytes: &[u8],
) -> Result<Message, <impl BinaryCodec as BinaryCodec>::Error>; // conceptual signature

pub fn encode_from_message(
    codec: &impl BinaryCodec,
    message: &Message,
    template_id: TemplateId,
) -> Result<Vec<u8>, <impl BinaryCodec as BinaryCodec>::Error>; // conceptual signature
```

Builds a `Message`'s `header`/`body`/`trailer` `FieldMap`s using `Field::new`/typed constructors +
`FieldMap::set`/`add_group` while decoding; reads them back via `FieldMap::members()` (which sees
`Group` entries, unlike `.fields()`) while encoding. A `truefix_core::Message` construct with no
FAST/SBE-side representation (per spec Acceptance Scenario US3.5) MUST surface a named typed error
from this module, not silently drop the unrepresentable content.

## `truefix-session` / `truefix-config`

### `Protocol` (new, public enum on `SessionConfig`)

```rust
pub enum Protocol {
    Soh,   // default — today's existing, unchanged behavior
    Fast,
    Sbe,
}
```

Added as `pub protocol: Protocol` on `SessionConfig` (mirrors the existing `TimeStampPrecision`
field's shape). A `Protocol::Fast`/`Protocol::Sbe` session additionally carries a resolved
`FastTemplateSet`/`SbeSchemaSet` (loaded via `truefix-config::builder`'s bundled-name-then-
filesystem-fallback pattern, research.md Decision 3) — modeled as an `Option` field alongside
`protocol`, populated only for the matching variant, `None` for `Protocol::Soh`.

**Validation rules** (FR-007): Two sessions with different `Protocol`s attempting to connect MUST
fail Logon with a named typed error (research.md Decision 4 — primarily surfaced from
`truefix-transport`'s decode stage, not silently misinterpreted as a different failure class).
