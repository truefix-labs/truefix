# Contract: Dictionary and Codec Model Completeness (US9; FR-022–FR-031)

The largest, most structurally varied theme in this feature (research.md §17a–§17h) — nine
sub-contracts, one per FR, each touching a different part of `truefix-dict`'s model/parser/validator/
codegen or `truefix-core`'s codec/field-map. Constitution Principle IV (dual-track dictionary) applies
to every model-shape change here: both the runtime `DataDictionary` path and the `codegen.rs` dual-track
path MUST reflect each addition, verified by the existing content-hash equality test.

## Surface

```text
// truefix-dict::model
pub enum FieldType {
    // ...existing 16 variants...
    PriceOffset, LocalMktDate, DayOfMonth, UtcDate, Time, Currency, Exchange,
    MultipleValueString, MultipleStringValue, MultipleCharValue, Country,   // NEW (11, FR-022)
}
pub struct FieldDef {
    // ...existing fields...
    pub open_enum: bool,                          // NEW (FR-023), default false
    pub value_labels: BTreeMap<String, String>,   // NEW (FR-030), default empty
}
pub struct GroupDef {
    // ...existing fields...
    pub child: Option<Box<DataDictionary>>,   // NEW (FR-024), default None
}
pub struct MessageDef {
    // ...existing fields...
    pub field_order: Option<Vec<u32>>,   // NEW (FR-027), default None
}
pub struct DataDictionary {
    // ...existing fields...
    version_meta: Option<VersionMeta>,   // NEW (FR-028), default None
}
pub struct VersionMeta { pub major: u8, pub minor: u8, pub service_pack: Option<u8>, pub extension_pack: Option<u8> }  // NEW

// truefix-core::field_map
impl FieldMap {
    pub fn replace_group(&mut self, count_tag: u32, index: usize, entry: FieldMap);   // NEW (FR-025)
    pub fn remove_group(&mut self, count_tag: u32, index: usize);                       // NEW
    pub fn get_group(&self, count_tag: u32, index: usize) -> Option<&FieldMap>;         // NEW
}

// truefix-core::codec::encode / truefix-dict::codegen — both gain an optional field-order parameter
// (FR-027), keeping the dual-track byte-identical when a MessageDef declares one.

// truefix-dict::parser — normalized .fixdict grammar gains 4 additive directive modifiers:
//   `open` (field value list, FR-023), `ordered` (message block, FR-027),
//   `version-meta` (top-level, FR-028), a label-carrying value syntax (FR-030).
```

## Behaviour

