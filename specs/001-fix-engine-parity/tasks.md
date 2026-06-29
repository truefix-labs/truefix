---
description: "Task list for TrueFix — full QuickFIX/J-parity production FIX engine"
---

# Tasks: TrueFix — Full QuickFIX/J-Parity Production FIX Engine

**Input**: Design documents from `specs/001-fix-engine-parity/`
**Prerequisites**: plan.md, spec.md (US1–US13), research.md (R1–R10), data-model.md, contracts/, checklists/parity.md

**Tests**: REQUIRED (Constitution Principle V; spec FR-M / US12). Table-driven unit tests + two-process
integration tests + ported AT suite. Test-first preferred (red → green → refactor).

**Organization**: Phases follow the plan's dependency-ordered stages **S0–S9** (one-pass full parity,
internally staged with a verifiable checkpoint each). Tasks carry `[US#]` labels mapping to spec user
stories for traceability. Because this is one engine (not independent stories), stages form a dependency
chain — later stages depend on earlier checkpoints.

**Format**: `- [ ] [TaskID] [P?] [Story?] Description with file path`
- `[P]` = parallelizable (different files, no incomplete dependency).
- Crate paths are under `crates/`; integration tests under `tests/`; benches under `benches/`.

**Parity baselines**: spec Appendix A (~150 config keys), Appendix B (73 server AT scenarios + special
suites). `checklists/parity.md` is the requirements-quality coverage guard.

---

## Phase 1: Setup — Stage S0 (Workspace & CI)

**Purpose**: Workspace skeleton, lint policy, CI, license audit, dictionary-source pipeline scaffold.

