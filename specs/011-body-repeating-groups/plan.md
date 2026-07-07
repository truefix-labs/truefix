# Implementation Plan: Body-Level Repeating Group Decoding & Vendor Dictionary Support

**Branch**: `011-body-repeating-groups` | **Date**: 2026-07-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/011-body-repeating-groups/spec.md`

## Summary

Today `classify_buffered` (`truefix-transport`) always decodes body content through
`HeaderTrailerGroupsOnly`, a `GroupSpec` adapter that only reports group definitions for
header/trailer count tags — deliberately scoped that way in feature 006 (GAP-26/FR-032) because
`validate_groups`'s body-group walk and codegen's group accessors both depended on `.fields()`'s
flat, group-skipping view. Reading the current `validate.rs`, that scoping is now **more
conservative than necessary**: feature 007 (BUG-55/FR-034) already added `validate_structured_group`,
a fully working structured-group validator that recurses over `Member::Group` entries directly. The
remaining flat, position-scanning `validate_group`/`validate_groups` walk exists only because body
content was never actually structured in production — once it is, that walk simply stops matching
anything (all body groups are already handled by `validate_structured_group`) and becomes dead code
to remove, not a validator to rewrite from scratch.

The plan has four parts, in dependency order:

- **A**: Add a public `FieldMap::members()` iterator (yielding both fields and groups) so callers —
  and one real internal gap this work surfaces — can see structured content that `.fields()` hides.
  Renames the existing crate-private `members()` (used only by the encoder) to avoid a name clash.
- **B**: Drop `HeaderTrailerGroupsOnly`; `classify_buffered` decodes header/body/trailer against the
  real `DataDictionary`/`FixtDictionaries::transport()` directly, for both single- and dual-dictionary
  sessions. For FIXT 1.1 application-layer messages, the *processing loop* (which alone knows the
  session's negotiated `ApplVerID`) re-structures the already-decoded flat body against the resolved
  application dictionary right before validation/delivery — the reader task itself never needs to
  learn the negotiated `ApplVerID`.
- **C**: Delete the now-dead flat `validate_group`/`validate_groups` position-scanning code, keep
  `validate_structured_group`, and fix the one real gap structuring exposes: `present()` (used for
  `mdef.required`/header/trailer-required checks) uses `FieldMap::get()`/`contains()`, which return
  `false` for a required tag that is itself a group's count tag once that group is `Member::Group`-
  structured (a plain-field lookup never matches a group member). This is fixed with a group-aware
  presence check built on the new `members()` iterator.
- **D**: A new `dict-tooling`-gated `vendor_xml` module (parallel to `orchestra`/`fix_repository`)
  converts vendor-published, QuickFIX-dialect-shaped dictionary XML (e.g. Binance's
  `spot-fix-oe.xml`/`spot-fix-md.xml`) into the engine's native `.fixdict` text, reusing
  `crate::parse()` — same pattern as the two existing converters. `load_from_file` sniffs file
  content and calls it automatically when `dict-tooling` is enabled; the CLI gets a
  `--format vendor-xml` option alongside the existing two.

## Technical Context

**Language/Version**: Rust edition 2024, workspace MSRV `1.96`, `unsafe_code = "forbid"`.

**Primary Dependencies**: No new external dependencies. `truefix-core`, `truefix-dict`,
`truefix-transport`, `truefix-session` (existing workspace crates only). Part D reuses the existing
`quick-xml` dependency, already present but gated behind the `dict-tooling` feature
(`crates/truefix-dict/Cargo.toml`) for `orchestra`/`fix_repository` — Apache-2.0 OR MIT compatible,
already vetted.

**Storage**: N/A (no persistence changes).

**Testing**: `cargo test` per crate. Table-driven unit tests in `truefix-core` (member iteration,
group restructuring), `truefix-dict` (validation, vendor XML parsing/round-trip, CLI), and
integration/process-level tests in `truefix-transport`/`truefix-session` for FIXT dual-dictionary
receive behavior (acceptor and initiator both, per Constitution Principle VI).

**Target Platform**: Server-side Rust library/engine, existing workspace targets.

**Project Type**: Cargo workspace library/engine. No new crates.

**Performance Goals**: No new performance target. Must not regress the documented anti-allocation
discipline already applied to `classify_buffered`'s hot path (NEW-133, feature 006): passing the
real dictionary instead of `HeaderTrailerGroupsOnly` must not add allocations beyond what
`decode_with_groups` already performs for header/trailer groups today.

**Constraints**: Critical-path code (decode, validate, the new FIXT post-decode restructuring step)
must avoid panic/unwrap/expect/unreachable/unchecked indexing (Constitution Principle I). Vendor XML
parsing runs at dictionary-load time (config parsing / CLI), not on the message hot path, so it is
held to normal (not hot-path) error-handling discipline — but still no panics, since a malformed
operator-supplied file must produce a typed `DictLoadError`/converter error, not a crash.

**Scale/Scope**: 3 user stories (P1 core fix, P2 FIXT, P3 vendor XML), touching 4 crates
(`truefix-core`, `truefix-dict`, `truefix-transport`, `truefix-session`) plus their CLI
(`truefix-dict` binary) and config loader (`truefix-config`).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. Production-Ready First | New public API (`members()`) is documented; critical-path code (decode, restructure, validate) stays panic-free and typed-error; group/FIXT-fallback events are the kind of thing already covered by existing structured logging/metrics hooks, no new observability gap introduced. | PASS |
| II. Protocol Correctness First | Behavior is grounded in the FIX repeating-group specification (count tag + delimiter + ordered members) already implemented by `decode_with_groups`/`validate_structured_group`; this feature extends *where* that existing, spec-correct machinery applies (body, FIXT application layer) rather than inventing new protocol semantics. | PASS |
| III. License & Provenance | `vendor_xml` converts Binance's own independently-licensed published XML using the same "parse the published shape, emit `.fixdict` text, feed the existing `parse()`" pattern as `orchestra.rs`/`fix_repository.rs` — no QuickFIX/QuickFIX-J private spec/data files are read, translated, or copied. Recorded explicitly in research.md given the project's prior removal of `qfj_xml` for this exact class of concern. | PASS |
| IV. Dual-Track Data Dictionary | Both codegen (typed group accessors) and runtime `DataDictionary` already read the same normalized dictionary source and the same `Member::Group` structure; this feature makes the runtime *decode* path stop silently diverging from what codegen already assumed it could rely on — closing a semantic gap between the two tracks, not opening one. | PASS |
| V. Test Discipline | Table-driven unit tests for `members()`, restructuring, and validation; process-level integration tests for FIXT dual-dictionary receive (extending existing `fixt_group_aware_decode.rs`/`header_trailer_groups_production.rs`-style tests); no protocol-visible session-lifecycle behavior changes, so no new AT scenarios are anticipated — confirmed in Phase 0 research. | PASS |
| VI. Acceptor/Initiator Parity | Body-group structuring and FIXT application-dictionary resolution apply identically regardless of session role; User Story 2's tests exercise both acceptor- and initiator-side sessions. | PASS |
| VII. Inventory-Based Completeness | Not an inventory-driven feature (no `docs/todo/NEW-*` items); traceability is instead to the external proposal and to feature 006's own deferred `GAP-26`/`FR-032` research note, both cited in spec.md/research.md. | PASS (N/A for inventory linkage) |

**Result**: No gate failures. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/011-body-repeating-groups/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   ├── README.md
│   ├── core-members-api.md
│   ├── decode-and-restructure.md
│   ├── validation.md
│   └── vendor-xml-dictionary.md
├── checklists/
│   └── requirements.md
└── tasks.md              # created by /speckit-tasks, not by this command
```

