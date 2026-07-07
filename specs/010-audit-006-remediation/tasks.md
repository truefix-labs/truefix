# Tasks: Audit 006 Remediation

**Input**: Design documents from `specs/010-audit-006-remediation/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Required by spec and constitution. Write targeted failing tests before implementation tasks wherever behavior changes.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel because it touches different files or only adds tests/docs.
- **[Story]**: User story label for story phases only.
- Every task includes an exact file path.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prepare traceability, shared fixtures, and validation scaffolding used by all stories.

- [X] T001 Create an audit disposition table for all non-retired `docs/todo/006.md` items in `specs/010-audit-006-remediation/research.md`
- [X] T002 [P] Add a `NEW-*` traceability checklist skeleton for implementation evidence in `specs/010-audit-006-remediation/quickstart.md`
- [X] T003 [P] Add shared test fixture notes for acceptor/initiator role coverage in `specs/010-audit-006-remediation/contracts/session-lifecycle.md`
- [X] T004 [P] Add shared backend feature-gate notes for Mongo/SQL/MSSQL tests in `specs/010-audit-006-remediation/contracts/store-log.md`
- [X] T005 [P] Add public API compatibility note placeholders for new additive APIs in `specs/010-audit-006-remediation/contracts/README.md`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared model/API seams that block multiple user stories.

**CRITICAL**: No user story implementation should begin until these contract and API seams are in place.

- [X] T006 Define shared test helpers for session state-machine action assertions in `crates/truefix-session/tests/audit006_session_lifecycle.rs`
- [X] T007 [P] Define shared two-process transport harness helpers for shutdown, TLS, and bounded-staging tests in `crates/truefix-transport/tests/audit006_transport_engine.rs`
- [X] T008 [P] Define shared store failure-injection helpers for file sequence/body persistence tests in `crates/truefix-store/tests/audit006_store_durability.rs`
- [X] T009 [P] Define shared config fixtures for strict parsing/default parity tests in `crates/truefix-config/tests/audit006_config_defaults.rs`
- [X] T010 [P] Define shared malformed-message fixtures for codec/dictionary validation tests in `crates/truefix-core/tests/audit006_codec_strictness.rs`
- [X] T011 [P] Define shared dictionary validation fixtures for required fields, section order, and nested groups in `crates/truefix-dict/tests/audit006_validation.rs`
- [X] T012 Add a common audit006 test naming convention comment to `specs/010-audit-006-remediation/quickstart.md`

**Checkpoint**: Foundation ready; user story phases can proceed independently by crate/domain.

---

## Phase 3: User Story 1 - Critical protocol and lifecycle integrity (Priority: P1) MVP

**Goal**: Fix P1 session, transport, engine, and store failures that can corrupt state, lose durable messages, hang resources, or expose protocol-visible lifecycle defects.

**Independent Test**: Run `cargo test -p truefix-session -p truefix-transport -p truefix-store -p truefix` plus any backend-gated tests available; P1 failures should be demonstrated by targeted tests before implementation and pass after implementation.

### Tests for User Story 1

- [X] T013 [P] [US1] Add tests for pre-logon logout, Logon rejection sequence consumption, teardown Logout replay, queued validation Logout, and schedule Logout wait in `crates/truefix-session/tests/audit006_session_lifecycle.rs`
- [X] T014 [P] [US1] Add tests for dictionary-validated `SequenceReset`, non-gap-fill queue cleanup, and gap-fill last-processed metadata in `crates/truefix-session/tests/audit006_sequence_reset.rs`
- [X] T015 [P] [US1] Add tests for `PossResend(97)` visibility and cancel-on-disconnect application hook behavior in `crates/truefix-session/tests/audit006_application_parity.rs`
- [X] T016 [P] [US1] Add tests for dynamic acceptor per-identity services and engine shutdown of accepted sessions in `crates/truefix/tests/audit006_engine_lifecycle.rs`
- [X] T017 [P] [US1] Add tests for bounded inbound staging and administrative fairness under pressure in `crates/truefix-transport/tests/audit006_backpressure.rs`
- [X] T018 [P] [US1] Add tests for TLS handshake timeout on acceptor and initiator paths in `crates/truefix-transport/tests/audit006_tls_timeout.rs`
- [X] T019 [P] [US1] Add tests for file-store corrupt-tail recovery and post-corruption append visibility in `crates/truefix-store/tests/audit006_corrupt_tail.rs`
- [X] T020 [P] [US1] Add tests for file-store in-memory sequence rollback after persistence errors in `crates/truefix-store/tests/audit006_seqfile_atomicity.rs`
- [X] T021 [P] [US1] Add AT scenarios for externally visible logout, `SequenceReset`, `PossResend`, and cancel-on-disconnect behavior in `crates/truefix-at/tests/audit006_protocol_scenarios.rs`

### Implementation for User Story 1

- [X] T022 [US1] Implement pre-logon logout handling and Logon rejection sequence consumption in `crates/truefix-session/src/state.rs`
- [X] T023 [US1] Route `SequenceReset` through dictionary validation and shared sequence checking in `crates/truefix-session/src/state.rs`
- [X] T024 [US1] Prevent teardown/error Logout messages from being persisted for later replay in `crates/truefix-session/src/state.rs`
- [X] T025 [US1] Clean stale queued messages on non-gap-fill `SequenceReset` and send Logout on queued validation disconnect in `crates/truefix-session/src/state.rs`
- [X] T026 [US1] Add last-processed metadata to gap-fill `SequenceReset` sends in `crates/truefix-session/src/state.rs`
- [X] T027 [US1] Change schedule-driven logout to wait for peer Logout or timeout in `crates/truefix-session/src/state.rs`
- [X] T028 [US1] Expose inbound `PossResend(97)` to the application callback path in `crates/truefix-session/src/lib.rs`
- [X] T029 [US1] Implement cancel-on-disconnect application/session hook plumbing in `crates/truefix-session/src/lib.rs`
- [X] T030 [US1] Wire cancel-on-disconnect actions into disconnect handling without inventing session-owned order state in `crates/truefix-session/src/state.rs`
- [X] T031 [US1] Delay acceptor LoggedOn transition until sequence handling completes in `crates/truefix-session/src/state.rs`
- [X] T032 [US1] Implement per-dynamic-identity store/services factory support in `crates/truefix/src/lib.rs`
- [X] T033 [US1] Propagate per-dynamic-identity services through acceptor routing in `crates/truefix-transport/src/lib.rs`
- [X] T034 [US1] Track accepted connection tasks and cancel them during engine shutdown in `crates/truefix-transport/src/lib.rs`
- [X] T035 [US1] Await or cancel accepted session tasks from engine shutdown/drop paths in `crates/truefix/src/lib.rs`
- [X] T036 [US1] Bound pre-application staging so `InChanCapacity` covers queued application messages in `crates/truefix-transport/src/lib.rs`
- [X] T037 [US1] Remove indefinite admin starvation by bounding admin bursts or using fair polling in `crates/truefix-transport/src/lib.rs`
- [X] T038 [US1] Wrap acceptor and initiator TLS handshakes in configured timeout behavior in `crates/truefix-transport/src/lib.rs`
- [X] T039 [US1] Ensure TLS timeout clears single-session active flags and spawned task state in `crates/truefix-transport/src/lib.rs`
- [X] T040 [US1] Repair file-store corrupt-tail open behavior so later valid appends remain indexed after restart in `crates/truefix-store/src/file.rs`
- [X] T041 [US1] Persist file-store sequence updates before committing in-memory sequence state in `crates/truefix-store/src/file.rs`

**Checkpoint**: US1 MVP is independently testable and closes the highest data-loss/lifecycle/resource risks.

---

## Phase 4: User Story 2 - Configuration, operational parity, and store robustness (Priority: P2)

**Goal**: Make defaults, config validation, operational APIs, logging, store reset, and transport options fail visibly and match clarified parity behavior.

**Independent Test**: Run `cargo test -p truefix-config -p truefix-transport -p truefix-store -p truefix-log -p truefix-dict -p truefix` and backend-gated tests where services are available.

### Tests for User Story 2

- [X] T042 [P] [US2] Add default parity and strict boolean/TLS protocol parsing tests in `crates/truefix-config/tests/audit006_config_defaults.rs`
- [X] T043 [P] [US2] Add duplicate session, exact initiator port-key, and `#` value parsing tests in `crates/truefix-config/tests/audit006_config_parser.rs`
- [X] T044 [P] [US2] Add accept-loop transient error, socket option diagnostic, TLS scheduled initiator, and backlog tests in `crates/truefix-transport/tests/audit006_transport_options.rs`
- [X] T045 [P] [US2] Add engine handle/session identity/state/skipped-session API tests in `crates/truefix/tests/audit006_engine_api.rs`
- [X] T046 [P] [US2] Add store reset transactionality and NoopStore sequence tracking tests in `crates/truefix-store/tests/audit006_store_reset.rs`
- [X] T047 [P] [US2] Add file-store save-and-advance and reset crash-safety tests in `crates/truefix-store/tests/audit006_file_store_safety.rs`
- [X] T048 [P] [US2] Add file log timestamp, write-error visibility, and LogConfig option threading tests in `crates/truefix-log/tests/audit006_file_log.rs`
- [X] T049 [P] [US2] Add required header/trailer and BeginString version validation tests in `crates/truefix-dict/tests/audit006_required_version.rs`
- [X] T050 [P] [US2] Add queued validation, schedule logout, and gap-fill metadata regression coverage in `crates/truefix-session/tests/audit006_operational_parity.rs`

