# Tasks: Complete Remaining QuickFIX/J Parity (excl. QuickFIX/Go-only)

**Feature**: `002-qfj-parity-completion` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Organized by user story (priority order P1 → P3), mapped to the plan's internal stages G1–G10. Each
story phase is an independently testable increment with a checkpoint. Test tasks are included because the
constitution mandates test-first discipline and the AT suite as the release gate (Principles II/V).

**Conventions**: `[P]` = parallelizable (distinct files, no incomplete-task dependency). `[USx]` = user
story. The full workspace gate (fmt / clippy `-D warnings` / tests / AT conformance) MUST be green at
every checkpoint (SC-013). No reference source/data copied (Principle III).

---

## Phase 1: Setup

- [X] T001 [P] Enable `sqlx` Postgres+MySQL features, add `rustls-pemfile`, and confirm `metrics` is a real dependency in `Cargo.toml` (workspace) and the consuming crates' `Cargo.toml`
- [X] T002 [P] Add Postgres + MySQL service containers and `DATABASE_URL_PG`/`DATABASE_URL_MYSQL` env to CI; gate SQL tests on availability (SQLite always) in `.github/workflows/` (or existing CI config)
- [X] T003 [P] Run `cargo deny` license audit for new deps (`rustls-pemfile`, `sqlx-postgres`, `sqlx-mysql`, `metrics`); record Apache-2.0/MIT compatibility in `docs/parity-matrix.md` (Principle III, research R11)
- [X] T004 [P] Scaffold `MIGRATION.md` (breaking callback change section placeholder) and a stance-update table stub in `docs/parity-matrix.md`
- [ ] T005 [P] (folded into US1/US7 when first consumer lands) Add shared test fixtures: `.cfg` fixtures under `crates/truefix/tests/fixtures/`, an `rcgen` cert helper, a `metrics` test-recorder helper, and a DB-availability gate helper in `crates/truefix-store/tests/common/`

## Phase 2: Foundational (blocking prerequisites)

- [X] T006 Define the shared typed-error surfaces with docs so dependent stories compile against stable types: `ConfigError` in `crates/truefix-config/src/error.rs`, and callback outcome types `Reject`/`DoNotSend`/`BusinessReject` in `crates/truefix-core/src/error.rs` (bodies filled in their owning stories)

---

## Phase 3: User Story 1 — Config-driven engine start (P1, stage G3)

**Goal**: Start an initiator and an acceptor from one `.cfg` with no code (FR-013/014/015).
**Independent test**: Engine starts both sessions from a fixture `.cfg`, completes logon→heartbeat; a bad key fails fast with a typed `ConfigError` naming key+session (SC-001).

- [X] T007 [P] [US1] Integration test: start engine from a one-acceptor + one-initiator `.cfg`, assert logon→heartbeat in `crates/truefix/tests/config_start.rs`
- [X] T008 [P] [US1] Negative tests: each `ConfigError` variant (MissingRequired / InvalidValue / UnusableResource / UnknownConnectionType) with no partial start in `crates/truefix/tests/config_errors.rs`
- [X] T009 [US1] Implement `EngineConfig::from_settings` with DEFAULT+per-SESSION precedence in `crates/truefix-config/src/builder.rs`
- [X] T010 [US1] Implement typed `ConfigError { key, session, kind }` and per-key validation in `crates/truefix-config/src/error.rs` + `builder.rs`
- [X] T011 [US1] Resolve `StoreSpec`/`LogSpec`/`Schedule`/`TransportSpec`/`TlsSpec` from keys in `crates/truefix-config/src/builder.rs`
- [X] T012 [US1] Add `Engine::start(settings)` routing each session by `ConnectionType` to acceptor/initiator construction in `crates/truefix/src/lib.rs`
- [X] T013 [US1] Move config-mapping keys `Recognized → Implemented` in `crates/truefix-config/src/keys.rs` (FR-027)

**Checkpoint G3**: `cargo test -p truefix --test config_start` green; negative cases typed; gate green.

---

## Phase 4: User Story 2 — Restart-survivable resend (P1, stage G1)

**Goal**: Satisfy a post-restart ResendRequest from durable storage (FR-001/002/003).
**Independent test**: Restart against the same file store; a ResendRequest replays exact PossDup bodies; admin ranges gap-fill; `PersistMessages=N` gap-fills (SC-002).