### Source Code (repository root)

```text
crates/
├── truefix-core/         # FieldMap::members()/MemberRef (A); new body-restructuring primitive
│                          # used by the FIXT post-decode step (B)
├── truefix-dict/          # validate.rs rewrite (C); new vendor_xml.rs converter + load_from_file
│                          # content-sniffing + CLI --format vendor-xml (D)
├── truefix-transport/      # classify_buffered no longer wraps dictionaries in
│                          # HeaderTrailerGroupsOnly (B, single/transport-dictionary case)
├── truefix-session/        # Session::validate_app-adjacent restructuring step that applies the
│                          # resolved application dictionary to an already-decoded FIXT message's
│                          # body before validation/delivery (B, FIXT case)
└── truefix-config/         # No behavior change expected; load_from_file's new vendor-XML support
                           # is transparent to its two existing call sites (single + dual dictionary)
```

**Structure Decision**: Existing cargo workspace and crate boundaries; no new crates. Work lands in
the crates that already own each behavior, matching this repository's established pattern (e.g.
feature 010's plan).

## Phase 0: Research Summary

See [research.md](./research.md). Key resolved decisions:

- `members()`/`MemberRef` API shape and the `raw_members()` rename to avoid the existing
  crate-private name clash.
- The FIXT application-dictionary body-restructuring design: a post-decode, in-memory restructuring
  step in the processing loop (`truefix-session`), not new cross-task shared state between the
  reader task and the session state machine.
- Confirmation that `validate_structured_group` (feature 007) already correctly validates structured
  body groups, narrowing part C to deleting dead flat-walk code plus fixing the group-aware
  `present()` gap — not a full rewrite.
- `vendor_xml`'s scope (the nested `<fix>` shape), its `dict-tooling` feature gating, and how
  `BeginString`/version is derived from the root element's attributes.
- Confirmation that no AT (acceptance-test) scenarios are implicated, since no session-layer
  protocol *behavior* (handshake, sequencing, resend) changes — only message body structure
  visibility and dictionary-loading format.

## Phase 1: Design Summary

See [data-model.md](./data-model.md) for the affected entities/types and [contracts/](./contracts/)
for the per-crate API contracts. The design is additive at every public API surface it touches
(`members()` is new; `.fields()`, `.group()`, `DataDictionary::validate`'s signature, and
`FixtDictionaries`'s public methods are all unchanged) except for the internal removal of
`HeaderTrailerGroupsOnly` and the dead flat-validation code, both crate-internal.

## Post-Design Constitution Check

| Principle | Re-check | Status |
|---|---|---|
| I | Contracts specify typed errors for vendor XML parsing and require the FIXT restructuring step to stay panic-free on malformed/partial input. | PASS |
| II | `decode-and-restructure.md` and `validation.md` ground every behavior change in the existing FIX repeating-group model already implemented by `decode_with_groups`/`validate_structured_group`. | PASS |
| III | `vendor-xml-dictionary.md` restates the provenance boundary (Binance's own files, independent implementation, no QuickFIX data-file reuse) as an explicit contract clause. | PASS |
| IV | `core-members-api.md` and `validation.md` keep codegen's existing group accessors and runtime validation reading the same structured data with no new divergence. | PASS |
| V | `quickstart.md` enumerates the unit/integration test slices per user story; no new AT scenarios required (confirmed in research.md). | PASS |
| VI | `decode-and-restructure.md` requires FIXT dual-dictionary tests on both acceptor and initiator sessions. | PASS |
| VII | N/A — no inventory linkage for this feature (external proposal, not an audit pass). | PASS |

## Complexity Tracking

No constitution violations require justification.