- [X] T001 Create cargo workspace `Cargo.toml` with members for all crates (truefix-core, -dict, -session, -store, -log, -transport, -config, truefix, -at) and `examples/*`, per plan.md structure
- [X] T002 [P] Add empty library crates with `#![forbid(unsafe_code)]` and crate-level docs in `crates/*/src/lib.rs`
- [X] T003 [P] Define shared workspace lints (deny `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`, `clippy::indexing_slicing` on lib targets) in workspace `Cargo.toml` `[workspace.lints]`
- [X] T004 [P] Add shared dependencies at workspace level (tokio, rustls, tokio-rustls, thiserror, rust_decimal, time, serde, sqlx, tracing, metrics) with versions pinned
- [X] T005 [P] Configure CI: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` in `.github/workflows/ci.yml`
- [X] T006 [P] Add `cargo-deny` config `deny.toml` (allow Apache-2.0/MIT/BSD/ISC/Zlib/Unicode-DFS; deny GPL/LGPL/MPL into source) and a CI `cargo deny check` job (Constitution III; research R10)
- [X] T007 Scaffold dictionary-source pipeline: vendoring of FIX Orchestra/Repository inputs + a generator stub that transforms them into TrueFix's normalized dictionary format in `crates/truefix-dict/dict-src/` (research R4; FR-A2). No QuickFIX/J XML copied
- [X] T008 [P] Add MSRV pin (concrete version, e.g. the stable release at S0 start) + `rust-toolchain.toml` and document the MSRV bump policy in workspace `README.md`
- [ ] T099 [P] [US2] Capture byte-exact reference wire vectors for the canonical + per-version message set by running QuickFIX/J and recording its **output bytes** (BodyLength/CheckSum), stored as test fixtures in `crates/truefix-core/tests/fixtures/reference/`. Output data only — no QuickFIX/J source copied (Constitution III). Prerequisite of T010 and T072. (SC-002)

**Checkpoint S0**: `cargo build`/`fmt`/`clippy -D warnings`/`test` green on empty crates; `cargo deny` clean; dictionary-source pipeline stub runs.

---

## Phase 2: Foundational — Stage S1 (truefix-core: model & codec) 🎯 MVP-blocking

**Purpose**: Message model + SOH codec + field types + BodyLength/CheckSum + typed errors. BLOCKS all
later stages. Serves US2.

**⚠️ No later stage can begin until S1 checkpoint passes.**

### Tests for S1 (write first)

- [X] T009 [P] [US2] Table-driven field-type/converter tests (Int, Decimal, Char, Boolean, String, Data, UtcTimestamp ns/ps, UtcTimeOnly, UtcDateOnly, MonthYear) in `crates/truefix-core/tests/field_types.rs`
- [X] T010 [P] [US2] Round-trip + byte-exact BodyLength/CheckSum vectors for canonical messages (NewOrderSingle, ExecutionReport, Logon, Heartbeat, ResendRequest, SequenceReset, Reject) against the reference fixtures from T099 in `crates/truefix-core/tests/roundtrip.rs` (SC-002)
- [X] T011 [P] [US2] Negative codec tests (garbled, truncated, BodyLength mismatch, bad checksum, non-numeric tag) asserting typed errors and no panic in `crates/truefix-core/tests/garbled.rs` (FR-B8, SC-005)
- [X] T012 [P] [US2] Nested repeating-group parse/encode tests (delimiter ordering, count=0, missing nested delimiter) in `crates/truefix-core/tests/groups.rs`

### Implementation for S1

- [X] T013 [P] [US2] Implement typed error enums (`DecodeError`, `FieldError`) with thiserror in `crates/truefix-core/src/error.rs`
- [X] T014 [P] [US2] Implement `FieldValue` types and converters (rust_decimal, time ns timestamps) in `crates/truefix-core/src/types/`
- [X] T015 [US2] Implement `Field` (with retained raw form for round-trip) in `crates/truefix-core/src/field.rs`
- [X] T016 [US2] Implement `FieldMap` (ordered fields + nested groups) in `crates/truefix-core/src/field_map.rs`
- [X] T017 [US2] Implement `Group` (nestable, delimiter, count) in `crates/truefix-core/src/group.rs`
- [X] T018 [US2] Implement `Message` (header/body/trailer regions, msg_type/begin_string) in `crates/truefix-core/src/message.rs`
- [X] T019 [US2] Implement SOH decoder `bytes → Result<Message, DecodeError>` (verifies BodyLength/CheckSum, preserves raw forms) in `crates/truefix-core/src/codec/decode.rs`
- [X] T020 [US2] Implement encoder `Message → bytes` (header order, auto BodyLength/CheckSum, RawData length-prefix) in `crates/truefix-core/src/codec/encode.rs`
- [X] T021 [P] [US2] Add `MessageFactory` and `MessageCracker` trait skeletons (typed dispatch contract) in `crates/truefix-core/src/factory.rs` and `crates/truefix-core/src/cracker.rs`

**Checkpoint S1**: T009–T012 pass; round-trip + byte-exact BodyLength/CheckSum match reference vectors; no-panic on garbled input verified. (parity.md CHK038, CHK040; contracts/core-codec.md)

---

## Phase 3: Stage S2 (Minimal session over TCP) — US1

**Goal**: Initiator↔Acceptor Logon/Heartbeat/TestRequest/Logout over TCP for FIX 4.2 & 4.4.
**Independent Test**: Two processes complete logon → sustained heartbeats → TestRequest/Heartbeat → clean Logout.
**Depends on**: S1.

### Tests for S2 (write first)

- [X] T022 [P] [US1] Two-process integration test: logon→heartbeat→logout on FIX 4.2 & 4.4 in `tests/integration_logon.rs`
- [X] T023 [P] [US1] Session state-machine transition table tests (Disconnected→Logon→LoggedOn→Logout) in `crates/truefix-session/tests/state_machine.rs`

### Implementation for S2

- [X] T024 [P] [US8] Implement minimal `SessionSettings` + `.cfg` parser (`[DEFAULT]`/`[SESSION]`, `${name}` interpolation) sufficient for session/transport bootstrap in `crates/truefix-config/src/`
- [X] T025 [P] [US1] Implement `SessionID` (BeginString/Sender/Target/Sub/Location/Qualifier) in `crates/truefix-session/src/session_id.rs`
- [X] T026 [US1] Implement session state machine core (states + transitions) in `crates/truefix-session/src/state.rs`
- [X] T027 [US1] Implement admin messages Logon/Logout/Heartbeat/TestRequest build+handle in `crates/truefix-session/src/admin/`
- [X] T028 [US1] Implement heartbeat/test-request timers (HeartBtInt, ~1.2× idle, multipliers, DisableHeartBeatCheck) in `crates/truefix-session/src/timers.rs`
- [X] T029 [P] [US9] Implement async `Application` trait (onCreate/onLogon/onLogout/toAdmin/fromAdmin/toApp/fromApp) in `crates/truefix/src/application.rs` (FR-J1, FR-F7)
- [X] T030 [US1] Implement `truefix-transport` Initiator + Acceptor read/write/timer task loops over tokio TCP (single session) in `crates/truefix-transport/src/`
- [X] T031 [US9] Wire facade re-exports + `MessageCracker` dispatch into `crates/truefix/src/lib.rs`

**Checkpoint S2**: T022 passes — US1 logon/heartbeat/logout works two-process on 4.2 & 4.4. (contracts/session.md, transport.md, application-api.md)

---

## Phase 4: Stage S3 (Sequence recovery & resend) — US3

**Goal**: Gap detection, ResendRequest (+ chunking), SequenceReset (GapFill/Reset), PossDup, 789 sync.
**Independent Test**: Scripted high/low-seq and resend scenarios produce spec-correct behavior.
**Depends on**: S2.

### Tests for S3 (write first)

- [ ] T032 [P] [US3] Recovery tests: high-seq→ResendRequest+queue; low-seq w/o PossDup→disconnect in `crates/truefix-session/tests/recovery.rs` (FR-D12)
- [ ] T033 [P] [US3] Resend reply tests: app msgs PossDupFlag=Y+OrigSendingTime; admin→SequenceReset-GapFill in `crates/truefix-session/tests/resend.rs` (FR-D7/D9)
- [ ] T034 [P] [US3] Chunked resend tests (`ResendRequestChunkSize`, 0=unbounded) in `crates/truefix-session/tests/chunked_resend.rs`
- [ ] T035 [P] [US3] NextExpectedMsgSeqNum(789) logon-sync tests for FIX≥4.4 in `crates/truefix-session/tests/next_expected.rs`

### Implementation for S3

- [ ] T036 [US3] Implement sequence-number management + in-order queue + recovery in `crates/truefix-session/src/sequence.rs`
- [ ] T037 [US3] Implement ResendRequest issue/handle + range chunking in `crates/truefix-session/src/resend.rs`
- [ ] T038 [US3] Implement SequenceReset GapFill & Reset modes in `crates/truefix-session/src/seqreset.rs`
- [ ] T039 [US3] Implement PossDup/PossResend/OrigSendingTime handling (RequiresOrigSendingTime, AllowPosDup) in `crates/truefix-session/src/possdup.rs`
- [ ] T040 [US3] Implement NextExpectedMsgSeqNum(789) + EnableLastMsgSeqNumProcessed(369) in `crates/truefix-session/src/next_expected.rs`
- [ ] T041 [US10] Implement session-level Reject + Logon/Logout timeouts + CheckLatency/MaxLatency in `crates/truefix-session/src/reject.rs` (FR-D11, FR-K2)
- [ ] T042 [US3] Implement ResetSeqNumFlag(141) + ResetOn{Logon,Logout,Disconnect} + RefreshOnLogon in `crates/truefix-session/src/reset.rs` (FR-D4/D5)

**Checkpoint S3**: T032–T035 pass; sequence/resend AT-relevant behavior correct. (parity.md CHK021/CHK022/CHK037; Appendix B sequence cases)

---

## Phase 5: Stage S4 (Dual-track data dictionary) — US5

**Goal**: Normalized dict → runtime `DataDictionary` validator + build.rs codegen, both from one source.
**Independent Test**: Per-toggle accept/reject correct; FIXT split; codegen/runtime same-source assert.
**Depends on**: S1 (uses core model). Can proceed in parallel with S3 after S2.

### Tests for S4 (write first)

- [ ] T043 [P] [US5] Dictionary parse tests (fields/components/groups/messages) for one version in `crates/truefix-dict/tests/parse.rs`
- [ ] T044 [P] [US5] Validation-toggle tests — one case per toggle (ValidateFieldsOutOfOrder, ValidateFieldsHaveValues, ValidateUnorderedGroupFields, ValidateUserDefinedFields, ValidateIncomingMessage, ValidateChecksum, AllowUnknownMsgFields, CheckCompID, FirstFieldInGroupIsDelimiter) in `crates/truefix-dict/tests/toggles.rs` (FR-C3; parity.md CHK014)
- [ ] T045 [P] [US5] Two-layer rejection tests (dictionary failure vs FIX basic-validity) in `crates/truefix-dict/tests/rejection_layers.rs` (FR-C4; CHK015)
- [ ] T046 [P] [US5] FIXT 1.1 transport/application split + DefaultApplVerID resolution tests in `crates/truefix-dict/tests/fixt.rs` (FR-A3/A4)
- [ ] T047 [P] [US5] Codegen/runtime same-source (version/hash) assertion test in `crates/truefix-dict/tests/dual_track.rs` (Principle IV)

### Implementation for S4

- [ ] T048 [US5] Implement normalized dictionary model + parser in `crates/truefix-dict/src/model.rs`, `parser.rs`
- [ ] T049 [US5] Implement runtime `DataDictionary::validate(&Message)` (field/type/required/group/enum/version rules; UDF; custom dicts) in `crates/truefix-dict/src/validate.rs` (FR-C1/C5)
- [ ] T050 [US5] Implement `build.rs` codegen of strongly-typed messages from the normalized source in `crates/truefix-dict/build.rs` (Principle IV)
- [ ] T051 [US5] Implement FIXT 1.1 transport/application dictionary selection + DefaultApplVerID/ApplVerID resolution in `crates/truefix-dict/src/fixt.rs`
- [ ] T052 [US5] Integrate dictionary validation into the session inbound path (RejectInvalidMessage, RejectGarbledMessage, Business Message Reject) in `crates/truefix-session/src/validate_hook.rs` (FR-C4, FR-K2). **Depends on S2** (session inbound path) in addition to S4 — unlike the rest of S4, this task is NOT parallel with S3; sequence it after the S2 checkpoint.

**Checkpoint S4**: T043–T047 pass; dual-track invariant enforced in CI. (contracts/dictionary.md; parity.md CHK014/CHK015)

---

## Phase 6: Stage S5 (Stores & logs) — US7

**Goal**: MessageStore (memory/file/cached/SQL/noop) + Log (screen/file/tracing/composite/SQL).
**Independent Test**: Restart-survivable resend; message/event log separation.
**Depends on**: S3 (resend uses store).

### Tests for S5 (write first)

- [ ] T053 [P] [US7] Restart-survivable resend test across memory/file/cached/SQL stores in `crates/truefix-store/tests/restart_resend.rs` (SC-006)
- [ ] T054 [P] [US7] Composite log fan-out + incoming/outgoing/event stream separation test in `crates/truefix-log/tests/composite.rs` (FR-H2)

### Implementation for S5

- [ ] T055 [P] [US7] Implement `MessageStore` trait + `MessageStoreFactory` in `crates/truefix-store/src/lib.rs`
- [ ] T056 [P] [US7] Implement MemoryStore, FileStore, CachedFileStore, NoopStore in `crates/truefix-store/src/{memory,file,cached,noop}.rs`
- [ ] T057 [P] [US7] Implement SQL store via sqlx (messages/sessions tables; JDBC-equivalent) in `crates/truefix-store/src/sql.rs` (research R7)
- [ ] T058 [P] [US7] Implement `Log` trait + `LogFactory` + ScreenLog/FileLog/TracingLog/CompositeLog in `crates/truefix-log/src/`
- [ ] T059 [P] [US7] Implement SQL log via sqlx (incoming/outgoing/event tables) in `crates/truefix-log/src/sql.rs`
- [ ] T060 [US7] Implement ForceResendWhenCorruptedStore recovery path in `crates/truefix-store/src/recovery.rs`
- [ ] T061 [US7] Wire store/log selection from config into session/transport in `crates/truefix-session/src/wiring.rs`

**Checkpoint S5**: T053/T054 pass; resend survives restart on every v1 store backend. (contracts/store-log.md; parity.md CHK037)

---

## Phase 7: Stage S6 (Multi-session, scheduling, dynamic sessions, reconnect) — US4, US6

**Goal**: Multi-session acceptor/initiator; dynamic sessions; scheduling+reset; reconnect; allow-listing.
**Independent Test**: 2 static + 1 dynamic session reach logged-on; disallowed remote refused; scheduled reset.
**Depends on**: S2, S5.

### Tests for S6 (write first)

- [ ] T062 [P] [US4] Multi-session + dynamic-session integration (2 static + 1 template-matched) in `tests/multi_dynamic.rs` (SC-003)
- [ ] T063 [P] [US4] Disallowed-remote refusal test (AllowedRemoteAddresses) in `crates/truefix-transport/tests/allowlist.rs`
- [ ] T064 [P] [US6] Scheduling tests: daily window reset (disconnect→reset→clear→reconnect) + NonStopSession exclusion in `crates/truefix-session/tests/schedule.rs` (FR-E1/E2/E3)
- [ ] T065 [P] [US4] Initiator reconnect test (ReconnectInterval) in `crates/truefix-transport/tests/reconnect.rs`

### Implementation for S6

- [ ] T066 [US4] Implement multi-session management + SessionID routing in acceptor/initiator in `crates/truefix-transport/src/sessions.rs`
- [ ] T067 [US4] Implement dynamic sessions (AcceptorTemplate, DynamicSession) + AllowedRemoteAddresses in `crates/truefix-transport/src/dynamic.rs` (FR-F3)
- [ ] T068 [US4] Implement initiator reconnect loop (ReconnectInterval) in `crates/truefix-transport/src/reconnect.rs`
- [ ] T069 [P] [US6] Implement `Schedule` (StartTime/EndTime, StartDay/EndDay, Weekdays, TimeZone, NonStopSession) + in-session window calc in `crates/truefix-session/src/schedule.rs`
- [ ] T070 [US6] Implement scheduled-reset semantics (disconnect→reset seq→clear store→reconnect) in `crates/truefix-session/src/schedule_reset.rs` (FR-E3)
- [ ] T071 [P] [US4] Implement socket options (keep-alive, nodelay, reuse, linger, buffers, etc.) in `crates/truefix-transport/src/socket_opts.rs` (FR-F5)

**Checkpoint S6**: T062–T065 pass; acceptor multi/dynamic at parity with initiator (Constitution VI). (contracts/transport.md; parity.md CHK041/CHK039)

---

## Phase 8: Stage S7 (All FIX versions + full codegen breadth) — US2, US5 (extended)

**Goal**: Bundle & support all 9 versions (4.0–5.0SP2 + FIXT 1.1); codegen typed messages for each.
**Independent Test**: Round-trip + validation pass for every version.
**Depends on**: S1, S4.

### Tests for S7 (write first)

- [ ] T072 [P] [US2] Per-version round-trip + BodyLength/CheckSum vectors (4.0/4.1/4.2/4.3/4.4/5.0/5.0SP1/5.0SP2/FIXT) against the reference fixtures from T099 in `crates/truefix-core/tests/versions.rs`
- [ ] T073 [P] [US5] Per-version dictionary validation smoke tests in `crates/truefix-dict/tests/versions.rs`

### Implementation for S7

- [ ] T074 [US5] Generate normalized dictionaries for all 9 versions from Orchestra source in `crates/truefix-dict/dict-src/` (FR-A1/A2)
- [ ] T075 [US5] Extend build.rs codegen to emit typed messages for all versions in `crates/truefix-dict/build.rs`
- [ ] T076 [US2] Verify version-specific header/trailer & message-set differences in codec/dictionary in `crates/truefix-core/src/versions.rs`

**Checkpoint S7**: T072/T073 pass for all 9 versions. (parity.md CHK034; FR-A1)

---

## Phase 9: Stage S8 (TLS, authentication, monitoring) — US10, US11 (+ TLS for US1/US4)

**Goal**: rustls TLS both roles; Logon auth; tracing+metrics monitoring (JMX-equivalent).
**Independent Test**: TLS handshake both roles; auth accept/reject; monitoring surface reports state.
**Depends on**: S2 (transport), S3 (auth hooks).

### Tests for S8 (write first)

- [ ] T077 [P] [US10] TLS handshake integration (initiator+acceptor, client-auth, SNI) in `tests/tls.rs` (FR-F6)
- [ ] T078 [P] [US10] Logon Username/Password auth accept/reject via from/toAdmin in `crates/truefix-session/tests/auth.rs` (FR-K1)
- [ ] T079 [P] [US11] Monitoring surface test: session state, seq numbers, connection health, reset action in `crates/truefix/tests/monitoring.rs` (SC-007)

### Implementation for S8

- [ ] T080 [US10] Implement rustls TLS for initiator+acceptor + SSL config keys (keystore/truststore/protocols/ciphers/client-auth/SNI) in `crates/truefix-transport/src/tls.rs` (FR-F6; Appendix A SSL group)
- [ ] T081 [US10] Implement Logon Username/Password + custom auth via toAdmin/fromAdmin in `crates/truefix-session/src/auth.rs` (FR-K1)
- [ ] T082 [P] [US11] Implement monitoring (tracing structured events + metrics gauges/counters for session state/seq/health) in `crates/truefix/src/monitor.rs` (FR-L1)
- [ ] T083 [US11] Implement operational actions (reset seq, force logout) reflected in monitoring in `crates/truefix/src/admin_ops.rs` (FR-L2)

**Checkpoint S8**: T077–T079 pass; TLS+auth+monitoring operational. (contracts/transport.md; parity.md CHK009/CHK044)

---

## Phase 10: Stage S9 (Acceptance Test suite + examples) — US12 (GATE), US13

**Goal**: Port full AT suite as black-box fixtures; all targeted versions pass. Ship examples.
**Independent Test**: AT report shows every targeted version passing (modulo justified deferrals).
**Depends on**: S2–S8 (AT exercises the whole engine).

### AT framework & porting (tests are the deliverable here)

- [ ] T084 [US12] Implement AT runner (scenario steps: configure/connect/initiate/expect/expect-disconnect; timestamp/seq wildcard matching) driving a real acceptor + facade Application in `crates/truefix-at/src/runner.rs` (FR-M1; contracts/at-runner.md)
- [ ] T085 [US12] Enumerate & author the 73 distinct **server** AT scenarios as independent fixtures (logon 1a–1e, sequence 2a–2c/10/11, PossDup 2e–2g/19, validity/reject 2i–2t/3/14/15/21, heartbeat 4/6, routing 8/ReverseRoute, named regressions QFJ*/MinQty/SessionReset) in `crates/truefix-at/scenarios/server/` (Appendix B; parity.md CHK019–CHK026). No copied source
- [ ] T086 [P] [US12] Author special-category suites: nextExpectedMsgSeqNum, lastMsgSeqNumProcessed, resendRequestChunkSize, validateChecksum, rejectGarbledMessages, timestamps, resynch in `crates/truefix-at/scenarios/special/` (Appendix B; CHK027)
- [ ] T087 [US12] Wire per-version scenario matrix using the **available** AT sets (fix40/41/42/43/44/50 + fixLatest) + applicability map; map FIX 5.0→fix50 and 5.0SP1/5.0SP2/FIXT 1.1→fixLatest (no distinct SP1/SP2 AT sets exist — their conformance is also covered by per-version round-trip T072 + dict validation T073) in `crates/truefix-at/src/matrix.rs` (CHK028; FR-M2/M3)
- [ ] T088 [US12] Add AT report (`--report`: scenario × version pass/deferred) + CI gate requiring all targeted versions green in `crates/truefix-at/src/report.rs` and CI (FR-M3; CHK030)

### Examples (US13)

- [ ] T089 [P] [US13] `executor` example (acceptor that executes orders) in `examples/executor/`
- [ ] T090 [P] [US13] `banzai` example (initiator client) in `examples/banzai/`
- [ ] T091 [P] [US13] `ordermatch` example (order-matching acceptor) in `examples/ordermatch/`
- [ ] T100 [P] [US13] `multi_acceptor` example (single acceptor serving multiple sessions, incl. a dynamic-session template) in `examples/multi_acceptor/` — the 4th example required by FR-N1
- [ ] T101 [P] [US13] Example smoke tests asserting each example runs end-to-end (executor↔banzai order→ExecutionReport; ordermatch crossing; multi_acceptor serves ≥2 sessions) in `tests/examples_smoke.rs` (closes SC-001 for domain N)

**Checkpoint S9 (RELEASE GATE)**: T084–T088 green — all targeted FIX versions pass in-scope AT scenarios (deferrals listed+justified); examples runnable (T089–T091, T100) and smoke-tested (T101). (US12; FR-M3; Constitution II/V)

---

## Phase 11: Polish & Cross-Cutting Concerns

- [ ] T092 [US8] Full Appendix A config-key coverage: implement remaining keys (proxy, all socket opts, file/SQL store+log keys, SLF4J-equiv keys) and a coverage enumeration test asserting every key is implemented or documented-unsupported in `crates/truefix-config/tests/key_coverage.rs` (FR-I2, SC-004; parity.md CHK001)
- [ ] T093 [US8] Document Sleepycat/JE key group as unsupported-with-reason (v1 deferral) in `crates/truefix-config/src/unsupported.rs` + docs (parity.md CHK012)
- [ ] T094 [P] Establish reproducible benchmarks (codec throughput, session round-trip latency) in `benches/` and run in CI for regression visibility (no numeric gate) (SC-008)
- [ ] T095 [P] Public-API docs pass: ensure every public type/trait/fn has doc comments; `#![deny(missing_docs)]` on facade in `crates/truefix/src/lib.rs` (Constitution I)
- [ ] T096 [P] Add a parity traceability matrix (Appendix A key → owning crate/test; Appendix B scenario → fixture/version) in `docs/parity-matrix.md` (parity.md CHK033/CHK036)
- [ ] T097 [P] No-panic audit on critical paths (clippy lint review + manual scan of codec/session/io/timer) documented in `docs/no-panic-audit.md` (SC-005; parity.md CHK038)
- [ ] T098 Run `quickstart.md` V1–V9 end-to-end and record results; finalize `cargo deny` clean report (release acceptance)

