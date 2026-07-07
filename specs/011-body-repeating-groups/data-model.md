# Data Model: Body-Level Repeating Group Decoding & Vendor Dictionary Support

This feature does not introduce new persisted or configuration entities. It extends existing
in-memory types (`truefix-core`'s `FieldMap`/`Member`, `truefix-dict`'s `DataDictionary`/
`FixtDictionaries`) and adds one new dictionary-source representation (vendor XML). Entities below
are grouped by crate.

## `truefix-core`

### `Member` (existing, `pub(crate)`) — unchanged shape, newly reachable in more places

```rust
pub(crate) enum Member {
    Field(Field),
    Group { count_tag: u32, entries: Vec<FieldMap>, declared_count: Option<i64> },
}
```

No change to this type. What changes is *where* `Member::Group` values get created: today only for
header/trailer count tags recognized by `truefix_core::tags::is_header`/`is_trailer`; after this
feature, for any count tag the active dictionary declares, in header, body, or trailer alike (single
dictionary), or in the resolved application dictionary's scope (FIXT).

### `MemberRef<'a>` (new, public)

```rust
pub enum MemberRef<'a> {
    Field(&'a Field),
    Group {
        count_tag: u32,
        entries: &'a [FieldMap],
        declared_count: Option<i64>,
    },
}
```

