# Contract: `truefix-core` — `FieldMap::members()` / `MemberRef`

Traces: spec.md FR-001, FR-002; research.md R1.

## API

```rust
pub enum MemberRef<'a> {
    Field(&'a Field),
    Group {
        count_tag: u32,
        entries: &'a [FieldMap],
        declared_count: Option<i64>,
    },
}

impl FieldMap {
    pub fn members(&self) -> impl Iterator<Item = MemberRef<'_>>;
}
```

## Behavior Contract

1. `members()` yields every top-level member of the `FieldMap` in wire order — both plain fields and
   repeating groups — with no filtering. A `FieldMap` with no groups yields the same tags, in the
   same order, as `.fields()` (wrapped in `MemberRef::Field`).
2. A `MemberRef::Group`'s `entries` is the group's full ordered entry list; each entry is itself a
   `FieldMap` whose own `members()` recurses correctly into any nested group.
3. `declared_count` mirrors `Group::declared_count` (NEW-22, feature 009): `Some(n)` only when this
   group was decoded from the wire and its declared `NoXxx` value didn't match `entries.len()`;
   `None` otherwise (including for any group built by application code, which always has a consistent
   count).
4. `.fields()` is unchanged: it continues to yield only `Member::Field` entries, skipping groups,
   exactly as before this feature (FR-002). No existing caller of `.fields()` observes a behavior
   change.
5. The crate-private `pub(crate) fn members(&self) -> &[Member]` is renamed to `raw_members()`; its
   two call sites in `codec/encode.rs` are updated. This is a non-observable, crate-internal change.

## Body-Restructuring Primitive (internal, enables research.md R3)

A `truefix-core` function that takes an already-decoded, flat `FieldMap` and a `&dyn GroupSpec`, and
restructures `Member::Field` runs into `Member::Group` entries wherever a count tag matches —
algorithmically identical to `decode_section_with_groups`'s token-based grouping
(`codec/decode.rs`), applied to an in-memory field sequence instead of freshly tokenized bytes.

1. Given a flat `FieldMap` whose fields include a group's count tag followed by the correct number
   of delimiter-led entries (in the shape `spec.group_of` describes), restructuring produces the same
   `Member::Group` result `decode_with_groups` would have produced had `spec` been used at original
   decode time — same entry count, same per-entry field content, same nesting for nested groups.
2. Restructuring is idempotent and non-destructive for content `spec` does not recognize: a tag with
   no matching `group_of` entry remains an ordinary `Member::Field`, byte-for-byte content preserved.
3. Restructuring never panics on malformed/truncated group content (e.g. a declared count tag with
   fewer members present than the group definition expects) — it degrades to leaving the
   under-populated tail as plain fields (mirroring `decode_section_with_groups`'s own tolerance),
   deferring correctness enforcement to `DataDictionary::validate` (see validation.md), not to this
   primitive.
4. Nesting depth is bounded consistently with `decode_section_with_groups`'s existing
   `MAX_GROUP_NESTING_DEPTH` (32) — restructuring must not introduce an unbounded-recursion path the
   original decode doesn't already have.

## Test Expectations

- Table-driven unit tests in `truefix-core` covering: flat-only maps, single-level groups, nested
  groups, a group with a declared/actual count mismatch (`declared_count` surfaced correctly), and
  confirming `.fields()`'s output is unchanged before/after this feature for the same inputs.
- A dedicated test exercising the restructuring primitive directly: same input tokens fed once
  through `decode_with_groups` (bytes → structured `Message`) and once through flat `decode` +
  restructuring (bytes → flat `Message` → restructured `body`), asserting both produce
  structurally-equal results.