---

## Dependencies & Execution Order

### Stage dependency chain (one-pass engine — stages are NOT independent releases)

- **S0 (Setup)** → blocks everything.
- **S1 (core codec)** → blocks all later stages (Foundational).
- **S2 (minimal session)** depends on S1 → enables S3, S6, S8.
- **S3 (recovery/resend)** depends on S2 → enables S5; AT sequence cases.
- **S4 (dictionary)** depends on S1 → can run parallel to S3 after S1; feeds S7, S2 validation hook.
- **S5 (store/log)** depends on S3 (resend uses store).
- **S6 (multi/dynamic/schedule/reconnect)** depends on S2 + S5.
- **S7 (all versions + codegen)** depends on S1 + S4.
- **S8 (TLS/auth/monitoring)** depends on S2 + S3.
- **S9 (AT + examples)** depends on S2–S8 (exercises whole engine) — **release gate**.
- **Polish** depends on all desired stages.

### Notable parallel opportunities

- S0: T002–T006, T008 all `[P]`.
- S1: tests T009–T012 `[P]`; impl T013/T014/T021 `[P]` (different files) before T015–T020 chain.
- **S4 can proceed in parallel with S3** once S1 is done (different crates: truefix-dict vs truefix-session).
- S5: stores (T055–T057,T060) and logs (T058,T059) are `[P]` across the two crates.
- S6: T062–T065 tests `[P]`; T069/T071 `[P]`.
- S9: examples T089–T091, T100, T101 `[P]`; special suites T086 `[P]`.
- Polish: T094–T097 `[P]`.

