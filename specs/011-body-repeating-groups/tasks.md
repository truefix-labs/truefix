# Tasks: Body-Level Repeating Group Decoding & Vendor Dictionary Support

**Input**: Design documents from `specs/011-body-repeating-groups/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Required by spec and constitution (Principle V). Write targeted failing tests before implementation tasks wherever behavior changes.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

**Status**: All 37 tasks complete. `cargo test --workspace --features dict-tooling` and `cargo test -p truefix-at` (483/483 AT scenario runs) pass with zero regressions; `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --features dict-tooling` are clean. Three implementation-time corrections to the original plan are called out inline below (T014, T017-T021's file location, T022's call site) — see research.md for full rationale.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel because it touches different files or only adds tests/fixtures.
- **[Story]**: User story label for story phases only.
- Every task includes an exact file path.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare fixtures shared across multiple User Story 3 tasks.

- [X] T001 [P] Add a self-contained vendor-XML dictionary fixture (fields, a body-level repeating group, a `<component>` reference, a nested `<group>`, and a multi-character custom `MsgType`) at `crates/truefix-dict/tests/fixtures/vendor_dict_sample.xml`
- [X] T002 [P] Add malformed-vendor-XML fixtures (missing root version attributes, an undefined `<component>`/`<group>` reference, broken XML syntax) under `crates/truefix-dict/tests/fixtures/vendor_dict_malformed/`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: `truefix-core` API additions that both US1 (validation fix) and US2 (FIXT restructuring) depend on.

**CRITICAL**: No User Story 1 or User Story 2 implementation should begin until this phase is complete. (User Story 3 does not depend on this phase — see its own dependency note in Phase 5.)

- [X] T003 Add `MemberRef<'a>` and `pub fn members(&self) -> impl Iterator<Item = MemberRef<'_>>` to `FieldMap`, **and**, in the same change, rename the pre-existing crate-private `pub(crate) fn members(&self) -> &[Member]` to `raw_members()` and update its two call sites — all in `crates/truefix-core/src/field_map.rs` and `crates/truefix-core/src/codec/encode.rs`. These two edits land together: adding the new public `members()` without renaming the old crate-private one in the same commit is a duplicate-method-name compile error, so this must not be split across separate tasks/commits.
- [X] T004 Add `pub fn restructure_groups(map: &mut FieldMap, spec: &dyn GroupSpec)` (re-grouping an already-flat `FieldMap` using the same boundary logic as `decode_section_with_groups`) in `crates/truefix-core/src/codec/decode.rs`, and re-export it from `crates/truefix-core/src/lib.rs`
- [X] T005 [P] Add table-driven unit tests for `members()`/`MemberRef` (flat-only, single-level group, nested group, declared/actual count mismatch, `.fields()` output unchanged) in `crates/truefix-core/tests/members_api.rs`
- [X] T006 [P] Add unit tests proving `restructure_groups` produces a structurally-equal result to `decode_with_groups` for the same input tokens (bytes → structured `Message` vs. bytes → flat `decode` → `restructure_groups`) in `crates/truefix-core/tests/group_restructure.rs`

**Checkpoint**: `truefix-core` public API ready — US1 and US2 crate-level work can now proceed (US3 could already have started in parallel — see Phase 5).

---

## Phase 3: User Story 1 - Structured body-level repeating groups on receive, single dictionary (Priority: P1) 🎯 MVP

**Goal**: Single-`DataDictionary` sessions structure and validate body-level repeating groups on receive, with zero entries silently dropped through either the low-level group API or codegen-generated accessors, and zero validation regressions.

**Independent Test**: `cargo test -p truefix-transport -p truefix-dict` — feed a decoded raw message with a multi-entry body-level group through the receive path with one `DataDictionary` attached and confirm full entry visibility plus unchanged accept/reject outcomes for the existing test suite.

### Tests for User Story 1

- [X] T007 [P] [US1] Add a single-dictionary body-level multi-entry group decode test (full entry visibility via `.group()`, unchanged `.fields()` flat view) in `crates/truefix-transport/tests/body_group_decode_production.rs`
- [X] T008 [P] [US1] Add a nested body-group (group of groups) decode test in `crates/truefix-transport/tests/body_group_decode_production.rs`
- [X] T009 [P] [US1] Add a codegen-generated body-group typed-accessor regression test (real multi-entry message → accessor returns every entry, not empty) in `crates/truefix-dict/tests/codegen.rs`
- [X] T010 [P] [US1] Add body-level group validation tests (missing required member, out-of-order member, `NoXxx` count mismatch) in `crates/truefix-dict/tests/group_validation.rs`
- [X] T011 [P] [US1] Add a regression test for the `present()` group-aware fix: a dictionary declaring a group's own count tag as message-level-required, a message carrying that group with ≥1 entry, asserting `validate()` no longer reports it missing, in `crates/truefix-dict/tests/header_group_validation.rs`

### Implementation for User Story 1

- [X] T012 [US1] Delete `HeaderTrailerGroupsOnly` and change `classify_buffered`'s single-`DataDictionary` branch to call `decode_with_groups(raw, dict)` directly, in `crates/truefix-transport/src/lib.rs`
- [X] T013 [US1] Change `classify_buffered`'s FIXT-transport-only branch (`services.fixt_dictionaries` present, `services.validator` absent) to call `decode_with_groups(raw, dicts.transport())` directly, in `crates/truefix-transport/src/lib.rs`
- [X] T014 [US1] **Implementation-time correction**: `validate_group`/`validate_groups`'s flat position-scanning branch was *not* deleted — `DataDictionary::validate` is a general-purpose public API also fed hand-built flat `Message`s (`FieldMap::add_field` directly) and plainly-`decode()`d ones, and deleting the flat path regressed 4 existing tests immediately. Kept both paths in `crates/truefix-dict/src/validate.rs`; additionally taught `validate_structured_group` to check declared count and member order (a real, previously-latent gap that became a live AT regression — 3 scenario failures — the moment body groups started structuring routinely; see research.md R4).
- [X] T015 [US1] Fix `present()` to recognize a `Member::Group` whose count tag matches as present, using `FieldMap::members()`, in `crates/truefix-dict/src/validate.rs` (depends on T003)
- [X] T016 [US1] Run the full pre-existing `truefix-dict`/`truefix-transport`/`truefix-core` test suites, confirm 100% still pass unchanged (SC-002), and record the result in `specs/011-body-repeating-groups/quickstart.md`

**Checkpoint**: User Story 1 is independently functional and testable — single-dictionary sessions fully structure and validate body-level groups with no regressions.

---

## Phase 4: User Story 2 - Structured body-level repeating groups under FIXT 1.1 dual dictionaries (Priority: P2)

**Goal**: FIXT 1.1 sessions structure application-layer message bodies using the correctly-resolved application dictionary, leave session-layer messages on the transport dictionary, and fall back safely when `ApplVerID` isn't yet resolvable — for both acceptor and initiator roles.

**Independent Test**: `cargo test -p truefix-transport --test fixt_application_group_restructure` — configure a session with `FixtDictionaries`, send an application-layer message with a body-level group under a negotiated `ApplVerID`, and confirm full entry visibility independent of User Story 3's vendor-XML work.

### Tests for User Story 2

**Implementation-time correction**: all five tests below live in `crates/truefix-transport/tests/fixt_application_group_restructure.rs`, not `crates/truefix-session/tests/` as originally planned — `truefix-session` has no `tokio` dependency at all (it's the sans-IO state machine crate); the real acceptor/initiator networking primitives (`Acceptor::bind_with`/`connect_initiator_with`) these tests need only exist in `truefix-transport`, matching where every other production-decode-path test in this feature already lives.

- [X] T017 [P] [US2] Add a FIXT application-layer body-group decode test on an **acceptor** session (full entry visibility using the resolved application dictionary) in `crates/truefix-transport/tests/fixt_application_group_restructure.rs`
- [X] T018 [P] [US2] Add the same test on an **initiator** session (Constitution Principle VI parity) in `crates/truefix-transport/tests/fixt_application_group_restructure.rs`
- [X] T019 [P] [US2] Add a session-layer-message-unaffected regression test (e.g. a message carrying `NoHops`) confirming transport-dictionary structuring is unchanged under FIXT, in `crates/truefix-transport/tests/fixt_application_group_restructure.rs`
- [X] T020 [P] [US2] Add an unresolvable-per-message-`ApplVerID` fallback test (an explicit `ApplVerID`(1128) naming an unregistered application version, which takes precedence over an otherwise-valid negotiated default) confirming transport-scoped fallback rather than rejection or misattribution, in `crates/truefix-transport/tests/fixt_application_group_restructure.rs`
- [X] T021 [P] [US2] Add a multi-`ApplVerID` test (messages under different registered `ApplVerID`s in the same connection each resolve their own application dictionary, no cached single choice), in `crates/truefix-transport/tests/fixt_application_group_restructure.rs`

### Implementation for User Story 2

- [X] T022 [US2] **Implementation-time correction to the call site**: the restructuring call does *not* live inside `Session::on_received`/the processing loop — `Application::from_admin`/`from_app` are called directly from `truefix-transport`'s `handle_inbound`, *before* `dispatch`/`Session::handle` ever runs (there is no `Action::Deliver`; `Action` only carries `Send`/`Resend`/`Disconnect`/`ResetStore`), so a restructuring step inside the state machine is always too late for the application to see it — proven by an end-to-end test that still saw the flat body. Implemented as a new `pub fn restructure_fixt_application_body(&self, msg: &mut Message)` on `Session` (`crates/truefix-session/src/state.rs`), called from `handle_inbound` (`crates/truefix-transport/src/lib.rs`) right after its existing `would_reject_before_processing` precheck and before the `is_admin(&msg)`/`from_admin`/`from_app` dispatch. Resolves `ApplVerID` via the same precedence `validate_app` already uses (own header tag 1128, else negotiated tag 1137, else `FixtDictionaries`'s default) and calls `truefix_core::restructure_groups` on the message's `body`.
- [X] T023 [US2] Ensure the restructuring call site skips admin/session-layer messages (already correctly transport-structured) and no-ops (transport-scoped fallback, no rejection) when no application dictionary resolves for the message's `ApplVerID` — both handled inside `restructure_fixt_application_body` itself (`crates/truefix-session/src/state.rs`).

**Checkpoint**: User Stories 1 and 2 both work independently — FIXT dual-dictionary sessions now match the single-dictionary case's correctness for both acceptor and initiator roles.

---

## Phase 5: User Story 3 - Load a vendor-published FIX dictionary XML directly (Priority: P3)

**Goal**: A vendor-published dictionary XML file (Binance's published shape) loads directly as a working, validating, group-structuring `DataDictionary` — via `load_from_file` or the CLI — with zero manual translation, gated behind the existing `dict-tooling` feature.

**Independent Test**: `cargo test -p truefix-dict --features dict-tooling` — convert/load the Setup-phase fixture and confirm it validates and structures a matching sample message equivalently to a native `.fixdict`.

**Dependency note**: This story depends only on Phase 1 (Setup, T001-T002 fixtures) — it does not depend on Phase 2 (Foundational) or on anything in User Story 1/2. `vendor_xml.rs` is a standalone XML-to-`.fixdict`-text converter; it never touches `FieldMap`, `members()`, or `restructure_groups`.

### Tests for User Story 3

- [X] T024 [P] [US3] Add a `vendor_xml::convert` round-trip test using the `vendor_dict_sample.xml` fixture (fields, body-group, component reference, nested group, multi-character `MsgType` all resolve correctly) in `crates/truefix-dict/tests/vendor_xml_conversion.rs`
- [X] T025 [P] [US3] Add `vendor_xml::convert` error-path tests using the `vendor_dict_malformed/` fixtures (malformed XML, missing version attributes, undefined component/group reference each produce a `VendorXmlError`, never a panic or partial output) in `crates/truefix-dict/tests/vendor_xml_conversion.rs`
- [X] T026 [P] [US3] Extend `crates/truefix-dict/tests/provenance_headers.rs` to assert `vendor_xml.rs` carries the same independent-implementation provenance doc comment/header convention already enforced for `orchestra.rs`/`fix_repository.rs` (FR-017) — this is the executable check backing FR-017's "no third-party private spec file translation" requirement, which otherwise has no test coverage
- [X] T027 [P] [US3] Extend `load_from_file` tests with the vendor-XML fixture, both with and without the `dict-tooling` feature enabled, in `crates/truefix-dict/tests/load_from_file.rs`
- [X] T028 [P] [US3] Extend CLI tests with a `generate-dict --format vendor-xml` invocation against the fixture, in `crates/truefix-dict/tests/cli.rs`

### Implementation for User Story 3

- [X] T029 [US3] Add `VendorXmlError` and `pub fn convert(xml: &str) -> Result<String, VendorXmlError>`, parsing the nested `<fix><header>/<messages>/<message>/<trailer>/<components>/<fields>` shape into `.fixdict` text (fields, messages incl. multi-character `msgtype`, `component:Name` references, nested `group` definitions), in new file `crates/truefix-dict/src/vendor_xml.rs` (`dict-tooling`-gated). Carries the same provenance doc-comment header convention as `orchestra.rs`/`fix_repository.rs` (verified by T026).
- [X] T030 [US3] Derive and emit a `version FIX.M.N[SPk]` directive from the root `<fix>` element's version attributes in `convert`, returning `VendorXmlError` when unrecognizable rather than guessing, in `crates/truefix-dict/src/vendor_xml.rs`
- [X] T031 [US3] Register `pub mod vendor_xml` (`dict-tooling`-gated) and export `VendorXmlError` in `crates/truefix-dict/src/lib.rs`
- [X] T032 [US3] Add a `dict-tooling`-gated content-sniffing branch to `load_from_file` that routes `<?xml`/`<fix `-prefixed content through `vendor_xml::convert`, then `parse()`, in `crates/truefix-dict/src/lib.rs` (depends on T031). Added the new `DictLoadError::VendorXml { path: String, source: VendorXmlError }` variant for a conversion failure.
- [X] T033 [US3] Add a `--format vendor-xml` match arm to `generate_dict()`, calling `vendor_xml::convert` and writing its output like the existing `orchestra`/`fix-repository` arms, in `crates/truefix-dict/src/bin/truefix-dict.rs`

**Checkpoint**: All three user stories are independently functional — vendor-published dictionaries load and convert correctly, gated behind `dict-tooling`, with typed errors on malformed input.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Cross-story hygiene and final regression confirmation.

- [X] T034 [P] Add complete doc comments (Constitution Principle I) to every new public item — `FieldMap::members`/`MemberRef` (`crates/truefix-core/src/field_map.rs`), `restructure_groups` (`crates/truefix-core/src/codec/decode.rs`), `Session::restructure_fixt_application_body` (`crates/truefix-session/src/state.rs`), `vendor_xml::convert`/`VendorXmlError`/`DictLoadError::VendorXml` (`crates/truefix-dict/src/vendor_xml.rs`, `crates/truefix-dict/src/lib.rs`)
- [X] T035 Ran `cargo fmt --all` and `cargo clippy --workspace --all-targets --features dict-tooling` — two `clippy::type_complexity` warnings found and fixed (type aliases added in `vendor_xml.rs` and `fixt_application_group_restructure.rs`); zero warnings remain.
- [X] T036 Ran `cargo test --workspace --features dict-tooling` (including AT scenarios) — zero regressions. **Note**: the first full run surfaced 3 real AT scenario regressions (`14i_RepeatingGroupCountNotEqual`, `14j_OutOfOrderRepeatingGroupMembers`, `QFJ934_MissingDelimiterNestedRepeatingGroup`) from the pre-existing `validate_structured_group` count/order gap (T014); fixed, then re-verified 483/483 AT scenario runs pass and the full workspace suite is green.
- [X] T037 Executed `specs/011-body-repeating-groups/quickstart.md`'s validation steps for all three user stories via the real test suite (see quickstart.md's own recorded results).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **Foundational (Phase 2)**: No dependency on Setup; BLOCKS User Story 1 and User Story 2 only (T012-T023 depend on the `truefix-core` API from T003/T004). Does **not** block User Story 3 (see Phase 5's dependency note).
- **User Story 1 (Phase 3)**: Depends on Foundational. No dependency on US2/US3.
- **User Story 2 (Phase 4)**: Depends on Foundational (specifically T004). Independent of US1's `validate.rs` changes and of US3 entirely — can run in parallel with US1 once Foundational is done, though landing US1 first is recommended (see Implementation Strategy).
- **User Story 3 (Phase 5)**: Depends on Setup (T001-T002 fixtures) only — independent of Foundational and of US1/US2's `truefix-core`/`truefix-transport`/`truefix-session` changes; only shares the `truefix-dict` crate.
- **Polish (Phase 6)**: Depends on all three user stories being complete.

### Within Each User Story

- Tests are written first and must fail before their corresponding implementation task.
- `truefix-core` foundational API before any crate that consumes it.
- Decode/structuring changes before validation changes that assume structured input (US1: T012-T013 before T014-T015).
- Story complete (checkpoint) before starting Polish.

### Parallel Opportunities

- T001-T002 (Setup) in parallel.
- T005-T006 (Foundational tests) in parallel with each other, after T003-T004.
- All Tests-for-User-Story-N tasks marked [P] within a story run in parallel (different assertions in the same or sibling files — verify no file-write conflicts before parallelizing tasks that share one file, e.g. T017-T021 all touch `fixt_application_group_restructure.rs`; treat same-file tasks as sequential in practice even though listed [P] for conceptual independence).
- User Story 3 (T024-T033) can proceed in parallel with Foundational, US1, and US2 from the very start — it has no dependency on any of them (see Phase 5's dependency note).
- US1 and US2 both touch `truefix-transport`/`truefix-dict` — coordinate to avoid merge conflicts; landing US1 first is recommended (see Implementation Strategy).

---

## Parallel Example: Foundational Phase

```bash
# After T003-T004 land:
Task: "Add table-driven unit tests for members()/MemberRef in crates/truefix-core/tests/members_api.rs"
Task: "Add unit tests proving restructure_groups matches decode_with_groups in crates/truefix-core/tests/group_restructure.rs"
```

## Parallel Example: User Story 1 Tests

```bash
Task: "Add single-dictionary body-level multi-entry group decode test in crates/truefix-transport/tests/body_group_decode_production.rs"
Task: "Add codegen-generated body-group typed-accessor regression test in crates/truefix-dict/tests/codegen.rs"
Task: "Add body-level group validation tests in crates/truefix-dict/tests/group_validation.rs"
Task: "Add present() group-aware fix regression test in crates/truefix-dict/tests/header_group_validation.rs"
```

## Parallel Example: User Story 3 (independent of Foundational)

```bash
# Can start immediately alongside Setup/Foundational/US1/US2:
Task: "Add vendor_xml::convert round-trip test in crates/truefix-dict/tests/vendor_xml_conversion.rs"
Task: "Add vendor_xml::convert error-path tests in crates/truefix-dict/tests/vendor_xml_conversion.rs"
Task: "Extend provenance_headers.rs to cover vendor_xml.rs in crates/truefix-dict/tests/provenance_headers.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks US1 and US2, not US3)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: `cargo test -p truefix-transport -p truefix-dict`; confirm zero entries dropped for a multi-entry body group and zero validation regressions
5. This alone closes the core silent-data-loss defect the external proposal identified — a complete, independently valuable increment

