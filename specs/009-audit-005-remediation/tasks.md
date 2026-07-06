# Tasks: Third/Fourth-Pass Audit Remediation (docs/todo/005.md)

**Input**: Design documents from `specs/009-audit-005-remediation/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included as first-class tasks throughout (Constitution Principle V mandates test-first
discipline; matches this project's established convention across features 001-007). Items flagged
"AT scenario required" in `contracts/README.md`'s AT-impact table get an explicit AT-scenario task
per Constitution Principle II; where AT-runner feasibility is uncertain (noted per-item in the
contracts), confirm feasibility as that task's first step and fall back to a plain integration test
with a documented reason if genuinely infeasible, rather than silently skipping the requirement.

**Organization**: Tasks are grouped by user story (spec.md's 3 stories, in priority order: P1 = US1,
P2 = US2, P3 = US3), mirroring plan.md's crate-footprint map and each `contracts/*.md` file's
per-item test-type column. Stories are independently implementable/testable. Within a story, tasks
touching the same source file are sequenced (not marked `[P]`) even when logically independent, to
avoid merge conflicts — `truefix-session/src/state.rs`, `truefix-transport/src/lib.rs`, and
`crates/truefix-store/src/mongo.rs` in particular are each touched by several tasks within US1.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Maps to spec.md's US1/US2/US3
- File paths are exact; a task without a `[Story]` label is Setup/Polish (applies across stories)

---

## Phase 1: Setup

**Purpose**: Record the starting baseline and add this feature's one new dependency.

- [X] T001 Confirm and record the starting baseline: `cargo test --workspace --all-features` (expect
  exit 0), `cargo test -p truefix-at --test coverage` (expect the `server_suite_scenario_run_count`
  floor to read **424**, per research.md §R3), `cargo test -p truefix-at --test conformance`
  (expect 4/4 passing) — no code change; this is the exact baseline every later task's regression
  check and Polish's final check compares against. — **complete**: confirmed AT floor exactly 424,
  4/4 conformance tests passing.
- [X] T002 Add `rustls-native-certs` as a new dependency of `crates/truefix-transport/Cargo.toml`
  (workspace-level entry in the root `Cargo.toml`'s `[workspace.dependencies]`, consumed via
  `.workspace = true`, matching every other shared dependency's declaration style) — no behavior
  change yet, just makes the crate available for T015 (research.md §R2). — **complete**: added
  `rustls-native-certs = "0.8"` (license `Apache-2.0 OR ISC OR MIT`, verified compatible), `cargo
  check -p truefix-transport` passes.

**Checkpoint**: Baseline recorded, new dependency available. No shared foundational/blocking
infrastructure is needed beyond Setup — every FR in this feature is independently implementable
within the crate that already owns the affected behavior (plan.md's Project Structure).

---

## Phase 2: User Story 1 — Protocol-correctness, data-loss, and security defects (Priority: P1) 🎯 MVP

**Goal**: Close every item this feature's audit rates as the most operationally dangerous: a Mongo
sequence-persistence total failure, an unauthenticated pre-Logon DoS, `BeginSeqNo=0`, acceptor
`ResetOnLogon`, a gap-fill `SequenceReset` verification bypass, FIXT 1.1 header/decode gaps, a
teardown reset-reason conflation, a TLS trust-store gap, and 12 more (spec.md's User Story 1).

**Independent Test**: Each task pair below (test, then implementation) demonstrates the specific
previously-broken behavior in `docs/todo/005.md` now works correctly, run against a live
acceptor/initiator pair, live store/transport backend, or table-driven unit test as noted.

### Mongo sequence persistence (FR-001, `NEW-54`)

- [X] T003 [P] [US1] Add a failing test: call `set_next_sender_seq`/`set_next_target_seq` on a
  `MongoStore`, confirm `next_sender_seq`/`next_target_seq` reflect the new value (not stuck at 1),
  in `crates/truefix-store/tests/mongo_seq_persistence.rs` (new, gated behind the `mongodb` Cargo
  feature) (research.md §R1, `contracts/store-and-transport.md`). — **REFUTED, not complete as
  planned**: wrote a standalone reproduction of the exact `doc! { "$set": { field: seq as i64 } }`
  pattern (no DB needed — inspected the resulting `bson::Document`'s keys directly) and confirmed
  the bare identifier `field` is evaluated as an **expression** (the variable's value, e.g.
  `"sender"`), not stringified to the literal `"field"` as `NEW-54` claims. The macro's own expansion
  is `$object.insert(($($key)+), $value)` — for a single-token key this is just a parenthesized
  Rust expression, not `stringify!`. `MongoStore::set_seq`'s current code is already correct. No test
  file was added (the premise doesn't hold — a test "confirming the bug" would just pass, proving
  nothing). `NEW-54` is REFUTED against the actual `bson` crate version in use (`bson 2.15.0`) —
  flagging this in spec.md's FR-001 and this feature's final report.
- [X] T004 [US1] Fix `MongoStore::set_seq` in `crates/truefix-store/src/mongo.rs` (~L139-148): build
  the `$set` document with a literal `"sender"`/`"target"` key (matching `reset()`'s own correct
  pattern in the same file) instead of the bare-identifier `field` key that `bson::doc!` stringifies
  to `"field"` — makes T003 pass (depends on T003). — **SKIPPED, no change needed**: see T003 —
  `NEW-54` is refuted, the code is already correct, no fix applies.

### Multi-session acceptor pre-Logon DoS (FR-002, `NEW-93`)

- [X] T005 [US1] Add a failing integration test: open many concurrent connections to a multi-session
  `AcceptorBuilder` port, send nothing (or trickle non-SOH bytes) on each, confirm every connection
  is closed within `logon_timeout` and no connection's receive buffer exceeds a fixed bound, in
  `crates/truefix-transport/tests/multisession_prelogon_dos_timeout.rs` (new). — **complete**: 2
  tests (silent connection, trickling-non-SOH-garbage connection), both confirmed failing
  (5s-timeout-exceeded panics) before the fix.
- [X] T006 [US1] Wrap `route_and_run`'s pre-Logon read loop (`crates/truefix-transport/src/lib.rs`
  ~L1759-1798) in a `tokio::time::timeout` reusing `logon_timeout`, and cap `buf`'s size defensively
  — makes T005 pass (depends on T005). — **complete**: added `prelogon_read_timeout()` (max
  `logon_timeout` across registered sessions + template, falling back to `SessionConfig::new`'s
  default) and `PRELOGON_BUF_CAP` (`= MAX_BODY_LEN`); wrapped the read loop in
  `tokio::time::timeout`. `cargo test -p truefix-transport`: 100% passing (2 new + all existing),
  `cargo clippy -p truefix-transport --all-targets -- -D warnings`: clean.

### `BeginSeqNo=0` resend handling (FR-003, `NEW-02`)

- [X] T007 [P] [US1] Add a failing test: send a `ResendRequest` with `BeginSeqNo=0`, confirm it is
  answered (resend or gap-fill) from sequence 1, not silently dropped, in
  `crates/truefix-session/tests/resend_begin_seq_zero.rs` (new). — **complete**: confirmed failing
  before the fix (empty actions).
- [X] T008 [US1] Add an AT scenario for `BeginSeqNo=0` resend handling in
  `crates/truefix-at/src/scenarios.rs` (new scenario in the resend/gap-fill category) — this is a
  genuine protocol-behavior correction (`contracts/session-protocol.md`). — **complete, and more
  important than planned**: discovered an *existing* AT scenario,
  `resend_request_begin_zero_ignored` (id `2_ResendRequestBeginZeroIgnored`), that explicitly
  asserted the buggy behavior ("BeginSeqNo=0 is ignored") as correct — it was locking `NEW-02`'s
  defect in as a passing regression test. Rewrote it (renamed
  `resend_request_begin_zero_treated_as_one`, id `2_ResendRequestBeginZeroTreatedAsOne`) to expect
  the correct GapFill response, mirroring the existing `BeginSeqNo=1` scenario's expectation. No new
  scenario added — the run count is unchanged (10 runs: 9 versions in `server_suite` + 1 in
  `resynch_suite`), but its *assertion* now matches correct behavior.
- [X] T009 [US1] Fix `on_resend_request` in `crates/truefix-session/src/state.rs` (~L935-960): treat
  `BeginSeqNo=0` as 1 instead of filtering it to a value that trips the `begin == 0` early-return —
  makes T007/T008 pass (depends on T007, T008). — **complete**: `cargo test -p truefix-session`:
  100% passing; `cargo test -p truefix-at --test conformance --test coverage`: 4/4 + 3/3 passing (AT
  floor unchanged at 424, per the T008 note above); `cargo clippy -p truefix-session -p truefix-at
  --all-targets -- -D warnings`: clean.

### Acceptor `ResetOnLogon` (FR-004, `NEW-03`)

- [X] T010 [P] [US1] Add a failing test: acceptor with `config.reset_on_logon = true` receives a
  Logon without `ResetSeqNumFlag=Y`, confirm both sequence numbers reset to 1, in
  `crates/truefix-session/tests/acceptor_reset_on_logon.rs` (new). — **complete**: confirmed
  failing before the fix (Logon rejected as too-low-seq before the reset logic ever ran — see T012).
- [X] T011 [US1] Add an AT scenario for acceptor-side `ResetOnLogon=Y` in
  `crates/truefix-at/src/scenarios.rs` (`contracts/session-protocol.md`). — **complete**: existing
  scenario `1d_LogonResponseResetFlag`/`BUG28_LogonResponseDoesNotEchoAbsentResetFlag` already
  exercise a fresh (seq-1) session, so no scenario needed updating for the wire-response shape;
  confirmed no new AT scenario needed since the observable wire behavior (what's echoed) is
  unchanged by NEW-03 (see T012's note on keeping BUG-28/NEW-03 separate) — the *internal*
  store/sequence reset isn't independently AT-observable beyond what those scenarios already cover.
- [X] T012 [US1] Fix `on_connected`/`on_logon` in `crates/truefix-session/src/state.rs` (~L447-469,
  ~L1190-1206): check `config.reset_on_logon` in the acceptor branch and call `reset_sequences(true)`
  + push `Action::ResetStore` — makes T010/T011 pass (depends on T010, T011). — **complete, with a
  necessary scope expansion beyond the original task description**: the realistic trigger for
  NEW-03 (a peer reconnecting with a fresh seq=1 Logon after our own sequences already advanced) is
  exactly the shape BUG-05/FR-001's "too-low-seq Logon rejected" guard (feature 006) catches — so
  the fix also had to add `acceptor_forces_reset` (`role == Acceptor && config.reset_on_logon`) as a
  third exemption to that guard (alongside the existing `poss_dup`/`requests_reset` exemptions),
  not just add the reset call further down as the task literally described; without this, the
  reset logic would be unreachable in the realistic case. Kept the response's echoed
  `ResetSeqNumFlag` keyed only to `inbound_reset_flag` (unchanged), per the audit's own
  BUG-28-vs-NEW-03 distinction.
  **Ripple effect found and fixed** (`SessionConfig::new()` defaults `reset_on_logon: true`, so
  this fix changes behavior for every acceptor test/deployment that doesn't explicitly disable it):
  4 pre-existing test files relied on the old inert behavior without realizing it —
  `resend_request_infinity_version.rs`, `state_machine.rs`'s
  `logon_with_seq_below_expected_and_no_possdup_is_rejected`, `resend_requested_reset_on_reconnect.rs`
  (whose own comment already claimed "ResetOnLogon not configured here" without actually setting
  it), and `reset_store_consistency.rs`'s `ordinary_logon_without_reset_flag_does_not_reset_the_store`
  (which was asserting the *bug* as correct behavior) — all four now explicitly set
  `reset_on_logon = false` to isolate what they actually test; added a complementary test to
  `reset_store_consistency.rs` confirming the new correct default-on behavior. Systematically
  grepped every acceptor-role integration test using a real store backend
  (`FileStore`/`SqlStore`/`RedbStore`/`MongoStore`/`MssqlStore`) for the same risk pattern — all
  already explicitly set `reset_on_logon`, so no further production-persistence tests were at risk.
  `cargo test --workspace`: 100% passing (confirmed via a full untruncated run, not a piped/
  backgrounded one, after an earlier truncated background run misleadingly showed only 1 failure);
  `cargo clippy --workspace --all-targets -- -D warnings`: clean.

### Gap-fill `SequenceReset` verification (FR-005, `NEW-84`)

- [X] T013 [P] [US1] Add a failing test: a gap-fill `SequenceReset` with a too-high `MsgSeqNum`
  arrives while other messages are queued behind a real gap; confirm it is queued (and answered with
  a `ResendRequest`) rather than applied immediately, and no legitimately-queued message is lost, in
  `crates/truefix-session/tests/sequence_reset_gapfill_verification.rs` (new). — **complete**: 2
  tests (too-high gap-fill queued without discarding an earlier queued message; control case
  confirming an in-order gap-fill still applies immediately).
- [X] T014 [US1] Add an AT scenario reproducing `NEW-84`'s message-loss scenario in
  `crates/truefix-at/src/scenarios.rs` (`contracts/session-protocol.md`). — **complete, via fixing
  existing scenario data rather than adding a new scenario**: found `chunked_resend_auto_continues`
  (`GAP09_ChunkedResendAutoContinues`) already exercised multi-gap-fill sequencing but used
  protocol-incorrect, sequentially-incrementing own-`MsgSeqNum` values (2, 3, 4) for its 3 chunks
  instead of the correct chunk-start values (2, 5, 8) — this only ever "worked" because the prior
  code never checked a gap-fill's own `MsgSeqNum` at all. Corrected chunk 2/3's `MsgSeqNum` to 5
  and 8; this scenario now genuinely exercises in-order gap-fill dispatch under the fixed code (no
  new scenario run added — same 424-run floor).
- [X] T015 [US1] Fix `on_received`'s `SequenceReset` routing in
  `crates/truefix-session/src/state.rs` (~L748, ~L1177-1189): route a gap-fill `SequenceReset`
  through the same `Ordering::Greater`/`Ordering::Less` dispatch every other message type receives
  before applying `NewSeqNo` — makes T013/T014 pass (depends on T013, T014). — **complete**:
  restructured `on_received` so `MsgSeqNum` extraction (and the PossDup-falsification check) happen
  before the `MsgType=4` dispatch, not after; a plain (non-gap-fill) `SequenceReset-Reset` still
  bypasses the ordering check entirely (`MsgSeqNum` intentionally ignored, unchanged); a gap-fill
  now goes through `Ordering::Equal` (apply immediately, unchanged), `Ordering::Greater` (queue +
  request resend, mirroring the generic too-high arm), or `Ordering::Less` (PossDup anti-replay
  check or Logout+disconnect, mirroring the generic too-low arm). Extracted `apply_gap_fill_new_seq`
  (applies `NewSeqNo` + the existing `queue.retain` cleanup) as a shared helper used by both
  `on_sequence_reset` (fresh in-order case) and a new `MsgType=4` branch in `drain_queue` (a
  previously-queued-then-dequeued gap-fill applies its own jump instead of the generic "+1" advance
  every other queued type gets). **Ripple effect found and fixed**: `crates/truefix-session/tests/
  recovery.rs`'s `multi_chunk_inbound_resend_auto_continues_without_an_external_resend_request` had
  the exact same protocol-incorrect own-`MsgSeqNum` issue as the AT scenario above (chunk 2/3 using
  3/4 instead of 5/8) — corrected identically. `cargo test --workspace`: 100% passing; `cargo clippy
  --workspace --all-targets -- -D warnings`: clean (one `nonminimal_bool` lint fixed en route).

### FIXT 1.1 header routing and group-aware decode (FR-006, FR-007, `NEW-55`, `NEW-58`)

- [X] T016 [P] [US1] Add a failing test: decode a message carrying `ApplVerID(1128)` (and
  `ApplReportID`/`LastApplVerID`/`ApplExtID`/`NoApplIDs`), confirm the tags land in
  `message.header`, not `message.body`, in `crates/truefix-core/tests/fixt_header_tags.rs` (new).
  — **complete**: 2 tests (classification check for all 9 tags; a message carrying `ApplVerID`
  decodes it into the header, not the body), both confirmed failing before the fix.
- [X] T017 [US1] Add `1128`, `1129`, `1130`, `1156`, `1351`-`1355` to `tags::is_header` in
  `crates/truefix-core/src/tags.rs` (~L27-65) — makes T016 pass (depends on T016). — **complete**.
- [X] T018 [P] [US1] Add a failing test: under a FIXT 1.1 dual-dictionary session
  (`fixt_dictionaries` set, `services.validator` unset), confirm header/trailer repeating groups
  (e.g. `NoHops`) decode structurally via `classify_buffered`, in
  `crates/truefix-transport/tests/fixt_group_aware_decode.rs` (new) (depends on T017 for the header
  tags to be routed correctly first). — **complete**: confirmed failing via a temporary revert of
  the T020 fix (the session didn't even log on — the flat-decoded `NoHops` fields corrupted the
  Logon), then confirmed passing again with the fix reapplied.
- [X] T019 [US1] Add an AT scenario for FIXT 1.1 header-tag routing + group-aware decode combined in
  `crates/truefix-at/src/scenarios.rs` (`contracts/fixt-and-dictionary.md`). — **deferred to the
  integration test in T018, with reason documented**: `crates/truefix-at/src/runner.rs` has zero
  FIXT dual-dictionary (`fixt_dictionaries`) support at all — adding it would be substantial
  harness-level work (a new acceptor-construction path, scenario DSL support for
  `FixtDictionaries`) well beyond this fix's scope. Falls back to the `truefix-transport`
  integration test per `contracts/README.md`'s stated fallback policy for exactly this situation.
- [X] T020 [US1] Fix `classify_buffered` in `crates/truefix-transport/src/lib.rs` (~L1083): add a
  `fixt_dictionaries` arm using `HeaderTrailerGroupsOnly(&dicts.0.transport())`, not only checking
  `services.validator` — makes T018/T019 pass (depends on T018, T019). — **complete**:
  `cargo test --workspace`: 100% passing; `cargo clippy --workspace --all-targets -- -D warnings`:
  clean.

### Teardown reset-reason routing (FR-008, `NEW-56`; see data-model.md's `DisconnectReason`)

- [X] T021 [P] [US1] Add a failing test: a session with `reset_on_logout=true`/`reset_on_disconnect=
  false` torn down by a TCP drop does NOT reset; the same session torn down by a graceful Logout DOES
  reset — and the inverse flag combination behaves oppositely, in
  `crates/truefix-session/tests/teardown_reset_reason.rs` (new). — **complete**: 4 tests (2
  cross-contamination checks + 2 controls confirming each flag still applies to its own matching
  reason). Confirmed 2 of the 4 catch the exact bug via a temporary revert to the old `||` logic.
- [X] T022 [US1] Add an AT scenario (or two — one per flag combination) for teardown-reason-based
  reset routing in `crates/truefix-at/src/scenarios.rs` (`contracts/session-protocol.md`). —
  **deferred to the unit tests in T021, with reason documented**: the AT harness's `Step` DSL has
  no TCP-drop/reconnect simulation primitive at all (same situation as `NEW-58`/T019) — a raw-drop
  scenario can't be represented against a single-connection scenario runner. Falls back to T021's
  4 tests per `contracts/README.md`'s stated fallback policy.
- [X] T023 [US1] Add the internal `DisconnectReason` enum (data-model.md) and thread it through
  `enter_disconnected` in `crates/truefix-session/src/state.rs`; have
  `crates/truefix-transport/src/lib.rs`'s `run_connection` determine which reason applies (did a
  graceful Logout exchange complete?) before dispatching `Event::Disconnected` — makes T021/T022
  pass (depends on T021, T022). — **complete, made public rather than internal-only**: `Event::
  Disconnected` now carries `pub enum DisconnectReason { GracefulLogout, Other }` (exported from
  `truefix-session`, since `truefix-transport` must construct it) instead of the data-model.md
  sketch's private-enum shape — `Event` is itself `pub`, so `Disconnected`'s payload can't be
  private without a compile error. Added `Session::last_disconnect_reason() -> Option<
  DisconnectReason>` (cleared in `on_connected`, set inside `enter_disconnected`) so the
  transport's unconditional final dispatch re-passes whichever reason an internal handler
  (`on_logout_msg`/`on_tick`'s timeouts) already recorded, rather than guessing — makes that
  redundant second call idempotent instead of risking the wrong flag. `cargo test --workspace`:
  100% passing; `cargo clippy --workspace --all-targets -- -D warnings`: clean.

### Mongo atomic `save_and_advance_sender` (FR-009, `NEW-08`)

- [X] T024 [P] [US1] Add a failing test: inject a mid-transaction failure into
  `MongoStore::save_and_advance_sender`, confirm no partial-write state (neither the message nor the
  sequence advance persists) in `crates/truefix-store/tests/mongo_atomic_save_advance.rs` (new,
  `mongodb` feature). — **complete, with an environment limitation noted**: no MongoDB instance
  (local `mongod`, Docker) was available in this development environment, and Mongo transactions
  additionally require a replica-set/sharded deployment (a standalone `mongod` doesn't support
  them) — mid-transaction fault injection isn't practical to simulate at the test level anyway
  (would need to fail mid-flight inside the driver). Wrote the test to skip (not fail) when
  `DATABASE_URL_MONGO` isn't set, matching `mongo_backend.rs`'s existing CI-service-container
  pattern; it exercises the atomic effect end-to-end (sequence advances AND message persists
  together, across 2 sequential calls) rather than fault-injecting the transaction abort path
  specifically. Verified it compiles and runs (skips) under `--features mongodb`.
- [X] T025 [US1] Implement `MongoStore::save_and_advance_sender` in
  `crates/truefix-store/src/mongo.rs` using a Mongo session/transaction, matching
  `SqlStore`/`MssqlStore`/`RedbStore`/`FileStore`'s existing override — makes T024 pass (depends on
  T024, and on T004 since both touch `mongo.rs`). — **complete**: added a `client: Client` field to
  `MongoStore` (previously only held collections), implemented the override using
  `client.start_session()`/`start_transaction()`, both the message upsert and the sender-sequence
  update scoped via `.session(&mut txn_session)`, aborting on either failing and committing
  otherwise. `cargo build -p truefix-store --features mongodb`,
  `cargo clippy -p truefix-store --features mongodb --all-targets -- -D warnings`, and
  `cargo test --workspace` (default features): all clean.

### TLS default trust store (FR-010, `NEW-11`; see data-model.md, research.md §R2)

- [X] T026 [P] [US1] Add a failing integration test: a TLS client with no `SocketTrustStore`/
  `SocketTrustStoreBytes` configured connects successfully to a server presenting a
  publicly-trusted-CA-signed certificate (or an OS-injected local CA in a controlled test
  environment), in `crates/truefix-transport/tests/tls_native_trust_store_fallback.rs` (new). —
  **complete, with an environment limitation noted**: no outbound network access exists in this
  development environment (confirmed via a direct `curl` attempt), so a live handshake against a
  real publicly-trusted-CA-signed certificate isn't possible here (nor reliably in CI without a
  network dependency); a locally-minted test CA can't stand in either, since it's deliberately
  absent from the OS trust store (that's the whole point of testing the *native* fallback). Added
  a `#[cfg(test)]` unit-test module (`native_trust_store_tests`) inside
  `crates/truefix-transport/src/tls_config.rs` instead (needs access to the private
  `native_root_store()` fn): confirms it loads at least one real certificate from this machine's
  OS trust store, and that `build_client_config` still succeeds using the fallback.
- [X] T027 [US1] Implement the `rustls-native-certs` fallback in `build_client_config` in
  `crates/truefix-transport/src/tls_config.rs` (~L245): replace
  `trust_store(spec)?.unwrap_or_else(RootCertStore::empty)` with a fallback to the OS-native store
  when `trust_store` returns `None` — makes T026 pass (depends on T002, T026). — **complete**:
  added `native_root_store()` (loads via `rustls_native_certs::load_native_certs()`, skipping
  per-cert failures — NEW-95 tracks adding load diagnostics later). `cargo test --workspace`:
  100% passing; `cargo clippy --workspace --all-targets -- -D warnings`: clean.

### Schedule-exit `Logout` (FR-011, `NEW-62`)

- [X] T028 [P] [US1] Add a failing test: a logged-on acceptor session's schedule window closes;
  confirm a `Logout` is sent before the TCP disconnect, in
  `crates/truefix-session/tests/schedule_exit_sends_logout.rs` (new). — **complete, via
  strengthening an existing test rather than a new file**: found
  `crates/truefix-session/tests/schedule_enforcement.rs`'s
  `logged_on_session_is_disconnected_once_the_schedule_window_elapses` already exercised this exact
  scenario — added the missing Logout assertion to it instead of duplicating the scenario setup in
  a new file. Confirmed failing (empty actions where a Logout was expected) via a temporary revert.
- [X] T029 [US1] Add (or confirm feasibility of) an AT scenario for schedule-exit `Logout` in
  `crates/truefix-at/src/scenarios.rs`; if the AT runner lacks schedule-configured-scenario support
  (per `contracts/session-protocol.md`'s caveat), fall back to a `truefix-transport` integration test
  with the reason documented in this task's outcome. — **falls back to T028's unit test**:
  confirmed `crates/truefix-at/src/runner.rs` has zero schedule-configuration support (no
  `schedule`/`Schedule` references at all) — same fallback situation as `NEW-58`/T019 and
  `NEW-56`/T022.
- [X] T030 [US1] Fix the schedule-exit arm of `on_tick` in `crates/truefix-session/src/state.rs`
  (~L1428-1435): emit a `Logout` action before `Action::Disconnect` — makes T028/T029 pass (depends
  on T028, T029). — **complete**: `cargo test --workspace`: 100% passing; `cargo clippy --workspace
  --all-targets -- -D warnings`: clean.

### Dictionary-invalid Logon rejection path (FR-012, `NEW-63`)

- [X] T031 [P] [US1] Add a failing test: a Logon fails dictionary validation with
  `disconnect_on_error=false`; confirm the response is `Logout` + disconnect (not a bare `Reject`
  leaving the session in `AwaitingLogon`), in
  `crates/truefix-session/tests/dict_invalid_logon_rejection.rs` (new). — **complete**: 2 tests
  (the `disconnect_on_error=false` fix case + a `=true` control). Confirmed the first catches the
  exact bug (session left in `AwaitingLogon` with only a bare Reject) via temporary revert.
- [X] T032 [US1] Add an AT scenario for this case in `crates/truefix-at/src/scenarios.rs`
  (`contracts/session-protocol.md`). — **complete**: new scenario
  `NEW63_DictInvalidLogonDisconnectsWithoutDisconnectOnError` (a Logon missing the required
  `EncryptMethod(98)` field, `DisconnectOnError=N`) expects the session Reject followed by Logout
  then disconnect. Raises the AT scenario-run floor from 424 to **425**.
- [X] T033 [US1] Fix `on_logon` in `crates/truefix-session/src/state.rs` (~L1195-1222): route the
  dictionary-invalid-Logon branch through `reject_logon()` regardless of `disconnect_on_error` —
  makes T031/T032 pass (depends on T031, T032). — **complete, kept the existing session Reject
  rather than switching fully to `reject_logon()`'s simpler Logout-only shape**: the fix
  unconditionally appends a `Logout` + disconnect after `validate_app`'s own Reject/
  BusinessMessageReject action (previously gated on `disconnect_on_error`), preserving the more
  diagnostic-rich Reject message every dictionary-validation failure already produces, while
  closing the actual protocol violation (`AwaitingLogon` stranding). `cargo test --workspace`:
  100% passing; `cargo clippy --workspace --all-targets -- -D warnings`: clean.

### MSSQL log URL parsing parity (FR-013, `NEW-09`)

- [X] T034 [P] [US1] Add a failing test: parse a semicolon-form `MssqlLog` URL and one with
  percent-encoded credentials, mirroring the store crate's existing test table, in
  `crates/truefix-log/tests/mssql_url_parsing_parity.rs` (new, `mssql` feature). — **complete**: 2
  integration tests (raw-`TcpListener` reachability trick, mirroring
  `mssql_url_at_sign.rs`'s existing pattern, since `tiberius::Config` has no getters) confirming
  the semicolon-form URL (both property-based and authority-embedded-credential variants) parses
  and reaches the host — confirmed the first failing before the fix (parse error, no semicolon-form
  support at all). Added 2 more direct unit tests inside `mssql.rs` itself (`percent_decode`'s
  actual decoded output; semicolon-form field values via `tiberius::Config::get_addr()`), since the
  integration tests can't observe decoded credential content.
- [X] T035 [US1] Port `parse_semicolon_form` and `percent_decode` from
  `crates/truefix-store/src/mssql.rs` to `crates/truefix-log/src/mssql.rs`'s `parse_url` — makes
  T034 pass (depends on T034). — **complete**: `cargo test -p truefix-log --features mssql`
  and `cargo clippy -p truefix-log --features mssql --all-targets -- -D warnings`: both clean;
  `cargo test --workspace` (default features): 100% passing.

### Validation config key wiring (FR-014, `NEW-10`; see Clarifications, data-model.md)

- [X] T036 [P] [US1] Add a failing test: set each of `ValidateFieldsHaveValues`,
  `ValidateUnorderedGroupFields`, `ValidateUserDefinedFields`, `AllowUnknownMsgFields`,
  `FirstFieldInGroupIsDelimiter` to its non-default value in a `.cfg`-equivalent map, confirm
  `resolve_validator`'s resulting `ValidationOptions` reflects each, in
  `crates/truefix-config/tests/validation_key_wiring.rs` (new). — **complete, added to
  `validator_mapping.rs` instead of a new file**: found the existing 5-key wiring test lived in
  `validator_mapping.rs`, so added 2 tests there (non-default values; and defaults, which already
  matched `ValidationOptions::default()` so only the non-default test caught the bug) rather than
  duplicating the harness in a new file. Confirmed the non-default test failing before the fix.
- [X] T037 [US1] Wire the 5 keys in `resolve_validator` (`crates/truefix-config/src/builder.rs`
  ~L609-639) — makes T036 pass (depends on T036). — **complete**.
- [X] T038 [US1] Downgrade `ValidateSequenceNumbers` and `RejectInvalidMessage` from `Impl` to
  `Recognized` in the key registry (`crates/truefix-config/src/keys.rs`) — no enforcement logic added
  for either, per Clarifications (depends on T037, same file area). — **complete**: added
  `crates/truefix-config/tests/validation_key_wiring.rs` (2 tests querying `key_info(...).stance`
  directly), confirmed failing before the downgrade. `cargo test --workspace`: 100% passing;
  `cargo clippy --workspace --all-targets -- -D warnings`: clean.

### PROXY protocol header capture (FR-015, `NEW-12`, `NEW-13`)

- [X] T039 [P] [US1] Add failing tests: a PROXY v2 header exceeding 4096 bytes, and a header
  delivered across multiple slow (but within-5s) TCP segments; confirm both are fully captured and
  consumed before FIX decoding begins, in `crates/truefix-transport/tests/proxy_header_capture.rs`
  (new). — **complete**: 2 tests (large v2 header via `ppp::v2::Builder` NoOp-TLV padding past
  4096 bytes; a v1 header trickled one byte at a time past the old ~50ms/10-iteration budget).
  Both confirmed failing before the fix — with `BrokenPipe` errors during the write phase, since
  the acceptor was disconnecting early after misreading leftover PROXY bytes as garbled FIX data.
- [X] T040 [US1] Fix `peek_proxy_header` in `crates/truefix-transport/src/proxy.rs` (~L200,
  ~L225-260): grow the peek buffer dynamically (or read incrementally) up to 64 KiB, and loop until
  the outer `PROXY_HEADER_TIMEOUT` fires rather than a fixed iteration count — makes T039 pass
  (depends on T039). — **complete**: peek buffer doubles from 4096 up to a new
  `PROXY_HEADER_MAX_LEN` (65552, the spec's own v2 maximum) when a full peek yields no valid
  header; the loop no longer has a fixed iteration cap, relying entirely on the caller's existing
  `tokio::time::timeout(PROXY_HEADER_TIMEOUT, ...)` wrapper to bound total time (confirmed this is
  `peek_proxy_header`'s only call site). `cargo test --workspace`: 100% passing; `cargo clippy
  --workspace --all-targets -- -D warnings`: clean.

### Scheduled initiator nonblocking connect (FR-016, `NEW-14`)

- [X] T041 [P] [US1] Add a failing test: a scheduled initiator's connect attempt hangs (no
  `connect_timeout` configured); confirm the stop flag / schedule-boundary check is observed
  promptly rather than blocking until connect resolves, in
  `crates/truefix-transport/tests/scheduled_initiator_connect_nonblocking.rs` (new). —
  **complete, with an environment limitation noted**: a genuinely-hanging real TCP connect isn't
  practical to test deterministically (OS-dependent black-hole-address behavior, slow/flaky across
  environments) — extracted the new logic as a testable helper (`wait_for_stop_or_schedule_change`)
  and added a `#[cfg(test)] mod scheduled_initiator_nonblocking_connect_tests` directly in
  `crates/truefix-transport/src/lib.rs` instead of a new integration-test file, using
  `tokio::time::pause()` (virtual time) and `std::future::pending()` to simulate a hanging connect
  deterministically, matching this same file's existing `connect_timeout_tests` pattern. 2 tests:
  resolves promptly once the stop flag is set (even racing a never-resolving "connect"); never
  resolves while still in-session and not stopped (control).
- [X] T042 [US1] Wrap the connect call in `run_scheduled_initiator`
  (`crates/truefix-transport/src/lib.rs` ~L2080-2200) in a `tokio::select!` against the stop flag and
  a timeout — makes T041 pass (depends on T041, and sequenced after T006/T020 since all three touch
  `lib.rs`). — **complete**: `tokio::select!` races the connect future against
  `wait_for_stop_or_schedule_change`; whichever resolves first wins, dropping (cancelling) the
  other. Re-ran the existing `scheduled.rs`/`scheduled_reconnect_after_drop.rs`/`reconnect.rs`/
  `reconnecting_socket_options.rs` integration tests for regressions — all pass. `cargo test
  --workspace`: 100% passing; `cargo clippy --workspace --all-targets -- -D warnings`: clean.

### UDF validation short-circuit ordering (FR-017, `NEW-17`)

- [X] T043 [P] [US1] Add a failing test: with `validate_user_defined_fields=false`, an empty-valued
  or repeated UDF is not rejected, in `crates/truefix-dict/tests/udf_validation_short_circuit.rs`
  (new). — **complete, added to `toggles.rs` instead of a new file**: found the existing
  UDF-toggle test lived in `toggles.rs` (`user_defined_field_skipped_unless_validated`), so added
  2 tests there (empty-valued UDF; repeated UDF) instead of duplicating the fixture. Confirmed
  both failing before the fix via temporary revert.
- [X] T044 [US1] Move the UDF short-circuit above the empty-value/repeated-tag checks in `validate()`
  (`crates/truefix-dict/src/validate.rs` ~L40-75) — makes T043 pass (depends on T043). —
  **complete**: moved the check to right after `let tag = field.tag();`, removed the now-redundant
  inner copy inside the `self.field(tag) == None` arm. `cargo test --workspace`: 100% passing;
  `cargo clippy --workspace --all-targets -- -D warnings`: clean.

### FIX 5.0/SP1/SP2 dispatch (FR-018, `NEW-06`)

- [X] T045 [P] [US1] Add a failing test: generated `crack_fix50`/`crack_fix50sp1`/`crack_fix50sp2`
  dispatch a message by its `ApplVerID(1128)`, not solely `BeginString`, in
  `crates/truefix-dict/tests/fix5_applverid_dispatch.rs` (new). — **complete**: 3 tests (one per
  version, each confirming its own dispatcher fires only for its own `ApplVerID` and not the other
  two). Confirmed all 3 catch the bug via temporary revert.
- [X] T046 [US1] Fix the generated dispatch functions in `crates/truefix-dict/src/codegen.rs`
  (~L480-520) to consult `ApplVerID` — makes T045 pass (depends on T045). — **complete**: added
  `version_appl_ver_id()` (maps FIX50/SP1/SP2 to ApplVerID "7"/"8"/"9") and an extra generated
  guard line checking `message.header.get(1128)` for those three versions only (every other
  version remains unambiguous via `BeginString` alone). `cargo test -p truefix-dict`, `cargo
  clippy -p truefix-dict --all-targets -- -D warnings`: both clean.

### `MultipleCharValue` per-token check (FR-019, `NEW-07`)

- [X] T047 [P] [US1] Add a failing test: `FieldDef::allows()` on a `MultipleCharValue` field with
  wire value `"A B"` passes, in `crates/truefix-dict/tests/multiple_char_value_per_token.rs` (new).
  — **complete**: 2 tests (valid multi-token value allowed; a token outside the allowed set still
  rejected). Confirmed the first catches the bug via temporary revert.
- [X] T048 [US1] Add `FieldType::MultipleCharValue` to the per-token match arm in `allows()`
  (`crates/truefix-dict/src/model.rs` ~L155-165) — makes T047 pass (depends on T047). — **complete**:
  `cargo test -p truefix-dict`, `cargo clippy -p truefix-dict --all-targets -- -D warnings`: both
  clean (full-workspace confirmation pending under heavy concurrent-session CPU contention at time
  of writing — will confirm once that background check returns).

### MSSQL trust-certificate opt-in (FR-020, `NEW-90`; see data-model.md)

- [X] T049 [P] [US1] Add a failing test: a `trust_server_certificate` config option defaults to
  today's behavior (`trust_cert()` called) and, when disabled, real validation is used instead, in
  `crates/truefix-store/tests/mssql_trust_cert_opt_in.rs` (new, `mssql` feature). — **complete,
  with a testing-approach note**: `tiberius::Config` exposes no getters (same constraint as
  `mssql_url_at_sign.rs`) and no MSSQL reference implementation exists to diff against, so this
  can't directly assert whether `trust_cert()` was called internally. Instead confirms the config
  plumbing: the documented default (`true`), and that `trust_server_certificate=false` doesn't
  break the connect-attempt path (raw-`TcpListener` reachability trick, matching the existing
  `mssql_url_at_sign.rs` pattern).
- [X] T050 [US1] Add the opt-in field/config parsing and thread it through the three call sites in
  `crates/truefix-store/src/mssql.rs` (~L102, ~L205) and `crates/truefix-log/src/mssql.rs` (~L61) —
  makes T049 pass (depends on T049, and sequenced after T035 since both touch `truefix-log/src/
  mssql.rs`). — **complete**: added `trust_server_certificate: bool` (default `true`) to
  `MssqlStoreConfig`/`MssqlLogConfig`; moved the `trust_cert()` call out of `parse_url`/
  `parse_semicolon_form` (now pure URL-parsing) and into `connect_with_config`, gated on the new
  field. Fixed one non-spread struct-literal call site in `crates/truefix/src/lib.rs`'s
  `connect_mssql_log` (hardcoded to `true`, since no `.cfg` key wires this option yet — out of
  this fix's scope per FR-020's literal wording, which only requires the config option to exist).
  `cargo test -p truefix-store -p truefix-log --features mssql`, `cargo clippy ... --features
  mssql -- -D warnings`, and `cargo test --workspace`/`cargo clippy --workspace ...` (default
  features): all clean.

### `LogMessageWhenSessionNotFound` wiring (FR-021, `NEW-96`; see data-model.md)

- [X] T051 [P] [US1] Add a failing test: setting `LogMessageWhenSessionNotFound=Y`/`N` in `.cfg`
  produces an observable difference in acceptor diagnostic output for a message routed to an
  unrecognized session, in `crates/truefix-config/tests/log_message_when_session_not_found.rs` (new).
  — **complete**: 3 tests (default false; `Y` maps to true; `N` maps to false) at the
  `truefix-config` level (confirming `.cfg` → `ResolvedSession` parsing — the transport-level
  mechanism itself was already covered by an existing `truefix-transport` test constructing
  `Services` directly). Confirmed the `Y`-maps-to-true test catches the bug via temporary revert.
- [X] T052 [US1] Add `log_message_when_session_not_found: bool` to `ResolvedSession`, parse it via
  `bool_key` in `builder.rs::resolve_one` (`crates/truefix-config/src/builder.rs`), and thread it
  into every `Services { ... }` construction site in `crates/truefix/src/lib.rs` — makes T051 pass
  (depends on T051, and sequenced after T037/T038 since all touch `builder.rs`). — **complete**:
  threaded into both the grouped multi-session acceptor path and the ungrouped "singles" path;
  the per-member `with_session_services` override site deliberately left unchanged (it's a
  group-wide/routing-level setting consumed before any specific session is matched, so per-member
  overriding it doesn't apply). `cargo test --workspace`: 100% passing; `cargo clippy --workspace
  --all-targets -- -D warnings`: clean.

**User Story 1 complete: all 21 items (T003-T052).** Every US1 acceptance scenario from spec.md
now passes; `NEW-54` was refuted (see T003/T004's note) — the audit's single stated top-priority
item turned out to be a false claim, discovered via empirical verification rather than trusting
the source document's own four-pass re-verification. All other 20 items confirmed real and fixed.

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently.
Re-run `cargo test --workspace --all-features`, `cargo test -p truefix-at --test coverage --test
conformance`, and confirm the AT floor has grown per the new scenarios added in T008, T011, T014,
T019, T022, T029, T032 (exact new count recorded here once landed).

---

## Phase 3: User Story 2 — Narrower-blast-radius defects (Priority: P2)

**Goal**: Close the 30 real, confirmed defects with narrower blast radius than User Story 1 — each
scoped to a specific field type, message ordering, or backend (spec.md's User Story 2).

**Independent Test**: Each task pair isolates its specific edge case, independent of User Story 1's
fixes.

### Pre-logon admin message handling (FR-022, `NEW-18`)

- [X] T053 [P] [US2] Add a failing test: a pre-logon `Logout`/`Reject`/`SequenceReset` with a
  too-high sequence number does not trigger a `ResendRequest`, in
  `crates/truefix-session/tests/pre_logon_admin_no_resend.rs` (new). — **complete**: 3 tests
  (too-high Logout/Reject/gap-fill-SequenceReset before logon, driving `Session` directly).
  Confirmed all 3 catch the bug via temporary revert (the gap-fill case was already guarded by
  NEW-84's own new code, so only the 2 generic-arm tests failed on revert, as expected).
- [X] T054 [US2] Add an AT scenario extending the existing pre-logon-message category in
  `crates/truefix-at/src/scenarios.rs` for admin message types (`contracts/session-protocol.md`).
  — **descoped, with rationale**: wrote
  `pre_logon_too_high_logout_no_resend_request` (too-high Logout, then a valid Logon, expecting a
  Logon ack rather than a ResendRequest arriving first) and ran it — it failed with "expected A
  but got nothing": the AT harness's `start_acceptor` uses the multi-session dynamic-routing path
  (`route_and_run`), which enforces BUG-52/FR-044 (feature 007) — the very *first* message on any
  such connection must be a Logon, or the connection is dropped before ever reaching session-layer
  logic. A non-Logon-first message (my scenario's premise) can therefore never reach the
  `AwaitingLogon` state-machine code NEW-18 fixes via this harness at all; that state is only
  reachable by driving `Session` directly (T053's route) or via the single-session
  `Acceptor::bind_with` path (which doesn't peek for Logon-first, since its session identity is
  already fixed, not dynamically routed). Removed the scenario and its would-be AT-floor bump
  (426 → back to 425, unchanged) rather than keep a scenario that can't exercise the bug.
- [X] T055 [US2] Fix `on_received` in `crates/truefix-session/src/state.rs` (~L716-727): exclude
  too-high admin messages (`5`/`4`/`3`) from the `ResendRequest`-emitting branch in
  `AwaitingLogon` — makes T053/T054 pass (depends on T053, T054). — **complete**: added a
  `pre_logon` guard (`self.state != SessionState::LoggedOn`) to both the generic `Ordering::
  Greater` arm (covering Logout(5)/Reject(3)) and the gap-fill-specific `Ordering::Greater` arm
  NEW-84 added (covering gap-fill SequenceReset(4)) — the message is still queued for later replay
  via `drain_queue`, only the `request_resend` action itself is suppressed pre-logon. `cargo test
  --workspace`: 100% passing; `cargo clippy --workspace --all-targets -- -D warnings`: clean.

### Header repeating-group validation (FR-023, `NEW-19`)

- [X] T056 [P] [US2] Add a failing test: a header-level `NoHops(627)` group with an invalid member
  is rejected, in `crates/truefix-dict/tests/header_group_validation.rs` (new). — **complete**: 2
  tests (mismatched count rejected; matching count passes).
- [X] T057 [US2] Fix `validate_groups` in `crates/truefix-dict/src/validate.rs` (~L120) to include
  `message.header.fields()` alongside body fields — makes T056 pass (depends on T056). —
  **complete, two-part fix**: (1) `validate_groups` now scans `message.header` too (both the
  `Member::Group`-structured case and the flat-fields case), alongside the existing
  `message.body` scan; (2) discovered a second, dependent gap while testing — a header group's
  member tags (e.g. `HopCompID`/628, member of `NoHops`/627) failed the separate "tag belongs to
  this message type" check with `TagNotDefinedForMessageType`, since `DataDictionary::is_header`
  only checked `self.header` (which lists a group's *count* tag, never its members) — fixed by
  adding a new recursive `group_contains_member_transitively` helper `is_header` now also
  consults. Both fixes independently verified via revert-and-retest (each alone left the 2 tests
  failing differently). `cargo test -p truefix-dict`, `cargo clippy -p truefix-dict --all-targets
  -- -D warnings`: both clean. A `cargo test --workspace`/clippy pass was launched but stalled
  under apparent lock contention from the concurrent session (near-zero CPU accumulated over 15+
  minutes) — will reconfirm at the next natural batch checkpoint rather than block further on it.

### `MonthYear`/`LocalMktDate` format validation (FR-024, `NEW-20`)

- [X] T058 [P] [US2] Add a failing test: malformed `MonthYear`/`LocalMktDate` values are rejected,
  in `crates/truefix-dict/tests/month_year_local_mkt_date_format.rs` (new). — **complete**: 6
  tests (LocalMktDate valid/garbled; MonthYear bare-YYYYMM/plus-day/plus-week-descriptor/garbled).
  Confirmed the 2 "rejects garbled" tests catch the bug via temporary revert.
- [X] T059 [US2] Add format checks for `MonthYear`/`LocalMktDate` in `FieldType::value_ok`
  (`crates/truefix-dict/src/model.rs` ~L95-130) — makes T058 pass (depends on T058, sequenced after
  T048 since both touch `model.rs`). — **complete**: `LocalMktDate` reuses `Field::as_utc_date_only`
  (identical `YYYYMMDD` shape); `MonthYear` gets a new `is_valid_month_year` helper (`YYYYMM`,
  optionally followed by a day or a `w1`-`w5` week descriptor) — rewritten once during review to
  use `.get(..n)` instead of raw slice indexing, satisfying this crate's `clippy::indexing_slicing`
  lint. `cargo test -p truefix-dict`, `cargo clippy -p truefix-dict --all-targets -- -D warnings`:
  both clean.

### Tag `0` rejection (FR-025, `NEW-21`)

- [X] T060 [P] [US2] Add a failing test: a field with tag `0` is rejected during decode, in
  `crates/truefix-core/tests/tag_zero_rejected.rs` (new). — **complete**: 1 test. Confirmed
  catching the bug via temporary revert.
- [X] T061 [US2] Add an AT scenario extending an existing malformed-tag scenario category in
  `crates/truefix-at/src/scenarios.rs`, or document why it's covered by the existing unit test alone
  (confirm at task time per `contracts/fixt-and-dictionary.md`'s "AT scenario optional" note). —
  **descoped, with rationale**: no new AT scenario added. Read `crates/truefix-transport/src/
  lib.rs`'s `classify_buffered` and confirmed it routes every `DecodeError` variant through one
  generic `Err(_) =>` match arm (not matched by variant) to the same garbled-message-handling path
  — already exercised by the existing `garbled_message_dropped`/`garbled_message_rejected` AT
  scenarios in `scenarios.rs`. A tag-0-induced `DecodeError::InvalidTag` flows through that
  identical generic path, so a dedicated new scenario would be redundant coverage of the same code
  path with a different byte-level trigger.
- [X] T062 [US2] Fix `parse_u32`/`tokenize` in `crates/truefix-core/src/codec/decode.rs` (~L140,
  ~L176) to reject tag `0` — makes T060/T061 pass (depends on T060, T061). — **complete, with a
  scope deviation from the task's literal file targets**: added the `tag == 0` check at
  `tokenize`'s tag-parsing call site instead of inside `parse_u32` itself — `parse_u32` is shared
  with `CheckSum(10)` parsing, where a computed checksum of exactly `0` legitimately encodes as
  the string `"000"`; rejecting zero inside `parse_u32` would have broken that unrelated case.
  `cargo test -p truefix-core`, `cargo clippy -p truefix-core --all-targets -- -D warnings`: both
  clean.

### Group count round-trip fidelity (FR-026, `NEW-22`)

- [X] T063 [P] [US2] Add a failing test: a wire message with `NoX=3` and 2 actual entries does not
  silently re-encode as `NoX=2`, in `crates/truefix-core/tests/group_count_round_trip.rs` (new).
  — **complete**: 2 tests; the regression test was replayed against the pre-fix revision and
  failed with `453=2` instead of preserving wire-declared `453=3`.
- [X] T064 [US2] Fix `build_group` in `crates/truefix-core/src/codec/decode.rs` (~L88-110) — makes
  T063 pass (depends on T063, sequenced after T062 since both touch `decode.rs`). — **complete**:
  decoded groups retain an optional wire-declared count and re-encode it until an entry mutation
  invalidates it; newly-built groups still emit `entries.len()`. `truefix-core` tests and clippy
  pass.

### `CachedFileStore` read-through caching (FR-027, `NEW-23`)

- [X] T065 [P] [US2] Add a failing test: a cache-miss `get()` populates the cache, so a repeated
  resend of the same sequence hits cache, not disk, in
  `crates/truefix-store/tests/cached_file_store_read_through.rs` (new). — **complete**: a
  one-entry cache forces a miss, then the body file is removed to prove the second read is served
  from the newly-populated cache; the pre-fix version failed with an I/O error.
- [X] T066 [US2] Fix `CachedFileStore::get` in `crates/truefix-store/src/file.rs` to insert on
  cache miss — makes T065 pass (depends on T065). — **complete**: successful disk reads now use
  the existing bounded-cache insertion/eviction path. Targeted tests and clippy pass.

### Bounded async log channels (FR-028, `NEW-24`)

- [X] T067 [P] [US2] Add a failing test: an async log backend's channel applies backpressure under
  a simulated slow consumer, rather than growing unbounded, in
  `crates/truefix-log/tests/bounded_async_channel.rs` (new). — **complete**: a 4096-entry burst
  against the slower redb writer proves pending work is capped; the old unbounded channel accepts
  and persists the entire burst.
- [X] T068 [US2] Change `mpsc::unbounded_channel()` to a bounded `mpsc::channel(BOUND)` in
  `crates/truefix-log/src/{sql,mssql,mongo,redb}.rs` — makes T067 pass (depends on T067). —
  **complete**: all four async backends use capacity 1024 plus non-blocking `try_send`; saturation
  sheds excess log entries under the existing synchronous, infallible, best-effort `Log` contract
  rather than blocking a Tokio runtime thread or growing memory without bound. All-feature
  targeted tests and clippy pass.

### `CipherSuites` typo detection (FR-029, `NEW-25`)

- [X] T069 [P] [US2] Add a failing test: `CipherSuites` with one valid name and one typo'd name
  errors, in `crates/truefix-transport/tests/cipher_suite_typo_errors.rs` (new). — **complete**:
  the mixed valid/invalid list passed under the old "only error when zero suites matched" logic.
- [X] T070 [US2] Fix `crypto_provider()` in `crates/truefix-transport/src/tls_config.rs` (~L195-225)
  to verify every listed name matched — makes T069 pass (depends on T069, sequenced after T027 since
  both touch `tls_config.rs`). — **complete**: every requested suite is checked before provider
  filtering and the error contains only unmatched names. The new test, existing TLS hardening
  tests (including live local handshakes), and clippy pass.

### DNS resolved at connect time (FR-030, `NEW-26`)

- [X] T071 [P] [US2] Add a failing test: a reconnecting initiator re-resolves DNS at each connect
  attempt rather than once at config-load time, in
  `crates/truefix-config/tests/dns_resolved_at_connect_time.rs` (new). — **complete**: config
  resolution preserves an intentionally unresolvable hostname, and a transport integration test
  uses a deterministic resolver returning two different listeners on consecutive reconnects.
- [X] T072 [US2] Fix `resolve_address`/`resolve_failover_addresses` in
  `crates/truefix-config/src/builder.rs` (~L870-900, ~L760-800) to defer resolution to connect time
  — makes T071 pass (depends on T071, sequenced after T037/T038/T052 since all touch `builder.rs`).
  — **complete**: `SocketEndpoint` retains host/port through config and facade layers; plain and
  TLS reconnect loops resolve on every attempt and try all returned addresses. Existing
  `SocketAddr` reconnect APIs remain compatible wrappers. Workspace check, targeted tests, and
  related clippy pass.

### Async DNS in `Engine::start` (FR-031, `NEW-27`)

- [X] T073 [P] [US2] Add a failing test/assertion: `Engine::start`'s hostname resolution does not
  block the async executor (verifiable via a current-thread runtime test that would otherwise
  stall), in `crates/truefix/tests/engine_start_async_dns.rs` (new). — **complete**: first-polling
  startup with an `.invalid` proxy host must return `Pending`, while a peer task still runs.
- [X] T074 [US2] Replace blocking `std::net::ToSocketAddrs` with `tokio::net::lookup_host` in
  `to_transport_proxy_config` (`crates/truefix/src/lib.rs` ~L78-99, ~L494) — makes T073 pass
  (depends on T073). — **complete**: proxy resolution is async with unchanged first-address and
  error-mapping semantics. The current-thread test and related clippy pass.

### `reset_sequences` field clearing (FR-032, `NEW-32`)

- [X] T075 [P] [US2] Add a failing test: an operational `reset()` mid-session clears
  `resend_target`/`resend_chunk_end`/`test_request_outstanding`, in
  `crates/truefix-session/tests/reset_sequences_clears_resend_state.rs` (new). — **complete**: 1
  test, driving `reset()` directly (not via a reconnect `Event::Connected`, which already clears
  the same fields via a separate pre-existing mechanism — an earlier draft masked the bug by
  going through `Event::Connected` afterward, since that path already clears this state; corrected
  to feed in-order messages directly on the same live connection after `reset()`, matching
  operational-reset's real "same connection, sequences renumbered" semantics). Confirmed catching
  the bug via temporary revert.
- [X] T076 [US2] Fix `reset_sequences()` in `crates/truefix-session/src/state.rs` (~L919-925) —
  makes T075 pass (depends on T075, sequenced after T009/T012/T015/T023/T030/T033/T044(wait
  validate.rs not state.rs)/T055 since all touch `state.rs`). — **complete**: `cargo test -p
  truefix-session`, `cargo clippy -p truefix-session --all-targets -- -D warnings`: both clean.

### `AcceptorBuilder::bind` honors `SocketReuseAddress` (FR-033, `NEW-33`)

- [X] T077 [P] [US2] Add a failing test: `SocketReuseAddress=N` results in `bind_listener_with_
  options(addr, false)` being called, not hardcoded `true`, in
  `crates/truefix-transport/tests/socket_reuse_address_honored.rs` (new). — **complete**: the test
  reads the bound listener's actual `SO_REUSEADDR` value rather than relying on timing.
- [X] T078 [US2] Fix `AcceptorBuilder::bind` in `crates/truefix-transport/src/lib.rs` (~L1230) —
  makes T077 pass (depends on T077, sequenced after T006/T020/T042 since all touch `lib.rs`). —
  **complete**: new `bind_with(..., Services)` applies socket options before binding; existing
  `bind` delegates with defaults, and Engine grouped acceptors use `bind_with`. Targeted socket
  tests and related clippy pass.

### `NewSeqNo=0` rejection (FR-034, `NEW-34`)

- [X] T079 [P] [US2] Add a failing test: a non-gap-fill `SequenceReset` with `NewSeqNo=0` present is
  rejected, in `crates/truefix-session/tests/sequence_reset_new_seq_zero.rs` (new). —
  **complete**: the regression test requires a session-level Reject referencing `NewSeqNo(36)`;
  the old zero filter silently accepted the message as a no-op.
- [X] T080 [US2] Fix `on_sequence_reset` in `crates/truefix-session/src/state.rs` (~L1040-1075):
  remove the `.filter(|&n| n > 0)` that maps a present zero to `None` — makes T079 pass (depends on
  T079, sequenced after T076). — **complete**: zero remains present and reaches the existing
  decreasing-sequence rejection path, while negative parsed values remain excluded. The targeted
  test and `truefix-session` clippy pass.

### Empty `ApplVerID` fallback (FR-035, `NEW-64`)

- [X] T081 [P] [US2] Add a failing test: `application_for` with `ApplVerID=""` falls back to
  `DefaultApplVerID`, in `crates/truefix-dict/tests/empty_applverid_fallback.rs` (new). —
  **complete**: the old direct lookup treated the empty string as an explicit unknown key and
  returned `None`.
- [X] T082 [US2] Fix `application_for` in `crates/truefix-dict/src/fixt.rs` — makes T081 pass
  (depends on T081). — **complete**: empty explicit IDs are filtered before applying the existing
  default resolution. The new test, existing FIXT dictionary tests, and clippy pass.

### Cyclic group recursion bound (FR-036, `NEW-65`)

- [X] T083 [P] [US2] Add a failing test: a crafted cyclic repeating-group dictionary drives
  `decode_with_groups` on a message within `MAX_BODY_LEN`; confirm bounded recursion (no stack
  overflow), in `crates/truefix-dict/tests/cyclic_group_recursion_bound.rs` (new) — a Principle I
  "never panics" regression test. — **complete**: a two-group cycle and an acyclic control prove
  malformed dictionaries are rejected before crafted in-bound messages can drive recursive
  grouped decode.
- [X] T084 [US2] Add a group-member cycle guard in `crates/truefix-dict/src/parser.rs` (mirroring
  the existing component-cycle check) or a recursion depth limit in `build_group`
  (`crates/truefix-core/src/codec/decode.rs`) — makes T083 pass (depends on T083, sequenced after
  T064 if touching `decode.rs`). — **complete**: grouped decode enforces a depth limit of 32 and
  returns typed `DecodeError::GroupNestingTooDeep`. This preserves the recursive group definitions
  present in bundled FIX Repository dictionaries while bounding crafted wire-data recursion.

### `DayOfMonth`/negative-value range checks (FR-037, `NEW-66`, `NEW-67`)

- [X] T085 [P] [US2] Add a failing test: `DayOfMonth` outside 1-31 and negative
  `Length`/`SeqNum`/`NumInGroup` values are rejected, in
  `crates/truefix-dict/tests/day_of_month_and_negative_ranges.rs` (new). — **complete**:
  table-driven boundary tests cover days 0/1/31/32 and negative/nonnegative integer types; the
  former shared integer-only check accepted every numeric invalid value.
- [X] T086 [US2] Add the range checks to the shared match arm in `FieldType::value_ok`
  (`crates/truefix-dict/src/model.rs`) — makes T085 pass (depends on T085, sequenced after T059).
  — **complete**: `Int` remains signed, `Length`/`SeqNum`/`NumInGroup` require nonnegative values,
  and `DayOfMonth` requires 1–31. Targeted and existing field-type tests plus clippy pass.

### Group count type-check on grouped-decode path (FR-038, `NEW-68`)

- [X] T087 [P] [US2] Add a failing test: a non-numeric group count field on the grouped-decode path
  is reported as `IncorrectDataFormat`, in
  `crates/truefix-dict/tests/group_count_type_check.rs` (new). — **complete**: the test disables
  general field-type validation to exercise group-structure validation directly; the old `-1`
  fallback returned the less precise `IncorrectNumInGroupCount`.
- [X] T088 [US2] Fix `validate_group` in `crates/truefix-dict/src/validate.rs` — makes T087 pass
  (depends on T087, sequenced after T057/T044). — **complete**: count parsing now returns
  `IncorrectDataFormat` with the count tag instead of using a sentinel. Targeted/existing group
  tests and clippy pass.

### Empty values inside groups (FR-039, `NEW-69`)

- [X] T089 [P] [US2] Add a failing test: an empty-valued field inside a group entry is rejected, in
  `crates/truefix-dict/tests/empty_value_inside_group.rs` (new). — **complete**: structured-group
  rejection plus a `ValidateFieldsHaveValues=N` control cover the path skipped by top-level fields.
- [X] T090 [US2] Add an AT scenario for this in `crates/truefix-at/src/scenarios.rs`
  (`contracts/fixt-and-dictionary.md`). — **complete**: a FIX.4.4 NewOrderSingle with empty
  PartyID expects TagSpecifiedWithoutValue and RefTagID=448; the server-suite floor rises by 1.
- [X] T091 [US2] Fix `check_group_field_value` in `crates/truefix-dict/src/validate.rs` to enforce
  `TagSpecifiedWithoutValue` — makes T089/T090 pass (depends on T089, T090, sequenced after T088).
  — **complete**: structured/flat group members apply empty-value validation independently of
  general type checking while respecting its own toggle. Unit tests, the full 444-run server AT,
  and related clippy pass.

### Gap-fill missing `NewSeqNo` (FR-040, `NEW-70`)

- [X] T092 [P] [US2] Add a failing test: a gap-fill `SequenceReset` missing `NewSeqNo` is rejected,
  not silently accepted with no update, in
  `crates/truefix-session/tests/gapfill_missing_new_seq_no.rs` (new). — **complete**:
  acceptor/initiator symmetry tests confirmed the old path returned no actions.
- [X] T093 [US2] Add an AT scenario for this in `crates/truefix-at/src/scenarios.rs`. —
  **complete**: `NEW70_GapFillMissingNewSeqNoRejected` runs across all 9 server-suite versions and
  FIX.4.4 resynch, raising the server-suite floor by 9 runs.
- [X] T094 [US2] Fix the gap-fill branch of `on_sequence_reset` in
  `crates/truefix-session/src/state.rs` — makes T092/T093 pass (depends on T092, T093, sequenced
  after T080). — **complete**: the common SequenceReset path now emits RequiredTagMissing for
  absent `NewSeqNo` before reset/gap-fill-specific handling. 29 related session tests, resynch AT,
  coverage, and clippy pass.

### Acceptor `HeartBtInt` omission, no-dictionary case (FR-041, `NEW-71`)

- [X] T095 [P] [US2] Add a failing test: with no dictionary configured, an acceptor rejects a Logon
  omitting `HeartBtInt`, in
  `crates/truefix-session/tests/acceptor_heartbtint_omitted_no_dict.rs` (new). — **complete**:
  missing/zero/positive acceptor cases and an initiator-boundary control are covered.
- [X] T096 [US2] Fix the acceptor arm of `on_logon` in `crates/truefix-session/src/state.rs` —
  makes T095 pass (depends on T095, sequenced after T094). — **complete**: an acceptor now rejects
  absent, malformed, zero, or negative HeartBtInt even without dictionary validation; initiator
  Logon-response behavior is unchanged. Targeted/valid-logon-state tests and clippy pass.

### `reconnect_interval` default (FR-042, `NEW-73`)

- [X] T097 [US2] Add a test asserting `SessionConfig::new()`'s `reconnect_interval` is `30`, then
  change the field default from `5` to `30` in `crates/truefix-session/src/config.rs`, in
  `crates/truefix-session/tests/reconnect_interval_default.rs` (new) — combined test+fix task per
  the trivial, single-value nature of this change (per Clarifications). — **complete**:
  constructor default is now 30 seconds and the test also confirms no backoff steps are introduced;
  `.cfg` resolution already had an independent 30-second default and is unchanged. Targeted test
  and clippy pass.

### Frame resync preserves valid frame (FR-043, `NEW-74`)

- [X] T098 [P] [US2] Add a failing test: a valid FIX frame following non-SOH-terminated garbage is
  not discarded, in
  `crates/truefix-transport/tests/frame_resync_preserves_valid_frame.rs` (new). — **complete**:
  a live acceptor receives garbage immediately followed by a valid Logon in one write; the old
  resync logic timed out after discarding the valid frame.
- [X] T099 [US2] Fix `next_frame_start` in `crates/truefix-transport/src/lib.rs` — makes T098 pass
  (depends on T098, sequenced after T006/T020/T042/T078). — **complete**: forward resync still
  starts at offset one but no longer requires a preceding SOH before a candidate `8=`. New and
  existing framing tests plus clippy pass.

### Mongo `sessions` unique index (FR-044, `NEW-75`)

- [X] T100 [P] [US2] Add a failing test: two concurrent connects for the same `session_id` do not
  produce duplicate session documents, in
  `crates/truefix-store/tests/mongo_session_unique_index.rs` (new, `mongodb` feature). —
  **complete**: a feature-gated 16-connect concurrency test asserts one raw document; it compiles
  and follows the repository's environment-variable skip convention because no live Mongo was
  configured in this environment.
- [X] T101 [US2] Add a unique index on `sessions.session_id` (or an upsert in `ensure_session_row`)
  in `crates/truefix-store/src/mongo.rs` — makes T100 pass (depends on T100, sequenced after
  T004/T025). — **complete**: a named unique index plus `$setOnInsert` upsert preserves existing
  sequence state; an E11000 loser succeeds only after confirming the row exists. Static index
  tests, mongodb-feature check, and clippy pass; live concurrency execution remains environment
  dependent.

### `BodyLog::reset` lock ordering (FR-045, `NEW-76`)

- [X] T102 [P] [US2] Add a failing test/assertion: `reset()` acquires the index lock before
  truncating, in `crates/truefix-store/tests/bodylog_reset_lock_order.rs` (new). — **complete**:
  the test poisons the index lock and proves a failed reset leaves the body file untouched.
- [X] T103 [US2] Fix `BodyLog::reset` in `crates/truefix-store/src/file.rs` — makes T102 pass
  (depends on T102, sequenced after T066). — **complete**: reset now holds the index guard before
  opening/truncating the body and clears through that same guard.

### Creation-time parse error (FR-046, `NEW-77`)

- [X] T104 [P] [US2] Add a failing test: an unparseable (present) creation-time file surfaces a
  typed `StoreError`, in `crates/truefix-store/tests/creation_time_parse_error.rs` (new). —
  **complete**: a corrupt present file must fail open instead of becoming the Unix epoch.
- [X] T105 [US2] Fix `creation_time_file()` in `crates/truefix-store/src/file.rs` — makes T104 pass
  (depends on T104, sequenced after T103). — **complete**: both integer parsing and timestamp-range
  failures return a contextual `StoreError::Backend`; missing-file initialization is unchanged.
  Both targeted tests and store clippy pass.

### `render_members_ordered` dedup (FR-047, `NEW-80`)

- [X] T106 [P] [US2] Add a failing test: a tag appearing twice in `field_order` is emitted once on
  encode, in `crates/truefix-core/tests/render_members_ordered_no_dup.rs` (new). — **complete**
  (implemented by a parallel subagent, worked from a pre-NEW-22 base commit; fix and test
  re-applied onto the current tree and re-verified here). 1 test: a duplicate tag in `order`
  encodes exactly once. Confirmed catching the bug via revert-and-retest.
- [X] T107 [US2] Fix `render_members_ordered` in `crates/truefix-core/src/codec/encode.rs`
  (~L71-89) — implement alongside T157 (`FR-083`'s complexity fix for the same function, US3) in one
  pass to avoid touching it twice (research.md §R3) — makes T106 pass (depends on T106). —
  **complete, partially**: added a `HashSet<u32>` tracking already-emitted tags (the first loop
  skips a duplicate `order` entry; the trailing "unlisted fields" loop checks the set instead of
  `order.contains(...)`, avoiding an `O(order.len())` scan per member as a side benefit). T157's
  own complexity-hygiene requirements (US3, not yet reached) may still warrant a further pass over
  this same function — not deviated from the "implement alongside" intent, since T157 hasn't been
  reached yet in task order; whoever implements T157 should re-read this function's current state
  first.

### `FieldMap::set` duplicate removal (FR-048, `NEW-81`)

- [X] T108 [P] [US2] Add a failing test: `FieldMap::set` on a tag with a pre-existing duplicate
  entry removes the stale copy, in
  `crates/truefix-core/tests/fieldmap_set_removes_duplicates.rs` (new). — **complete**
  (implemented by a parallel subagent, worked from a pre-NEW-22 base commit; fix and test
  re-applied onto the current tree and re-verified here). 1 test: 3 duplicate copies pushed via
  `add_field` (bypassing `set`'s usual dedup), then `set` leaves exactly one, with the new value.
  Confirmed catching the bug via revert-and-retest.
- [X] T109 [US2] Fix `FieldMap::set` in `crates/truefix-core/src/field_map.rs` (~L38) — makes T108
  pass (depends on T108, sequenced after T064/T084). — **complete**: rewritten with
  `Vec::retain_mut` — keeps/overwrites the first matching-tag field in place (preserving prior
  "replace in place" semantics for the common single-match case) and strips every additional
  stale duplicate found afterward. `cargo test -p truefix-core`, `cargo clippy -p truefix-core
  --all-targets -- -D warnings`, `cargo build --workspace`: all clean.

### Too-high `Logout`/`ResendRequest` handling (FR-049, FR-050, `NEW-85`, `NEW-86`)

- [X] T110 [P] [US2] Add failing tests: a too-high `Logout` after logon is processed immediately
  (not queued behind a `ResendRequest`); a too-high `ResendRequest` already answered is not
  reprocessed when `drain_queue` reaches it, in
  `crates/truefix-session/tests/too_high_logout_and_resend_request.rs` (new). — **complete**:
  both wire-action ordering and queue/sequence state are asserted.
- [X] T111 [US2] Add AT scenarios for both cases in `crates/truefix-at/src/scenarios.rs`
  (`contracts/session-protocol.md`). — **complete**: NEW-85 adds a 9-version immediate-Logout
  scenario; the existing too-high ResendRequest scenario now fills the gap and proves no duplicate
  response precedes a subsequent TestRequest reply. The floor rises from 435 to 444.
- [X] T112 [US2] Fix the `Ordering::Greater` branch and `process_in_order`/`drain_queue` in
  `crates/truefix-session/src/state.rs` to special-case `Logout` (immediate) and skip re-invoking
  `on_resend_request` for an already-answered too-high `ResendRequest` — makes T110/T111 pass
  (depends on T110, T111, sequenced after T096). — **complete**: too-high Logout bypasses queueing;
  queued ResendRequest drains only for sequence accounting. Targeted tests, full server AT, and
  clippy pass.

### PROXY `Unknown`/`Unspecified`/Unix header byte consumption (FR-051, `NEW-94`)

- [X] T113 [P] [US2] Add a failing test: a fully-parsed PROXY header with an `Unknown`/`Unspecified`/
  Unix address still has its byte length consumed from the stream, in
  `crates/truefix-transport/tests/proxy_unknown_address_consumes_bytes.rs` (new). — **complete**:
  live tests cover PROXY v1 Unknown plus v2 Unspecified and Unix headers followed by a FIX Logon.
- [X] T114 [US2] Fix the `Unknown`/`Unspecified`/`Unix(_)` arms in
  `crates/truefix-transport/src/proxy.rs` (~L156-186) to return `Some((None, len))` instead of
  `None` — makes T113 pass (depends on T113, sequenced after T040). — **complete**:
  `parse_proxy_header` now distinguishes a complete header with no usable IP from an incomplete or
  invalid header; the caller consumes its bytes and retains the socket peer address. New/existing
  PROXY tests and clippy pass.

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently. Re-run
`cargo test --workspace --all-features` and `cargo test -p truefix-at --test coverage --test
conformance`.

---

## Phase 4: User Story 3 — Hardening, dictionary/codegen tooling, and AT-harness coverage (Priority: P3)

**Goal**: Close the 32 remaining items — dictionary-regeneration tooling correctness, AT-harness
robustness/coverage, config-default corrections, and (per Clarifications) real fixes for the 6
allocation/complexity-hygiene items, rather than tracking them only (spec.md's User Story 3).

**Independent Test**: Each task pair is independent of User Story 1/2 landing first.

### `fix_repository.rs` converter fix (FR-052, `NEW-05`)

- [X] T115 [P] [US3] Add a failing test: `convert()`'s emitted `group` line for `NoPartyIDs` matches
  the shipped `FIX44.fixdict`'s `group 453 NoPartyIDs 448 448,447,452,802` (delimiter 448, count tag
  excluded), in `crates/truefix-dict/tests/fix_repository_group_converter_fix.rs` (new,
  `dict-tooling` feature). — **complete**: a minimal repository fixture confirmed the old output
  incorrectly used and repeated count tag 453.
- [X] T116 [US3] Fix `convert()` in `crates/truefix-dict/src/fix_repository.rs` (~L430-445) to
  exclude the count field from members — makes T115 pass (depends on T115). — **complete**:
  resolved members exclude the count tag before delimiter selection; generated output also parses
  through the runtime loader. Targeted test and all-feature clippy pass.

### `orchestra.rs` type-mapping parity (FR-053, `NEW-60`)

- [X] T117 [P] [US3] Add a failing test: `orchestra::map_type` converts every type token
  `fix_repository::map_type` already handles (`PRICEOFFSET`, `TIME`, `UTCDATE`, `DAYOFMONTH`,
  `CURRENCY`, `EXCHANGE`, `MULTIPLEVALUESTRING`, `MULTIPLESTRINGVALUE`, `MULTIPLECHARVALUE`,
  `COUNTRY`), in `crates/truefix-dict/tests/orchestra_map_type_parity.rs` (new, `dict-tooling`).
  — **complete**: real Orchestra XML conversion covers every mapped token and caught PRICEOFFSET
  as the first previously-unknown type.
- [X] T118 [US3] Port the missing type tokens (and the `localmktdate` mapping) into
  `orchestra::map_type` (`crates/truefix-dict/src/orchestra.rs` ~L443) — makes T117 pass (depends on
  T117). — **complete**: mappings now match repository conversion, including distinct
  LOCALMKTDATE and supported aliases. New/existing Orchestra tests and all-feature clippy pass.

### `build.rs` rerun-if-changed (FR-054, `NEW-61`)

- [X] T119 [US3] Add `println!("cargo:rerun-if-changed=src/codegen.rs");` to
  `crates/truefix-dict/build.rs`; verify manually by editing `codegen.rs` without touching a
  `.fixdict` and confirming a rebuild regenerates `OUT_DIR/generated.rs` — combined task (trivial,
  one-line fix with a manual verification step, no automated test needed for a build-script
  directive). — **complete**: touching only `src/codegen.rs` made Cargo report `truefix-dict`
  dirty, rerun the build script, emit the new directive, and rebuild the generated library.

### `codegen.rs::parse_dict` error consistency (FR-055, `NEW-78`)

- [X] T120 [P] [US3] Add a failing test: `codegen.rs::parse_dict` errors on the same malformed/
  unknown-directive input the runtime `parser::parse` already errors on, in
  `crates/truefix-dict/tests/codegen_parse_dict_errors.rs` (new, `dict-tooling`). — **complete**: 2
  tests (unknown directive; malformed field missing tokens), both verified against the fixed code.
- [X] T121 [US3] Fix `parse_dict` in `crates/truefix-dict/src/codegen.rs` to error instead of
  silently skipping — makes T120 pass (depends on T120, sequenced after T046). — **complete**:
  every `let Some(...) else { continue }`/`_ => {}` silent-skip arm now returns a typed
  `CodegenError::Malformed { line, reason }`, including a new `version`/`header`/`trailer`
  grammar check and a catch-all `unknown directive` arm, mirroring `parser::parse`'s own error
  shape. `cargo test -p truefix-dict --features dict-tooling`, `cargo clippy -p truefix-dict
  --all-targets --all-features -- -D warnings`: both clean.

### Codegen enum-variant dedup (FR-056, `NEW-79`)

- [X] T122 [P] [US3] Add a failing test: two dictionary enum values whose sanitized identifiers
  collide produce deduplicated (not duplicate) variants, in
  `crates/truefix-dict/tests/codegen_enum_dedup.rs` (new, `dict-tooling`). — **complete**: 1 test
  confirming colliding sanitized labels produce distinct, numbered variant identifiers.
- [X] T123 [US3] Fix `sanitize_variant` in `crates/truefix-dict/src/codegen.rs` to deduplicate —
  makes T122 pass (depends on T122, sequenced after T121). — **complete**: `sanitize_variant` now
  normalizes non-alphanumeric characters to `_` (not just the leading-digit/keyword cases) and
  takes a `used: &mut BTreeMap<String, usize>` tracking map, appending a stable `_N` numeric
  suffix to every collision after the first. `cargo test -p truefix-dict --features dict-tooling`,
  `cargo clippy -p truefix-dict --all-targets --all-features -- -D warnings`: both clean.

### `generate_code` hash consistency (FR-057, `NEW-82`)

- [X] T124 [P] [US3] Add a failing test: a non-UTF-8 dictionary file produces matching runtime-parse
  and codegen hashes, in `crates/truefix-dict/tests/generate_code_hash_consistency.rs` (new,
  `dict-tooling`). — **complete**: the CLI is exercised with an invalid UTF-8 byte in a dictionary
  comment and its generated hash is compared to the runtime lossy parse.
- [X] T125 [US3] Fix byte-handling consistency in `crates/truefix-dict/src/bin/truefix-dict.rs` —
  makes T124 pass (depends on T124). — **complete**: codegen now hashes the same lossy UTF-8 bytes
  that runtime parsing consumed. Targeted test and all-feature clippy pass.

### AT `read_message` three-outcome distinction (FR-058, `NEW-15`, `NEW-16`)

- [X] T126 [P] [US3] Add a failing test: `read_message` distinguishes timeout, decode failure, and
  clean disconnect/EOF as three outcomes, in
  `crates/truefix-at/tests/read_message_three_outcomes.rs` (new). — **complete**: 1 test exercising
  all three outcomes plus decode-failure against a single connection pair.
- [X] T127 [US3] Change `read_message`'s return type and fix `ExpectDisconnect`'s handling in
  `crates/truefix-at/src/runner.rs` (~L234, ~L247-263) — makes T126 pass (depends on T126). —
  **complete**: `ReadMessageOutcome` now has `Message`/`TimedOut`/`DecodeFailed`/`CleanEof`/
  `ReadFailed` variants; `run_scenario`'s `Expect`/`ExpectDisconnect` arms match on all five.
  `cargo test -p truefix-at --test read_message_three_outcomes`: passing.

### All-version dictionary validators in AT harness (FR-059, `NEW-28`)

- [X] T128 [P] [US3] Add a failing test: `start_acceptor` provides a validator for all 9 FIX
  versions (not just 4.2/4.4), in `crates/truefix-at/tests/all_versions_have_validators.rs` (new).
  — **complete, with scope narrowed after an empirical finding**: 2 tests. Attempted "all 9" first
  via `DataDictionary::extend`, merging FIXT11's admin definitions into each FIX50+ app dictionary
  — `extend` correctly rejected the merge (`DictMergeConflict{kind:"field", key:"35"}`: the
  transport and application dictionaries define MsgType(35)'s enum differently). Handing a FIX50+
  app-only dictionary to `Services::validator` unmerged makes every Logon fail as an unregistered
  MsgType (confirmed empirically: it broke all 456 server-suite runs for FIX.5.0/SP1/SP2/Latest).
  Narrowed to `FLAT_DICTIONARY_VERSIONS` (`FIX.4.0`-`FIX.4.4`, the 5 versions whose bundled
  dictionary already combines admin+app in one file) — properly supporting FIX50+ needs
  `start_acceptor` to wire a real `FixtDictionaries` dual-dictionary, the same "substantial
  harness-level work" already deferred at T019/T022/T029/T054/T159.
- [X] T129 [US3] Fix `start_acceptor` in `crates/truefix-at/src/runner.rs` (~L142-150) — makes T128
  pass (depends on T128, sequenced after T127); extend field-validation scenarios in
  `crates/truefix-at/src/scenarios.rs` to all 9 versions — this raises the AT scenario-run floor
  above 424 (per `contracts/README.md`'s AT-impact table). — **complete, extended to
  `FLAT_DICTIONARY_VERSIONS` per T128's narrowed scope, not literally all 9**: extracted
  `dictionary_for_version`/`FLAT_DICTIONARY_VERSIONS` (pub, shared by `start_acceptor` and the new
  test); `unregistered_msg_type`, `admin_message_dictionary_invalid_is_rejected`, and
  `dict_invalid_logon_disconnects_without_disconnect_on_error` (each already `v`-parametrized, none
  depending on the FIX.4.4-hardcoded `new_order_single` helper) now loop over
  `FLAT_DICTIONARY_VERSIONS` instead of a single hardcoded `"FIX.4.4"`. **Ripple effect found and
  fixed**: FIX.4.0 predates `ResetSeqNumFlag`(141) (added in FIX.4.1) entirely — the shared `logon()`
  helper (used by nearly every scenario) unconditionally set it, so wiring FIX.4.0's now-real
  validator broke all 42 FIX.4.0 server-suite runs with `TagNotDefinedForMessage`; fixed `logon()`
  to skip tag 141 for FIX.4.0, and `logon_response_carries_reset_flag`'s FIX.4.0 case to expect the
  field absent (correctly echoing nothing, per BUG-28/FR-005's echo-only-what-was-sent rule) rather
  than asserting a fabricated `Y`. `cargo test -p truefix-at --test conformance --test coverage
  --test all_versions_have_validators`: all passing (server-suite floor: 424 → 456; the 3 new
  loops add 5 versions × 3 scenarios = 15 runs each newly-covered version/scenario pair beyond the
  pre-existing FIX.4.4-only single run, net +32 over the prior 424, plus this feature's earlier
  additions already counted in that 424 baseline).

### Latency scenario reject-reason assertions (FR-060, `NEW-29`)

- [X] T130 [US3] Strengthen `check_latency_timestamps`, `invalid_logon_bad_sending_time`,
  `sending_time_value_out_of_range`, `unparseable_sending_time_fails_latency` in
  `crates/truefix-at/src/scenarios.rs` to assert on `Text(58)`/reject-reason, not just "a Logout
  occurred" — no separate test task; this task directly strengthens existing scenario assertions
  (no new scenario runs). — **complete**: all four now assert `.field(58, "SendingTime accuracy
  problem")` on their Logout; `sending_time_value_out_of_range` additionally asserts
  `SessionRejectReason(373)="10"` on its preceding Reject, version-filtered for FIX.4.0/4.1 (which
  omit tag 373 entirely per BUG-59/FR-023, feature 007) — matches `reject_field_correctness.rs`'s
  existing unit coverage of that same version rule. `cargo test -p truefix-at --test conformance
  --test coverage`: all passing (456/456 scenario runs, floor unchanged).

### Buffer-empty-at-end assertion (FR-061, `NEW-30`)

- [X] T131 [P] [US3] Add a failing test: `run_scenario` fails if a spurious extra message remains
  buffered after all steps complete, in
  `crates/truefix-at/tests/buffer_empty_at_scenario_end.rs` (new). — **complete**: a hand-rolled
  fake TCP server (no production acceptor needed) answers a Logon then sends one extra,
  unrequested Heartbeat; confirmed the pre-fix `run_scenario` returned `Ok(())` regardless.
- [X] T132 [US3] Add the buffer-empty assertion to `run_scenario` in
  `crates/truefix-at/src/runner.rs` — makes T131 pass (depends on T131, sequenced after T127/T129).
  — **complete**: after the step loop, one more `read_message` call with a short (25ms) grace
  window — long enough for a same-host loopback straggler, short enough not to dominate suite
  runtime — must resolve `TimedOut`/`CleanEof`; `Message`/`DecodeFailed`/`ReadFailed` all fail the
  scenario. Chose 25ms after measuring: 100ms added ~39s to the 456-run server suite (12s → 51s);
  25ms keeps it at ~27s. `cargo test -p truefix-at --test conformance --test coverage --test
  buffer_empty_at_scenario_end`: all passing, no regressions from the new trailing check.

### Fixed-identity AT acceptor mode (FR-062, `NEW-83`; see data-model.md, Clarifications)

- [X] T133 [P] [US3] Add a failing test: `start_fixed_identity_acceptor` rejects a Logon whose
  SenderCompID/TargetCompID/BeginString don't match the configured fixed identity, in
  `crates/truefix-at/tests/fixed_identity_acceptor_scenarios.rs` (new). — **complete**: 4 tests
  (matching identity accepted as a control; wrong SenderCompID/TargetCompID/BeginString each
  refused — connection dropped with no reply, confirmed via `read_message`'s `CleanEof` outcome).
- [X] T134 [US3] Implement `start_fixed_identity_acceptor` in `crates/truefix-at/src/runner.rs`,
  additive alongside the existing `start_acceptor` (per Clarifications — no existing scenario setup
  changes) — makes T133 pass (depends on T133, sequenced after T129/T132). — **complete**: mirrors
  `start_acceptor`'s tweaks/validator wiring but calls `AcceptorBuilder::with_session` (a fixed
  `SessionConfig` keyed by its own `SessionId`) instead of `with_dynamic_template` — `route_and_run`
  already refuses (silently drops) any inbound Logon whose BeginString/CompIDs don't resolve to a
  registered static session and there's no template fallback, so no transport-layer change was
  needed, only this new harness-side constructor.
- [X] T135 [US3] Add the `1c_InvalidSenderCompID`/`1c_InvalidTargetCompID`/wrong-`BeginString`
  scenarios in `crates/truefix-at/src/scenarios.rs` using the new mode — raises the AT scenario-run
  floor (depends on T134). — **complete**: added `SessionTweaks::fixed_identity: Option<(String,
  String)>` so `run_report` can start either acceptor mode per-scenario without a parallel
  `Scenario`-running path; 3 new scenarios (`1c_InvalidSenderCompID`, `1c_InvalidTargetCompID`,
  `1d_InvalidLogonWrongBeginString`) run across all 9 `SUITE_VERSIONS`. `cargo test -p truefix-at
  --test conformance --test coverage --test fixed_identity_acceptor_scenarios`: all passing
  (server-suite floor: 456 → 483, +27 = 9 versions × 3 new scenarios).

### `check_match` flags extra fields (FR-063, `NEW-50`)

- [X] T136 [P] [US3] Add a failing test: `check_match` flags an unexpected extra field in an actual
  message, in `crates/truefix-at/tests/check_match_flags_extra_fields.rs` (new). — **complete**: 3
  tests (an extra field fails an exact match; the same extra field is ignored without `.exact()`,
  preserving today's default behavior; an exact match with no extra field passes as a control).
- [X] T137 [US3] Fix `check_match` in `crates/truefix-at/src/runner.rs` (~L216-228) — makes T136
  pass (depends on T136, sequenced after T132/T134). — **complete, as an opt-in `ExpectMsg::exact()`
  rather than the new default**: made exhaustive-field checking opt-in because this suite's ~90
  existing scenarios deliberately list only the fields they care about (e.g. `ExpectMsg::of("A")`
  for a Logon ack, never enumerating its own `EncryptMethod`/`HeartBtInt`) — checking exhaustively
  by default would require rewriting every one of them, well beyond this single item's scope. Added
  `ExpectMsg::exact`/`ALWAYS_ALLOWED_TAGS` (the always-legitimate framing/identity/volatile fields:
  BeginString/BodyLength/MsgType/MsgSeqNum/Sender+TargetCompID/SendingTime/PossDupFlag/
  OrigSendingTime/CheckSum); `check_match` iterates header+body+trailer when `expect.exact` is set
  and fails on any tag outside `fields` ∪ `ALWAYS_ALLOWED_TAGS`. `cargo test -p truefix-at --test
  conformance --test coverage --test check_match_flags_extra_fields`: all passing, no regressions
  (every existing scenario stays non-exact, so behavior for them is unchanged).

### Outbound `MsgSeqNum` ordering assertions (FR-064, `NEW-51`)

- [X] T138 [US3] Add outbound `MsgSeqNum(34)` ordering assertions to server-sent-sequence scenarios
  in `crates/truefix-at/src/scenarios.rs` that currently lack them — strengthens existing scenarios,
  no new runs (depends on T135). — **complete, scoped to the resend/gap-fill/sequence-reset
  scenario group** (the ones NEW-51 names as "server-sent-sequence scenarios") rather than all ~90
  scenarios in the file — exhaustively asserting `MsgSeqNum` on every `Step::Expect` across the
  whole suite is a much larger, unrelated undertaking; added it to `resend_request_gap_fill`,
  `sequence_reset_reset`, `sequence_reset_reset_missing_new_seq_no_rejected`,
  `sequence_reset_gap_fill_missing_new_seq_no_rejected`, `resend_request_missing_begin_seq_no_rejected`,
  `resend_request_missing_end_seq_no_rejected`, `gap_fill_drained_message_is_validated`,
  `resend_request_not_duplicated`, `resend_request_begin_zero_treated_as_one`,
  `resend_request_bounded_end`, `resend_request_too_high_answered_immediately`,
  `sequence_reset_gap_fill_advances`, `sequence_reset_gap_fill_backward_ignored`,
  `resend_request_nothing_to_resend`, `resend_request_chunk_size`, `chunked_resend_auto_continues`,
  `msgseqnum_correct`, `poss_dup_not_received`, `seq_reset_new_seq_no_equal`,
  `seq_reset_new_seq_no_less`, `seq_reset_gap_fill_new_seq_no_equal` (21 functions). **Real
  correctness finding made while wiring this up**: a GapFill sent in direct response to a
  `ResendRequest` (e.g. `resend_request_gap_fill`'s reply) carries the `MsgSeqNum` of the historical
  slot it fills (here, `1` — the Logon's own outbound slot), not a freshly-incremented live
  sequence number — resending/gap-filling replays a historical position rather than consuming a new
  one. First guessed `2` (treating it like any other new outbound message) and 4 scenario runs
  failed immediately; corrected to `1` for the 4 affected scenarios
  (`resend_request_gap_fill`/`resend_request_bounded_end`/`resend_request_begin_zero_treated_as_one`'s
  sole GapFill, and `BUG29_ResendRequestTooHighAnsweredImmediately`'s first GapFill — its
  *own subsequent* ResendRequest and Heartbeat correctly stayed on the live counter, `2` then `3`).
  `cargo test -p truefix-at --test conformance --test coverage`: all passing, 483/483 (floor
  unchanged — no new scenario runs, per this task's own note).

### Per-scenario suite reporting (FR-065, `NEW-52`)

- [X] T139 [P] [US3] Add a failing test: `server_acceptance_suite_passes` reports per-scenario pass/
  fail rather than one all-or-nothing result, in
  `crates/truefix-at/tests/per_scenario_reporting.rs` (new). — **complete**: added
  `runner::per_scenario_report(&[ScenarioResult]) -> String` (one `PASS`/`FAIL` line per run,
  including the failure reason); 2 unit tests against fabricated result sets confirm every run gets
  its own line (a single boolean would collapse a mixed pass/fail set down to just "failed") and
  an all-passing set has no `FAIL` lines.
- [X] T140 [US3] Restructure `server_acceptance_suite_passes` in
  `crates/truefix-at/tests/conformance.rs` — makes T139 pass (depends on T139). — **complete**:
  replaced the old "eprintln each failure, then assert on a bare count" shape with
  `eprintln!("{}", per_scenario_report(&results))` (every run, not just failures) followed by the
  same pass-count summary and a count-based assert (libtest doesn't support dynamically generating
  one `#[test]` per runtime-discovered scenario, so the itemized report — shown in full on failure
  via captured-output — is the mechanism, not literal sub-tests). `cargo test -p truefix-at --test
  conformance --test coverage --test per_scenario_reporting`: all passing.

### Duplicate dictionary definitions (FR-066, `NEW-43`)

- [X] T141 [P] [US3] Add a failing test: duplicate tag/name/msg_type definitions in a `.fixdict`
  file error on load, in `crates/truefix-dict/tests/duplicate_dict_definitions_error.rs` (new). —
  **complete**: 3 tests (duplicate field tag, duplicate field name, duplicate message type), all
  asserting a typed `ParseError::DuplicateDefinition { line, kind, key }`.
- [X] T142 [US3] Fix `parser.rs` (runtime loader) to detect duplicates — makes T141 pass (depends on
  T141, sequenced after T084). — **complete**: `parse()` rejects a repeated field tag, a repeated
  field name (different tag), and a repeated message MsgType, each as a `DuplicateDefinition` error
  naming the offending line. `cargo test -p truefix-dict --all-features`, `cargo clippy -p
  truefix-dict --all-targets --all-features -- -D warnings`: both clean.

### `strip_comment` preserves `#` in STRING values (FR-067, `NEW-44`)

- [X] T143 [P] [US3] Add a failing test: a STRING enum value containing `#` is not truncated, in
  `crates/truefix-dict/tests/strip_comment_preserves_hash_in_string.rs` (new). — **complete**: 3
  tests (a value literal containing `#` survives; a genuine whitespace-preceded trailing comment
  is still stripped; a full-line comment is still ignored). Confirmed the first test catches the
  bug via temporary revert.
- [X] T144 [US3] Fix `strip_comment()` in `crates/truefix-dict/src/parser.rs` — makes T143 pass
  (depends on T143, sequenced after T142). — **complete, with a dual-track ripple fix**: `#` now
  only starts a comment at line-start or when preceded by whitespace. `codegen.rs::parse_dict` had
  an identical inline `raw.find('#')` bug (not sharing `strip_comment`, per the dual-track design)
  — applied the same corrected rule there too for Principle IV parity, rather than leaving codegen
  with the old naive behavior. `cargo test -p truefix-dict --all-features`, `cargo clippy -p
  truefix-dict --all-targets --all-features -- -D warnings`: both clean.

### FIX 5.0/SP1/SP2 `BeginString` parsing (FR-068, `NEW-45`)

- [X] T145 [P] [US3] Add a failing test: `parse_fix_begin_string` distinguishes FIX 5.0/SP1/SP2, in
  `crates/truefix-dict/tests/fix5_subversion_parsing.rs` (new). — **complete, rewritten from a
  broken draft**: an existing draft test file only set `BeginString`/`MsgType` on the probe message
  and used a dictionary with no `field`/`header` definitions at all, so `validate()` failed on an
  unrelated "tag not defined in dictionary" error before ever reaching the BeginString-match check
  — rewrote it following `version_meta.rs`'s existing full header/trailer-field pattern so the test
  actually isolates the SP-mismatch behavior. Confirmed catching the bug via temporary revert.
- [X] T146 [US3] Fix `parse_fix_begin_string` in `crates/truefix-dict/src/validate.rs` (~L265) —
  makes T145 pass (depends on T145, sequenced after T091). — **complete**: the function already
  returned a `(major, minor, service_pack, extension_pack)` 4-tuple and `validate()`'s BeginString
  match check already compared all four fields — this item's logic was already fixed in a prior
  pass; T145's own test file was the only remaining gap. `cargo test -p truefix-dict
  --all-features`, `cargo clippy -p truefix-dict --all-targets --all-features -- -D warnings`:
  both clean.

### Circular config variable references (FR-069, `NEW-46`)

- [X] T147 [P] [US3] Add a failing test: a self-referential/circular `${var}` errors instead of
  emitting literal unresolved text, in
  `crates/truefix-config/tests/circular_var_reference_errors.rs` (new). — **complete**: 3 tests
  (self-reference; two-hop mutual cycle; a genuinely non-circular transitive chain still resolves
  fully — this last case doubles as a fix for a pre-existing, more basic bug: interpolation was
  only ever one level deep, so `A=${B}`/`B=literal` previously left `A` as the literal text
  `"${B}"` rather than resolving through to `B`'s value). Both new-error tests fail to even
  *compile* against the pre-fix code (no such variant existed), the strongest possible confirmation
  the behavior is new.
- [X] T148 [US3] Fix variable resolution in `crates/truefix-config/src/lib.rs` (~L200-240) — makes
  T147 pass (depends on T147). — **complete**: `interpolate_value` now recurses through a chain of
  `${var}` references (fixing the one-level-only limitation above) while tracking the in-progress
  variable names on a `resolving: &mut Vec<String>` stack; encountering a name already on that
  stack returns a new `ConfigError::CircularVariableReference { line, name }` instead of emitting
  the literal unresolved text. `cargo test -p truefix-config`, `cargo clippy -p truefix-config
  --all-targets --all-features -- -D warnings`: both clean.

### `Monitor` entry removal on disconnect (FR-070, `NEW-48`)

- [X] T149 [P] [US3] Add a failing test: a `Monitor` entry is removed when its connection
  disconnects, in `crates/truefix/tests/monitor_removes_entry_on_disconnect.rs` (new). —
  **complete**: 1 test driving a real acceptor/initiator pair through `Monitor`, confirming the
  session's entry disappears from `monitor.sessions()`/`is_connected()`/`status()` once the
  initiator's connection is aborted. Confirmed catching the bug via temporary revert.
- [X] T150 [US3] Fix `Monitor`/`MonitorEntry` in `crates/truefix-transport/src/lib.rs` (~L247-270)
  to remove entries on disconnect — makes T149 pass (depends on T149, sequenced after T042/T078/T099
  since all touch `lib.rs`). — **complete, with a follow-on simplification**: `mark_disconnected`
  now removes the entry from the map instead of flipping a `connected: bool` flag — a
  reconnecting fixed-identity session simply gets re-inserted by its next `register()` call. Since
  every entry remaining in the map is therefore always connected, the now-redundant `connected`
  field was removed from `MonitorEntry` and `is_connected` simplified to a plain
  `contains_key` check. `cargo test -p truefix-transport --all-features`, `cargo test -p truefix`,
  `cargo clippy -p truefix-transport --all-targets --all-features -- -D warnings`: all clean.

### HTTP CONNECT proxy response handling (FR-071, `NEW-49`)

- [X] T151 [P] [US3] Add a failing test: HTTP CONNECT status-token validation rejects a non-numeric
  token, in `crates/truefix-transport/tests/http_connect_status_validation.rs` (new). —
  **complete**: 2 tests against a fake fixed-response HTTP CONNECT proxy (a `"2xx"` non-numeric
  token is rejected; a genuine `"200"` is still accepted). Confirmed the first catches the bug via
  temporary revert.
- [X] T152 [US3] Fix the CONNECT response reader in `crates/truefix-transport/src/proxy.rs`
  (~L95-140) to validate the token is numeric and read the response efficiently — makes T151 pass
  (depends on T151, sequenced after T040/T114). — **complete**: replaced the byte-at-a-time read
  loop with `peek_proxy_header`'s established peek-then-`read_exact` pattern (grow a peek buffer
  looking for `\r\n\r\n`, then consume exactly that many bytes) — leaves any bytes the proxy might
  send immediately after the header untouched in the socket. Status validation now parses the
  token as `u16` and checks it's in `200..300`, rejecting a merely `'2'`-prefixed non-numeric
  token. `cargo test -p truefix-transport --all-features`, `cargo clippy -p truefix-transport
  --all-targets --all-features -- -D warnings`: both clean.

### `DataDictionary::extend()` collision detection (FR-072, `NEW-53`)

- [X] T153 [P] [US3] Add a failing test: `extend()` detects a `field_by_name` collision (different
  tag, same name) and recomputes `member_tags` after merge, in
  `crates/truefix-dict/tests/extend_detects_name_collision.rs` (new). — **complete**: 2 tests (a
  colliding field name aborts the merge with a typed `DictMergeConflict{kind:"field name"}`; a
  merge that newly introduces a group definition updates an already-existing message's
  `member_tags` to recognize that group's members). Confirmed both catch the bug via temporary
  revert.
- [X] T154 [US3] Fix `DataDictionary::extend()` in `crates/truefix-dict/src/model.rs` (~L290-360) —
  makes T153 pass (depends on T153, sequenced after T048/T059/T086). — **complete**: added a
  `field_by_name` collision check (same name, different tag) to the existing dry-run conflict pass
  as a new `DictMergeConflict{kind:"field name"}`; after merging, every message's `member_tags` is
  recomputed against `self`'s now-merged `groups` map (previously left stale from whichever
  dictionary's own parse-time `groups` map happened to be smaller). Made `parser::
  collect_group_members` `pub(crate)` so `model.rs` could reuse it rather than duplicating the
  recursive-expansion logic. `cargo test -p truefix-dict --all-features`, `cargo clippy -p
  truefix-dict --all-targets --all-features -- -D warnings`: both clean.

### Session `Reject` before `Logout` (FR-073, `NEW-87`)

- [X] T155 [P] [US3] Add a failing test: a non-Logon latency/CompID-mismatch/PossDup-falsification
  disconnect sends a session `Reject` before the `Logout`, in
  `crates/truefix-session/tests/reject_before_logout.rs` (new). — **complete**: 5 tests (latency
  failure; CompID mismatch; incorrect-BeginString control staying Logout-only; PossDup
  falsification; a Logon-specific bad-SendingTime control also staying Logout-only, matching QFJ's
  own distinct `logoutWithErrorMessage` path for Logon). Confirmed the 3 non-control tests catch
  the bug via temporary revert.
- [X] T156 [US3] Add an AT scenario for the two-message sequence in
  `crates/truefix-at/src/scenarios.rs` (`contracts/hardening-and-tooling.md`). — **complete, via
  strengthening 3 existing scenarios rather than adding new ones**:
  `GAP08_PossDupOrigSendingTimeAfterSendingTime`, `2k_CompIDDoesNotMatchProfile`, and
  `2o_SendingTimeValueOutOfRange` each already exercised one of the three affected failure modes
  but asserted only the old Logout-only shape — added a `Step::Expect(ExpectMsg::of("3"))`
  immediately before each's existing Logout expectation (SessionRejectReason itself isn't asserted
  at this layer, since it's version-filtered per `reject_field_correctness.rs`'s existing unit
  coverage). No new scenario runs added (floor unchanged at 444); `cargo test -p truefix-at --test
  conformance --test coverage`: 100% passing (27 failures across 9 versions × 3 scenarios fixed).
- [X] T157 [US3] Fix the latency/identity-failure branches in `on_received`
  (`crates/truefix-session/src/state.rs` ~L702-721) and `poss_dup_falsification_check`'s caller
  (~L780-784) to send `Reject` then `Logout` — makes T155/T156 pass (depends on T155, T156,
  sequenced after T112/T096). — **complete, with a necessary distinction the task description
  didn't spell out**: added `build_session_reject` (builds+sends just the wire Reject) and
  `session_reject_before_logout` (that, plus `reject_logon`'s existing Logout+disconnect) as new
  helpers; `identity_problem`'s return type changed from a bare `&'static str` to a new
  `IdentityProblem { BeginString, CompId }` enum, since only the CompID case gets the new
  two-message treatment — an incorrect BeginString has no QFJ equivalent and stays Logout-only.
  Also discovered mid-implementation that the check must additionally be skipped for a *Logon*
  message specifically (computed a new `is_logon` flag up-front in `on_received`): the source
  audit's own text notes a Logon-specific bad-time/bad-CompID uses QFJ's
  `logoutWithErrorMessage`(Logout-only) rather than this two-message shape — without the guard,
  every existing Logon-latency/Logon-CompID test/AT-scenario would have started failing.
  Additionally applied the same fix to the two `poss_dup_anti_replay_violation` call sites
  (too-high-gap-fill and generic too-low arms) beyond the single `poss_dup_falsification_check`
  call site the source document literally named — same PossDup-falsification category, same bug
  shape. `cargo test -p truefix-session`, `cargo test -p truefix-at --test conformance --test
  coverage`, `cargo clippy -p truefix-session -p truefix-at --all-targets --all-features -- -D
  warnings`: all clean.

### `DefaultApplVerID` validation (FR-074, `NEW-88`)

- [X] T158 [P] [US3] Add a failing test: an invalid/typo'd `DefaultApplVerID(1137)` on a FIXT 1.1
  Logon is rejected, in `crates/truefix-session/tests/defaultapplverid_validated.rs` (new). —
  **complete**: 3 tests (unregistered value rejected; a valid/registered value still logs on; no
  `FixtDictionaries` configured at all leaves the old unvalidated behavior in place, since there's
  nothing to validate against). Confirmed the first catches the bug via temporary revert.
  **Ripple effect found and fixed**: `crates/truefix-session/tests/validation_hook.rs`'s
  `fixt_dictionaries_with_no_resolvable_appl_ver_id_skips_validation_like_no_dictionary` logged on
  with an *unregistered* Logon-level DefaultApplVerID and asserted that succeeded — exactly the gap
  this fix closes. Rewritten (renamed
  `unresolvable_per_message_appl_ver_id_skips_validation_like_no_dictionary`) to use a valid,
  registered DefaultApplVerID on Logon and instead exercise the still-intentionally-unvalidated
  case: a later message's own per-message ApplVerID(1128) resolving to nothing (a `validate_app`
  dict-selection concern, unrelated to Logon-time `DefaultApplVerID` validation).
- [X] T159 [US3] Add an AT scenario for this in `crates/truefix-at/src/scenarios.rs`. —
  **deferred to the unit tests in T158, with reason documented**: confirmed (again)
  `crates/truefix-at/src/runner.rs` has zero FIXT dual-dictionary (`fixt_dictionaries`) support —
  same fallback situation as `NEW-58`/T019, `NEW-56`/T022, `NEW-62`/T029, and `NEW-18`/T054.
- [X] T160 [US3] Fix `on_logon` in `crates/truefix-session/src/state.rs` (~L1363-1369) to validate
  `DefaultApplVerID` against the transport dictionary before accepting — makes T158/T159 pass
  (depends on T158, T159, sequenced after T157). — **complete**: validates a present, non-empty
  inbound `DefaultApplVerID(1137)` against `FixtDictionaries::application_for` (reusing its
  existing lookup rather than adding a new registry-membership API) before storing it into
  `negotiated_appl_ver_id`; a value that resolves to no registered application dictionary now
  routes through `reject_logon` with `SessionRejectReason=5`/`RefTagID=1137`, matching QFJ's
  `nextLogon`. Only applies when a `FixtDictionaries` is actually configured on the session.
  `cargo test -p truefix-session`, `cargo clippy -p truefix-session --all-targets --all-features
  -- -D warnings`: both clean.

### `LogoutTimeout` default (FR-075, `NEW-89`)

- [X] T161 [US3] Add a test asserting `builder.rs`'s `.cfg`-parsing default for `LogoutTimeout` is
  `2`, then change `u32_key(map, "LogoutTimeout", &session, 10)` to `2` in
  `crates/truefix-config/src/builder.rs` (~L478), in
  `crates/truefix-config/tests/logout_timeout_default.rs` (new) — combined test+fix task (trivial
  single-value change); **flag prominently in the PR description**: unlike `NEW-73`, this changes
  real `.cfg`-driven deployment behavior (research.md's disclosure) — sequenced after T037/T038/T052/
  T072 since all touch `builder.rs`. — **complete — flagged: real deployment-behavior change**:
  an acceptor/initiator that didn't explicitly set `LogoutTimeout` in its `.cfg` will now wait only
  2 seconds (not 10) for the counterparty's `Logout` reply before disconnecting after sending its
  own. `SessionConfig::new()`'s own Rust-level default (`truefix-session/src/config.rs`, used when
  a `SessionConfig` is constructed directly rather than through `.cfg` parsing) is intentionally
  left at `10` — untouched, since FR-075/NEW-89 only names the `.cfg`-parsing default. 2 tests
  (unset defaults to 2; still configurable to another value). `cargo test -p truefix-config`,
  `cargo clippy -p truefix-config --all-targets --all-features -- -D warnings`: both clean.

### Async log-writer shutdown (FR-076, `NEW-91`; see data-model.md)

- [X] T162 [P] [US3] Add a failing test: `Engine::shutdown()` flushes/awaits outstanding entries
  queued in each async log backend before returning, in
  `crates/truefix-log/tests/async_writer_flush_on_shutdown.rs` (new). — **complete**: 2 tests
  (queued entries flushed before `shutdown()` returns; `shutdown()` is idempotent).
- [X] T163 [US3] Add a `shutdown`/flush method to each async log backend's handle (retaining the
  `JoinHandle`, closing the channel, awaiting drain) in
  `crates/truefix-log/src/{sql,mssql,mongo,redb}.rs`, and call it from `Engine::shutdown()`/`Drop` in
  `crates/truefix/src/lib.rs` — makes T162 pass (depends on T068, T162). — **complete, with a
  deliberate deviation from wiring into `Engine::shutdown()`/`Drop` directly**: added
  `Log::shutdown()` (default no-op, `async_trait`) to the trait, implemented on all four async
  backends (`tx`/`task` moved behind `std::sync::Mutex<Option<_>>` so `&self` can `take()` them)
  plus the `Box<dyn Log>`/`CompositeLog`/`SessionPrefixLog` forwarding impls. `Engine::shutdown()`
  and `Drop for Engine` intentionally remain synchronous (BUG-21/FR-023, feature 007) — instead
  added `Engine::logs() -> &[Arc<dyn truefix_log::Log>]`, collecting every log built during
  `Engine::start`, so a caller wanting a graceful flush can `for log in engine.logs() { log.shutdown().await; }`
  before calling `shutdown()`, mirroring the existing `initiators()`-then-`.logout().await` pattern.
  `cargo test -p truefix-log --features mongodb,mssql,sql,redb --test async_writer_flush_on_shutdown`:
  2/2 passing.

### Deterministic acceptor-group startup order (FR-077, `NEW-92`)

- [X] T164 [P] [US3] Add a failing test: `Engine::start` processes multi-session acceptor groups in
  a stable, sorted order across repeated runs, in
  `crates/truefix/tests/acceptor_group_start_order_deterministic.rs` (new). — **complete, as a
  `#[cfg(test)]` unit module in `crates/truefix/src/lib.rs` instead of a new integration-test
  file, with reason documented**: `ResolvedSession` (the acceptor-group `HashMap`'s value type) has
  no path from an external test to construct the exact grouping `Engine::start` builds internally
  — same situation as `scheduled_initiator_nonblocking_connect_tests`'s existing precedent in
  `truefix-transport/src/lib.rs`. Extracted the sort into a small, independently-testable
  `sorted_by_endpoint<T>(HashMap<SocketEndpoint, T>) -> Vec<(SocketEndpoint, T)>` and added
  `acceptor_group_start_order_tests::sorted_by_endpoint_is_stable_regardless_of_input_order`:
  builds the same 3-entry map two ways (forward and reverse insertion order) and confirms both
  produce the identical, ascending-`(host, port)` output.
- [X] T165 [US3] Sort `acceptor_groups` by `SocketAddr` before iterating in `Engine::start`
  (`crates/truefix/src/lib.rs` ~L326-339) — makes T164 pass (depends on T163, T164). —
  **complete, sorted by `SocketEndpoint`'s `(host, port)` rather than a resolved `SocketAddr`**:
  `acceptor_groups` is keyed by the pre-resolution `SocketEndpoint` (DNS resolution happens later,
  per T072/NEW-26's connect-time-resolution fix), so there's no `SocketAddr` available yet at this
  point to sort by; `(host, port)` gives the same reproducible-across-runs guarantee. `cargo test -p
  truefix --lib acceptor_group_start_order_tests --test multi_session_acceptor_cfg --test
  config_start --test continue_on_error`: all passing, no regressions.

### TLS trust-store load diagnostics (FR-078, `NEW-95`)

- [X] T166 [P] [US3] Add a failing test: a trust-store file with one or more malformed certificates
  surfaces a warning (or an error if the resulting store is empty), in
  `crates/truefix-transport/tests/tls_trust_store_load_diagnostics.rs` (new). — **complete**: 2
  tests (empty trust-store bytes now error rather than silently building a non-functional empty
  store; a trust store with at least one valid certificate still succeeds).
- [X] T167 [US3] Track and surface per-cert load failures in `load_root_store`/
  `load_root_store_bytes` (`crates/truefix-transport/src/tls_config.rs` ~L117-135) — makes T166 pass
  (depends on T027, T069, T166). — **complete, per data-model.md's own sketch**: added
  `TlsConfigError::EmptyTrustStore` and a shared `add_certs_tracking_failures` helper — counts
  `RootCertStore::add` failures (still individually best-effort-skipped, unchanged) and emits
  `tracing::warn!` when any occurred; both loaders now return `EmptyTrustStore` when the resulting
  store has zero certificates (previously `Ok(RootCertStore::empty())`, functionally identical to
  NEW-11's no-trust-store-configured failure mode but for an operator who *did* configure one).
  **Regression found and fixed while running the full `truefix-transport` suite** (pre-existing,
  from an earlier T160/NEW-88 change, unrelated to this task):
  `fixt_group_aware_decode.rs`'s test built a `FixtDictionaries` with zero registered application
  dictionaries, then sent a Logon with `DefaultApplVerID=9` — T160's stricter validation now
  rejects that as an unresolvable ApplVerID, which this test (about header-group structuring, not
  ApplVerID validation) didn't anticipate. Fixed by registering a FIX50SP2 application dictionary
  under key `"9"`. `cargo test -p truefix-transport`: full suite passing.

### `MAX_BODY_LEN` consistency between `decode` and `frame_length` (FR-079, `NEW-04`)

- [X] T168 [P] [US3] Add a failing test: `tokenize_validated`/`decode` called directly (bypassing
  `frame_length`) rejects a body length exceeding `MAX_BODY_LEN`, in
  `crates/truefix-core/tests/decode_max_body_len_consistency.rs` (new). — **complete**: 2 tests (a
  declared BodyLength of `MAX_BODY_LEN + 1` is rejected with `BodyLengthTooLarge`, matching
  `frame_length`'s own error shape; a normal, well-formed message still decodes as a control).
- [X] T169 [US3] Add the `MAX_BODY_LEN` guard to `tokenize_validated` in
  `crates/truefix-core/src/codec/decode.rs` (~L103-129) — makes T168 pass (depends on T168,
  sequenced after T062/T064). — **complete**: added the identical `declared_bl > MAX_BODY_LEN` →
  `DecodeError::BodyLengthTooLarge` check `frame_length` already had, right after parsing
  BodyLength (before the MsgType-presence/CheckSum/BodyLength-mismatch checks that follow).
  **Ripple effect found and fixed**: `frame_checksum_overflow_hardening.rs`'s
  `decode_checksum_does_not_overflow_or_panic_for_a_very_large_message` deliberately round-tripped
  a ~17,000,000-byte message through `decode()` specifically *because* `decode()` had no
  `MAX_BODY_LEN` cap of its own before this fix — now that it does, that message can no longer
  reach `decode()`'s checksum computation at all (`17,000,000 > MAX_BODY_LEN`'s 16,777,216).
  Converted to a source-level check (renamed `decode_checksum_sum_still_uses_a_u64_accumulator`,
  mirroring this same file's existing `frame_length_uses_checked_add_not_bare_addition` pattern for
  changes whose triggering condition became unreachable via any real, allocatable input) confirming
  the `u64` accumulator fix is still present as defense-in-depth. `cargo test -p truefix-core`: all
  passing (28 test binaries, 0 failures).

### `SessionId::Display` includes `SubID`/`LocationID` (FR-080, `NEW-39`)

- [X] T170 [P] [US3] Add a failing test: two sessions differing only by `SubID`/`LocationID` render
  distinguishably, in `crates/truefix-session/tests/sessionid_display_includes_subid.rs` (new). —
  **complete**: 3 tests (SubID-only difference; LocationID-only difference; a plain 3-field
  SessionId still renders exactly as before, as a control against a format-breaking change).
- [X] T171 [US3] Fix `SessionId`'s `Display` impl in `crates/truefix-session/src/session_id.rs`
  (~L93) — makes T170 pass (depends on T170). — **complete**: appends `/sub_id` and `/location_id`
  after each CompID when present (`SERVER/SUB1/LOC1->CLIENT`), via a small shared
  `write_comp_id_with_qualifiers` helper for the sender/target halves; the no-sub/no-location case
  is byte-for-byte unchanged (`"FIX.4.4:SERVER->CLIENT"`). `cargo test -p truefix-session`: full
  suite passing, no regressions from the format change.

### `UnresolvedVariable` line-number tracking (FR-081, `NEW-47`)

- [X] T172 [P] [US3] Add a failing test: an `UnresolvedVariable` error after per-session merging
  reports the original source line, not `line: 0`, in
  `crates/truefix-config/tests/unresolved_variable_line_number.rs` (new). — **complete**: 3 tests
  (a plain session's own line; a key inherited unchanged from `[DEFAULT]` reports `[DEFAULT]`'s own
  line; a key overridden per-session reports the session's own line, not the shadowed default's).
  Confirmed catching the bug via temporary revert (old code always reported `line: 0`).
- [X] T173 [US3] Thread line-number tracking through config merging in
  `crates/truefix-config/src/lib.rs` (~L215-235) — makes T172 pass (depends on T148, T172). —
  **complete**: introduced `RawSection = BTreeMap<String, (String, usize)>`, threading each key's
  original 1-based source line through parsing, `[DEFAULT]`/`[SESSION]` merging, and interpolation
  (previously plain `BTreeMap<String, String>`, discarding line info immediately after parsing); a
  new `strip_lines` helper converts back to the public `BTreeMap<String, String>` contract once
  interpolation succeeds. `cargo test -p truefix-config`, `cargo clippy -p truefix-config
  --all-targets --all-features -- -D warnings`: both clean.

### Store efficiency: single-handle resend, no-collect reset (FR-082, `NEW-41`, `NEW-42`)

- [X] T174 [P] [US3] Add a failing test/assertion: `BodyLog::read`'s `range()` reuses a single file
  handle across a multi-message resend (not one open per message), in
  `crates/truefix-store/tests/bodylog_read_single_handle_per_range.rs` (new). — **complete**: 2
  tests (a multi-entry range still returns every message's correct bytes; a source-level check
  that `range()` contains exactly one `File::open` call, mirroring this codebase's established
  pattern for internal-shape checks a black-box test can't directly observe).
- [X] T175 [P] [US3] Add a failing test/assertion: `redb::reset()` does not collect all keys into an
  intermediate `Vec` before removing them, in
  `crates/truefix-store/tests/redb_reset_no_intermediate_collect.rs` (new). — **complete**: 2 tests
  (`reset()` still clears every stored message and resets both sequence counters, any message
  count; a source-level check that `reset()` contains no `Vec<` and does use `retain_in`).
- [X] T176 [US3] Fix `BodyLog::read` in `crates/truefix-store/src/file.rs` and `reset()` in
  `crates/truefix-store/src/redb.rs`, with behavior otherwise unchanged (existing store test suites
  must still pass) — makes T174/T175 pass (depends on T105, T174, T175). — **complete**: extracted
  `BodyLog::read_body_at(&mut File, offset, len)` (seek + read_exact only) shared by `read()`
  (opens its own handle, unchanged behavior) and `range()` (resolves every `(offset, len)` entry
  under one lock acquisition, then opens exactly one handle reused across the whole range).
  `RedbStore::reset()` now calls `messages.retain_in(session_range, |_, _| false)` directly instead
  of collecting every key into a `Vec<(String, u64)>` and removing each individually. `cargo test -p
  truefix-store --all-features`: full suite passing, no regressions.

### Codec allocation/complexity hygiene (FR-083, `NEW-35`, `NEW-36`, `NEW-37`, `NEW-38`)

- [X] T177 [US3] Confirm the full existing `truefix-core` codec/framing test suite passes unchanged
  before starting (behavior-preservation baseline) — no new file, a pre-check step. — **complete**:
  confirmed passing both before and after T178's changes (`cargo test -p truefix-core`).
- [X] T178 [US3] Reduce manual linear byte-scanning in the decode/framing hot path
  (`crates/truefix-core/src/codec/decode.rs` ~L155, `crates/truefix-core/src/framing.rs` ~L61),
  double/triple allocation of field values on decode (~L38, ~L50), and per-field `tag.to_string()`
  allocation on encode (`crates/truefix-core/src/codec/encode.rs` ~L122) — implement alongside T107
  (`FR-047`'s `render_members_ordered` dedup, same function/file area) — makes T177's baseline still
  pass unchanged (depends on T107, T177); re-run the full `truefix-core` test suite plus any new
  allocation-count assertions from T107. — **complete, three independent fixes**: (1) promoted the
  already-transitive `memchr` crate (Unlicense OR MIT) to a direct `truefix-core` dependency and
  replaced both hand-rolled `.iter().position()` scans (`framing.rs`'s `find`, `decode.rs`'s
  `memchr`) with `memchr::memchr` — SIMD-accelerated instead of a scalar byte-by-byte loop; (2)
  `decode()`'s per-field loop now consumes `fields` via `into_iter()` instead of `.iter()` +
  `.clone()`, moving each value directly into its `Field` rather than allocating a second copy of
  every field's bytes; (3) `render_raw` no longer allocates a `String` per field via
  `tag.to_string()` — added `write_tag` (writes decimal digits directly into the output buffer via
  a small stack scratch array, `clippy::indexing_slicing`-clean via `get`/`get_mut`). `cargo test -p
  truefix-core` and `cargo clippy -p truefix-core --all-targets -- -D warnings`: both clean; `cargo
  build --workspace --all-features`: clean.

**Checkpoint**: All user stories should now be independently functional.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Final regression sweep and documentation, spanning all three user stories.

- [X] T179 [P] Run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`; fix any diffs/warnings introduced by this feature's changes. —
  **complete**: `cargo fmt --all` reformatted the new US3 code (mechanical only, no logic change);
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean across all 9
  crates.
- [X] T180 Run `cargo test --workspace --all-features` and `cargo test -p truefix-at --features
  mongodb,mssql --test conformance --test coverage`; confirm the AT scenario-run floor has grown
  above 424 by exactly the count of new scenarios added in T008, T011, T014, T019, T022, T029, T032,
  T054, T090, T093, T111, T129, T135, T156, T159 — record the final number in this task's outcome
  and update `quickstart.md`'s Prerequisites section with it. — **complete, with a corrected
  command**: `truefix-at` has no `mongodb`/`mssql` Cargo features (those belong to
  `truefix-store`/`truefix-log`/`truefix-config`/`truefix`; the AT harness doesn't touch either
  backend) — ran `cargo test -p truefix-at --test conformance --test coverage` instead.
  `cargo test --workspace --all-features`: 249 test binaries, 0 failures. AT scenario-run floor:
  **483** (up from 424 at this feature's Setup baseline) — the growth comes from this feature's
  own additions across both US1/US2 (T008/T011/T014/T019/T022/T029/T032/T054/T090/T093/T111,
  landing at 444 per T111's own note) and US3 (T128/T129's `FLAT_DICTIONARY_VERSIONS` extension of
  3 existing scenarios to 5 versions each, +12 net beyond the original single-FIX.4.4 runs; T135's
  3 new Logon-identity-mismatch scenarios × 9 versions, +27) — 444 + 12 + 27 = 483, confirmed by
  `cargo test -p truefix-at --test conformance server_acceptance_suite_passes -- --nocapture`'s
  `AT report: 483/483 scenario runs passed`. `quickstart.md`'s Prerequisites section updated (see
  T181).
- [X] T181 [P] Update `quickstart.md`'s placeholder test-file names with the actual landed file names
  from every task above (the same correction 006's T092 and 007's T118-equivalent Polish task made).
  — **complete**: fixed 15 stale/wrong references (verified every remaining `--test <name>`
  reference resolves to a real file via a script cross-checking the whole document against the
  repo) — `NEW-54`'s line now notes it was refuted (T003/T004, no file); `NEW-11`/`NEW-14`/`NEW-17`
  point at their actual `#[cfg(test)]` unit-test-module or existing-file locations instead of
  never-created integration-test files; `NEW-96`/`NEW-89` corrected from `truefix-transport`/
  `truefix-session` to their real crate (`truefix-config`); `NEW-85`/`NEW-86` merged into the
  single `too_high_logout_and_resend_request.rs` file that actually landed; `NEW-62` points at the
  strengthened `schedule_enforcement.rs` rather than a new file; `NEW-29`/`NEW-51`/`NEW-61`/
  `NEW-35`-`38` noted as "no new file" (existing scenarios/suite strengthened in place, or a
  manually-verified one-line build-script directive); `NEW-92` points at its actual
  `#[cfg(test)]` module in `truefix/src/lib.rs`; added the missing `NEW-52`
  (`per_scenario_reporting.rs`) entry; corrected the Prerequisites/Final-regression-check sections'
  424→483 floor and removed the invalid `truefix-at --features mongodb,mssql` (that crate has no
  such features). Spot-verified a representative sample of the corrected commands actually run.
- [X] T182 Verify no reference-implementation source-copying occurred across this feature's changes
  (Constitution Principle III) — spot-check the FIXT/dictionary/codegen-heavy tasks (T017, T020,
  T046, T048, T059, T086, T116-T125, T146, T154) in particular, since those areas most closely track
  QuickFIX/J/Go documented behavior. — **complete**: grepped the whole changed surface
  (`truefix-dict`/`truefix-core`/`truefix-session`) for copyright notices, QFJ/QFGo source URLs,
  and Java-isms (`public class`, `@Override`, `import java.`, etc.) that would indicate a verbatim
  port — zero hits. Read `tags::is_header`'s tag list (T017), `codegen::version_appl_ver_id`/
  `version_begin_string` (T046), and `model::DataDictionary::extend` (T154) directly: all are
  idiomatic, original Rust using only the FIX protocol's own public standard tag numbers/enum
  values (not QFJ/QFGo's proprietary source), matching this feature's established
  no-source-copying discipline throughout.
- [X] T183 Confirm every acceptor-only fix (T006/T009 acceptor role, T012, T030, T078, T150) and
  every symmetric session-protocol fix (T009, T015, T023, T033, T055, T080, T094, T112, T157, T160)
  has both acceptor-side and initiator-side test coverage per Constitution Principle VI — file a
  follow-up task for any gap found rather than closing this task with a gap silently accepted. —
  **complete, one real gap found and fixed**: grepped `state.rs` for every `role ==`/
  `Role::Acceptor`/`Role::Initiator` branch (only 4 exist in the whole file) and cross-referenced
  each listed task's fix location against them. T006/T009's pre-logon DoS timeout and T078's
  `SocketReuseAddress` are genuinely acceptor-only (no initiator equivalent exists — initiators
  don't accept unauthenticated inbound connections or bind listening sockets); T012's
  `acceptor_forces_reset` is deliberately acceptor-only by design (the initiator's own `ResetOnLogon`
  handling is a separate, pre-existing, unmodified code path in `on_connected`); T150's Monitor-entry
  removal test already drives a real acceptor/initiator pair. T009/T015/T023/T033/T055/T080/T112/
  T157/T160's fixes all sit outside any role-branch (confirmed by reading the surrounding code:
  `identity_problem`, `latency_ok`, `validate_app`'s dictionary-invalid-Logon path, the
  `DefaultApplVerID` check, etc. all execute identically before `on_logon`'s role split), so their
  single-role (`Role::Acceptor`) unit tests are representative of both, matching this project's own
  established `initiator_validation.rs` precedent. T094 already had both roles in its own test.
  **T030 was a genuine gap**: NEW-62's schedule-exit Logout fix (`on_tick`'s `SessionState::LoggedOn`
  arm) has no role check at all, but `schedule_enforcement.rs` only ever exercised it for
  `Role::Acceptor`. Added
  `logged_on_initiator_session_is_disconnected_once_the_schedule_window_elapses`, confirming the
  identical behavior for an initiator session. `cargo test -p truefix-session --test
  schedule_enforcement`: 6/6 passing (was 5).
- [X] T184 Run `quickstart.md`'s full "Final regression check" section end-to-end as the last gate
  before considering this feature complete. — **complete**: `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` re-confirmed clean after
  T183's addition to `schedule_enforcement.rs`. `cargo test --workspace --all-features` (249 test
  binaries, 0 failures) and the AT suite at 483/483 scenario runs were already confirmed at T180;
  re-ran only the specifically-touched `cargo test -p truefix-session --test schedule_enforcement`
  (6/6 passing, was 5) rather than repeating the full workspace run, per user instruction. All of
  US3 (T115-T178) plus Polish (T179-T184) complete — feature 009 finished.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **User Story 1 (Phase 2)**: Depends on Setup (T002 specifically gates T027). No blocking
  foundational phase — every US1 task pair is independently startable once Setup completes, subject
  only to the same-file sequencing noted inline (state.rs, lib.rs, mongo.rs, mssql.rs, builder.rs
  clusters).
- **User Story 2 (Phase 3)**: Depends on Setup; several tasks are sequenced after specific US1 tasks
  that touch the same file (noted inline per task) — not a hard phase-level gate, since US2 is
  designed to be independently testable per spec.md.
- **User Story 3 (Phase 4)**: Depends on Setup; similarly sequenced after specific US1/US2 tasks
  sharing a file (noted inline), especially T107/T178 (same function, `FR-047`+`FR-083`).
- **Polish (Phase 5)**: Depends on all three user stories being complete.

### Parallel Opportunities

- All `[P]`-marked test tasks within a story can be written in parallel (different files).
- Within a story, only the *first* task touching a shared file (`state.rs`, `lib.rs`, `mongo.rs`,
  `mssql.rs`, `builder.rs`, `model.rs`, `validate.rs`, `decode.rs`) needs to land before the next
  task touching the same file starts — see each task's inline "sequenced after" note.
- Different user stories can be worked on in parallel by different contributors once Setup
  completes, at the cost of resolving the inline same-file sequencing notes across story boundaries
  (e.g. T076 (US2) sequenced after several US1 `state.rs` tasks) via normal merge/rebase discipline.

---

## Parallel Example: User Story 1

```bash
# Launch independent test-writing tasks for User Story 1 together (different files):
Task: "Add failing test for Mongo sequence persistence in crates/truefix-store/tests/mongo_seq_persistence.rs"
Task: "Add failing test for FIXT header tags in crates/truefix-core/tests/fixt_header_tags.rs"
Task: "Add failing test for BeginSeqNo=0 in crates/truefix-session/tests/resend_begin_seq_zero.rs"
Task: "Add failing test for TLS default trust store in crates/truefix-transport/tests/tls_native_trust_store_fallback.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: User Story 1 (21 items — the audit's own highest-severity tier).
3. **STOP and VALIDATE**: Run `cargo test --workspace --all-features` and the AT suite; confirm
   every US1 acceptance scenario in spec.md passes.
4. This is the release-blocking MVP — everything else is narrower blast radius or hardening.

### Incremental Delivery

1. Setup → User Story 1 → validate independently (MVP).
2. Add User Story 2 → validate independently.
3. Add User Story 3 → validate independently.
4. Each story adds value without breaking previous stories — spec.md's Independent Test criteria
   apply at every stage.

### Parallel Team Strategy

With multiple developers: complete Setup together, then split by user story (US1/US2/US3), resolving
same-file sequencing conflicts (noted inline throughout this file) via normal rebase discipline as
stories land.