---

## Implementation Strategy

### MVP scope
**MVP = S1 + S2** (US2 codec + US1 live session): a standards-correct FIX session that logs on,
heartbeats, and logs out over TCP with byte-exact framing. Demonstrable two-process. Everything else
builds on this spine.

### Incremental, checkpoint-gated delivery
Complete each stage to its **checkpoint** before starting the next dependent stage. Do not open a later
stage until its prerequisites' checkpoints pass — this keeps the one-pass build continuously verifiable
and prevents an un-testable big-bang (per user directive + Constitution VI/VII).

### Conformance as the gate
S9's AT suite across **all targeted versions** is the release gate (FR-M3). Per-scenario deferrals are
the only permitted exception and must be listed with justification.

### Test-first
Within each stage, author the stage's tests (the `### Tests for S#` block) before its implementation
tasks; verify red → green → refactor (Constitution V).

---

## Notes

- `[P]` = different files, no incomplete dependency. `[US#]` traces each task to a spec user story.
- Total tasks: 101 across 11 phases (S0–S9 + Polish). IDs T099–T101 were appended post-`/speckit-analyze`
  remediation (F1/C1/U1): T099 runs in Stage S0 (reference vectors), T100/T101 run in Stage S9 (4th
  example + example smoke tests). Execution order follows the stage dependencies, not ID numbering.
- Parity coverage is guarded by `checklists/parity.md`; the traceability matrix (T096) maps every
  Appendix A key and Appendix B scenario to a task/test.
- License discipline (Constitution III): T007/T074 derive dictionaries from FIX Orchestra (not QFJ XML);
  T084–T087 author AT fixtures as black-box contracts (no copied source/runner).
