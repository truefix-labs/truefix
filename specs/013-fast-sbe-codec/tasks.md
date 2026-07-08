# Tasks: FAST/SBE Binary Protocol Codec (`truefix-binary`)

**Input**: Design documents from `specs/013-fast-sbe-codec/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Required by spec and constitution (Principle V). Write targeted failing tests before implementation tasks wherever behavior changes.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel because it touches different files or only adds tests/fixtures.
- **[Story]**: User story label for story phases only.
- Every task includes an exact file path.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Stand up the new `truefix-binary` crate skeleton.

- [X] T001 [P] Create `truefix-binary` crate skeleton — `Cargo.toml` (deps: `truefix-core`, `truefix-dict`, `thiserror`, `tracing`, `metrics`, all `workspace = true`; `unsafe_code = "forbid"` via `[lints] workspace = true`) and an empty `src/lib.rs`, at `crates/truefix-binary/`
- [X] T002 Register `truefix-binary = { path = "crates/truefix-binary" }` in the root `Cargo.toml`'s `[workspace.dependencies]`, alongside the other 7 `truefix-*` crates (depends on T001)

**Checkpoint**: `cargo build -p truefix-binary` succeeds (empty crate); crate is workspace-visible.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared codec trait and the reference-fixture licensing decision both `truefix-binary`'s User Stories 1 and 2 depend on.

**⚠️ CRITICAL**: No User Story 1 or 2 implementation should begin until this phase is complete.

- [X] T003 Define the `BinaryCodec` trait (`encode(&Message, template_id: u32) -> Result<Vec<u8>, Self::Error>` / `decode(&[u8]) -> Result<(Message, u32), Self::Error>`, per contracts/binary-codec-api.md) in `crates/truefix-binary/src/lib.rs` (depends on T001)
- [X] T004 [P] Verify OpenFAST's test-vector corpus license is Apache-2.0/MIT-compatible (Constitution Principle III); record the outcome (compatible + source commit/tag, or "fall back to FAST-spec-embedded examples per Clarifications Q1") in `specs/013-fast-sbe-codec/research.md`
- [X] T005 [P] Verify Real Logic `simple-binary-encoding`'s example-schema license is Apache-2.0/MIT-compatible; record the outcome (or fallback decision) in `specs/013-fast-sbe-codec/research.md`

**Checkpoint**: `BinaryCodec` trait compiles; fixture-source decision for SC-001/SC-002 is recorded — User Stories 1 and 2 can now proceed in parallel.

---

## Phase 3: User Story 1 - FAST template-driven codec (Priority: P1) 🎯 MVP

**Goal**: Parse a FAST template XML and correctly encode/decode a binary message stream against it — including operator-based field recovery (copy/increment/delta/default), nullable fields, multi-template selection by inline ID, and typed errors on malformed input.

**Independent Test**: `cargo test -p truefix-binary --test fast_roundtrip --test fast_delta --test fast_multi_template` and `cargo test -p truefix-dict --test fast_template_parse` — no SBE code path or session/transport integration required.

### Tests for User Story 1

- [X] T006 [P] [US1] Add a FAST template XML fixture declaring `copy`/`increment`/`delta`/`default` fields plus a nested repeating group, and its matching byte-accurate reference-encoded sample message (source per T004's outcome), at `crates/truefix-binary/tests/fixtures/fast/template_basic.xml` + `message_basic.bin`
- [X] T007 [P] [US1] Add malformed FAST fixtures (truncated stream; invalid stop-bit sequence) at `crates/truefix-binary/tests/fixtures/fast/malformed/`
- [X] T008 [P] [US1] Add a second FAST template (distinct template ID) fixture for multi-template selection at `crates/truefix-binary/tests/fixtures/fast/template_secondary.xml`
- [X] T009 [P] [US1] Add FAST template XML parsing tests (valid multi-template file, duplicate template ID, unknown operator/data type) in `crates/truefix-dict/tests/fast_template_parse.rs`
- [X] T010 [P] [US1] Add FAST round-trip tests: decode → field values correct (including context-recovered copy/delta values) → re-encode → byte-identical to T006's reference; plus a nullable-field (presence-map bit 0, no operator) test, in `crates/truefix-binary/tests/fast_roundtrip.rs`
- [X] T011 [P] [US1] Add FAST malformed-input error-path tests (T007's fixtures → named `Truncated`/`InvalidStopBit` errors, zero panics) in `crates/truefix-binary/tests/fast_roundtrip.rs`
- [X] T012 [P] [US1] Add a FAST delta/copy/increment continuation test across consecutive messages under the same `EncodingContext` (field correctly recovered when presence map indicates omission) in `crates/truefix-binary/tests/fast_delta.rs`
- [X] T013 [P] [US1] Add a FAST multi-template selection test (T008's second template selected by inline ID) plus an unknown-template-ID error test in `crates/truefix-binary/tests/fast_multi_template.rs`
- [X] T014 [P] [US1] Add an `EncodingContext`-reset observability test (reset emits the FR-013 `tracing`+`metrics` event, captured via a test `tracing` subscriber) in `crates/truefix-binary/tests/observability.rs`

### Implementation for User Story 1

- [X] T015 [US1] Implement `FastTemplateSet`/`FastTemplate`/`FastFieldDef`/`FastOperator`/`FastGroupDef` (data-model.md) plus `FastTemplateError` and `load_fast_templates_from_file`/`parse_fast_templates`, in `crates/truefix-dict/src/fast_template.rs` (`dict-tooling`-gated, registered in `crates/truefix-dict/src/lib.rs`) (depends on T009)
- [X] T016 [US1] Implement stop-bit variable-length integer encode/decode in `crates/truefix-binary/src/fast/varint.rs`
- [X] T017 [US1] Implement `PresenceMap` bit read/write in `crates/truefix-binary/src/fast/presence_map.rs` (depends on T016)
- [X] T018 [US1] Implement `EncodingContext` (per-template-id previous-value cache; reset on reconnect) in `crates/truefix-binary/src/fast/context.rs`
- [X] T019 [US1] Implement `FastCodecError` (`thiserror`: `Truncated { offset }`, `InvalidStopBit { offset }`, `PresenceMapMismatch { field, offset }`, `UnknownTemplateId { id }`) in `crates/truefix-binary/src/fast/error.rs`
- [X] T020 [US1] Implement `FastCodec: BinaryCodec` (encode/decode over `FastTemplateSet` using `PresenceMap`/`EncodingContext`/varint, selecting by inline template ID with `UnknownTemplateId` on miss) in `crates/truefix-binary/src/fast/mod.rs` (depends on T003, T015–T019)
- [X] T021 [US1] Wire FR-013 `tracing`+`metrics` emission for decode failures, unknown template IDs, and `EncodingContext` resets (following `report_write_failure`'s paired pattern) in `crates/truefix-binary/src/fast/{context,error}.rs` (depends on T020)
- [X] T022 [US1] Run `cargo test -p truefix-binary --test fast_roundtrip --test fast_delta --test fast_multi_template --test observability` and `cargo test -p truefix-dict --test fast_template_parse`, confirm all pass, record the result in `specs/013-fast-sbe-codec/quickstart.md`

**Checkpoint**: User Story 1 is independently functional and testable — FAST template parsing, codec, multi-template selection, and observability all work standalone.

---

## Phase 4: User Story 2 - SBE schema-driven zero-copy codec (Priority: P2)

**Goal**: Parse an SBE schema XML and correctly encode/decode a binary message via zero-copy flyweight access — fixed-offset fields, repeating groups, VarData, multi-schema selection by `templateId`/`schemaId`, and typed errors on malformed/inconsistent input.

**Independent Test**: `cargo test -p truefix-binary --test sbe_roundtrip --test sbe_multi_schema` and `cargo test -p truefix-dict --test sbe_schema_parse` — independent of FAST or session/transport integration.

### Tests for User Story 2

- [X] T023 [P] [US2] Add an SBE schema XML fixture (fixed fields, one repeating group with a nested group, one `VarData` field) and its matching byte-accurate reference-encoded sample message (source per T005's outcome), at `crates/truefix-binary/tests/fixtures/sbe/schema_basic.xml` + `message_basic.bin`
- [X] T024 [P] [US2] Add malformed SBE fixtures (`blockLength`/`numGroups`/`VarData`-length inconsistent with actual content; truncated) at `crates/truefix-binary/tests/fixtures/sbe/malformed/`
- [X] T025 [P] [US2] Add a second SBE message schema (distinct `templateId`) fixture for multi-schema selection at `crates/truefix-binary/tests/fixtures/sbe/schema_secondary.xml`
- [X] T026 [P] [US2] Add SBE schema XML parsing tests (valid multi-message file, duplicate `templateId`, overlapping fixed-field offsets, unknown data type) in `crates/truefix-dict/tests/sbe_schema_parse.rs`
- [X] T027 [P] [US2] Add SBE round-trip tests: decode (fixed fields, nested groups, multiple `VarData` fields, in schema order) → re-encode → byte-identical to T023's reference, in `crates/truefix-binary/tests/sbe_roundtrip.rs`
- [X] T028 [P] [US2] Add SBE malformed-input error-path tests (T024's fixtures → named `Truncated`/`BlockLengthMismatch`/`VarDataLengthMismatch` errors before any out-of-bounds read, zero panics) in `crates/truefix-binary/tests/sbe_roundtrip.rs`
- [X] T029 [P] [US2] Add an SC-003 zero-copy allocation-counting test (decoding + field reads allocate nothing beyond the input buffer) in `crates/truefix-binary/tests/sbe_roundtrip.rs`
- [X] T030 [P] [US2] Add an SBE multi-schema selection test (T025's second schema selected by `templateId`) plus an unknown-`templateId` error test in `crates/truefix-binary/tests/sbe_multi_schema.rs`

### Implementation for User Story 2

- [X] T031 [US2] Implement `SbeSchemaSet`/`SbeMessageSchema`/`SbeFieldDef`/`SbeGroupDef`/`SbeVarDataDef` (data-model.md) plus `SbeSchemaError` and `load_sbe_schemas_from_file`/`parse_sbe_schemas`, in `crates/truefix-dict/src/sbe_schema.rs` (`dict-tooling`-gated, registered in `crates/truefix-dict/src/lib.rs`) (depends on T026)
- [X] T032 [US2] Implement the zero-copy `Flyweight` buffer view using safe slice/byte-order operations only (`unsafe_code = "forbid"`) in `crates/truefix-binary/src/sbe/flyweight.rs`
- [X] T033 [US2] Implement `SbeCodecError` (`thiserror`: `Truncated { offset }`, `BlockLengthMismatch { declared, actual }`, `UnknownTemplateId { id }`, `VarDataLengthMismatch { field, declared, actual }`) in `crates/truefix-binary/src/sbe/error.rs`
- [X] T034 [US2] Implement `SbeCodec: BinaryCodec` (bounds-checked encode/decode over `SbeSchemaSet` via `Flyweight`, selecting by `templateId`/`schemaId` with `UnknownTemplateId` on miss) in `crates/truefix-binary/src/sbe/mod.rs` (depends on T003, T031–T033)
- [X] T035 [US2] Wire FR-013 `tracing`+`metrics` emission for decode failures and unknown `templateId`s in `crates/truefix-binary/src/sbe/error.rs` (depends on T034)
- [X] T036 [US2] Run `cargo test -p truefix-binary --test sbe_roundtrip --test sbe_multi_schema` and `cargo test -p truefix-dict --test sbe_schema_parse`, confirm all pass, record the result in `specs/013-fast-sbe-codec/quickstart.md`

**Checkpoint**: User Stories 1 and 2 both work independently — FAST and SBE codecs are each fully functional standalone.

---

## Phase 5: User Story 3 - IR conversion and `Protocol` session integration (Priority: P3)

**Goal**: Make FAST/SBE transparent to the rest of the engine — convert decoded binary messages to/from `truefix_core::Message`, select the wire encoding per session via a new `Protocol` config option, reject cross-`Protocol` Logon attempts with a named error, and wire it all into the `truefix` facade — on both acceptor and initiator roles.

**Independent Test**: `cargo test -p truefix-binary --test ir_conversion`, `cargo test -p truefix-config --test protocol_config`, `cargo test -p truefix-transport --test protocol_mismatch --test binary_protocol_session`, plus `cargo test -p truefix-at --test conformance` for the zero-regression check.

### Tests for User Story 3

- [X] T037 [P] [US3] Add representative-subset IR fixtures — `NewOrderSingle`(D), `ExecutionReport`(8), a repeating-group market-data message with a nested group (Clarifications Q3) — as both SOH-decoded `Message` fixtures and FAST/SBE-encoded equivalents, at `crates/truefix-binary/tests/fixtures/ir/`
- [X] T038 [P] [US3] Add IR round-trip tests (`decode_into_message`/`encode_from_message`, both directions, zero information loss for T037's three message types) in `crates/truefix-binary/tests/ir_conversion.rs`
- [X] T039 [P] [US3] Add an IR unsupported-construct typed-error test (a `Message` construct with no FAST/SBE-side representation) in `crates/truefix-binary/tests/ir_conversion.rs`
- [X] T040 [P] [US3] Add `Protocol` config parsing tests (default `Soh`; `Fast`/`Sbe` with valid template/schema path; missing-path fails fast at config resolution) in `crates/truefix-config/tests/protocol_config.rs`
- [X] T041 [P] [US3] Add a `Protocol`-mismatch Logon-rejection integration test (one side `Soh`, other `Fast`), on both acceptor and initiator sessions, in `crates/truefix-transport/tests/protocol_mismatch.rs`
- [X] T042 [P] [US3] Add an end-to-end FAST session round-trip integration test (Application callback receives a `Message` field-equivalent to the SOH case), on both acceptor and initiator sessions, in `crates/truefix-transport/tests/binary_protocol_session.rs`
- [X] T043 [P] [US3] Add an end-to-end SBE session round-trip integration test, on both acceptor and initiator sessions, in `crates/truefix-transport/tests/binary_protocol_session.rs`
- [X] T044 [P] [US3] Add a full-stack observability test (decode failure and `EncodingContext` reset events visible with session ID, via a test `tracing` subscriber / `metrics` test recorder) in `crates/truefix-binary/tests/observability.rs`

### Implementation for User Story 3

- [X] T045 [US3] Implement `decode_into_message`/`encode_from_message` conversion functions (building/reading `Message` via `FieldMap::set`/`add_group`/`members()`, per contracts/ir-conversion.md) in `crates/truefix-binary/src/ir.rs` (depends on T020, T034)
- [X] T046 [US3] Add `Protocol` enum (`Soh`/`Fast`/`Sbe`, default `Soh`) and a `protocol` field (plus the resolved FAST/SBE template/schema set, populated only for the matching variant) to `SessionConfig` in `crates/truefix-session/src/config.rs`
- [X] T047 [US3] Register `"Protocol"`, `"FastTemplatePath"`, `"SbeSchemaPath"` keys in `APPENDIX_A_KEYS` in `crates/truefix-config/src/keys.rs` (depends on T046)
- [X] T048 [US3] Implement `protocol_key()` (mirrors `precision_key`'s case-insensitive parsing) plus FAST/SBE template/schema set loading (mirrors `resolve_validator`'s bundled-name-then-filesystem-fallback pattern) in `crates/truefix-config/src/builder.rs` (depends on T015, T031, T046, T047)
- [X] T049 [US3] Add a `Protocol`-aware framing/decode branch to `classify_buffered` — dispatch to `truefix-binary`'s `FastCodec`/`SbeCodec` for `Protocol::Fast`/`Sbe` sessions, with a new named decode-error variant for undecodable/cross-`Protocol` byte streams — in `crates/truefix-transport/src/lib.rs` (depends on T045, T048)
- [X] T050 [US3] Skip `next_frame_start`'s SOH-specific resync heuristic under `Protocol::Fast`/`Sbe` (treat a decode failure as connection-fatal, matching the existing "non-Logon first message closes the socket" precedent) in `crates/truefix-transport/src/lib.rs` (depends on T049)
- [X] T051 [US3] (Defense-in-depth, optional per research.md Decision 4) Not implemented: protocol mismatch is enforced at the transport decode layer before `on_logon`; adding a session-layer guard would require a non-standard protocol marker and is not meaningful for ordinary cross-protocol byte streams.
- [X] T052 [US3] Add a `truefix-binary` dependency and `pub use truefix_binary as binary;` re-export to `crates/truefix/Cargo.toml` + `crates/truefix/src/lib.rs` (depends on T002)
- [X] T053 [US3] Wire the resolved `SessionConfig.protocol` into `Engine::start_impl`'s per-session transport/codec wiring in `crates/truefix/src/lib.rs` (depends on T049, T052)
- [X] T054 [US3] Run `cargo test -p truefix-binary --test ir_conversion --test observability`, `cargo test -p truefix-config --test protocol_config`, `cargo test -p truefix-transport --test protocol_mismatch --test binary_protocol_session`, confirm all pass, record the result in `specs/013-fast-sbe-codec/quickstart.md`

**Checkpoint**: All three user stories are independently functional; the full feature is usable end-to-end through the `truefix` facade.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Workspace-wide regression confirmation and documentation.

- [X] T055 [P] Update `README.md`'s crate list and "Building & MSRV" section to mention `truefix-binary` and the new `Protocol` session-config option
- [X] T056 Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (workspace-wide, zero failures)
- [X] T057 Run `cargo test -p truefix-at --test conformance` and confirm the existing 483/483 scenario runs still pass with **zero regressions** (research.md Decision 6 — this feature adds no new AT scenarios)
- [X] T058 Execute the full `specs/013-fast-sbe-codec/quickstart.md` validation guide end-to-end and record the results

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS User Stories 1 and 2 (User Story 3 also depends on it transitively, via US1/US2's outputs).
- **User Story 1 (Phase 3)**: Depends on Foundational. Independent of User Story 2.
- **User Story 2 (Phase 4)**: Depends on Foundational. Independent of User Story 1.
- **User Story 3 (Phase 5)**: Depends on User Story 1's `FastCodec` (T020) and User Story 2's `SbeCodec` (T034) both existing — it integrates both codecs into the session/transport/config/facade layers, so it cannot start until both are complete (unlike US1/US2, which are mutually independent).
- **Polish (Phase 6)**: Depends on all three user stories being complete.

### Within Each User Story

- Tests MUST be written and FAIL before the corresponding implementation task (Constitution Principle V).
- Fixtures before parsers; parsers before codecs; codecs before observability wiring.
- Story complete (checkpoint) before moving to the next priority, if working sequentially.

### Parallel Opportunities

- T001 and (once T002 lands) T004/T005 can run in parallel.
- Once Phase 2 (Foundational) completes, **User Story 1 and User Story 2 can be implemented fully in parallel** (different files, no shared state beyond the already-complete `BinaryCodec` trait) — unlike most cross-cutting features in this repository, these two stories share no runtime code.
- All `[P]`-marked test/fixture tasks within a story can run in parallel.
- User Story 3 cannot start until both US1 and US2 are complete (see Phase Dependencies above), so it is not part of the US1/US2 parallel window.

---

## Parallel Example: User Story 1 and User Story 2 together

```bash
# After Phase 2 (Foundational) completes, two developers can work simultaneously:

