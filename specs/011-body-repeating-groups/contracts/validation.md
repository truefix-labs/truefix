# Contract: `truefix-dict` — Validation of Structured Body Groups

Traces: spec.md FR-005, FR-006, FR-007; research.md R4.

## Behavior Contract

1. `DataDictionary::validate(&self, message: &Message, opts: &ValidationOptions) ->
   Result<(), ValidationError>`'s public signature is unchanged.
2. `validate_groups` keeps **both** of its existing code paths, not just one (implementation-time
   correction — see research.md R4's "Implementation-time correction"): a structured path
   (`validate_structured_group`, driven by `self.groups.keys()` against
   `message.header.members()`/`message.body.members()`) for `Member::Group` input, and the flat,
   position-scanning `validate_group` path for `Member::Field` input. `DataDictionary::validate` is
   a general-purpose public API — hand-built `Message`s (via `FieldMap::add_field` directly,
   bypassing `add_group`/`decode_with_groups`) and plainly-`decode()`d messages are both still
   flat, and deleting the flat path regressed real existing tests when tried.
3. `validate_structured_group` now also checks: the group's declared wire count
   (`Member::Group`'s `declared_count`, surfaced via the new `find_structured_group_with_next`
   helper reading `FieldMap::members()`) against its actual entry count
   (`RejectReason::IncorrectNumInGroupCount`); and each entry's own actual member order (walked via
   `.members()`, not `gdef.members`' declared order) against the group definition
   (`RejectReason::RepeatingGroupFieldsOutOfOrder`, gated on `validate_unordered_group_fields`) —
   closing a gap that predates this feature (feature 007 never added these checks to the structured
   path) but became a live regression the moment body-level structuring became routine on the
   production path: the AT suite dropped from 483/483 to 480/483 without this fix. A
   deeply-nested-delimiter-violation refinement (peeking a group's next sibling in its parent's
   member sequence, falling back to the *enclosing* group's own next sibling when this group was
   the last member of its entry) reconstructs QFJ's `FirstFieldInGroupIsDelimiter` reason code for
   that specific case too.
4. `present()` (used by `mdef.required`, `header_required_tags`, `trailer_required_tags` checks) is
   changed to recognize a `Member::Group` whose count tag matches as "present", using
   `FieldMap::members()` — fixing the gap where a dictionary-required group's own count tag, once
   structured, would otherwise be invisible to a plain `.get()`/`.contains()` lookup (research.md R4).
5. For any message that passes validation today (a well-formed message, its groups either flat or
   already `Member::Group`-structured), validation after this change produces an equivalent
   accept/reject outcome — this is a hard regression bar (spec.md FR-006, SC-002), verified by the
   full existing `truefix-dict`/`truefix-at` test suites passing unmodified (483/483 AT scenarios).
6. Per-entry field-value checks (type/enum/format via `check_group_field_value`) and required-member
   presence are unchanged in kind — this feature adds count/order detection to the structured path
   and the `present()` fix; it does not change how individual field values are checked.

## Non-Goals

- No change to `ValidationOptions`'s field set or defaults.
- No change to `ValidationError`/`RejectReason` variants (the count/order fix reuses
  `IncorrectNumInGroupCount`/`RepeatingGroupFieldsOutOfOrder`, both pre-existing).
- No rewrite of `validate()`'s top-level per-field loop — unaffected in scope (see research.md R4).

## Test Expectations

- Extend `crates/truefix-dict/tests/group_validation.rs`,
  `crates/truefix-dict/tests/group_recursive_validation.rs`, and
  `crates/truefix-dict/tests/header_group_validation.rs`-style coverage to add body-level-group
  cases exercising: correct multi-entry acceptance, missing required member rejection, out-of-order
  member rejection, count-mismatch rejection — mirroring the existing header/trailer-group test
  matrix but for body.
- A dedicated regression test for the `present()` fix: a dictionary declaring a group's count tag as
  message-level-required, a message carrying that group as a real `Member::Group` with ≥1 entry,
  asserting `validate()` no longer spuriously reports it missing.
- A full pre-feature vs. post-feature equivalence sweep: run every existing passing validation
  fixture/test through the updated code and confirm identical accept/reject outcomes (spec.md
  SC-002) — this can be the same existing test suite, just required to keep passing unmodified.