1. **11 new `FieldType`s (FR-022)**: each validated per its own format rule in `value_ok`
   (`model.rs:72-84`'s existing match): `LocalMktDate`/`UtcDate` reuse `UtcDateOnly`'s check; `Time`
   reuses `UtcTimestamp`'s; `DayOfMonth` is a bounded integer (1-31); `Currency`/`Exchange`/`Country`
   are fixed-length alpha codes (format-only, matching QFJ's own converter behavior — no real
   currency/exchange/country code-list validation, out of scope); `PriceOffset` reuses the decimal
   check; `MultipleCharValue` is a space-separated list of single characters.
2. **Open-enum sentinel (FR-023)**: a `FieldDef` with `open_enum: true` skips the enum-membership check
   entirely (`validate.rs:125-133`'s existing check short-circuits to pass). `MultipleValueString`/
   `MultipleStringValue` fields that are also open combine both: space-split, then only enum-check
   tokens when not open.
3. **Per-group child dictionaries (FR-024)**: a `GroupDef` with `child: Some(...)` recurses `validate()`
   into each group instance's `FieldMap` using that child dictionary — reusing the existing top-level
   `validate()` entry point, not a parallel group-specific validator.
4. **Group mutation API (FR-025)**: `replace_group`/`remove_group`/`get_group` operate on the existing
   `Member::Group.entries: Vec<FieldMap>` storage — no new storage, straightforward index operations
   (out-of-bounds is a typed error, not a panic, per Constitution Principle I).
5. **Header/trailer repeating groups (FR-026)**: header/trailer decode gains the same
   `GroupSpec`-driven `build_group` path body-decoding already has, parameterized by a header/trailer
   `GroupSpec` set from whichever dictionary declares one (e.g. `NoHops`/tag 504) — dormant until a
   dictionary source actually declares such a group.
6. **Custom field order (FR-027)**: `Message::encode` emits fields present in `MessageDef.field_order`
   in that order (dictionary-unlisted/UDF fields appended after, matching QFJ's "unspecified fields
   last" semantics) when present; `Vec<Member>` insertion order otherwise (unchanged default). The
   dual-track codegen mirrors this for typed-struct emission.
7. **Version metadata + match validation (FR-028/FR-029)**: `DataDictionary.version_meta`, when
   present, is compared against an inbound/outbound message's `BeginString`/`ApplVerID` by a new
   `validate.rs` check; a mismatch is a session-level reject (matching QFJ's treatment). When absent
   (any dictionary predating this feature, or one that doesn't declare it), the check is a no-op — no
   behavior change for existing dictionaries.
8. **Value→label lookup (FR-030)**: `FieldDef.value_labels` (or an equivalent shape — see
   data-model.md's note on the replace-vs-parallel-field choice deferred to `/speckit-tasks`) exposes a
   human-readable label for values that have one declared in the dictionary source.
9. **Bundled dictionary content parity with QuickFIX/J (FR-031)**: for each of TrueFix's 9 bundled FIX
   versions with a QuickFIX/J XML source to diff against (FIX40 through FIX50SP2 — `FIX.Latest` is
   out of this FR's scope, sourced from Orchestra via a different mechanism per
   `docs/todo-gap-analysis.md`'s TODO-10), the normalized `.fixdict` source gains every field/message
   QuickFIX/J's own bundled dictionary defines for that version that TrueFix's current subset is
   missing.

## No breaking changes

- Every `FieldType`/`FieldDef`/`GroupDef`/`MessageDef`/`DataDictionary` growth is additive
  (new variant/field, existing ones untouched) — a dictionary source file using none of the new
  grammar directives parses to an identical in-memory shape as before this feature.
- `replace_group`/`remove_group`/`get_group` are new methods; `add_group` (the only pre-existing group
  mutation) is unchanged.
- `Message::encode`'s new field-order parameter is additive (an `Option`, `None` = today's behavior).

## Acceptance (maps to spec US9 scenarios)

- All 11 field types validate per-format, both well-formed and malformed examples (SC-014). ✔
- An open-enum field accepts an out-of-list value, matching QuickFIX/J (SC-015). ✔
- A nested group's fields validate against child-dictionary rules, not a flat tag list (SC-016). ✔
- Group mutation (replace/remove/get-by-index) behaves correctly across a group's lifecycle (SC-017). ✔
- A header/trailer repeating group round-trips through decode/validate/encode (SC-018). ✔
- A custom field order matches byte-for-byte on emission (SC-019). ✔
- A BeginString/dictionary-version mismatch is flagged once version metadata is present (SC-020). ✔
- A field value with a defined label resolves via lookup (SC-021). ✔
- Per-version coverage matches QuickFIX/J's own bundled dictionary, field-by-field and
  message-by-message (SC-023). ✔

## Test hooks

- `truefix-dict`: unit tests per new `FieldType` (format-valid + format-invalid examples), open-enum
  acceptance test, nested-group-with-child-dictionary validation test, header/trailer group decode
  test, custom-field-order encode test, version-mismatch rejection test (+ no-op-when-absent test, per
  spec.md's Edge Case), value-label lookup test — extending the existing `toggles.rs`/
  `rejection_layers.rs`/`versions.rs` table-driven pattern.
- `truefix-core`: `field_map.rs` unit tests for `replace_group`/`remove_group`/`get_group` (extends the
  existing `groups.rs` test file).
- **Dual-track parity**: every model-shape test above MUST also assert the codegen path (`codegen.rs`)
  produces a byte-identical typed struct/enum for the same dictionary source, extending the existing
  content-hash-equality test pattern (Constitution Principle IV, non-negotiable for this theme).
- `truefix-dict`: a new coverage-diff test (or a `--features dict-tooling`-gated CLI-driven check,
  matching this project's existing `dict-tooling` boundary for XML-reading tools) comparing each
  bundled `.fixdict`'s field/message set against the corresponding `thrdpty/quickfixj` XML source,
  asserting no gap remains (FR-031/SC-023).