### Implementation for User Story 2

- [X] T051 [US2] Change heartbeat timeout and test-request delay defaults to parity values in `crates/truefix-session/src/config.rs`
- [X] T052 [US2] Change field-order and PossDup default handling to strict parity behavior in `crates/truefix-config/src/builder.rs`
- [X] T053 [US2] Reject unrecognized `EnabledProtocols` values with typed config errors in `crates/truefix-config/src/builder.rs`
- [X] T054 [US2] Make boolean config parsing strict for all boolean keys in `crates/truefix-config/src/builder.rs`
- [X] T055 [US2] Detect duplicate `[SESSION]` identities during parsing in `crates/truefix-config/src/lib.rs`
- [X] T056 [US2] Report missing initiator port as `SocketConnectPort` in `crates/truefix-config/src/builder.rs`
- [X] T057 [US2] Preserve `#` inside config values outside documented comment positions in `crates/truefix-config/src/lib.rs`
- [X] T058 [US2] Continue accept loops after observable transient accept errors in `crates/truefix-transport/src/lib.rs`
- [X] T059 [US2] Surface explicit socket option application failures in `crates/truefix-transport/src/lib.rs`
- [X] T060 [US2] Add TLS scheduled initiator support following existing scheduled initiator patterns in `crates/truefix-transport/src/lib.rs`
- [X] T061 [US2] Add configurable listener backlog support in socket options and listener binding in `crates/truefix-transport/src/lib.rs`
- [X] T062 [US2] Add session identity accessors to session/reconnect/scheduled handles in `crates/truefix-transport/src/lib.rs`
- [X] T063 [US2] Expose acceptor session handles, session state query, and skipped sessions from engine APIs in `crates/truefix/src/lib.rs`
- [X] T064 [US2] Make SQL store reset atomic across messages and sequence numbers in `crates/truefix-store/src/sql.rs`
- [X] T065 [US2] Make Mongo store reset atomic across messages and sequence numbers in `crates/truefix-store/src/mongo.rs`
- [X] T066 [US2] Make MSSQL store reset atomic across messages and sequence numbers in `crates/truefix-store/src/mssql.rs`
- [X] T067 [US2] Reorder file-store save-and-advance so message body persistence cannot lag sequence advancement in `crates/truefix-store/src/file.rs` (re-evaluated: kept seq-first ordering, which is the actually-safe direction given this codebase's wire-write-before-persist invariant; see code comment and research.md disposition)
- [X] T068 [US2] Track NoopStore sender/target sequence numbers in memory in `crates/truefix-store/src/noop.rs`
- [X] T069 [US2] Prevent stale body replay after file-store reset crash in `crates/truefix-store/src/file.rs`
- [X] T070 [US2] Always timestamp file log event lines and surface write failures in `crates/truefix-log/src/file.rs`
- [X] T071 [US2] Thread file log backend options through generic log configuration in `crates/truefix-log/src/lib.rs`
- [X] T072 [US2] Add required header/trailer field validation support in `crates/truefix-dict/src/model.rs`
- [X] T073 [US2] Enforce required header/trailer and BeginString version validation in `crates/truefix-dict/src/validate.rs`

**Checkpoint**: US2 operational parity and diagnostics are independently testable after US2 tasks.

---

## Phase 5: User Story 3 - Lower-risk hardening, API consistency, and performance hygiene (Priority: P3)

**Goal**: Close strictness, API, performance, diagnostic, and lower-risk parity gaps without changing externally visible behavior except where stricter validation is intended.

**Independent Test**: Run `cargo test -p truefix-core -p truefix-dict -p truefix-store -p truefix-log -p truefix-transport -p truefix-session` plus affected AT scenarios.

### Tests for User Story 3

- [X] T074 [P] [US3] Add strict numeric parsing, checksum terminator, and three-digit checksum tests in `crates/truefix-core/tests/audit006_codec_strictness.rs`
- [X] T075 [P] [US3] Add validation-policy tests for section order and nested repeating groups in `crates/truefix-dict/tests/audit006_validation.rs`
- [X] T076 [P] [US3] Add file log/store growth policy tests in `crates/truefix-log/tests/audit006_file_growth.rs`
- [X] T077 [P] [US3] Add file store growth and duplicate detection tests in `crates/truefix-store/tests/audit006_growth_duplicate.rs`
- [X] T078 [P] [US3] Add classify-buffer allocation regression coverage in `crates/truefix-transport/tests/audit006_buffering.rs`
- [X] T079 [P] [US3] Add `as_bool` diagnostic fidelity tests in `crates/truefix-core/tests/audit006_field_diagnostics.rs`

### Implementation for User Story 3

- [X] T080 [US3] Remove whitespace trimming from integer and decimal typed field accessors in `crates/truefix-core/src/field.rs`
- [X] T081 [US3] Verify checksum trailer SOH in frame length detection in `crates/truefix-core/src/framing.rs`
- [X] T082 [US3] Require direct decode checksum values to be exactly three ASCII digits in `crates/truefix-core/src/codec/decode.rs`
- [X] T083 [US3] Enforce section-order violations through validation policy in `crates/truefix-dict/src/validate.rs`
- [X] T084 [US3] Use nested group definitions when validating nested group members in `crates/truefix-dict/src/validate.rs`
- [X] T085 [US3] Add file log size-limit or rotation configuration and behavior in `crates/truefix-log/src/file.rs`
- [X] T086 [US3] Add file store growth-bound policy preserving resend/recovery semantics in `crates/truefix-store/src/file.rs`
- [X] T087 [US3] Add duplicate sequence detection capability without changing `save()` contract in `crates/truefix-store/src/lib.rs`
- [X] T088 [US3] Implement duplicate detection for MemoryStore in `crates/truefix-store/src/memory.rs`
- [X] T089 [US3] Implement duplicate detection for file-backed stores in `crates/truefix-store/src/file.rs`
- [X] T090 [US3] Improve `as_bool` invalid UTF-8 diagnostics without lossy replacement in `crates/truefix-core/src/field.rs`
- [X] T091 [US3] Reduce per-message allocation in buffered frame classification in `crates/truefix-transport/src/lib.rs`

**Checkpoint**: US3 hardening and strictness changes are independently testable.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final traceability, docs, and verification across all stories.

- [X] T092 [P] Update session public API docs for `PossResend` and cancel-on-disconnect in `crates/truefix-session/src/lib.rs`
- [X] T093 [P] Update transport public API docs for shutdown, backlog, and TLS scheduled behavior in `crates/truefix-transport/src/lib.rs`
- [X] T094 [P] Update engine public API docs for session handles, session query, and skipped sessions in `crates/truefix/src/lib.rs`
- [X] T095 [P] Update store public API docs for duplicate detection and file growth behavior in `crates/truefix-store/src/lib.rs`
- [X] T096 [P] Update log public API docs for file growth behavior and backend options in `crates/truefix-log/src/lib.rs`
- [X] T097 [P] Update configuration/default compatibility notes in `specs/010-audit-006-remediation/quickstart.md`
- [X] T098 Update audit disposition table with implemented/refuted/deferred evidence in `specs/010-audit-006-remediation/research.md`
- [X] T099 Run full validation command `cargo test` and record outcome in `specs/010-audit-006-remediation/quickstart.md`
- [X] T100 Run focused validation commands from quickstart and record any gated/skipped backend tests in `specs/010-audit-006-remediation/quickstart.md`
- [X] T101 Run approved clippy gates and record outcome in `specs/010-audit-006-remediation/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- Phase 1 Setup: no dependencies.
- Phase 2 Foundational: depends on Phase 1.
- Phase 3 US1: depends on Phase 2 and is the MVP.
- Phase 4 US2: depends on Phase 2; may run parallel with US1 where files do not overlap, but final validation should run after US1.
- Phase 5 US3: depends on Phase 2; can run parallel after US1/US2 test scaffolding if crate ownership does not overlap.
- Phase 6 Polish: depends on selected story phases being complete.

### User Story Dependencies

- US1: no dependency on US2/US3; should complete first for MVP risk reduction.
- US2: independent operational parity layer after shared scaffolding, but some session tasks overlap with US1 and should merge after US1.
- US3: independent hardening layer after shared scaffolding, but dictionary/core strictness should be coordinated with US2 validation defaults.

### Within Each User Story

- Tests first, expected to fail before implementation.
- API/trait additions before backend implementations that depend on them.
- State-machine behavior before live transport/AT validation.
- Backend feature-gated tests can be added before implementation but may remain ignored until service setup is available.

## Parallel Opportunities

- T003-T005 can run in parallel after T001.
- T007-T011 can run in parallel after T006.
- US1 test tasks T013-T021 can run in parallel.
- US2 test tasks T042-T050 can run in parallel.
- US3 test tasks T074-T079 can run in parallel.
- Store/log tasks can run parallel with config/dict tasks when files do not overlap.

## Parallel Example: User Story 1

```bash
Task: "T013 [US1] Add session lifecycle tests in crates/truefix-session/tests/audit006_session_lifecycle.rs"
Task: "T016 [US1] Add engine lifecycle tests in crates/truefix/tests/audit006_engine_lifecycle.rs"
Task: "T019 [US1] Add file-store corrupt-tail tests in crates/truefix-store/tests/audit006_corrupt_tail.rs"
```

## Parallel Example: User Story 2

```bash
Task: "T042 [US2] Add config default tests in crates/truefix-config/tests/audit006_config_defaults.rs"
Task: "T044 [US2] Add transport option tests in crates/truefix-transport/tests/audit006_transport_options.rs"
Task: "T048 [US2] Add file log tests in crates/truefix-log/tests/audit006_file_log.rs"
```

## Parallel Example: User Story 3

```bash
Task: "T074 [US3] Add codec strictness tests in crates/truefix-core/tests/audit006_codec_strictness.rs"
Task: "T075 [US3] Add dictionary validation tests in crates/truefix-dict/tests/audit006_validation.rs"
Task: "T077 [US3] Add store growth duplicate tests in crates/truefix-store/tests/audit006_growth_duplicate.rs"
```

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 and Phase 2.
2. Complete all US1 tests T013-T021 and confirm they fail for current behavior.
3. Complete US1 implementation T022-T041.
4. Validate `cargo test -p truefix-session -p truefix-transport -p truefix-store -p truefix`.
5. Stop and review P1 behavior before starting broader operational parity.

### Incremental Delivery

1. Deliver US1 P1 lifecycle/data-integrity fixes.
2. Deliver US2 operational parity and diagnostics.
3. Deliver US3 strictness/performance/API hardening.
4. Run full quickstart and clippy gates.

### Validation Gates

- No task is complete without a linked `NEW-*` test or documented disposition.
- No critical-path implementation may introduce panic/unwrap/expect/unreachable.
- No public API addition is complete without docs and compatibility notes.