### Incremental Delivery

1. Setup + Foundational → foundation ready (US3 can already be underway in parallel)
2. Add User Story 1 → validate independently → this is the MVP
3. Add User Story 2 → validate independently (FIXT dual-dictionary correctness)
4. Add User Story 3 → validate independently (vendor XML ingestion)
5. Each story adds value without breaking the previous ones — confirmed by each story's own test suite plus the Polish phase's full-workspace regression run

### Parallel Team Strategy

With multiple developers:

- Developer A: User Story 1 (`truefix-transport`, `truefix-dict`'s `validate.rs`) — starts after Foundational.
- Developer B: User Story 3 (`truefix-dict`'s `vendor_xml.rs`/CLI) — starts immediately, no dependency on Foundational or on Developer A/C's work.
- Developer C: User Story 2 (`truefix-session`) — starts after Foundational; coordinate with Developer A once both need to touch `crates/truefix-transport/src/lib.rs`.

---

## Notes

- [P] tasks = different files, or additive test assertions with no dependency on an incomplete task.
- [Story] label maps task to specific user story for traceability.
- No AT (acceptance-test) scenario *additions* were needed (research.md R7 held) — but the existing AT suite did catch a real regression from the T014 gap (see T036), confirming its value as a gate.
- Confirm test failure before implementing (Constitution Principle V, test-first where practical).
- Commit after each task or logical group.
- Stop at any checkpoint to validate a story independently before continuing.