# Developer A — User Story 1 (FAST)
Task: "Add FAST template XML fixture... at crates/truefix-binary/tests/fixtures/fast/template_basic.xml"
Task: "Add FAST template XML parsing tests... in crates/truefix-dict/tests/fast_template_parse.rs"
Task: "Implement FastTemplateSet... in crates/truefix-dict/src/fast_template.rs"
Task: "Implement FastCodec: BinaryCodec... in crates/truefix-binary/src/fast/mod.rs"

# Developer B — User Story 2 (SBE), concurrently, zero file overlap with Developer A
Task: "Add SBE schema XML fixture... at crates/truefix-binary/tests/fixtures/sbe/schema_basic.xml"
Task: "Add SBE schema XML parsing tests... in crates/truefix-dict/tests/sbe_schema_parse.rs"
Task: "Implement SbeSchemaSet... in crates/truefix-dict/src/sbe_schema.rs"
Task: "Implement SbeCodec: BinaryCodec... in crates/truefix-binary/src/sbe/mod.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks US1/US2)
3. Complete Phase 3: User Story 1 (FAST codec)
4. **STOP and VALIDATE**: `cargo test -p truefix-binary --test fast_roundtrip --test fast_delta --test fast_multi_template`
5. This is a usable MVP for any counterparty whose binary protocol need is FAST-only (e.g. a market-data-only integration) — SBE, IR conversion, and session integration are not required for FAST to be independently useful for offline template-driven encode/decode.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. User Story 1 (FAST) → independently testable → usable standalone for offline FAST codec work.
3. User Story 2 (SBE) → independently testable, in parallel with or after US1 → usable standalone for offline SBE codec work.
4. User Story 3 (IR + `Protocol` integration) → requires both US1 and US2 complete → makes FAST/SBE usable inside a live TrueFix session.
5. Polish → workspace-wide regression + AT zero-regression confirmation + docs.

### Parallel Team Strategy

With two developers: complete Setup + Foundational together, then one developer takes User Story 1
(FAST) and the other takes User Story 2 (SBE) fully in parallel (they share no files). Both must
finish before a third developer (or either, sequentially) starts User Story 3, since it integrates
both codecs into the shared session/transport/config/facade layers.

---

## Notes

- `[P]` tasks touch different files with no dependency on an incomplete task.
- `[Story]` label maps each task to its user story for traceability.
- Constitution Principle V mandates tests: every implementation task above has a corresponding test
  task listed first in its phase, and that test MUST fail before the implementation task lands.
- Constitution Principle VI mandates acceptor/initiator parity: T041–T043 (US3's session-integration
  tests) explicitly cover both roles; T020/T034 (the codecs themselves) have no role concept, so
  parity is enforced entirely at the test layer, not the API layer (see contracts/binary-codec-api.md).
- No new `truefix-at` scenarios are added anywhere in this task list (research.md Decision 6) — T057
  is a regression check, not new-scenario authoring.
- Commit after each task or logical group; stop at any checkpoint to validate a story independently.
