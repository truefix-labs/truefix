# Contract: FIXT 1.1 and Dictionary/Validation Correctness (US1, US2, US3)

**Requirements**: FR-028–FR-043 (US1/US2), FR-064–FR-075 (US3 tooling)
**Research**: research.md §R1.6–R1.7, R1.9, R1.12–R1.13, R1.16, R1.21

## FIXT 1.1 transport header tags (FR-028)

**Contract**: `tags::is_header` recognizes `1128`/`1129`/`1130`/`1156`/`1351`-`1355`, so
`ApplVerID` and its siblings route into `message.header` on decode, not `message.body`.

**Protocol-behavioral**: yes — this is what makes `FR-029`/`FR-030`'s `ApplVerID`-driven logic
reachable at all. **AT scenario required**: a FIX 5.x message decode/round-trip test confirming
header placement, plus downstream verification that `validate_app`'s `msg.header.get(APPL_VER_ID)`
read (already written assuming this fix) actually succeeds end-to-end.

## FIXT group-aware decode via `fixt_dictionaries` (FR-029)

**Contract**: Transport-layer message classification uses group-aware decode
(`decode_with_groups`/`HeaderTrailerGroupsOnly`) when FIXT dual-dictionaries are configured, even
when no plain single-dictionary `validator` is set.

**Protocol-behavioral**: yes — header/trailer group structuring (e.g. `NoHops`) under FIXT 1.1.
**AT scenario required** if the AT harness can configure a FIXT dual-dictionary session; otherwise
a `truefix-transport` integration test covering the same `classify_buffered` code path.

## FIX 5.x `ApplVerID`-aware `crack_*` dispatch (FR-030)

**Contract**: `crack_fix50`/`crack_fix50sp1`/`crack_fix50sp2` dispatch based on both `BeginString`
and `ApplVerID`, not `BeginString` alone.

**Protocol-behavioral**: no (compile-time/codegen dispatch correctness, not a wire contract) — a
codegen fixture test exercising all three dispatchers against messages carrying each of the three
`ApplVerID` values is sufficient.

## `MultipleCharValue` per-token validation (FR-031)

**Contract**: `FieldDef::allows()` validates a `MultipleCharValue` field's wire value per
space-separated token, consistent with `value_ok()`.

**Protocol-behavioral**: no — dictionary-level unit test (`allows("A B")` for a field enumerating
`"A"`/`"B"` returns `true`).

## Validation config-key wiring (FR-032)

**Contract**: `ValidateFieldsHaveValues`/`ValidateUnorderedGroupFields`/
`ValidateUserDefinedFields`/`AllowUnknownMsgFields`/`FirstFieldInGroupIsDelimiter` actually change
`ValidationOptions` behavior when set in a `.cfg`; `ValidateSequenceNumbers`/`RejectInvalidMessage`
are downgraded to `Recognized` in the key registry unless a real field/effect is added.

**Protocol-behavioral**: no — config-resolution unit tests (`.cfg` with each key set produces the
expected `ValidationOptions`) plus the existing key-registry-consistency test (every `Impl` key has
a real wired effect) this crate already has, extended to cover these keys.

## UDF validation full-skip ordering (FR-033)

**Contract**: `validate_user_defined_fields=false` fully exempts UDFs (no empty-value or
repeated-tag rejection either), matching QuickFIX/J.

**Protocol-behavioral**: no — dictionary-level unit test (empty/repeated UDF passes validation with
the option off, still rejected with it on).

## Runtime validation gaps (US2) — grouped, dictionary/validation-level unit tests

- **FR-034**: header repeating groups (`NoHops`) validated like body groups.
- **FR-035**: `MonthYear`/`LocalMktDate` format-validated.
- **FR-036**: tag `0` rejected on decode.
- **FR-037**: `NoXxx` count/actual-entry-count mismatch not silently corrected without signal.
- **FR-038**: `DayOfMonth` range-checked 1-31.
- **FR-039**: `Length`/`SeqNum`/`NumInGroup` reject negative values in `value_ok`.
- **FR-040**: `NoXxx` count type-checked consistently on both flat- and group-aware-decode paths.
- **FR-041**: empty group-member values rejected (respecting the reverse-route-tag exemption).
- **FR-042**: `FixtDictionaries::application_for` treats empty-string `ApplVerID` as absent.
- **FR-043**: cyclic repeating-group definitions guarded against unbounded recursion in
  `decode_with_groups` — **also a robustness/DoS-adjacent item**: a fuzz-style or crafted-dictionary
  test proving bounded behavior (typed error, not stack growth) is required, not just a happy-path
  unit test.

## Dictionary/codegen tooling correctness (US3, FR-064–FR-075)

**Contract group**: affects only dictionary regeneration (`generate-dict`/`generate-code`) and
`build.rs`'s codegen output — never the shipped `.fixdict` files or any currently-running session
(Constitution Principle IV: both tracks stay in sync; these fixes make codegen match what the
runtime path already does, not a new divergence).

**Protocol-behavioral**: no — every item here is verified via the codegen/converter/parser test
suites (`truefix-dict`'s existing `dict-tooling`-feature-gated tests), never by a change to a
shipped `.fixdict`'s content (spec.md SC-005). No AT scenario applies to this group.

- **FR-064**: `fix-repository` converter excludes count field from group `members`.
- **FR-065**: `orchestra.rs` `map_type` handles the 10 tokens `fix_repository::map_type` already
  does, plus correct `localmktdate` mapping.
- **FR-066**: `build.rs` declares `cargo:rerun-if-changed=src/codegen.rs`.
- **FR-067**: `dict/parser.rs` errors on duplicate tag/name/message-type definitions.
- **FR-068**: `strip_comment` doesn't truncate a `#`-containing STRING enum value.
- **FR-069**: `parse_fix_begin_string` distinguishes FIX 5.0/5.0SP1/5.0SP2.
- **FR-070**: circular `${var}` config references reported as an error.
- **FR-071**: `UnresolvedVariable` reports the correct line number post-merge.
- **FR-072**: `DataDictionary::extend()` detects `field_by_name` collisions, recomputes
  `member_tags`.
- **FR-073**: codegen `parse_dict` errors on unknown directives/malformed lines (matching runtime
  `parser::parse`).
- **FR-074**: codegen enum-variant sanitization detects/resolves label collisions.
- **FR-075**: `generate_code` CLI's dual-track hash uses consistent byte semantics (both lossy or
  both raw) between the runtime-`parse` and `codegen::generate` paths.