- [X] T014 [P] [US2] Session test: store-backed resend replays exact PossDup bodies; admin → GapFill; `PersistMessages=N` → GapFill in `crates/truefix-session/tests/restart_resend.rs`
- [X] T015 [P] [US2] Two-process test: send→drop→restart→ResendRequest replays across process boundary in `crates/truefix-transport/tests/restart_continuity.rs`
- [X] T016 [US2] Session-store range access (MessageStore::get(begin,end) already provided it); added `Session::seed_sent_messages` as the rehydration injection point in `crates/truefix-session/src/state.rs`
- [X] T017 [US2] `Session::with_store`: `send_stored` writes the encoded body to the store; `build_resend` uses store-provided bodies; skip writes when `PersistMessages=N` in `crates/truefix-session/src/state.rs`
- [X] T018 [US2] Transport pre-loads the requested resend range from the async store and feeds `build_resend` in `crates/truefix-transport/src/lib.rs`
- [X] T019 [US2] Stance updates for `PersistMessages` in `crates/truefix-config/src/keys.rs` (FR-027)

**Checkpoint G1**: restart-resend tests green (SC-002); sans-IO `Session` purity preserved; gate green.

---

## Phase 5: User Story 3 — Repeating-group parsing & validation (P1, stage G2)

**Goal**: Dictionary-driven group/component decode + group validation (FR-004/005).
**Independent test**: Correct group accepted; wrong-count/missing-delimiter/out-of-order/nested/zero-count rejected per toggle (SC-003).

- [X] T020 [P] [US3] Core decode table tests: correct + wrong-count + missing-delimiter + out-of-order + nested + zero-count in `crates/truefix-core/tests/groups.rs`
- [X] T021 [P] [US3] Dict group-validation tests per toggle in `crates/truefix-dict/tests/group_validation.rs`
- [X] T022 [P] [US3] AT scenarios `14i`/`14j`/`21`/`QFJ934` in `crates/truefix-at/src/scenarios.rs`
- [X] T023 [US3] Implement `decode_with_dict` (group/component, nested, delimiter-bounded) in `crates/truefix-core/src/codec/decode.rs` and `crates/truefix-core/src/group.rs`
- [X] T024 [US3] Implement group validation (count/delimiter/order) + `ValidationOptions` toggles in `crates/truefix-dict/src/validate.rs` and `src/model.rs`
- [X] T025 [US3] Group validation flows through the acceptor's dict validator (Services.validator -> DataDictionary::validate, now group-aware); structured `decode_with_groups` available for US6 (proven by AT 14i/14j/21/QFJ934)
- [X] T026 [US3] Stance updates for `ValidateUnorderedGroupFields` / `FirstFieldInGroupIsDelimiter` in `crates/truefix-config/src/keys.rs`

**Checkpoint G2**: group decode/validation tests + 14i/14j/21/QFJ934 AT green (SC-003); gate green.

---

## Phase 6: User Story 4 — Inbound integrity, reject layers, garbled, precision, switches (P2, stage G4)

**Goal**: FIX message-integrity validation, two reject layers, RejectGarbledMessage, timestamp precision, correctness switches (FR-006/007/008/009/010/012).
**Independent test**: Each malformed class reaches its FIX-correct outcome; timestamps round-trip per precision; switches change behaviour (SC-004/005).

- [ ] T027 [P] [US4] (partial: 14h_RepeatedTag done) AT integrity scenarios: `3b_InvalidChecksum`, `2m_BodyLengthValueNotCorrect`, `1d_InvalidLogonWrongBeginString`, `1c_InvalidSenderCompID`/`1c_InvalidTargetCompID`, `2o_SendingTimeValueOutOfRange`, `2t_FirstThreeFieldsOutOfOrder`, `14g` in `crates/truefix-at/src/scenarios.rs`
- [X] T028 [P] [US4] AT garbled scenarios `2d`/`3c` with `RejectGarbledMessage` on/off in `crates/truefix-at/src/scenarios.rs`
- [X] T029 [P] [US4] Session tests: TimeStampPrecision round-trip + `HeartBeatTimeoutMultiplier`/`TestRequestDelayMultiplier` in `crates/truefix-session/tests/integrity.rs`
- [ ] T030 [US4] (partial: BeginString/CheckCompID/repeated-tag(13) done; BodyLength/CheckSum/SendingTime-range covered by decode+CheckLatency; remaining: first-three-field order(14)/header-body-trailer order(14) — needs wire-order tracking through decode, deferred) Implement integrity checks in `crates/truefix-session/src/state.rs` + `crates/truefix-dict/src/validate.rs`
- [X] T031 [US4] Implement `RejectGarbledMessage` path: transport notifies session of a garbled frame -> session emits Reject (35=3) when enabled in `crates/truefix-transport/src/lib.rs` + `crates/truefix-session/src/state.rs`
- [X] T032 [US4] Two-layer rejection routing (session 35=3 vs business 35=j) already existed from 001 (`validate_app` routes on `error.business`) — verified still correct
- [X] T033 [US4] Implement `TimeStampPrecision` (SECONDS/MILLIS/MICROS/NANOS, default MILLIS) formatting in `crates/truefix-core/src/field.rs` + `crates/truefix-session/src/config.rs`
- [ ] T034 [US4] (partial: PersistMessages [US2] + HeartBeatTimeoutMultiplier done; TestRequestDelayMultiplier pending) Honour switches in `crates/truefix-session/src/state.rs`
- [X] T035 [US4] Stance updates for the integrity/precision/switch keys in `crates/truefix-config/src/keys.rs`