A borrowed, read-only view mirroring `Member`, returned by the new `FieldMap::members()` iterator.
Exists so external callers (and `truefix-dict`'s validator) can distinguish plain fields from group
occurrences without `Member` itself becoming public API surface.

### `FieldMap` (existing) — one new method, one rename

- `pub fn members(&self) -> impl Iterator<Item = MemberRef<'_>>` — **new**. Yields every top-level
  member (field or group) in wire order.
- `pub(crate) fn raw_members(&self) -> &[Member]` — **renamed** from the previous `pub(crate) fn
  members(&self) -> &[Member]` (its two call sites, both in `codec/encode.rs`, are updated
  accordingly). No public-API change: this was never externally visible.
- `.fields()`, `.get()`, `.contains()`, `.group()`, `.get_group()`, `.replace_group()`,
  `.remove_group()`, `.add_field()`, `.add_group()`, `.set()` — **unchanged**.

### Body-group restructuring primitive (new, internal to `truefix-core`)

A function (exact name/signature is an implementation detail, e.g.
`pub(crate) fn restructure_groups(map: &mut FieldMap, spec: &dyn GroupSpec)` or an equivalent
public-but-narrow helper `truefix-session` can call) that takes an already-decoded, flat `FieldMap`
and re-groups `Member::Field` runs into `Member::Group` entries wherever `spec.group_of` matches a
tag — the same "count tag + delimiter + ordered members, recursing for nested groups" algorithm
`decode_section_with_groups` already implements over raw tokens (`codec/decode.rs`), applied instead
to an in-memory field sequence. Used by the FIXT post-decode restructuring step (R3 in research.md).
Whether this is exposed as a public API (for `truefix-session` to call across the crate boundary) or
kept crate-internal with a thin public wrapper is a task-level decision; either way it must not
duplicate `decode_section_with_groups`'s boundary-detection logic, per Constitution Principle IV's
single-source-of-truth spirit.

## `truefix-dict`

No new persisted fields on `DataDictionary`, `GroupDef`, `MessageDef`, `ComponentDef`, or `FieldDef`.
Relevant existing fields, unchanged:

- `DataDictionary.groups: BTreeMap<u32, GroupDef>` — already covers body-level groups (used today by
  `impl GroupSpec for DataDictionary`, previously only reachable via `HeaderTrailerGroupsOnly`'s
  header/trailer filter).
- `GroupDef { count_tag, delimiter, members: Vec<u32>, required: Vec<u32>, child: Option<Box<DataDictionary>> }`
  — unchanged; `required` (feature 007, BUG-54) already drives `validate_structured_group`'s
  per-entry required-field check.

### Validation-internal changes (no type changes, behavior changes — see research.md R4)

- `validate_group` (flat, position-scanning) — **removed**.
- `validate_groups`'s flat-walk branch (the `Vec<&Field>` position loop) — **removed**; the
  `self.groups.keys()`-driven structured loop (calling `validate_structured_group`) becomes the sole
  path.
- `present()` — **behavior change only**: now also treats a `Member::Group` whose count tag matches
  as present, using `FieldMap::members()` instead of `.get()`/`.contains()`.

### `vendor_xml` module (new, `dict-tooling`-gated)

```rust
pub fn convert(xml: &str) -> Result<String, VendorXmlError>
```

Parses the nested `<fix><header>/<messages>/<message>/<trailer>/<components>/<fields>` shape into
`.fixdict` line-oriented text (same output contract as `orchestra::convert`/`fix_repository::convert`),
including a synthesized `version FIX.M.N[SPk]` directive derived from the root element's version
attributes (research.md R6). `VendorXmlError` is a new, named error enum (Constitution Principle I:
typed errors), covering at minimum: malformed XML, a root element missing recognizable version
attributes, and a `<component>`/`<group>` reference to an undefined name.

### `load_from_file` (existing) — behavior change only

Gains a `dict-tooling`-gated content-sniffing branch: text beginning with `<?xml`/`<fix ` is passed
through `vendor_xml::convert` before `parse()`; everything else is parsed as `.fixdict` text exactly
as today. `DictLoadError`'s existing `Io`/`Parse` variants are reused — a vendor XML conversion
failure surfaces through the same `Parse` variant (wrapping a `ParseError`) or, if a dedicated
variant is clearer for callers, a new `DictLoadError` variant wrapping `VendorXmlError` is added
(task-level decision — either way, a typed, non-silent error per FR-016/SC-005).

## `truefix-session`

### `Session` (existing) — no new persisted fields; one new `pub` method

The FIXT post-decode restructuring reads `Session`'s already-existing `negotiated_appl_ver_id` and
`fixt_validator: Option<(FixtDictionaries, ValidationOptions)>` fields (the same ones
`validate_app` already reads) to resolve the application dictionary for an inbound message, then
calls the new `truefix-core` restructuring primitive on that message's `body`. No new field is
required on `Session` itself.

**Implementation-time correction**: this is exposed as a new `pub fn
restructure_fixt_application_body(&self, msg: &mut Message)` on `Session` — `pub`, not a private
method called from within `on_received` as first designed — because `Application::from_admin`/
`from_app` are invoked directly from `truefix-transport`'s `handle_inbound`, before
`dispatch`/`Session::handle`/`on_received` ever runs (there is no `Action::Deliver`). The method
must be called from `handle_inbound` itself for the application to ever see its effect; reuses
`validate_app`'s existing `ApplVerID` resolution precedence (own header tag 1128, else negotiated
tag 1137, else `FixtDictionaries`'s default) verbatim.

## `truefix-transport`

### `Services` (existing) — no field changes

`Services.validator: Option<(DataDictionary, ValidationOptions)>` and
`Services.fixt_dictionaries: Option<(FixtDictionaries, ValidationOptions)>` are unchanged in shape.
`HeaderTrailerGroupsOnly` (a private wrapper type, not a field) is deleted.

## Relationships Summary

```text
DataDictionary ──impl GroupSpec──> classify_buffered (decode_with_groups)   [single-dict case, R2]
FixtDictionaries.transport() ──impl GroupSpec──> classify_buffered          [FIXT session-layer, R2]
FixtDictionaries.application_for(appl_ver_id) ──impl GroupSpec──> Session's
    post-decode restructuring step (new)                                   [FIXT app-layer, R3]

FieldMap::members() (new) ──used by──> DataDictionary::present() (fixed, R4)
FieldMap::members() (new) ──used by──> external callers needing structure-aware iteration

vendor_xml::convert(xml) -> .fixdict text ──fed to──> crate::parse() ──> DataDictionary
    (same pattern as orchestra::convert / fix_repository::convert)
```