**Checkpoint G4**: integrity + garbled AT scenarios green; precision/switch tests green (SC-004/005); gate green.

---

## Phase 7: User Story 11 — Reverse routing (P3, stage G4)

**Goal**: Reverse routing header fields on replies/rejects (FR-011). Delivered alongside US4.
**Independent test**: Reply/reject to a routed message reverses the routing pairs; empty tags handled.

- [ ] T036 [P] [US11] AT scenarios `ReverseRoute` + `ReverseRouteWithEmptyRoutingTags` in `crates/truefix-at/src/scenarios.rs`
- [ ] T037 [US11] Implement a reverse-route header helper (OnBehalfOf↔DeliverTo pairs; empty-tags rule) in `crates/truefix-core/src/message.rs`, applied to engine replies/rejects in `crates/truefix-session/src/state.rs`

**Checkpoint**: ReverseRoute AT scenarios green; gate green.

---

## Phase 8: User Story 5 — Typed Application callback outcomes (P2, stage G5)

**Goal**: Typed Reject/DoNotSend/BusinessReject callback results — breaking change (FR-016).
**Independent test**: Callbacks cause refused logon / suppressed send / emitted 35=j with reason+ref tag (SC-006).

- [ ] T038 [P] [US5] AT/session tests for each typed outcome's effect in `crates/truefix-at/src/scenarios.rs`
- [ ] T039 [US5] Fill `Reject`/`DoNotSend`/`BusinessReject` types (reason code + optional ref tag) in `crates/truefix-core/src/error.rs`
- [ ] T040 [US5] Change `Application` callback signatures and plumb engine effects (refuse logon / suppress + don't store send / emit 35=j) in `crates/truefix-session/src/application.rs` + `crates/truefix-session/src/state.rs` + `crates/truefix/src/application.rs`
- [ ] T041 [US5] Update examples to the typed callbacks in `crates/truefix/examples/{executor,banzai,ordermatch,multi_acceptor}.rs`
- [ ] T042 [US5] Write the callback migration section in `MIGRATION.md` + record the semver bump (Principle I; addresses checklist CHK012)

**Checkpoint G5**: typed-callback effects verified; migration documented; gate green.

---

## Phase 9: User Story 6 — All-message typed codegen + MessageCracker (P2, stage G6)

**Goal**: Typed message/group/component structs for every message + a working cracker (FR-020/021/022; Principle IV).
**Independent test**: Typed messages round-trip byte-identically; cracker dispatches by MsgType/version; dual-track hash holds (SC-007).

- [ ] T043 [P] [US6] Codegen golden/shape tests in `crates/truefix-dict/tests/codegen.rs`
- [ ] T044 [P] [US6] Typed↔generic byte-identical round-trip test in `crates/truefix-dict/tests/typed_roundtrip.rs`
- [ ] T045 [P] [US6] MessageCracker dispatch test in `crates/truefix/tests/cracker.rs`
- [ ] T046 [P] [US6] Dual-track FNV-1a hash assertion (codegen vs runtime dict) in `crates/truefix-dict/tests/dual_track.rs`
- [ ] T047 [US6] Codegen field module: typed accessors + value enums in `crates/truefix-dict/src/codegen/fields.rs` (invoked from `build.rs`)
- [ ] T048 [US6] Codegen per-message structs (all messages, wrapping generic `Message`) in `crates/truefix-dict/src/codegen/messages.rs`
- [ ] T049 [US6] Codegen nested group/component structs in `crates/truefix-dict/src/codegen/groups.rs`
- [ ] T050 [US6] Implement `MessageCracker` dispatch by `(BeginString, MsgType)` in `crates/truefix/src/cracker.rs`
- [ ] T051 [US6] Generate across all targeted versions and verify dual-track hash in `crates/truefix-dict/build.rs`

**Checkpoint G6**: codegen + round-trip + cracker + dual-track tests green (SC-007; Principle IV complete); gate green.

---

## Phase 10: User Story 7 — TLS/mTLS from configuration (P3, stage G7)

**Goal**: TLS incl. mTLS/SNI/min-version from `.cfg` (FR-017).
**Independent test**: mTLS session from `.cfg` alone; un-credentialed client refused; low-version peer refused (SC-008).

- [ ] T052 [P] [US7] Transport TLS test: mTLS from config + min-version refusal + un-credentialed refusal in `crates/truefix-transport/tests/tls.rs`
- [ ] T053 [US7] Build `rustls::ServerConfig`/`ClientConfig` from `TlsSpec` (key/cert/CA via `rustls-pemfile`) in `crates/truefix-transport/src/lib.rs`
- [ ] T054 [US7] Implement mTLS client-cert verifier (`NeedClientAuth`), SNI/server-name, and minimum TLS version in `crates/truefix-transport/src/lib.rs`
- [ ] T055 [US7] Stance updates for the SSL/TLS keys in `crates/truefix-config/src/keys.rs`

**Checkpoint G7-TLS**: mTLS-from-config test green (SC-008); gate green.

---

## Phase 11: User Story 8 — Schedule reset + weekly windows (P3, stage G8)

**Goal**: Session-layer scheduled reset + weekly StartDay/EndDay (FR-018).
**Independent test**: Boundary crossing performs disconnect→reset→clear-store→reconnect; weekly windows correct; non-stop unaffected (SC-009).

- [ ] T056 [P] [US8] Weekly `is_in_session` tests across day boundaries in `crates/truefix-session/tests/schedule.rs`
- [ ] T057 [P] [US8] Schedule-reset integration (disconnect→reset→clear-store→reconnect; non-stop skip) in `crates/truefix-session/tests/schedule_reset.rs`
- [ ] T058 [US8] Add `StartDay`/`EndDay` weekly windows to `crates/truefix-session/src/schedule.rs`
- [ ] T059 [US8] Implement `schedule_reset` boundary semantics in `crates/truefix-session/src/schedule_reset.rs` (new), wired in `crates/truefix-transport/src/lib.rs`
- [ ] T060 [US8] Stance updates for `StartDay`/`EndDay`/schedule keys in `crates/truefix-config/src/keys.rs`

**Checkpoint G8**: schedule-reset + weekly tests green (SC-009); gate green.

---

## Phase 12: User Story 9 — Metrics export (P3, stage G9)

**Goal**: Export metrics via the `metrics` facade (FR-023; Principle I).
**Independent test**: Gauges/counters reflect a logon→traffic→reconnect cycle (SC-010).

- [ ] T061 [P] [US9] Metrics test via a test recorder over a logon→traffic→reconnect cycle in `crates/truefix-transport/tests/metrics.rs`
- [ ] T062 [US9] Emit `truefix_session_state`/`next_sender_seqnum`/`next_target_seqnum` gauges and `messages_sent_total`/`messages_received_total`/`reconnects_total` counters (labelled by SessionID) in `crates/truefix-transport/src/lib.rs`
- [ ] T063 [US9] Document the metrics surface in `docs/parity-matrix.md` (observability/Principle I)

**Checkpoint G9**: metrics test green (SC-010; Principle I complete); gate green.

---

## Phase 13: User Story 10 — Socket options + multi-endpoint failover (P3, stage G7)

**Goal**: Full socket options + initiator failover (FR-019).
**Independent test**: Socket options applied; initiator fails over to a backup when the primary is unreachable (SC-011).

- [ ] T064 [P] [US10] Failover test: backup-endpoint rotation on reconnect; all-unreachable retries without panic in `crates/truefix-transport/tests/failover.rs`
- [ ] T065 [P] [US10] Socket-options-applied test in `crates/truefix-transport/tests/socket_options.rs`
- [ ] T066 [US10] Extend `SocketOptions` and `apply()` for the full set (keep-alive, buffers, reuse-address, linger, etc.) in `crates/truefix-transport/src/lib.rs`
- [ ] T067 [US10] Implement `ConnectEndpointSet` from `SocketConnectHost<N>`/`SocketConnectPort<N>` and rotate on reconnect in `crates/truefix-transport/src/lib.rs` + `crates/truefix-session/src/config.rs`
- [ ] T068 [US10] Stance updates for socket/connect keys in `crates/truefix-config/src/keys.rs`

**Checkpoint G7-Socket**: failover + socket-options tests green (SC-011); gate green.

---

## Phase 14: User Story 12 — Storage & logging completeness (P3, stage G10)

**Goal**: SQL PG/MySQL/SQLite, CachedFileStore cache+fsync, log switches (FR-024/025/026).
**Independent test**: SQL suite green across the three DBs (gated); cached store caches/flushes + survives restart; log switches change output (SC-012).

- [ ] T069 [P] [US12] SQL store tests for Postgres/MySQL/SQLite (gated on availability) in `crates/truefix-store/tests/sql_backends.rs`
- [ ] T070 [P] [US12] CachedFileStore cache + `FileStoreSync` + restart test in `crates/truefix-store/tests/cached.rs`
- [ ] T071 [P] [US12] Log output-switch tests (heartbeat filter, ms, visibility, session-id prefix) in `crates/truefix-log/tests/switches.rs`
- [ ] T072 [US12] Add Postgres + MySQL pools + portable schema + table-name/pool config to `crates/truefix-store/src/sql.rs`
- [ ] T073 [US12] Add Postgres + MySQL to `crates/truefix-log/src/sql.rs`
- [ ] T074 [US12] Implement real in-memory cache + `FileStoreSync` fsync toggle in `crates/truefix-store/src/file.rs`
- [ ] T075 [US12] Implement log output switches in `crates/truefix-log/src/{screen,file}.rs`
- [ ] T076 [US12] Final Appendix A stance sweep + `key_coverage` assertion in `crates/truefix-config/src/keys.rs` and `crates/truefix-config/tests/key_coverage.rs` (FR-027; SC-014)

**Checkpoint G10**: SQL multi-backend + cached + log-switch tests green (SC-012); stances accurate (SC-014); gate green.

---

## Phase 15: Polish & Cross-Cutting

- [ ] T077 [P] Update `docs/acceptance-record.md` with the new AT scenario classes and counts
- [ ] T078 [P] Update `docs/parity-matrix.md` stances and tick off the resolved audit TODO IDs in `docs/todo-gap-analysis.md`
- [ ] T079 [P] Update `README.md` for config-driven start, the typed Application API, and TLS-from-config
- [ ] T080 Finalize `MIGRATION.md` + apply the semver bump across crate `Cargo.toml` versions (Principle I; FR-028)
- [ ] T082 [P] Add initiator-side validation parity coverage: a two-process test where the **initiator** receives malformed/group/garbled messages and produces the same FIX-correct outcomes as the acceptor (or document that the sans-IO `Session` makes role parity structural) in `crates/truefix-transport/tests/initiator_validation.rs` (Constitution VI)
- [ ] T081 Full-gate sweep across all stages: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo test -p truefix-at --test conformance`, `cargo test -p truefix-store --features sql` (SC-013)

---

## Dependencies & Story Completion Order

- **Setup (T001–T005)** → **Foundational (T006)** → user stories.
- **P1 stories are mutually independent**: US1 (G3), US2 (G1), US3 (G2) touch different crates and may
  proceed in parallel after T006.
- **Cross-story dependencies**:
  - US3 (group model) **blocks** US6 (codegen group/component structs) and parts of US4 (group-dependent
    validation toggles).
  - US1 (config mapping) **blocks** US7 (TLS-from-config), US8 (schedule-from-config), US10
    (socket/endpoints-from-config).
  - US4 delivers the reverse-route helper used by US11.
- **MVP = P1 (US1 + US2 + US3)**: a config-startable engine that survives restart and correctly handles
  repeating groups — the highest-value correctness + usability slice.

## Parallel Execution Examples

- **Setup**: T001–T005 all `[P]` (distinct files).
- **Within US3**: T020/T021/T022 (tests, distinct files) in parallel; then T023→T024→T025 sequential
  (shared decode/validation path), T026 in parallel with tests.
- **Across P1**: US1, US2, US3 implementation tracks run in parallel (different crates) once T006 lands.
- **Within US6**: T043–T046 (tests) `[P]`; T047/T048/T049 codegen modules `[P]` (distinct files) then
  T050→T051.
- **Within US12**: T069/T070/T071 (tests) `[P]`; T072/T073/T074/T075 `[P]` (distinct files).

## Implementation Strategy

1. Land Setup + Foundational, then the **P1 trio (US1/US2/US3)** to reach the MVP and a green gate.
2. Proceed to **P2 (US4 → US11 → US5 → US6)**; US6 is the largest (all-message codegen) — keep the
   dual-track hash and round-trip equality green continuously.
3. Finish **P3 (US7/US8/US9/US10/US12)** — config-dependent transport/schedule/storage breadth.
4. Polish: docs, migration, semver, full-gate sweep. The AT conformance suite is the release gate at
   every checkpoint (Principle II/V; SC-013).
