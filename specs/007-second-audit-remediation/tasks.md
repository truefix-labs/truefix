# Tasks: Second-Pass Audit Remediation (docs/todo/004.md)

**Input**: Design documents from `specs/007-second-audit-remediation/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included as first-class tasks throughout (Constitution Principle V mandates test-first
discipline; matches this project's established convention across features 001-006). Several US1
items are genuinely protocol-behavioral (Constitution Principle II) and are flagged for a new AT
scenario per `contracts/session-protocol.md`'s per-item notes — confirm AT-runner feasibility (in
particular for the acceptor-schedule and duplicate-connection items, which may hit the
already-known "one connection per AT scenario" harness limitation `docs/todo/003.md`'s own
AT-harness follow-ups noted) as each of those specific tasks' own first step, falling back to a
plain integration test with a documented reason if genuinely infeasible, rather than silently
skipping the AT-scenario requirement.

**Organization**: Tasks are grouped by user story (spec.md's 3 stories, in priority order: P1 = US1,
P2 = US2, P3 = US3), mirroring plan.md's R1→R2→R3 staging and research.md's §R1/§R2/§R3 sections
one-to-one. Stories are independently implementable/testable. One soft (shared-API, not
hard-blocking) sequencing note within US1 itself: T013 (`SessionHandle::abort`/`is_finished`) is
consumed by both T014-T015 (`Engine::shutdown`/`Drop`) and T016-T017 (scheduled-initiator
reconnect) — do T013 first within US1 (already the natural order below).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Maps to spec.md's US1/US2/US3
- File paths are exact; a task without a `[Story]` label is Setup/Polish (applies across stories)

---

## Phase 1: Setup

**Purpose**: Establish and record the starting baseline. No new dependency needed anywhere in this
feature (plan.md's Technical Context).

- [X] T001 [P] Confirm and record the starting baseline: `cargo test --workspace --all-features`
  (expect exit 0, 118 test binaries / 578 tests — feature 006's closing state), `cargo test -p
  truefix-at --test coverage` (expect the floor to match exactly **405**) — no code change; this
  establishes the exact numbers this feature's own AT-scenario-adding tasks (if any) and Polish's
  final regression check compare against. — **complete**: confirmed live, 118/118 binaries, 578/578
  tests passing, AT floor exactly 405.

**Checkpoint**: Baseline recorded. User story work can begin — no shared foundational infrastructure
is needed beyond Setup (every additive API surface change in this feature, e.g.
`SessionHandle::abort`/`is_finished`, is scoped entirely within US1 — see the sequencing note above,
not a cross-story blocker).

---

## Phase 2: User Story 1 — Pre-production-blocking correctness, durability, and resource-safety fixes (Priority: P1) 🎯 MVP

**Goal**: Close every must-fix-before-shipping item `docs/todo/004.md` identified: sequence-number
store crash-safety/format, the `ResetSeqNumFlag` handshake, a `ResendRequest` deadlock, duplicate
connections, `Engine` resource leaks, a scheduled-initiator reconnect bug, a `BodyLength=0`
acceptance gap, zero acceptor-side schedule awareness, and admin-message dictionary validation.

**Independent Test**: Each item below has its own targeted test (unit, two-process integration, or
AT scenario per `contracts/session-protocol.md`'s notes) demonstrating the previously-broken
behavior now works correctly.

### Sequence-number store: format change, crash-safety, migration (FR-001/FR-001a/FR-002)

- [X] T002 [P] [US1] Add a failing test for the atomic write path: inject a crash (simulated —
  interrupt/truncate the write) between the temp-file write and rename for both
  `senderseqnums`/`targetseqnums`, confirm the prior value is always recoverable, in
  `crates/truefix-store/tests/seqfile_crash_safety.rs` (new) (research.md §R1.1). — **complete**: a
  true crash-mid-write can't be simulated from a black-box unit test without OS-level fault
  injection (same caveat feature 006's `reset_ordering.rs` already documented for an equivalent
  case) — instead verified the observable atomicity properties: two separate files exist with
  independently-correct values, and no `.tmp` file survives a completed write (proving the
  write-then-rename mechanism is actually wired in, not just present in the source).
- [X] T003 [P] [US1] Add a failing test for corrupted-vs-missing-file distinction: a present but
  unparseable `senderseqnums`/`targetseqnums` file surfaces a typed `StoreError` at open time,
  distinct from a legitimately-absent file (which still defaults to sequence 1), in
  `crates/truefix-store/tests/seqfile_crash_safety.rs` (same file as T002) (research.md §R1.1). —
  **complete**.
- [X] T004 [P] [US1] Add a failing test for legacy-format auto-migration: a store directory
  containing only a legacy combined `seqnums` file, opened for the first time, ends up with both new
  files correctly split and the legacy file still present unchanged, in
  `crates/truefix-store/tests/seqfile_migration.rs` (new) (research.md §R1.1, `contracts/
  store-durability.md`'s explicit test obligation). — **complete**: also added a partial-migration
  test (one new file already migrated, the other still legacy-only) beyond the task's literal ask,
  since the implementation ended up per-file idempotent rather than all-or-nothing.
- [X] T005 [US1] Split `SeqFile` (`crates/truefix-store/src/file.rs`) into two independent files
  (`senderseqnums`/`targetseqnums`), each written via write-temp-then-rename instead of
  truncate-in-place; `reset()` deletes and recreates both wholesale; a parse failure on a present
  file returns a typed `StoreError` instead of defaulting — makes T002/T003 pass (depends on T002,
  T003) (research.md §R1.1, data-model.md). — **complete**: `write_seq_value` writes to a
  `<name>.tmp` sibling then `fs::rename`s over the real path; `load_seq_value` distinguishes
  `NotFound` (→ `Ok(1)`) from a parse failure (→ `StoreError::Backend`); `reset()` removes then
  rewrites both files. `cargo test -p truefix-store --test seqfile_crash_safety`: 5/5 passing.
- [X] T006 [US1] Implement legacy-to-two-file auto-migration in `SeqFile::open`
  (`crates/truefix-store/src/file.rs`): if neither new file exists but the legacy `seqnums` file
  does, read and split it into the new layout before proceeding, leaving the legacy file in place —
  makes T004 pass (depends on T004, T005) (research.md §R1.1). — **complete**: implemented
  per-file (not all-or-nothing) so migration is itself resumable after an interrupted attempt —
  `senderseqnums` and `targetseqnums` are each independently migrated only if that specific file is
  still absent. `cargo test -p truefix-store --test seqfile_migration`: 4/4 passing. Full
  `crates/truefix-store` suite re-run to confirm no regression from the on-disk format change:
  all existing tests (`reset_ordering.rs`, `restart_resend.rs`, etc.) still pass unchanged since
  they only exercise the public `MessageStore` trait API, never the on-disk file names directly.

### `SqlStore` migration ordering (FR-003)

- [X] T007 [P] [US1] Add a failing test: open a `SqlStore` against a pre-existing sessions table
  schema created *before* `creation_time` existed (construct the table manually without that
  column), confirm the store opens cleanly rather than failing on an INSERT referencing a
  nonexistent column, in `crates/truefix-store/tests/sql_schema_migration_order.rs` (new)
  (research.md §R1.4). — **complete**: pre-creates the legacy-schema table via raw `sqlx` (matching
  production's own `sqlx::AssertSqlSafe` pattern for constant-identifier SQL), then opens a
  `SqlStore` against it. Confirmed the test genuinely reproduces `BUG-26` before the fix (failed
  with `"table t_legacy_sessions has no column named creation_time"`).
- [X] T008 [US1] Reorder `SqlStore::ensure_schema` (`crates/truefix-store/src/sql.rs`) so
  `add_creation_time_column_if_missing()` (or equivalent) runs before any statement referencing that
  column, for all three SQL backends sharing this code path — makes T007 pass (depends on T007)
  (research.md §R1.4). — **complete**: moved the migration call between `CREATE TABLE` and `INSERT`
  in all three backend branches (SQLite/Postgres/MySQL). `cargo test -p truefix-store --features
  sql --test sql_schema_migration_order`: 1/1 passing; `--test sql_backends --test restart_resend`:
  20/20 passing, no regression.

### `MssqlStore` commit-failure rollback (FR-004)

- [X] T009 [P] [US1] Add a failing test: simulate a `COMMIT TRANSACTION` failure in
  `MssqlStore::save_and_advance_sender`, confirm a `ROLLBACK` is issued and the connection is left
  usable for subsequent operations, in `crates/truefix-store/tests/mssql_commit_rollback.rs` (new,
  `--features mssql`) (research.md §R1.5). — **complete with a disclosed scope narrowing**: forcing
  a failure specifically at the `COMMIT` step (not an earlier statement) isn't practically
  reproducible black-box over the tiberius wire protocol without exposing `MssqlStore`'s internal
  connection (unwanted API growth) — documented inline in the test file. Verified instead that a
  sequence of successful transactions never leaves the connection stuck (the surrounding contract
  the fix shares with the already-tested statement-failure branch); the commit-specific code change
  itself was verified correct by direct review, mirroring the already-tested statement-failure
  branch's exact rollback-then-propagate shape. `DATABASE_URL_MSSQL` unset in this sandbox — test
  compiles and correctly self-skips (`cargo test -p truefix-store --features mssql --test
  mssql_commit_rollback`: 1/1 passing via the skip path); will run for real in CI's `mssql` service
  container.
- [X] T010 [US1] Add a `ROLLBACK` to the commit-failure branch of `MssqlStore::
  save_and_advance_sender` (`crates/truefix-store/src/mssql.rs`), mirroring the existing
  statement-failure branch's error handling — makes T009 pass (depends on T009) (research.md §R1.5).
  — **complete**: captures the COMMIT result's error as an owned `String` (not the borrowing
  `QueryStream` the `Result` would otherwise hold) so the following `ROLLBACK` call can borrow
  `client` again — a compile error surfaced this borrow-checker subtlety during implementation,
  fixed by converting to an owned value immediately. `cargo test -p truefix-store --features mssql
  --test mssql_backend --test restart_resend`: 15/15 passing, no regression.

### `ResetSeqNumFlag` handshake (FR-005/FR-006)

- [X] T011 [P] [US1] Add a failing test: an acceptor's Logon response echoes `ResetSeqNumFlag=Y`
  only when the inbound Logon that triggered it carried the flag (not driven by
  `config.reset_on_logon` for the response), in `crates/truefix-session/tests/
  reset_seq_num_flag_handshake.rs` (new) (research.md §R1.2). — **complete**.
- [X] T012 [P] [US1] Add a failing test: an initiator that sent `ResetSeqNumFlag=Y` treats a Logon
  response that neither echoes the flag nor is inferable as acknowledged (response `MsgSeqNum != 1`)
  as a handshake failure (Logout + disconnect), in `crates/truefix-session/tests/
  reset_seq_num_flag_handshake.rs` (same file as T011) (research.md §R1.2). — **complete**: also
  added a companion test confirming an initiator that never requested a reset (`reset_on_logon =
  false`) has nothing to verify and never disconnects regardless of the response's flag.
- [X] T013 [US1] Thread the inbound Logon's own `RESET_SEQ_NUM_FLAG` value into `admin::logon()`'s
  response-echo construction (`crates/truefix-session/src/admin.rs`, `crates/truefix-session/src/
  state.rs`'s `on_logon`), replacing the static `config.reset_on_logon` read for the response case
  — makes T011 pass (depends on T011) (research.md §R1.2). — **complete**: added
  `admin::logon_with_reset_flag` (explicit boolean parameter) alongside the existing `admin::logon`
  (now a thin wrapper calling it with `config.reset_on_logon`, preserving the initiator's-own-first-
  Logon behavior unchanged); `on_logon` captures `inbound_reset_flag` once and reuses it both for
  the existing reset-sequences decision and the new response-echo call.
- [X] T014 [US1] Extend `on_logon`'s `Role::Initiator` arm (`crates/truefix-session/src/state.rs`)
  to verify the Logon response's own `RESET_SEQ_NUM_FLAG`/`MsgSeqNum` when this session sent
  `ResetSeqNumFlag=Y`, routing a failed verification through the existing `reject_logon`-style
  Logout+disconnect path — makes T012 pass (depends on T012, T013) (research.md §R1.2). —
  **complete**: added `Session.reset_on_logon_pending: bool`, set in `on_connected` when the
  initiator's own Logon carries `ResetSeqNumFlag=Y` (mirroring `config.reset_on_logon`, reset to
  `false` at the start of every new connection alongside the other per-connection timer/tracking
  state `on_connected` already resets), consumed and cleared in the `Role::Initiator` arm. `cargo
  test -p truefix-session --test reset_seq_num_flag_handshake`: 6/6 passing; full `cargo test -p
  truefix-session`: all pre-existing tests still green (14 in `state_machine.rs`, 8 in
  `validation_hook.rs`, etc. — no regression).
- [X] T015 [US1] Author a new AT scenario exercising both directions of the corrected
  `ResetSeqNumFlag` handshake in `crates/truefix-at/src/scenarios.rs`, wired into `server_suite()`
  (`contracts/session-protocol.md`: AT scenario required) (depends on T013, T014). — **complete**:
  the existing `1d_LogonResponseResetFlag` scenario already covered the "echo when inbound has it"
  direction (didn't actually distinguish old buggy vs new correct behavior, since both echo `Y` in
  that case) — added the missing direction, `BUG28_LogonResponseDoesNotEchoAbsentResetFlag`
  (inbound Logon without the flag, response must omit it too — the AT-suite acceptor's own
  `reset_on_logon` config default is `true`, so this genuinely exercises the fix). Required a small
  harness addition: `ExpectMsg` gained `without_field(tag)`/`fields_absent`, since no
  field-must-be-absent assertion existed before. `cargo test -p truefix-at --test conformance`:
  4/4 passing (all 9 versions × both directions).

### `ResendRequest`-vs-`ResendRequest` deadlock (FR-007)

- [X] T016 [P] [US1] Add a failing test: a `ResendRequest` for a range beyond the highest sequence
  sent is answered immediately (not queued) even while this session has its own outstanding
  `ResendRequest` toward the counterparty, in `crates/truefix-session/tests/
  resend_request_too_high.rs` (new) (research.md §R1.3). — **complete**: verified as a genuine
  regression test via bisection (temporarily removed the fix, confirmed both tests fail with
  concrete evidence, re-applied and confirmed green).
- [X] T017 [US1] Route the too-high-sequence `ResendRequest` case (`crates/truefix-session/src/
  state.rs`) through the existing `build_resend()` immediately, bypassing the generic out-of-order
  queue — makes T016 pass (depends on T016) (research.md §R1.3). — **complete, with a disclosed
  simplification**: the too-high `ResendRequest` is answered immediately via `on_resend_request`
  (not routed around the queue entirely) AND still inserted into the out-of-order queue as before,
  so it participates in normal gap-fill bookkeeping once our own gap resolves. This means it may be
  answered twice total (once immediately, once more when `drain_queue` naturally reaches it) rather
  than exactly once — harmless (a redundant PossDup-style resend, not a correctness violation) but
  a deliberate simplification versus QuickFIX/J's own behavior (which never queues it at all);
  documented inline in the code and the test. `cargo test -p truefix-session`: all pre-existing
  tests still green (no regression).
- [X] T018 [US1] Author a new AT scenario reproducing the two-sided deadlock scenario (both sessions
  holding outstanding `ResendRequest`s toward each other) and confirming it no longer hangs, in
  `crates/truefix-at/src/scenarios.rs`, wired into `server_suite()` (depends on T017). —
  **complete**: `BUG29_ResendRequestTooHighAnsweredImmediately` sends a too-high `ResendRequest`
  and confirms both (a) it draws an immediate GapFill response and (b) the session still separately
  requests its own gap — proving the exchange completes rather than hanging. `cargo test -p
  truefix-at --test conformance`: 4/4 passing (all 9 versions).

### `Engine::shutdown()`/`Drop` stopping plain initiators (FR-013)

- [X] T019 [P] [US1] Add `SessionHandle::abort(&self)` (sync) and `SessionHandle::is_finished(&self)
  -> bool` (sync, non-consuming) to `crates/truefix-transport/src/lib.rs`, both delegating to the
  existing private `task: JoinHandle<()>` field's own synchronous methods (data-model.md; additive,
  no existing method signature changes). — **complete**.
- [X] T020 [P] [US1] Add a failing test: an `Engine` started with a mix of acceptor + plain
  initiator + failover initiator, after `Engine::shutdown()`, has zero running session tasks
  (spec SC-004), in `crates/truefix/tests/engine_shutdown_stops_all_initiators.rs` (new) (research.md
  §R1.6, depends on T019). — **complete, with a disclosed scope narrowing**: no failover initiator
  in the actual test (acceptors/failover-initiators have no public liveness-introspection API to
  assert against, unlike the new `SessionHandle::is_finished()`; that path was already correctly
  stopped by the pre-existing `abort_acceptors_and_stop_failover`, unchanged by this feature, so no
  new coverage was needed there) — focuses on the actual regression this task targets: a plain
  initiator. Two tests: `shutdown()` explicitly, and dropping the `Engine` without calling it (the
  second test can't introspect a dropped value's internals, so it instead confirms a second
  `Engine` can immediately rebind the same port afterward, which fails with "address already in
  use" if the first engine's acceptor listener were still alive).
- [X] T021 [US1] Extend `Engine::shutdown()` (`crates/truefix/src/lib.rs`) to call
  `SessionHandle::abort()` on every plain initiator, in addition to its existing
  acceptor/failover-initiator handling — makes T020 pass (depends on T019, T020) (research.md §R1.6).
  — **complete**: extracted a shared `abort_everything` helper (acceptors + failover-initiators via
  the existing `abort_acceptors_and_stop_failover`, plus a new loop calling `.abort()` on every
  plain initiator) used by both `shutdown()` and the new `Drop` impl (T022).
- [X] T022 [US1] Add `impl Drop for Engine` (`crates/truefix/src/lib.rs`) performing the same
  abort-everything sweep as `shutdown()`, as a safety net for callers that never call it explicitly
  (research.md §R1.6, data-model.md) (depends on T021). — **complete**: `cargo test -p truefix
  --test engine_shutdown_stops_all_initiators`: 2/2 passing; full `cargo test -p truefix
  --all-features`: no regression (11 test binaries, all green, including `failover_engine.rs` and
  `continue_on_error.rs`'s existing `Engine::shutdown()`/partial-start-cleanup coverage).

### Scheduled-initiator reconnect after a drop + retry backoff (FR-012, FR-026)

- [X] T023 [P] [US1] Add a failing test: a scheduled initiator's active connection is force-dropped;
  confirm it reconnects on a subsequent loop iteration rather than hanging forever, in
  `crates/truefix-transport/tests/scheduled_reconnect_after_drop.rs` (new) (research.md §R1.7,
  depends on T019). — **complete**: accepts the initiator's raw TCP connection on a plain
  `tokio::net::TcpListener` (no FIX handshake), drops it to simulate a genuine mid-connection
  severance, then brings up a real `AcceptorBuilder` on the same address and confirms reconnection.
  Verified as a genuine regression test: temporarily removed the `is_finished()` check, confirmed
  the test fails (hangs past its 10s timeout), restored and confirmed green.
- [X] T024 [US1] In `run_scheduled_initiator` (`crates/truefix-transport/src/lib.rs`), check
  `current`'s `SessionHandle::is_finished()` each loop iteration and clear it to `None` when the
  held task has already finished, letting the existing `was_in_session && current.is_none()`
  reconnect condition fire — makes T023 pass (depends on T019, T023) (research.md §R1.7). —
  **complete**.
- [X] T025 [US1] Add a failing test: a scheduled initiator's failed connection attempts back off
  across a small number of retries rather than polling at a fixed 200ms interval indefinitely, in
  `crates/truefix-transport/tests/scheduled_reconnect_after_drop.rs` (same file as T023) (research.md
  §R1.7). — **complete**: an indirect timing-based test (no public attempt-count introspection
  exists) — lets one connect attempt fail against a refused port (fast, <300ms), then brings up a
  real acceptor and asserts the time-to-connect is a substantial fraction of the configured 2s
  backoff window (>=800ms) rather than near-instant (which the pre-fix 200ms fixed cadence would
  produce).
- [X] T026 [US1] Apply this project's existing `ReconnectInterval`-step backoff pattern to
  `run_scheduled_initiator`'s failed-connect-retry cadence specifically (not its
  schedule-boundary-check cadence, which stays fast) in `crates/truefix-transport/src/lib.rs` —
  makes T025 pass (depends on T024, T025). — **complete**: reused the existing `reconnect_delay()`
  helper (already used elsewhere for `ReconnectInterval`/`ReconnectIntervalN` steps), tracked via a
  `next_retry_at: Option<Instant>` that only gates the connect *attempt* itself — the loop's own
  200ms sleep (schedule-boundary responsiveness) is untouched. `cargo test -p truefix-transport
  --test scheduled_reconnect_after_drop`: 2/2 passing; full `cargo test -p truefix-transport
  --all-features`: 29 test binaries, all green, no regression.

### Duplicate/competing connection refusal (FR-008)

- [X] T027 [P] [US1] Add a failing test: a second inbound connection presenting an already-connected
  `SessionId` is refused before its Logon is processed, in `crates/truefix-transport/tests/
  duplicate_connection_refused.rs` (new) (research.md §R1.8).
  **Completed**: two tests written — `..._multi_session` (targets `AcceptorBuilder`/`route_and_run`,
  the path research.md §R1.8 names) and `..._single_session` (targets the structurally-separate
  single-session `Acceptor::serve` path — see T028's disclosure note for why this second test was
  added beyond the literal task text). Both assert the acceptor's `on_logon` count stays at 1 and
  the second (competing) initiator's `on_logon` never fires, after a second `connect_initiator` call
  presents the same `SessionId` while the first connection is still active.
- [X] T028 [US1] Add active-connection tracking (`Arc<Mutex<HashSet<SessionId>>>` or equivalent) to
  the acceptor's shared registry state in `crates/truefix-transport/src/lib.rs`, checked-and-inserted
  in `route_and_run` before spawning `run_connection`, removed when that connection's task ends —
  makes T027 pass (depends on T027) (research.md §R1.8, data-model.md).
  **Completed**: `Registry` gained `active: std::sync::Mutex<HashSet<SessionId>>`. `route_and_run`
  checks-and-inserts `sid` atomically under one lock acquisition immediately before spawning
  `run_connection`; a second connection for an already-`active` `SessionId` is refused (logged, then
  the function returns without ever spawning). An RAII `ActiveGuard` (custom `Drop`) removes `sid`
  from `active` when the connection's task ends via *any* exit path (normal completion, error, or
  panic-unwind), so a legitimate future reconnect for the same identity is never permanently locked
  out.
  **Disclosure — single-session `Acceptor::serve` extension (beyond research.md §R1.8's literal
  scope)**: `docs/todo/004.md`'s BUG-32 explicitly names *both* `route_and_run` and `Acceptor::serve`
  as affected (`crates/truefix-transport/src/lib.rs`, `route_and_run` / `Acceptor::serve`), but
  research.md's R1.8 decision only worked out `Registry.active` for the multi-session
  `AcceptorBuilder` path — the structurally-separate, single-session `Acceptor` struct (one static
  `SessionConfig` per acceptor, used by `Acceptor::bind`/`bind_with`) has its own `serve()` accept
  loop with no equivalent protection and was not mentioned in the decision. Per Constitution
  Principle VI (Acceptor/Initiator parity) and Principle VII (inventory-based completeness), and
  since the original bug report already named this path, the same class of fix was applied there too
  rather than deferred: a `SingleSessionActive = Arc<std::sync::Mutex<bool>>` (a single flag suffices
  since a single-session acceptor has exactly one identity) is checked-and-set before spawning each
  connection's task, and a `SingleSessionActiveGuard` (the same cleanup-on-any-exit-path `Drop`
  pattern as `ActiveGuard`) clears it when that task ends. Both `serve()` branches (TLS and non-TLS)
  now await the spawned session's `JoinHandle` from within a wrapping task carrying the guard,
  instead of fire-and-forgetting the `SessionHandle` as before.
  **Verification**: `cargo build -p truefix-transport` clean. `cargo test -p truefix-transport --test
  duplicate_connection_refused` — 2/2 pass. Bisection: temporarily short-circuited both the
  `Registry.active` check (`if !active.insert(...)`) and the single-session `if *guard` check to
  `if false && ...`, reran the test — both tests failed with concrete evidence (`left: 2, right: 1`
  logon counts, i.e. the competing connection *did* log on), confirming both are genuine regression
  guards, not false positives; reverted, reran, both pass again. Full regression: `cargo test -p
  truefix-transport --all-features` — all 29+ test binaries green, no regressions.

### `Event::Disconnected` / `ResetOnDisconnect` on unexpected TCP drop (FR-009)

- [X] T029 [P] [US1] Add a failing test: an active session's TCP connection is force-dropped (not a
  graceful Logout); confirm `ResetOnDisconnect`'s configured behavior (store reset) is honored
  exactly as it would be for a Logout-driven disconnect, in `crates/truefix-transport/tests/
  disconnect_event.rs` (new) (research.md §R1.9).
  **Completed**: `reset_on_disconnect_is_honored_when_the_tcp_connection_is_force_dropped`. Note on
  how the "force-drop" is produced: an initial attempt used `SessionHandle::abort()` on a
  `connect_initiator_with`-managed initiator, but that only aborts the *outer* connection task —
  `run_connection` internally `tokio::io::split`s the stream, and the spawned `read_loop` child task
  (not itself aborted, since `tokio::spawn`ed children aren't cascade-aborted by aborting their
  parent's `JoinHandle`) keeps its `ReadHalf` reference alive, which keeps the underlying socket from
  actually closing — the acceptor never observed a drop within the timeout. Fixed by driving a raw,
  unmanaged `TcpStream` for the initiator side instead: `truefix_session::Session` (already public)
  builds the Logon bytes directly (`Session::new(cfg).handle(Event::Connected)`'s `Action::Send`,
  `.encode()`d), written once over a raw connect; since the test owns the *only*, unsplit handle to
  that stream, `drop(raw)` is a genuine, complete close.
- [X] T030 [US1] Add `Event::Disconnected` to `crates/truefix-session/src/state.rs`'s `Event` enum
  and a handler routing to the existing `enter_disconnected()`, returning its resulting `Action`s
  through the same dispatch contract every other event uses (data-model.md) (depends on T029).
  **Completed**: `Event::Disconnected` added; `handle()`'s match arm is
  `Event::Disconnected => self.enter_disconnected().into_iter().collect()` — reuses the existing
  function verbatim, no new reset logic duplicated.
- [X] T031 [US1] Dispatch `Event::Disconnected` from `run_connection`'s shutdown tail
  (`crates/truefix-transport/src/lib.rs`), before `write_half.shutdown()`/session-drop, propagating
  any resulting `ResetStore` action to the store exactly as other actions already are — makes T029
  pass (depends on T029, T030) (research.md §R1.9).
  **Completed**: a `dispatch(&mut session, Event::Disconnected, ...)` call added immediately before
  `write_half.shutdown()`; `dispatch`/`perform_actions` is the same pipeline every other event
  (`Connected`/`Tick`/etc.) already goes through, so `Action::ResetStore` reaches `store.reset()`
  exactly as it would from any other path. Runs unconditionally on every shutdown (including a prior
  graceful Logout that already reset via its own `enter_disconnected()` call) — idempotent (resets to
  1/1 again, harmless) and matches research.md §R1.9's decision text.
  **Verification**: `cargo test -p truefix-transport --test disconnect_event` — 1/1 passes.
  Bisection: temporarily wrapped the new `dispatch(... Event::Disconnected ...)` call in `if false {
  }`, reran — failed with concrete evidence (`left: 2, right: 1`, i.e. the store's persisted sender
  seq stayed at 2 instead of resetting to 1), confirming genuine regression coverage; reverted, reran,
  passes. Full regression: `cargo test -p truefix-transport --all-features` (31 test binaries, all
  green) and `cargo test -p truefix-session --all-features` (15 binaries, all green) — no
  regressions.

### Callback-ordering restructure (FR-010)

- [X] T032 [P] [US1] Add a failing test: a test `Application` records the relative order of
  `from_admin`/`from_app` invocation versus session-layer sequence/identity/latency/PossDup/
  dictionary rejection; confirm the callback never fires for a message the session layer is about to
  reject, in `crates/truefix-transport/tests/callback_ordering.rs` (new) (research.md §R1.10).
  **Completed**: `from_admin_never_fires_for_a_message_the_session_layer_rejects`. A raw,
  hand-crafted (`truefix_core::Message`/`Field`) Heartbeat carrying `MsgSeqNum=1` (already consumed
  by the preceding Logon; no `PossDupFlag`) is sent after a valid Logon — a session-layer-fatal
  too-low-sequence condition (`on_received`'s `Ordering::Less`/no-`poss_dup` branch: Logout +
  Disconnect). Asserts the acceptor's `from_admin` invocation count stays at 1 (only the Logon),
  never advancing to 2 for the doomed Heartbeat.
- [X] T033 [US1] Restructure `handle_inbound` (`crates/truefix-transport/src/lib.rs`) so
  `dispatch(session, Event::Received(msg), ...)` (session-layer validation) runs before
  `app.from_admin`/`app.from_app` is invoked, preserving the existing reject-response behavior
  (`session.reject_logon`/`session.business_reject`) for messages the callback itself later rejects
  — makes T032 pass (depends on T032) (research.md §R1.10, this is the one US1 item requiring
  genuine control-flow restructuring, not just an additive check).
  **Correction to the planned approach**: a literal "run `dispatch` fully first, then the callback"
  (as R1.10/this task's text first suggested) was implemented, built, and passed T032 — but broke
  `crates/truefix-transport/tests/auth.rs`'s `wrong_password_is_rejected`. Root cause: `dispatch`
  performs the *entire* `session.handle(Event::Received(...))` call in one shot, including the
  Logon's state transition to `LoggedOn` and firing `Application::on_logon` — all of which now
  happened *before* `from_admin` ever got a chance to check the password, so an unauthenticated
  Logon logged on regardless of what `from_admin` returned. Unlike QuickFIX/J (whose `verify()`
  calls `fromCallback` in the middle of processing, strictly between sequence/identity checks and
  the message-type-specific handler that performs the state transition + response), this codebase's
  `Session::handle`/`dispatch` has no such internal seam to hook into — it is monolithic by design
  (sans-IO, single call in/single `Vec<Action>` out).
  **Actual fix**: rather than reordering wholesale, added a new read-only precheck,
  `Session::would_reject_before_processing(&self, msg: &Message) -> bool`
  (`crates/truefix-session/src/state.rs`), covering exactly the state-mutation-free hard-teardown
  conditions — stale `SendingTime` (`latency_ok`), mismatched `BeginString`/CompID
  (`identity_problem`), and (for a non-Logon/non-SequenceReset message) a too-low `MsgSeqNum` with no
  `PossDupFlag`. `handle_inbound` calls this first: if `true`, it skips straight to `dispatch`
  (callback never invoked — fixes BUG-34/T032's scenario). Otherwise, the *original* order is
  preserved unchanged (`from_admin`/`from_app` still run before `dispatch`, exactly as before this
  feature), which is what keeps `auth.rs`'s Logon-veto-before-`on_logon` semantics intact. Explicitly
  not exhaustive — dictionary (`validate_app`) and in-order gap-fill checks are tightly coupled to
  `next_in_seq` mutation inside `on_received` itself and can't be replicated as a pre-check without
  duplicating that logic; deferred (T034-T036, admin dictionary validation, covers the dictionary
  case for admin messages specifically).
  **Verification**: `cargo test -p truefix-transport --test callback_ordering --test auth` — 3/3
  pass together (confirming the fix satisfies both BUG-34's requirement and preserves the pre-existing
  authentication-veto behavior). Bisection: temporarily short-circuited the precheck call to
  `if false && session.would_reject_before_processing(...)`, reran — `callback_ordering` failed with
  concrete evidence (`left: 2, right: 1`) while `auth.rs` still passed (confirming they cover
  independent concerns); reverted, reran, both pass. Full regression: `cargo test -p truefix-transport
  --all-features` and `cargo test -p truefix-session --all-features` — all green, no regressions.
  `cargo fmt` applied.

### Admin-message dictionary validation (FR-011)

- [X] T034 [P] [US1] Add a failing test: a malformed admin message (e.g. a Logon with a
  dictionary-invalid field) is rejected via the plain session-level `Reject` path when a dictionary
  is configured, in `crates/truefix-session/tests/admin_dictionary_validation.rs` (new) (research.md
  §R1.13).
  **Completed**: 3 tests — `a_dictionary_invalid_logon_is_rejected_via_the_plain_reject_path` (a
  Logon missing dictionary-required `EncryptMethod(98)`), `a_dictionary_valid_logon_still_logs_on`
  (control), `a_dictionary_invalid_heartbeat_is_rejected_via_the_plain_reject_path` (a Heartbeat
  carrying an unknown tag below the UDF range).
- [X] T035 [US1] Remove `validate_app`'s (`crates/truefix-session/src/state.rs`) blanket admin-type
  early-return; run dictionary validation for admin messages too, routing a failure through the
  plain session-level `Reject` path (not `BusinessMessageReject`) — makes T034 pass (depends on
  T034) (research.md §R1.13).
  **Completed**: removed the `if is_admin_type(msg.msg_type()) { return None; }` early return.
  Two additional corrections beyond the literal task text, both required for correctness:
  1. **FIXT dictionary routing**: for a FIXT split (`fixt_validator`), admin messages now validate
     against `dicts.transport()` (the transport/session-layer dictionary), not the per-message
     *application* dictionary `application_for(appl_ver_id)` selects — admin messages don't carry
     application semantics, so resolving them against an application dict would spuriously reject
     every admin message as an undefined MsgType.
  2. **Logon itself wasn't covered**: `validate_app` is only called from `on_received`'s generic
     `Ordering::Equal` path and `drain_queue` — Logon(`"A"`) and SequenceReset(`"4"`) are handled by
     their own separate functions *before* that generic path is ever reached, so removing the
     blanket skip alone does nothing for a malformed Logon (T034's own primary example). Added an
     explicit `self.validate_app(&msg)` call inside `on_logon` itself, right after the existing
     duplicate-logon/too-low-seq early checks and before any reset/state-transition side effects,
     honoring `disconnect_on_error` (Logout+Disconnect) the same way the generic path does.
  Fallout fixed along the way: `crates/truefix-session/tests/validation_hook.rs`'s shared `logon()`
  fixture was missing `EncryptMethod(98)` (a dictionary-required Logon field) — silently tolerated
  pre-fix since Logon was never validated; now a genuinely valid fixture is required, so it was
  added. One FIXT-specific test (`fixt_dictionaries_selects_the_app_dict_via_per_message_appl_ver_id`)
  needed its Logon to carry `DefaultApplVerID(1137)`, itself a FIXT.1.1-transport-dictionary-required
  Logon field (swapped to reuse the existing `logon_with_default_appl_ver_id` helper). Also found
  (via the AT suite, see T036) and fixed: `crates/truefix-dict/src/validate.rs`'s
  `validate_fields_have_values` check now explicitly exempts the six Appendix-B "ReverseRoute"
  routing tags (115/116/128/129/144/145), which FIX legitimately allows present-but-empty
  (`Message::reverse_route`'s "presence — not content — governs" rule) — previously irrelevant since
  the `ReverseRouteWithEmptyRoutingTags` AT scenario's message (a TestRequest, admin-typed) was never
  dictionary-validated at all; enabling admin validation exposed that the empty-value check had no
  such carve-out.
  **Verification**: `cargo test -p truefix-session --test admin_dictionary_validation` — 3/3 pass.
  Bisection: temporarily restored the `is_admin_type` blanket early-return (`validate_app`) and
  short-circuited the new `on_logon` precheck (`if false { ... }`) — both new
  `admin_dictionary_validation.rs` tests failed with concrete evidence (Logon: `left: LoggedOn,
  right: LoggedOn` i.e. it logged on when it must not have; Heartbeat: `got []` i.e. no Reject sent);
  reverted, reran, both pass. Full regression: `cargo test -p truefix-session --all-features` (15
  binaries), `cargo test -p truefix-dict --all-features` (27 binaries), `cargo test -p
  truefix-transport --all-features`, `cargo test --workspace --all-features` (139 test-result blocks,
  0 `FAILED`) — all green.
- [X] T036 [US1] Author a new AT scenario exercising a dictionary-invalid admin message, in
  `crates/truefix-at/src/scenarios.rs`, wired into `server_suite()` (`contracts/
  session-protocol.md`: AT scenario required) (depends on T035).
  **Completed**: `admin_message_dictionary_invalid_is_rejected` ("BUG89_AdminMessageDictionaryInvalidIsRejected")
  — a post-Logon TestRequest carrying an unknown tag (4321, below the UDF range) draws a plain Reject
  (35=3). FIX.4.4-only (the AT harness only loads a dictionary for FIX.4.2/FIX.4.4). This is also the
  scenario that surfaced the `ReverseRouteWithEmptyRoutingTags` regression fixed under T035's notes
  above.
  **Verification**: `cargo test -p truefix-at --test conformance` — 424/424 scenario runs pass (423
  pre-existing + this one). Bisection: temporarily restored the `is_admin_type` blanket early-return
  — `server_acceptance_suite_passes` failed (`AT FAIL BUG89_AdminMessageDictionaryInvalidIsRejected
  [FIX.4.4]: Err("step 3: expected MsgType \"3\", got Some(\"0\")")`, 423/424); reverted, reran,
  424/424. `cargo fmt` applied workspace-wide (also cleaned up pre-existing formatting drift from
  earlier tasks T007/T020/etc., unrelated to this task but caught by the same `cargo fmt --check`
  pass).

### `BodyLength=0` rejection (FR-014)

- [X] T037 [P] [US1] Add a failing test: `frame_length` rejects a declared `BodyLength=0` as
  malformed, in `crates/truefix-core/tests/framing_bounds.rs` (extend, feature 006's existing
  frame-size-bound test file) (research.md §R1.11).
  **Correction**: no such file existed yet in `truefix-core/tests/` (the actual existing
  oversized-BodyLength coverage feature 006 added lives in `crates/truefix-transport/tests/
  framing_bounds.rs`, a connection-level test, not a `frame_length`-unit-level one) — created
  `crates/truefix-core/tests/framing_bounds.rs` new, with 2 tests:
  `a_declared_body_length_of_zero_is_rejected` and a `a_declared_positive_body_length_still_frames_normally`
  control.
- [X] T038 [US1] Reject `body_len == 0` in `frame_length` (`crates/truefix-core/src/framing.rs`),
  reusing or extending the existing `DecodeError` variant set — makes T037 pass (depends on T037)
  (research.md §R1.11).
  **Completed**: added a new `DecodeError::ZeroBodyLength` variant (rather than reusing
  `InvalidBodyLength`) — deliberately distinct so the transport layer (T039) can hard-close on it
  like `BodyLengthTooLarge`, rather than the generic malformed-frame skip-and-resync recovery every
  other `InvalidBodyLength` case (missing/non-numeric BodyLength) intentionally gets (B14/FR-028,
  feature 006). `frame_length` returns it right alongside the existing `MAX_BODY_LEN` check.
- [X] T039 [US1] Add a failing test + confirm live: a connection sending a `BodyLength=0` message is
  closed rather than processed, in `crates/truefix-transport/tests/framing_bounds.rs` (extend,
  feature 006's existing file) (depends on T038).
  **Completed**: `a_connection_declaring_a_zero_body_length_is_closed`, modeled directly on the
  existing `a_connection_declaring_an_oversized_body_length_is_closed`. Required a
  `classify_buffered` change beyond T038 alone: `Err(DecodeError::ZeroBodyLength) => return Err(())`
  added alongside the existing `BodyLengthTooLarge` hard-close arm — without it, `ZeroBodyLength`
  fell into the generic `Err(_)` malformed-prefix-recovery branch, which skips forward looking for
  the next plausible frame start, finds none, drains the whole buffer, and returns `Ok(())` — the
  connection is never closed, it just silently waits forever for more bytes (caught by the test's
  5s timeout on first attempt).
  **Verification**: `cargo test -p truefix-core --test framing_bounds` (2/2) and `cargo test -p
  truefix-transport --test framing_bounds` (3/3) pass. Bisection: temporarily disabled the core
  check (`if false && body_len == 0`) — `truefix-core`'s new test failed with concrete evidence
  (`Some(21)`, i.e. it framed the empty-body message instead of rejecting it); reverted. Separately
  temporarily disabled the transport-layer close arm (`Err(DecodeError::ZeroBodyLength) if false =>
  ...`) — the transport-layer test failed (5s timeout, connection never closed); reverted. Full
  regression: `cargo test -p truefix-core --all-features`, `cargo test -p truefix-transport
  --all-features`, `cargo test -p truefix-at --all-features` (424 AT scenario runs + coverage tests)
  — all green, no regressions. `cargo fmt` applied.

### Acceptor schedule enforcement + `forceResendWhenCorruptedStore` gap-fill semantics (FR-015/FR-016)

- [X] T040 [P] [US1] Add a failing test: an acceptor with a configured schedule rejects an inbound
  Logon arriving outside its active window, in `crates/truefix-session/tests/
  schedule_enforcement.rs` (new) (research.md §R1.12).
  **Completed**: `logon_outside_the_configured_schedule_is_rejected` +
  `logon_inside_the_configured_schedule_is_accepted` (control). Uses a weekday-exclusion schedule
  (restricted to a day that isn't today) rather than a time-of-day window, so the "out of session"
  condition is deterministic regardless of what time of day the suite actually runs — no race
  against wall-clock time.
- [X] T041 [P] [US1] Add a failing test: an already-`LoggedOn` acceptor session is disconnected once
  the current time crosses outside its configured schedule window, in
  `crates/truefix-session/tests/schedule_enforcement.rs` (same file as T040) (research.md §R1.12).
  **Completed**: `logged_on_session_is_disconnected_once_the_schedule_window_elapses` — a daily
  window ending ~1 real second in the future; logs on inside it, sleeps 1.5s (a real, short
  wall-clock wait — same established pattern as other timing tests in this codebase, e.g.
  `idle_heartbeat_emitted`), then confirms `Event::Tick` disconnects.
- [X] T042 [US1] Extend `on_logon` (`crates/truefix-session/src/state.rs`) to reject a Logon when
  `self.config.schedule` is set and `!schedule.is_in_session(now_utc)`, via the existing
  Logout+disconnect rejection path — makes T040 pass (depends on T040) (research.md §R1.12).
  **Completed**: added right after the existing duplicate-logon/too-low-seq early checks, before any
  reset/state-transition side effects (dictionary validation, `ResetSeqNumFlag` handling, etc.).
- [X] T043 [US1] Extend `on_tick` (`crates/truefix-session/src/state.rs`) with a schedule-boundary
  check for an already-`LoggedOn` session, disconnecting it when the schedule window is crossed —
  makes T041 pass (depends on T041) (research.md §R1.12).
  **Completed**: a new guarded `SessionState::LoggedOn if self.config.schedule...is_some_and(...)`
  match arm added ahead of the existing heartbeat-timeout arm, reusing `enter_disconnected()` +
  `Action::Disconnect` (the same pattern every other `on_tick` disconnect path already uses).
- [X] T044 [P] [US1] Add a failing test: `build_resend()` resends admin messages instead of
  gap-filling them when `force_resend_when_corrupted_store` is set, in
  `crates/truefix-session/tests/schedule_enforcement.rs` (same file as T040/T041, grouped by theme
  per research.md §R1.12) (research.md §R1.12).
  **Completed**: `admin_messages_are_resent_not_gap_filled_when_force_resend_when_corrupted_store_is_set`
  + `..._is_unset` (control). Discovered along the way: the acceptor's `on_logon` adopts the
  *inbound* Logon's own `HeartBtInt(108)`, overriding whatever the config started with — the test's
  shared `logon()` fixture hardcodes `108=30`, which silently made `heartbeat_interval=1` in the test
  config ineffective (ticks never crossed the real 30-tick threshold); added a `logon_with_hb(seq,
  hb_int)` variant carrying `108="1"` to make the fast-tick test actually deterministic. Also
  discovered mid-test: with the default `TestRequestDelayMultiplier`, two ticks at
  `heartbeat_interval=1` produce a Heartbeat *then* a TestRequest (not two Heartbeats as first
  assumed) — the assertion was corrected to accept either admin type (`"0" | "1"`) rather than
  requiring Heartbeat specifically, since the actual property under test (admin messages are resent,
  not gap-filled) doesn't depend on which admin type.
- [X] T045 [US1] Extend `build_resend()` (`crates/truefix-session/src/state.rs`) to check
  `self.config.force_resend_when_corrupted_store` and resend admin messages accordingly, matching
  QuickFIX/J's `resendMessages()` two-effect semantics — makes T044 pass (depends on T044) (research.md
  §R1.12).
  **Completed**: `build_resend()`'s `is_app` classification became `!is_admin_type(...) ||
  self.config.force_resend_when_corrupted_store` — when the flag is set, every stored message in the
  range (admin included) is resent via `prepare_resend`/`send_resend` (with PossDup) instead of being
  folded into a `GapFill`.
  **Verification (T040-T045)**: `cargo test -p truefix-session --test schedule_enforcement` — 5/5
  pass. Bisection: each of the three implementation changes (T042's `on_logon` check, T043's
  `on_tick` arm, T045's `is_app` change) was individually short-circuited in turn (`if false { ... }`
  wrapping, or `|| false` in place of the config check) and its corresponding test(s) reran — each
  failed with concrete evidence (Logon-outside-schedule: `left: LoggedOn, right: Disconnected`;
  schedule-boundary-tick: same; force-resend: `left: 0, right: 2` resent count) confirming all three
  are genuine regression guards; each reverted and reran to confirm passing again. Full regression:
  `cargo test -p truefix-session --all-features` (16 test binaries) and `cargo test -p
  truefix-transport --test role_parity` (confirms no conflict with the pre-existing, differently-named
  `force_resend_when_corrupted_store_forces_a_reset_on_the_initiator_side` store-open-time semantic,
  which this task leaves untouched — T044/T045 is additive, a *second* place this same config flag
  now takes effect) — all green.
- [X] T046 [US1] Author a new AT scenario for the acceptor-schedule-enforcement case — confirm
  AT-runner feasibility first (a schedule-configured acceptor scenario may need harness support this
  project's AT runner doesn't have yet, per `contracts/session-protocol.md`'s own caveat); if
  genuinely infeasible, fall back to relying on T040/T041's transport-integration-test coverage and
  document the reason inline in `crates/truefix-at/src/scenarios.rs` or this task's own completion
  note (depends on T042, T043).
  **Assessed as genuinely infeasible, falling back per the task's own instruction**: `crates/
  truefix-at/src/runner.rs`'s `start_acceptor`/`SessionTweaks` has no `schedule` field or plumbing at
  all, and — more fundamentally — the AT harness's scripted `Step` sequence has no mechanism to
  control or wait out "now" deterministically the way `schedule_enforcement.rs`'s unit tests do (the
  weekday-exclusion trick for T040, or the short real-time sleep for T041); a schedule-window AT
  scenario would either need to encode today's real weekday (making the scenario's pass/fail depend
  on which day the suite happens to run — unacceptable non-determinism for a CI-run conformance
  suite) or add new harness plumbing whose own correctness would need the same kind of testing
  `schedule_enforcement.rs` already provides more directly and deterministically at the session
  layer. T040/T041's transport-adjacent-enough session-level integration coverage (exercising the
  real `on_logon`/`on_tick` code paths transport-layer `dispatch` calls into unmodified) is relied
  upon instead; no AT scenario added.

**Checkpoint**: User Story 1 fully functional and testable independently — every P0-tier item from
`docs/todo/004.md` closed; `cargo test --workspace --all-features` and `cargo test -p truefix-at
--test conformance` both green.

---

## Phase 3: User Story 2 — Real defects with narrower blast radius (Priority: P2)

**Goal**: Close every confirmed defect requiring an unusual configuration, specific counterparty
behavior, or specific input to trigger.

**Independent Test**: Each item has its own targeted test exercising the specific narrow trigger
condition.

- [X] T047 [P] [US2] Add a failing test: a new connection resets `resend_requested` and the
  out-of-order queue, so a new gap on the new connection triggers a `ResendRequest` rather than
  being suppressed by stale state, in `crates/truefix-session/tests/
  resend_requested_reset_on_reconnect.rs` (new) (`BUG-35`, FR-017).
  **Completed**: `a_new_connection_resets_resend_requested_so_a_new_gap_triggers_a_fresh_resend_request`
  — logs on, triggers a gap (ResendRequest sent), reconnects (`Event::Connected` + a continuing-
  sequence Logon), then triggers a second gap and confirms a fresh ResendRequest is sent rather than
  suppressed.
- [X] T048 [US2] Reset `resend_requested` and `queue` in `on_connected`
  (`crates/truefix-session/src/state.rs`) — makes T047 pass (depends on T047).
  **Completed**: `self.resend_requested = false; self.queue.clear();` added alongside `on_connected`'s
  existing per-connection state resets (`ticks_awaiting`, `test_request_outstanding`, etc.).
  **Verification**: `cargo test -p truefix-session --test resend_requested_reset_on_reconnect` — 1/1
  pass. Bisection: temporarily wrapped the new reset lines in `if false { }`, reran — failed with
  concrete evidence (`got []`, no fresh ResendRequest sent); reverted, reran, passes. Full regression:
  `cargo test -p truefix-session --all-features` (17 test binaries) — all green.
- [X] T049 [P] [US2] Add a failing test: a non-Logon message arriving before the session completes
  its Logon exchange is rejected, in `crates/truefix-session/tests/valid_logon_state.rs` (new)
  (`BUG-42`, FR-018).
  **Completed**: 4 tests — a pre-Logon Heartbeat disconnects; a too-high-seq pre-Logon Heartbeat
  does not queue/ResendRequest; Logon/SequenceReset/Reject are still processed normally pre-Logon
  (validLogonState's own allow-list); an inbound Logout pre-Logon reaches its own normal handling
  (not the new rejection path — Logout naturally disconnects either way, so the test checks the
  Logout *text*, not just the resulting state).
- [X] T050 [US2] Add a `validLogonState`-equivalent check to `on_received`
  (`crates/truefix-session/src/state.rs`) — makes T049 pass (depends on T049).
  **Completed**: added right after `mt` is computed, before the Logon/SequenceReset early-return
  branches — rejects (Logout+disconnect) any message when `self.state != LoggedOn` unless its
  MsgType is Logon("A")/Logout("5")/SequenceReset("4")/Reject("3"), matching QFJ's
  `validLogonState()` allow-list.
  **Two pre-existing, previously-masked defects surfaced and fixed along the way** (both exposed by
  the same mechanism: before this fix, a message arriving while not-logged-on fell through to
  ordinary processing with no session-layer gate at all, so a Logon that had *silently failed*
  produced no visible symptom — subsequent admin messages just no-op'd through Heartbeat's
  empty-action handling. T050 makes "not logged on" observable — a rejected/disconnected session
  now visibly disconnects instead of silently absorbing traffic — which is exactly what caused two
  *pre-existing* `recovery.rs`/`validation_hook.rs` tests to start failing, tracing back to two
  independent, real defects, not to T050 itself):
  1. **`on_logon`'s too-low-seq check ran before `ResetSeqNumFlag` was honored**: a reconnecting
     counterparty legitimately sending `Logon(MsgSeqNum=1, ResetSeqNumFlag=Y)` after the local side's
     `next_in_seq` had already advanced past 1 was wrongly rejected as "MsgSeqNum too low" — the
     too-low check (BUG-05, feature 006) ran unconditionally, before this function's own (separate,
     later) `ResetSeqNumFlag` handling ever got a chance to apply the reset. Fixed by exempting a
     Logon carrying `ResetSeqNumFlag=Y` from the too-low-seq check (`requests_reset` computed
     alongside `poss_dup`, added to the guard). Surfaced by
     `recovery.rs::reconnecting_clears_stale_chunked_resend_tracking_so_a_fresh_connection_does_not_spuriously_request_a_resend`,
     which — before this fix — was unknowingly asserting on a session that had silently failed to
     reconnect at all (its "no ResendRequest issued" assertion passed vacuously, for the wrong
     reason: the session was disconnected, not successfully logged back on with no gap).
  2. **Two more stale Logon test fixtures missing dictionary-required fields** (same class of latent
     issue as T034/T035's `validation_hook.rs` fix, just not caught then because these two specific
     tests weren't exercising a dictionary+no-session-gating combination that made the failure
     externally observable until now): `recovery.rs`'s
     `invalid_message_drained_from_queue_after_a_gap_is_rejected_like_an_in_order_one` attaches a
     FIX.4.4 dictionary but its Logon fixture omitted `EncryptMethod(98)` — fixed by adding it.
     `validation_hook.rs`'s `fixt_dictionaries_with_no_resolvable_appl_ver_id_skips_validation_like_no_dictionary`
     used a FIXT.1.1 transport dictionary (which requires `DefaultApplVerID(1137)` be *present* on
     Logon) but its Logon carried no 1137 at all — fixed by giving it 1137 set to an *unregistered*
     `ApplVerID` value (`"FIX.9.9"`, not registered via `.with_application(...)`), which satisfies the
     transport dictionary's presence requirement while preserving the test's actual intent (an
     unresolvable-to-any-app-dict `ApplVerID` still skips validation like the no-dictionary case).
  **Verification**: `cargo test -p truefix-session --test valid_logon_state` (4/4),
  `--test recovery` (26/26), `--test validation_hook` (8/8) all pass. Bisection: temporarily
  short-circuited T050's check (`if false && ...`) — both of `valid_logon_state.rs`'s
  seq/no-resend-request tests failed with concrete evidence (wrong state / an actual ResendRequest
  sent); reverted. Separately short-circuited the `requests_reset` exemption (a temporary `let _ =
  requests_reset;` bypassing it) — `recovery.rs`'s reconnect-tracking test failed again with the
  same original evidence; reverted. Full regression: `cargo test -p truefix-session --all-features`
  (18 test binaries), `cargo test -p truefix-transport --all-features`, `cargo test -p truefix-at
  --all-features` (424 AT scenario runs) — all green. `cargo fmt` applied.
- [X] T051 [P] [US2] Add a failing test: an inbound Logon is rejected when `NextExpectedMsgSeqNum`
  exceeds messages sent, when `MsgSeqNum` is missing entirely, or when `HeartBtInt` is negative, in
  `crates/truefix-session/tests/logon_edge_cases.rs` (new) (`BUG-43`/`44`/`95`, FR-019).
  **Completed**: 4 tests — the three rejection cases (missing MsgSeqNum, negative HeartBtInt,
  NextExpectedMsgSeqNum higher than sent) plus a normal-Logon-still-succeeds control. The
  789-too-high test needed its reconnect Logon to carry seq 2 (in-order), not seq 1 — a seq-1
  reconnect Logon without `ResetSeqNumFlag=Y` would otherwise be intercepted by the pre-existing
  too-low-seq check first (`next_in_seq` was already 2 from the first Logon), which would make the
  test pass for the wrong reason.
- [X] T052 [US2] Add the three Logon-rejection checks to `on_logon`
  (`crates/truefix-session/src/state.rs`) — makes T051 pass (depends on T051).
  **Completed**: all three added as early Logout+disconnect rejections (via `reject_logon`),
  positioned right after the existing too-low-seq check and before the schedule/dictionary checks —
  before any reset/state-transition side effects run, matching the established pattern for every
  other Logon-rejection case in this function. Missing-MsgSeqNum checks `logon_seq_early.is_none()`.
  Negative-HeartBtInt reads tag 108 as a raw signed int (not through the existing `.filter(|&h| h >
  0)` adoption logic further down, which silently discards negative values instead of flagging
  them). Too-high-NextExpectedMsgSeqNum compares tag 789 against `self.next_out_seq` as it stands
  before this Logon's own response is sent (the existing too-low-side handling, near this function's
  end, is untouched).
  **Discovered and resolved: a real conflict with an existing, deliberately-authored AT scenario.**
  `crates/truefix-at/src/scenarios.rs`'s `qfj648_negative_heart_bt_int` (authored in an earlier
  feature round, referencing QuickFIX/J issue/JIRA **QFJ-648**) asserted the *opposite* of BUG-95: that
  a negative HeartBtInt should be silently tolerated (Logon still succeeds, invalid value ignored,
  configured default kept) rather than rejected. Since this is a direct behavioral contradiction
  between two "spec-like" sources in this repository (docs/todo/004.md's freshly-audited BUG-95 vs.
  an established, named-after-a-specific-QFJ-issue AT scenario), the user was asked to resolve it
  first; no response arrived within the wait window, so this was resolved via independent web research
  instead of guessing: QuickFIX/J's own issue history confirms **QFJ-648 is precisely the issue that
  *added* this rejection** to `Session.next()` (`RejectLogon("HeartBtInt must not be negative")`) —
  i.e., the existing scenario had encoded the *pre-fix* (buggy) QFJ behavior under a name that
  actually refers to the fix itself. BUG-95's premise is correct. Updated
  `qfj648_negative_heart_bt_int` to expect `Logout + disconnect` instead of a successful Logon, with
  an inline comment recording the correction and its source.
  **Verification**: `cargo test -p truefix-session --test logon_edge_cases` — 4/4 pass. Bisection: all
  three checks temporarily short-circuited together (`if false && ...` / equivalent), reran — all
  three corresponding tests failed with concrete evidence (`left: LoggedOn, right: LoggedOn`, i.e. no
  rejection occurred); each reverted individually and confirmed passing again. Full regression:
  `cargo test -p truefix-session --all-features` (19 test binaries) clean; `cargo test -p truefix-at
  --test conformance` — initially caught the QFJ648 conflict (415/424; the tail output showed 2
  explicit `AT FAIL` lines for FIX.5.0SP2/FIX.Latest, with the report summarizing 9 total failing
  scenario runs across the full 9-version matrix), fixed, reran to 424/424. `cargo test --workspace
  --all-features` — 142 test-result blocks, 0 FAILED. `cargo fmt` applied.
- [X] T053 [P] [US2] Add a failing test: a length-prefixed data field followed by a non-matching
  data tag, or a non-numeric length value, fails decoding cleanly rather than silently misparsing
  subsequent fields, in `crates/truefix-core/tests/data_field_verification.rs` (new) (`BUG-38`/`49`,
  FR-020).
  **Completed**: 4 tests. The first, naive attempt at BUG-38's scenario (length=5 followed by an
  unrelated tag whose value is a longer, unrelated string) turned out to *already* fail today (by
  accident — the declared length just didn't happen to land on an SOH boundary, so it errored for
  the wrong reason, not because the tag was verified). Investigating why led to a much sharper,
  genuinely adversarial second test,
  `a_length_that_spans_an_embedded_soh_and_a_whole_following_field_does_not_silently_swallow_it`:
  a phantom length whose value is *exactly* `"ab\x01100=cd"` (9 raw bytes, including one literal
  embedded SOH, constructed via `Field::new` which accepts arbitrary bytes) reproduces true silent
  corruption — before the fix, this decodes successfully with **no error at all**, but tag 100
  vanishes entirely from the decoded message (swallowed into tag 99's "value" along with a raw SOH
  byte). This is the actual shape of BUG-38's danger, not just an accidental decode failure. Also
  added a positive control (a correctly-paired length/data field decodes normally) and BUG-49's
  non-numeric-length-value case.
- [X] T054 [US2] Verify the data-tag match and reject non-numeric length values in the decode path
  (`crates/truefix-core/src/codec/decode.rs`) — makes T053 pass (depends on T053).
  **Completed**: `tokenize`'s `pending_data_len: Option<usize>` became `pending_data: Option<(u32,
  usize)>` — the expected data tag paired with its declared length, not just the length alone. On
  the next token, if `pending_data` is `Some((expected_tag, len))` and the current tag doesn't match
  `expected_tag`, decoding now fails with `GarbledField` (BUG-38) instead of blindly applying `len`
  to whatever tag actually appeared. A non-numeric length value itself now fails immediately
  (`GarbledField`) instead of silently leaving `pending_data` unset (BUG-49).
  **Verification**: `cargo test -p truefix-core --test data_field_verification` — 4/4 pass.
  Bisection: temporarily relaxed the tag-match arm (`Some((_, len)) => Some(len)`, dropping the
  `expected_tag == tag` guard) — the adversarial embedded-SOH test failed with concrete evidence
  (tag 100 missing from the decoded body, `Ok(...)` returned instead of an error), confirming it's a
  genuine guard (the naive first test still passed even with the guard disabled, confirming it
  really was passing for an unrelated, coincidental reason, not because of this specific check);
  reverted. Separately reverted the non-numeric-length check to its pre-fix form (silent skip) —
  that test failed with concrete evidence (`Ok(...)`, no error); reverted back. Full regression:
  `cargo test -p truefix-core --all-features` (16 test binaries) and `cargo test --workspace
  --all-features` (145 test-result blocks, 0 FAILED, including `truefix-at`'s 424/424 AT scenario
  runs) — all green. `cargo fmt` applied.
- [X] T055 [P] [US2] Add a failing test: the plain (non-TLS, non-failover) reconnecting initiator
  applies the same socket options every other connection path applies, in
  `crates/truefix-transport/tests/reconnecting_socket_options.rs` (new) (`BUG-39`, FR-021).
  **Disclosed testing-approach deviation**: two black-box behavioral test designs were attempted
  and abandoned before settling on a source-verification test. (1) `SO_LINGER(0)` (abortive
  RST-on-close vs. graceful FIN is normally externally observable): defeated by
  `run_connection`'s shutdown tail, which unconditionally calls a graceful `write_half.shutdown()`
  before the stream is ever dropped on *every* exit path, neutralizing `SO_LINGER`'s effect
  regardless of whether options were applied. (2) `TCP_NODELAY` (Nagle-coalescing timing):
  confirmed experimentally (a throwaway `rustc`-compiled probe) that a fresh `tokio::net::TcpStream`
  defaults to `nodelay=false`, so a genuine before/after difference exists in principle -- but
  Nagle's delay window is bounded by ACK round-trip time, sub-millisecond on loopback regardless of
  the `nodelay` setting, so no reliable, non-flaky timing signal exists to observe over a local test
  socket. Every other option in `SocketOptions` is either similarly unobservable from the peer's
  side (`keep_alive`, buffer sizes, `oob_inline`, `traffic_class`) or acceptor-listener-only
  (`reuse_address`). Settled on: a test that reads `crates/truefix-transport/src/lib.rs` at test
  time and asserts `connect_initiator_reconnecting_multi`'s body contains a
  `socket_options.apply` call -- `SocketOptions::apply` itself is already behaviorally verified
  elsewhere (`socket_options.rs`'s `full_option_set_applies_without_error`, which owns its stream
  outright and checks `nodelay()` directly); what was actually missing and needed a guard is
  specifically that *this one call site* invokes it.
- [X] T056 [US2] Apply `services.socket_options.apply(&stream)` in
  `connect_initiator_reconnecting_multi` (`crates/truefix-transport/src/lib.rs`) — makes T055 pass
  (depends on T055).
  **Completed**: added immediately after a successful `tcp_connect`, before `metrics_export::
  record_reconnect`/`run_connection` — matching the six other call sites of `socket_options.apply`
  already present in this same file.
  **Verification**: `cargo test -p truefix-transport --test reconnecting_socket_options` — 1/1
  passes. Bisection: temporarily removed the new `services.socket_options.apply(&stream)` line —
  the test failed with concrete evidence (assertion message: the call is missing from the function
  body); reverted, reran, passes. Full regression: `cargo test -p truefix-transport --all-features`
  clean (no FAILED/error matches). `cargo fmt` applied.
- [X] T057 [P] [US2] Add a failing test: an acceptor group's log/dictionary-validator/socket-options/
  TLS configuration is resolved per-session, not silently inherited from `members.first()`/the first
  TLS-configured member, in `crates/truefix-transport/tests/acceptor_group_member_resolution.rs`
  (new) (`BUG-61`, FR-022).
  **Completed**: `each_group_member_logs_to_its_own_per_session_log_not_the_primarys` — two static
  sessions in one `AcceptorBuilder` group, each given a distinct `CollectingLog` test double via the
  new `with_session_services`; asserts each session's own traffic lands in its own Log and never
  cross-contaminates the other's.
- [X] T058 [US2] Resolve log/validator/socket-options/TLS per connected session's own `.cfg` entry in
  the acceptor-group path (`crates/truefix-transport/src/lib.rs` and/or `crates/truefix/src/lib.rs`)
  — makes T057 pass (depends on T057).
  **Completed** (log/validator/fixt_dictionaries only — see disclosed scope narrowing below for
  socket-options/TLS):
  - `crates/truefix-transport/src/lib.rs`: `Registry`/`AcceptorBuilder` gained `session_services:
    HashMap<SessionId, Services>` and a new `AcceptorBuilder::with_session_services(session_id,
    services)` builder method, mirroring the existing `with_session_store`/`session_stores`
    per-session-store mechanism (BUG-07, feature 006) but for `log`/`validator`/`fixt_dictionaries`.
    `route_and_run` resolves it right after the existing per-session store lookup: `services.store`
    is preserved (untouched by this new mechanism), while `log`/`validator`/`fixt_dictionaries` are
    overwritten from the per-session override when one is registered for the resolved `SessionId`.
  - `crates/truefix/src/lib.rs`'s acceptor-group builder: for every statically-registered
    non-primary member, builds that member's *own* log (via the same `build_log_kind`/
    `build_sql_log`/`build_log` calls already used for `primary`, keyed by that member's own
    `SenderCompID`/`TargetCompID`-derived session-id string) and `fixt_dictionaries`/`validator`
    from that member's own `.cfg` fields, then registers them via `.with_session_services(...)`.
  **Disclosed scope narrowing — `socket_options`/TLS remain group-wide**: the task's own wording
  named `socket-options`/TLS alongside log/validator, but resolving per-session socket options this
  late (after `route_and_run` has read enough bytes to identify the session, potentially past a TLS
  handshake) turned out to require reaching back into the generic, possibly-TLS-wrapped stream
  parameter's underlying `TcpStream` — not expressible without either a new generic trait bound
  threaded through every caller of `route_and_run`, or unsafe raw-fd handling, which this project's
  `unsafe_code = "forbid"` (Constitution Principle I) forbids outright. Socket options continue to
  be applied once, group-wide, at accept time (before any session identity is known) — the same
  disclosed simplification the code already carried for TLS ("necessarily listener-scoped... the
  first member that enables it wins"). log/validator/fixt_dictionaries — the parts of BUG-61 that
  don't require raw socket access — are fully fixed.
  **Verification**: `cargo test -p truefix-transport --test acceptor_group_member_resolution` — 1/1
  passes. Bisection: temporarily wrapped the new `route_and_run` merge block in `if false { }`,
  reran — failed with concrete evidence ("session1's own per-session Log must have received its own
  traffic" — i.e. the per-session override was never applied, group-wide services.log stayed
  `None`); reverted, reran, passes. Full regression: `cargo test -p truefix-transport
  --all-features` and `cargo test -p truefix --all-features` (13 test binaries across both,
  including `multi_session_acceptor_cfg.rs`'s existing grouped-acceptor `.cfg` coverage) — all
  green. `cargo fmt` applied.
- [X] T059 [P] [US2] Add a failing test: an outbound `Reject` on a `FIX.4.2`+ session includes
  `RefMsgType`; a `SessionRejectReason` outside a `FIX.4.0`/`4.1` session's defined range is omitted,
  in `crates/truefix-session/tests/reject_field_correctness.rs` (new) (`BUG-58`/`59`, FR-023).
  **Completed**: 5 tests (FIX.4.0/4.1/4.2/4.3/4.4), each logging on then triggering
  `TagAppearsMoreThanOnce` (code 13, chosen specifically to straddle the FIX.4.2 (<=11) vs. FIX.4.3
  (<=15) boundary) via a Heartbeat with tag 112 repeated (`add_field` twice, not `.set()`, which
  would just replace). Incidental discovery while debugging an unexpected "no Reject sent at all"
  failure for FIX.4.0/4.1: those dictionaries type `SenderCompID`/`TargetCompID` as `CHAR` (a single
  character) rather than `STRING` — an unrelated, not-yet-fixed defect (BUG-62, a later US2 task)
  that was rejecting this test's original multi-character comp IDs (`"ME"`/`"YOU"`) as invalid CHAR
  values. Worked around (not fixed, since out of this task's scope) by using single-character comp
  IDs (`"M"`/`"Y"`) in this test's fixtures instead.
- [X] T060 [US2] Add `RefMsgType` and version-filtered `SessionRejectReason` to
  `reject`/`reject_with_reason` (`crates/truefix-session/src/admin.rs`) — makes T059 pass (depends
  on T059).
  **Completed**: a new private `fix_version(begin_string) -> Option<(u8, u8)>` helper (mirroring
  `truefix-dict`'s own, separate, crate-private `parse_fix_begin_string`), plus
  `ref_msg_type_applies`/`session_reject_reason_applies` gating functions. `reject()` gained a new
  `ref_msg_type: Option<&str>` parameter; `reject_with_reason()`'s existing parameter list gained the
  same, positioned before `ref_tag`. Both project-wide call sites in `state.rs` (6 total across
  `on_garbled`/the missing-MsgSeqNum case/`validate_app`/`on_resend_request`/`on_sequence_reset` ×2)
  updated to pass `msg.msg_type()` (or `None` for `on_garbled`, which has no decodable message to
  reference at all).
  **Discovered and fixed: 4 pre-existing AT scenarios asserted the old, incorrect
  unconditional-SessionRejectReason behavior for FIX.4.0/4.1** — `10_MsgSeqNumLess`,
  `BUG06_SequenceResetMissingNewSeqNoRejected`, `BUG22_ResendRequestMissingBeginSeqNoRejected`,
  `BUG22_ResendRequestMissingEndSeqNoRejected` (all looped across all 9 `SUITE_VERSIONS`, including
  FIX.4.0/4.1, where tag 373 must now be entirely absent per BUG-59). Added a shared
  `expect_session_reject_reason(v, msg_type, code)` helper in `crates/truefix-at/src/scenarios.rs`
  that asserts absence for FIX.4.0/4.1 and presence (with the given code) otherwise; all 4 scenarios
  switched to use it instead of a hardcoded `.field(373, ...)`.
  **Verification**: `cargo test -p truefix-session --test reject_field_correctness` — 5/5 pass.
  Bisection: `ref_msg_type_applies` temporarily forced to always return `false` — 3/5 tests failed
  with concrete evidence (RefMsgType missing where it should be present for FIX.4.2/4.3/4.4);
  reverted. `session_reject_reason_applies` temporarily forced to always return `true` — 3/5 tests
  failed with concrete evidence (SessionRejectReason present where it must be omitted, or present
  with a code exceeding FIX.4.2's own range); reverted. Full regression: `cargo test -p
  truefix-session --all-features` (20 test binaries), `cargo test -p truefix-at --test conformance`
  (initially caught the 4-scenario AT regression at 416/424; fixed, reran to 424/424), `cargo test
  --workspace --all-features` (143 test-result blocks, 0 FAILED) — all green. `cargo fmt` applied.
- [X] T061 [P] [US2] Add a failing test: a `PossDupFlag=Y` message whose sequence equals (not just is
  below) the expected value still gets the `OrigSendingTime`-vs-`SendingTime` anti-replay check, in
  `crates/truefix-session/tests/poss_dup_equal_seq.rs` (new) (`BUG-57`, FR-024).
  **Completed**: 3 tests — falsified PossDup at the expected sequence is rejected; falsified PossDup
  above the expected sequence is also rejected (per BUG-57's own wording, "including expected-seq
  and too-high"); a genuine (non-falsified) PossDup at the expected sequence is still processed
  normally (control).
- [X] T062 [US2] Extend the PossDup anti-replay check's trigger condition in `on_received`
  (`crates/truefix-session/src/state.rs`) to include the equal-sequence case — makes T061 pass
  (depends on T061).
  **Completed**: extracted a new, narrower `poss_dup_falsification_check` (just the
  `OrigSendingTime > SendingTime` signal) out of the existing `poss_dup_anti_replay_violation`,
  called unconditionally whenever `poss_dup` is true, ahead of the `Ordering::Equal`/`Greater`/
  `Less` dispatch — so it fires regardless of which of those three the message's sequence falls
  into. The existing `poss_dup_anti_replay_violation` (still called only from `Ordering::Less`, its
  original location) is left as-is; deliberately did *not* also broaden the *missing*-
  `OrigSendingTime`-on-a-too-low-message case (gated by `requires_orig_sending_time_on_low_seq`,
  whose name and sole existing call site already scope it to the too-low case specifically) — BUG-57
  and its own audit text describe only the unconditional falsification signal as needing to apply
  universally, not the missing-tag/low-seq-specific toggle.
  **Verification**: `cargo test -p truefix-session --test poss_dup_equal_seq` — 3/3 pass. Bisection:
  temporarily short-circuited the new top-level check (`if false && poss_dup`), reran — both
  rejection tests failed with concrete evidence (`left: LoggedOn, right: Disconnected`, i.e. the
  falsified message was processed as if genuine); reverted, reran, passes. Full regression: `cargo
  test -p truefix-session --all-features` (21 test binaries) and `cargo test -p truefix-at --test
  conformance` (424/424 AT scenario runs) — all green. `cargo fmt` applied.
- [X] T063 [P] [US2] Add a failing test: an initiator's first connection sends `ResetSeqNumFlag=Y`
  when any of `ResetOnLogon`/`ResetOnLogout`/`ResetOnDisconnect` is set, not only `ResetOnLogon`, in
  `crates/truefix-session/tests/reset_seq_num_flag_all_configs.rs` (new) (`BUG-92`/`109`, FR-025).
  **Completed**: 4 tests — `ResetOnLogout` alone sends the flag on a fresh connection; `ResetOnDisconnect`
  alone does too; no reset config at all never sends it (control); and — matching 004.md's own
  correction that QFJ additionally requires both sequence numbers to already be 1 — `ResetOnLogout`
  configured but sequences already advanced past 1 (via `seed_sequences`) does *not* send the flag,
  since no actual reset has happened yet.
- [X] T064 [US2] Extend `on_connected`'s reset-flag trigger condition
  (`crates/truefix-session/src/state.rs`) to check all three reset configs — makes T063 pass
  (depends on T063).
  **Completed**: matches QFJ's `isResetNeeded()`/QFGo's `shouldSendReset()` exactly per 004.md's own
  correction: `(reset_on_logon || reset_on_logout || reset_on_disconnect) && next_out_seq == 1 &&
  next_in_seq == 1`. The actual reset-to-1 side effect stays gated by `reset_on_logon` alone,
  unchanged (that's the only one of the three that forces a reset *at connect time*;
  `reset_on_logout`/`reset_on_disconnect` reset at a prior disconnect/logout instead, so the "both
  seqs == 1" half of the gate is what correctly detects that a real reset already happened for
  those two, rather than the config merely being set on an unrelated non-1 sequence). Also removed
  the now-dead `admin::logon()` wrapper (its only call site switched to calling
  `admin::logon_with_reset_flag` directly with the computed `should_send_reset`), folding its doc
  comment into `logon_with_reset_flag`'s own.
  **Verification**: `cargo test -p truefix-session --test reset_seq_num_flag_all_configs` — 4/4
  pass. Bisection: temporarily narrowed the gate back to `reset_on_logon` alone — both
  `ResetOnLogout`/`ResetOnDisconnect`-alone tests failed with concrete evidence (`left: None, right:
  Some("Y")`, i.e. no flag sent when one should have been); reverted, reran, passes. Full
  regression: `cargo test -p truefix-session --all-features` (22 test binaries, exercising the
  heavily-used default `reset_on_logon=true` path throughout with no behavior change there), `cargo
  test -p truefix-transport --all-features`, and `cargo test -p truefix-at --test conformance`
  (424/424 AT scenario runs) — all green. `cargo fmt` applied.
- [X] T065 [P] [US2] Add a failing test: a `SequenceReset` (gap-fill mode) discards now-superseded
  queued messages rather than retaining them indefinitely, in `crates/truefix-session/tests/
  sequence_reset_dequeue.rs` (new) (`BUG-96`, FR-027).
  **Completed**: added a new `pub fn queued_len(&self) -> usize` accessor to `Session` (no
  equivalent existed) so the test can observe the queue's internal state. The test queues two
  non-contiguous out-of-order messages (seq 10 and 20), applies a gap-fill jumping to `NewSeqNo=15`
  (superseding seq 10 but not seq 20), then fills the remaining gap up to 20 and confirms the queue
  ends up empty — before the fix, seq 10 stayed in the `BTreeMap` forever (never matching
  `next_in_seq` again after it jumped past it).
- [X] T066 [US2] Discard superseded queue entries in the gap-fill `SequenceReset` handler
  (`crates/truefix-session/src/state.rs`) — makes T065 pass (depends on T065).
  **Completed**: `self.queue.retain(|&seq, _| seq >= ns)` added right after `self.next_in_seq = ns`
  in the gap-fill branch, matching QFJ's `dequeueMessagesUpTo(newSequence)`. `drain_queue()` itself
  (only ever removes an entry exactly matching the current `next_in_seq`) is unchanged — it still
  runs immediately after, to drain any now-contiguous run starting at the new `next_in_seq`.
  **Verification**: `cargo test -p truefix-session --test sequence_reset_dequeue` — 1/1 passes.
  Bisection: temporarily wrapped the new `retain` call in `if false { }`, reran — failed with
  concrete evidence (`left: 1, right: 0`, i.e. seq 10 still present in the queue); reverted, reran,
  passes. Full regression: `cargo test -p truefix-session --all-features` (23 test binaries) and
  `cargo test -p truefix-at --test conformance` (424/424 AT scenario runs) — all green. `cargo fmt`
  applied.
- [X] T067 [P] [US2] Add a failing test: a stray Logon arriving in `AwaitingLogout` or
  `Disconnected` state is rejected rather than silently ignored, in
  `crates/truefix-session/tests/stray_logon_rejected.rs` (new) (`BUG-90`, FR-028).
  **Completed**: 3 tests — a stray Logon while `AwaitingLogout` (reached via `Event::StartLogout`)
  is rejected; a stray Logon while already `Disconnected` (reached via the existing, unrelated
  duplicate-Logon-while-LoggedOn rejection, GAP-18a) is also rejected; a normal Logon while
  genuinely `AwaitingLogon` still succeeds (control).
- [X] T068 [US2] Reject a Logon arriving outside `AwaitingLogon` state in `on_logon`
  (`crates/truefix-session/src/state.rs`) — makes T067 pass (depends on T067).
  **Completed**: added right after the existing duplicate-Logon-while-`LoggedOn` check (which
  already covers that one specific state) — any Logon while `state != AwaitingLogon` (so, in
  practice, `AwaitingLogout` or `Disconnected`) is rejected via the same `reject_logon`
  (Logout+disconnect) path, rather than falling through to `on_logon`'s final `_ => {}` catch-all
  and doing nothing.
  **Verification**: `cargo test -p truefix-session --test stray_logon_rejected` — 3/3 pass.
  Bisection: temporarily short-circuited the new check (`if false && self.state != AwaitingLogon`),
  reran — both rejection tests failed with concrete evidence (state stayed `AwaitingLogout` instead
  of `Disconnected`; no `Disconnect` action for the Disconnected-state case); reverted, reran,
  passes. Full regression: `cargo test -p truefix-session --all-features` (24 test binaries), `cargo
  test -p truefix-transport --all-features`, `cargo test -p truefix-at --test conformance` (424/424
  AT scenario runs) — all green. `cargo fmt` applied.
- [X] T069 [P] [US2] Add a failing test: a TCP disconnection during the Logon handshake itself
  (before login completes) still triggers the application's logout callback, in
  `crates/truefix-transport/tests/logout_callback_mid_handshake.rs` (new) (`BUG-93`, FR-029)
  (depends on T030, since this reuses `Event::Disconnected`'s plumbing).
  **Completed**: an initiator connects to a raw `TcpListener` that accepts but never replies at all
  (holds the connection briefly, then drops it) — the initiator's own Logon was sent (`logon_sent`
  true immediately via `on_connected`) but the handshake never completes. Confirms `on_logout`
  fires and `on_logon` never does.
- [X] T070 [US2] Extend `run_connection`'s shutdown tail (`crates/truefix-transport/src/lib.rs`) to
  fire `app.on_logout` when the connection had sent or received a Logon (not only when
  fully `logged_on`) — makes T069 pass (depends on T069).
  **Completed**: `Session` gained two new per-connection-reset fields/accessors,
  `logon_sent()`/`logon_received()` (the former already existed internally but had no public
  accessor; the latter is new, set unconditionally at the very top of `on_logon` before any
  accept/reject checks — matching QFJ's `logonReceived`, which tracks receipt, not acceptance).
  Both reset to `false` in `on_connected`. `run_connection`'s shutdown tail condition became
  `logged_on || session.logon_sent() || session.logon_received()`, matching QFJ
  (`logonReceived || logonSent`) and QFGo.
  **Verification**: `cargo test -p truefix-transport --test logout_callback_mid_handshake` — 1/1
  passes. Bisection: temporarily narrowed the condition back to `logged_on` alone, reran — failed
  with concrete evidence (on_logout never fired within the 5s wait); reverted, reran, passes. Full
  regression: `cargo test -p truefix-session --all-features` (24 test binaries), `cargo test -p
  truefix-transport --all-features` (35 test-result blocks, 0 FAILED), `cargo test -p truefix-at
  --test conformance` (424/424 AT scenario runs) — all green. `cargo fmt` applied.
- [X] T071 [P] [US2] Add a failing test: an MSSQL store/log connection URL with an `@` character in
  its password parses correctly, in `crates/truefix-store/tests/mssql_url_at_sign.rs` (new)
  (`BUG-70`, FR-030) (`--features mssql`).
  **Testing approach**: `tiberius::Config` exposes no getters, so the parsed host/user/password
  can't be asserted on directly, and no real MSSQL server is available in this sandbox. Instead,
  the test binds a raw local `TcpListener` (not a real server) and points the URL's host at it: a
  correctly-parsed host reaches that listener (`accept()` fires) before failing later at the TDS
  handshake step (this raw listener doesn't speak the protocol, so `MssqlStore::connect` always
  errors regardless — that's not what's under test); a corrupted host fails at DNS resolution
  before ever reaching the listener, so `accept()` never fires. No `DATABASE_URL_MSSQL`/live server
  needed — this test runs standalone under `--features mssql`.
  **Bug found in the test itself while developing it**: the first version used
  `tokio::join!(connect_fut, accept_fut)` with only `accept_fut` timeout-wrapped; since
  `tiberius::Client::connect`'s handshake has no timeout of its own, a *successful* TCP connect to
  the dumb listener (exactly the passing case) hung forever waiting for a TDS response that would
  never come, deadlocking the test process. Fixed by wrapping `connect_fut` in its own timeout too.
- [X] T072 [US2] Fix the credentials/host split in `parse_url`
  (`crates/truefix-store/src/mssql.rs`, `crates/truefix-log/src/mssql.rs`) to handle an `@` in the
  password unambiguously — makes T071 pass (depends on T071).
  **Completed**: `split_once('@')` → `rsplit_once('@')` in both files' `parse_url` — the host
  portion of a valid URL never itself contains `@`, so splitting at the *last* occurrence
  unambiguously separates credentials from host even when the password does (`userinfo.split_once
  (':')`, unchanged, still correctly separates user from password afterward, since that split
  happens on the now-correctly-isolated userinfo substring).
  **Verification**: `cargo test -p truefix-store --features mssql --test mssql_url_at_sign` — 1/1
  passes (3s, no hang). Bisection: temporarily reverted to `split_once('@')` — failed with concrete
  evidence (`accept()` never fired, i.e. DNS resolution failed on the corrupted host before ever
  reaching the listener); reverted back, reran, passes. Full regression: `cargo test -p
  truefix-store --features mssql --all-targets` (all binaries green, including the pre-existing
  `mssql_commit_rollback.rs`), `cargo build -p truefix-log --features mssql --all-targets` clean,
  `cargo test -p truefix-log --features mssql --test mssql_log` (2/2, self-skipping without
  `DATABASE_URL_MSSQL` as expected), `cargo test --workspace --all-features` (154 test-result
  blocks, 0 FAILED — this flag already exercises the `mssql` feature workspace-wide) — all green.
  `cargo fmt` applied.
- [X] T073 [P] [US2] Add a failing test: opening a `FileStore`/`CachedFileStore` against a long
  message history does not read every stored message body into memory just to rebuild the offset
  index (or warm the cache), in `crates/truefix-store/tests/file_store_open_memory_bound.rs` (new)
  (`BUG-45`/`81`, FR-031).
  **Disclosed testing-approach note**: an exact byte-for-byte memory-bound proof isn't achievable
  without either OS-level introspection or a custom tracking `#[global_allocator]` — the latter
  requires `unsafe impl GlobalAlloc`, which this workspace's `unsafe_code = "forbid"` lint (applied
  workspace-wide via `[lints] workspace = true`, including test targets) forbids outright. Used
  `ps -o rss=` (portable macOS/Linux) to observe this process's own RSS before/after opening a store
  backed by a genuine ~48MB body-log file (constructed by writing records directly to disk in the
  exact `BodyLog::append` wire format, bypassing the slow per-message API purely for test speed),
  with a generous 8MiB threshold — comfortably separates "read the whole ~48MB file" from "didn't"
  without being sensitive to unrelated allocator noise. 2 tests: one for `FileStore` (index
  rebuild), one for `CachedFileStore` with `max_cached_msgs=10` (cache warming), the latter also
  confirming `cached_len() == 10` after open.
- [X] T074 [US2] Rework `FileStore`/`CachedFileStore::open_with_options`
  (`crates/truefix-store/src/file.rs`) to rebuild the offset index (and, for `CachedFileStore`, warm
  its cache up to `max_cached_msgs`) without reading every message body — makes T073 pass (depends
  on T073).
  **Completed**:
  - `load_index` (BUG-45): rewritten to stream through the body-log file via `File`/`Seek` instead
    of `fs::read` — reads only 12-byte record headers, `seek`s past each body's bytes without ever
    reading them, and uses `metadata().len()` (one syscall) instead of the in-memory buffer's own
    length to detect trailing corruption. `parse_records`/`read_u64`/`read_u32` (the old
    buffer-based helpers) removed as now-dead code.
  - `CachedFileStore::open_with_options` (BUG-81): replaced `body.all()` (collect every body into
    one `Vec`, then insert-with-eviction) with `body.seqs_in_range(0, u64::MAX)` (index-only, no
    body reads) sliced down to the tail `max_cached_msgs` entries *before* reading anything, then
    `body.read()` one at a time for just that slice. `BodyLog::all()` removed as now-dead code
    (its only caller).
  **Verification**: `cargo test -p truefix-store --test file_store_open_memory_bound` — 2/2 pass.
  Bisection: (1) temporarily restored the exact old `fs::read`-based `load_index` body — the
  `FileStore` test failed with concrete evidence (RSS grew ~51MB vs. an 8MB threshold); reverted.
  (2) For `CachedFileStore`, first tried skipping only the tail-slicing (`warm_from = 0`, but still
  reading one-at-a-time via `body.read()`) — this *still passed*, revealing that the one-at-a-time
  read loop (vs. bulk-collecting into a `Vec` first) is what actually bounds peak memory, not the
  tail-slice itself (a good disclosure: tail-slicing is a *complementary* optimization — avoiding
  reads that would immediately be evicted — not what the memory-bound test itself verifies).
  Properly bisected by reproducing the *exact* original BUG-81 shape (collect all bodies into a
  `Vec` first, then insert) — failed with concrete evidence (RSS grew ~59MB); reverted to the real
  fix, reran, passes. Full regression: `cargo test -p truefix-store --all-features` (17 test
  binaries) and `cargo test --workspace --all-features` (155 test-result blocks, 0 FAILED) — all
  green. `cargo fmt` applied.
- [X] T075 [P] [US2] Add a failing test: the multi-endpoint reconnecting initiator prefers the
  primary endpoint again after any successful connection, not continuing to rotate forward, in
  `crates/truefix-transport/tests/multi_endpoint_rotation_reset.rs` (new) (`BUG-51`, FR-042).
  **Completed**: three raw endpoints A (freed port, always refused)/B (real listener)/C (a
  "detector" that must never be contacted). The initiator connects, lands on B (A refused first),
  B's connection is force-dropped before the handshake completes, and the test confirms the next
  reconnect attempt lands on B *again* (via A, refused) rather than rotating forward to C. Needed a
  fast `reconnect_interval = 1` in the test's config — the default 5s interval meant the first
  attempt (`cargo test`'s initial 5s timeout) never even reached B in time.
- [X] T076 [US2] Reset `next_endpoint` to 0 on successful connect in
  `connect_initiator_reconnecting_multi` (`crates/truefix-transport/src/lib.rs`) — makes T075 pass
  (depends on T075).
  **Completed**: `next_endpoint = 0` added right after a successful connect, alongside the existing
  `backoff_step = 0` reset. **Also applied the identical fix to
  `connect_initiator_reconnecting_multi_tls`** (found to have the exact same defect by direct
  inspection while implementing this — same `next_endpoint` pattern, same missing reset) — this
  variant isn't independently covered by its own test (would require substantially more TLS
  connection-test infrastructure for a one-line, code-identical fix); its correctness rests on
  being byte-for-byte the same change as the plain variant's, which the test above does verify.
  **Verification**: `cargo test -p truefix-transport --test multi_endpoint_rotation_reset` — 1/1
  passes. Bisection: temporarily removed the new `next_endpoint = 0` line — failed with concrete
  evidence (endpoint C was contacted); reverted, reran, passes. Full regression: `cargo test -p
  truefix-transport --all-features` (36 test-result blocks, 0 FAILED) — all green. `cargo fmt`
  applied.
- [X] T077 [P] [US2] Add a failing test: `ReconnectHandle::stop()` tears down a currently-active
  connection, not only future reconnect attempts, in `crates/truefix-transport/tests/
  reconnect_handle_stop_mid_connection.rs` (new) (`BUG-50`, FR-043) (depends on T019, reuses
  `SessionHandle::abort`).
  **Completed**: a raw `TcpListener` accepts the initiator's connection and never replies, so the
  connection stays "active" (the reconnect loop is blocked awaiting it) until something intervenes.
  The test drains the initiator's already-buffered outbound Logon bytes first (needed so the
  subsequent read observes the connection's actual close, not leftover handshake data already in
  flight), calls `handle.stop()`, then asserts the peer socket observes a prompt close (EOF or
  reset) within 5s rather than staying open indefinitely.
- [X] T078 [US2] Give `ReconnectHandle` a way to reach its currently-active connection's handle (or
  equivalent) and call `.abort()` on `stop()` (`crates/truefix-transport/src/lib.rs`) — makes T077
  pass (depends on T077).
  **Completed** in two parts, the second discovered only after the first still left the test
  failing:
  1. Added a `current: Arc<std::sync::Mutex<Option<tokio::task::AbortHandle>>>` field to
     `ReconnectHandle`, populated in both `connect_initiator_reconnecting_multi` and
     `connect_initiator_reconnecting_multi_tls`'s reconnect loops: on each successful connect, the
     per-connection `run_connection` future is `tokio::spawn`ed (rather than run inline), its
     `AbortHandle` (cheaply `Clone`, via `JoinHandle::abort_handle()`) is stored in `current` under
     the mutex, the owned `JoinHandle` is then `.await`ed *directly* (not through the mutex — an
     earlier design that tried to hold the guard across the `.await` would have deadlocked
     `stop()`, which is sync and called from another task/thread), and `current` is cleared back to
     `None` once the connection ends. `stop()` now locks `current`, and if a connection is live,
     calls `.abort()` on it in addition to setting the existing `AtomicBool` flag.
  2. Even with (1), the test still failed (timed out): the peer's socket never observed a close.
     Root cause: `run_connection` internally `tokio::spawn`s a `read_loop` task with the connection's
     split `ReadHalf`; aborting `run_connection`'s own outer future does not touch this orphaned
     child task, and `tokio::io::split()`'s underlying stream only fully closes once *both* the
     `ReadHalf` and `WriteHalf` are dropped — so the still-running `read_loop` kept the `ReadHalf`
     (and thus the socket) alive indefinitely. Fixed by wrapping the reader `JoinHandle` in a small
     RAII guard, `struct AbortReaderOnDrop(JoinHandle<()>)` with `impl Drop` calling `self.0.abort()`,
     bound to a local `_read_task_guard` right after spawning `read_loop`. Rust runs `Drop` even when
     an async fn's own future is cancelled mid-poll (which is exactly what happens when the outer
     `run_connection` task is aborted), so this reliably tears down the reader on every exit path,
     including external abortion — not just normal return.
  **Verification**: `cargo test -p truefix-transport --test reconnect_handle_stop_mid_connection` —
  1/1 passes. Bisection (two independent guards, each confirmed necessary on its own):
  (a) wrapped `stop()`'s `handle.abort()` call in `if false { ... }` — test failed with concrete
  evidence (peer read timed out, connection never closed); reverted, reran, passes.
  (b) replaced `let _read_task_guard = AbortReaderOnDrop(read_task);` with
  `let _ = &read_task; // TEMP-BISECT` (i.e., no abort-on-drop) — test failed with the same timeout
  symptom even with fix (a) in place; reverted, reran, passes. `cargo fmt` applied (cosmetic changes
  only to the new test file). Full workspace regression: `cargo test --workspace --all-features`
  (`/tmp/full_test_run11.log`) — 157 test-result blocks, 0 FAILED.
- [X] T079 [P] [US2] Add a failing test: an acceptor refuses a connection whose first inbound
  message is not a Logon, in `crates/truefix-transport/tests/
  acceptor_first_message_must_be_logon.rs` (new) (`BUG-52`, FR-044).
  **Completed**: builds a standalone, validly-framed/checksummed Heartbeat (MsgType "0") by hand
  via `truefix_core::Message`, sends it as the very first bytes on a fresh raw `TcpStream` to an
  `AcceptorBuilder`-bound listener, and asserts the socket is promptly closed (EOF or reset within
  5s) rather than left open, and that no logon is ever counted.
- [X] T080 [US2] Add a `MsgType == "A"` check before routing in `route_and_run`
  (`crates/truefix-transport/src/lib.rs`) — makes T079 pass (depends on T079).
  **Completed**: in `route_and_run`'s initial framing loop, the `Some(Ok(msg)) => break msg` arm
  became `Some(Ok(msg)) if msg.msg_type() == Some("A") => break msg`, with the `_ => return` catch-
  all now also covering a decodable-but-non-Logon first message (previously only covered decode
  failure/EOF/short reads). This matches QFJ/QFGo, which both close the socket immediately if the
  first message isn't a Logon rather than attempting to route it.
  **Verification**: `cargo test -p truefix-transport --test acceptor_first_message_must_be_logon` —
  1/1 passes. Bisection: reverted the arm to unconditional `Some(Ok(msg)) => break msg` — failed
  with concrete evidence (`expected the connection to close, got 16 more bytes`, i.e. routing was
  attempted and something was written back); reverted, reran, passes. Full regression: `cargo test
  -p truefix-transport --all-features` (`/tmp/t079_080_transport.log`) — 38 test-result blocks, 0
  FAILED.
- [X] T081 [P] [US2] Add a failing test: the transport's internal admin-message channel applies
  bounded backpressure rather than growing without limit, in
  `crates/truefix-transport/tests/admin_channel_backpressure.rs` (new) (`BUG-53`, FR-045).
  **Completed**: mirrors `sync_writes.rs`'s outbound-backpressure test but for the inbound
  direction. An `Application::from_admin` lets the Logon through but then blocks forever
  (`std::future::pending`) on every subsequent admin message, simulating a wedged processing loop.
  The acceptor is configured with a small `recv_buffer_size` (2048). The test client then floods
  padded Heartbeats (~4KB each, via an oversized Text(58) field, same trick as `sync_writes.rs`'s
  `order()` helper) and asserts that at least one `write_all` eventually blocks past a short
  per-write timeout — proof that this side stopped draining the socket, which can only happen if
  the admin channel itself has a finite capacity.
- [X] T082 [US2] Change `read_loop`'s admin channel (`crates/truefix-transport/src/lib.rs`) from
  `mpsc::unbounded_channel()` to a bounded channel with a documented capacity — makes T081 pass
  (depends on T081).
  **Completed**: added `const ADMIN_CHANNEL_CAPACITY: usize = 256;` (not user-configurable, unlike
  `in_chan_capacity` — this is a memory-safety bound, not a behavior trade-off). Changed the admin
  channel's construction from `mpsc::unbounded_channel()` to `mpsc::channel(ADMIN_CHANNEL_CAPACITY)`,
  changed `admin_tx`'s type from `UnboundedSender<Inbound>` to `Sender<Inbound>` throughout
  (`read_loop`, `classify_buffered`), made `classify_buffered` itself `async` (it now `.await`s
  `admin_tx.send(...)`, which can genuinely block once the channel is full), and propagated
  `.await` to `classify_buffered`'s 3 call sites in `read_loop`. Updated the surrounding doc
  comments (previously described the admin channel as "unbounded — never blocks", now describing
  the bounded-with-generous-headroom design and its interaction with the pre-existing
  `pending_app`/`Sender::reserve()` staging mechanism for the *application* channel.
  **Sizing note**: initially chose `1024` as a "generous" capacity, but the T081 test (500 padded
  Heartbeats) passed too easily even *without* the fix applied on a first bisection attempt at that
  size for a different reason — 500 sent messages never even filled a 1024-capacity channel, so no
  backpressure could be observed regardless of boundedness. Reduced to `256`, which is still far
  more headroom than any real deployment's admin traffic pattern (driven by heartbeat intervals,
  not line-rate) would need, while keeping the test's flood size practical.
  **Verification**: `cargo test -p truefix-transport --test admin_channel_backpressure` — 1/1
  passes. Bisection: temporarily widened the channel to `1_000_000` (effectively unbounded) —
  failed with concrete evidence (500 padded Heartbeats all wrote through instantly, no blocked
  write observed); reverted to `256`, reran, passes. `cargo fmt` applied. Full workspace
  regression: `cargo test --workspace --all-features` (`/tmp/full_test_run12.log`) — 159
  test-result blocks, 0 FAILED.

**Checkpoint reached**: User Stories 1 AND 2 both work independently — `cargo test --workspace
--all-features` is green (159 test-result blocks, 0 FAILED, confirmed above).

---

## Phase 4: User Story 3 — Low-priority hygiene and defensive hardening (Priority: P3)

**Goal**: Close the long tail of low-probability edge cases, parsing-leniency gaps versus the
reference implementations, and codegen robustness issues.

**Independent Test**: Each item has its own targeted test exercising the specific edge case.

- [X] T083 [P] [US3] Add a failing test: frame-length arithmetic uses `checked_add` (or equivalent)
  so it cannot silently wrap even in principle, in `crates/truefix-core/tests/
  frame_checksum_overflow_hardening.rs` (new) (`BUG-23`, FR-032).
  **Completed**: genuinely overflowing `body_start + body_len + 7` would require a buffer many
  exabytes long (impossible to allocate in a test), so — consistent with this session's established
  pattern for changes whose triggering condition is unreachable via any real input — this is a
  source-inspection test: it reads `framing.rs`'s own source, isolates `frame_length`'s body, and
  asserts it contains `checked_add` and no longer contains the old bare `body_start + body_len + 7`
  expression.
- [X] T084 [US3] Use `checked_add` for the `body_start + body_len + 7` computation in
  `frame_length` (`crates/truefix-core/src/framing.rs`) — makes T084's own test pass (depends on
  T083).
  **Completed**: both `body_start`'s and `total`'s arithmetic now chain `.checked_add(...)` calls,
  returning `DecodeError::BodyLengthTooLarge` (reusing the existing hard-close error/handling path)
  on overflow instead of silently wrapping. `body_len` is already capped by `MAX_BODY_LEN` and
  `soh1`/`soh2_rel` are bounded by `buf.len()`, so this is unreachable via the transport path today
  (per the audit's own "二次核实" follow-up note); the fix is depth-in-defense against that ceasing
  to hold, not a currently-exploitable gap.
  **Verification**: `cargo test -p truefix-core --test frame_checksum_overflow_hardening` — 3/3
  pass. Bisection: reverted to the bare `+` form — `frame_length_uses_checked_add_not_bare_addition`
  failed with concrete evidence (source no longer contains `checked_add`); reverted, reran, passes.
- [X] T085 [P] [US3] Add a failing test: encoding a message whose byte length approaches `u32`
  checksum-accumulator overflow does not silently wrap to an incorrect checksum, in
  `crates/truefix-core/tests/frame_checksum_overflow_hardening.rs` (same file as T083) (`BUG-24`,
  FR-032).
  **Completed**: unlike T083, this overflow *is* directly reachable with a real, allocatable
  message: `encode()` has no `MAX_BODY_LEN`-style cap of its own (that cap only applies on the
  *decode* path, in `frame_length`), so a message with a ~17,000,000-byte Data field (tag 96) filled
  with `0xFE` bytes has a byte-value sum (~4.318 billion) that exceeds `u32::MAX` (~4.295 billion).
  The test builds exactly that message, calls `encode()` (expecting no panic), then verifies the
  embedded CheckSum against an independently-computed reference (`u8::wrapping_add` byte-by-byte —
  a different code path than the fix itself, not a tautological check). A second test round-trips
  the same message through `decode()` too, covering `decode.rs`'s identical construct.
- [X] T086 [US3] Use a wider (`u64`) or checked accumulator for the checksum sum in `encode.rs`
  (`crates/truefix-core/src/codec/encode.rs`) — makes T085 pass (depends on T085).
  **Completed**: changed `encode.rs`'s checksum sum from `.map(|&b| u32::from(b)).sum::<u32>()` to
  `.map(|&b| u64::from(b)).sum::<u64>()`, cast back to `u32` after the `& 0xFF` mask (`& 0xFF` gives
  an identical mod-256 result at either width, since both 2^32 and 2^64 are multiples of 256, so
  this is a pure overflow-safety fix with no behavior change for any message that previously encoded
  correctly). **Extended to `decode.rs`'s mirror-image construct too** (its own checksum-mismatch
  verification, at the same line the audit's BUG-24 finding named alongside `encode.rs`) for parity
  — `Message::decode` is a public API callable directly with arbitrary bytes, bypassing
  `frame_length`'s `MAX_BODY_LEN` cap entirely, so it has the identical reachable-overflow exposure.
  **Verification**: `cargo test -p truefix-core --test frame_checksum_overflow_hardening` — 3/3
  pass, in both `--release` and default (debug) profiles (the debug profile is where the original
  bug actually panics, since `Iterator::sum` overflow-checks in debug builds). Bisection (both
  `encode.rs` and `decode.rs` independently reverted): each reversion reproduced the exact panic the
  audit predicted — `attempt to add with overflow` at `core::iter::traits::accum` — with concrete
  evidence; both reverted, reran, pass. `cargo fmt -p truefix-core` applied. Full crate regression:
  `cargo test -p truefix-core --all-features` — all test binaries green, 0 failures.
- [X] T087 [P] [US3] Add a failing test: `HeartBtIntTimeoutMultiplier` preserves fractional
  precision and its use in timeout arithmetic cannot overflow for realistic heartbeat intervals, in
  `crates/truefix-session/tests/heartbeat_timeout_multiplier.rs` (new) (`BUG-36`, FR-033).
  **Completed**: 3 tests. (1) A `1.4` multiplier with `heartbeat_interval=1` must disconnect
  exactly on the 4th idle tick (threshold `1*1.4+2=3.4`) — not the 3rd (truncation to `1`) or 5th
  (rounding up to `2`) — a boundary only genuine fractional precision produces. (2) A `1.0`
  multiplier still behaves exactly as before (regression guard for the common case). (3) A
  pathologically large `heartbeat_interval`/multiplier (`1_000_000` each) must not panic.
- [X] T088 [US3] Change `heartbeat_timeout_multiplier`'s type to `f64` (config parsing + `on_tick`'s
  arithmetic, `crates/truefix-session/src/config.rs`, `crates/truefix-config/src/builder.rs`,
  `crates/truefix-session/src/state.rs`) and use saturating arithmetic — makes T087 pass (depends on
  T087).
  **Completed**: changed `SessionConfig::heartbeat_timeout_multiplier` from `u32` to `f64` (default
  `2.0`), switched `builder.rs`'s parsing from `u32_key` to the already-existing `f64_key` (used
  identically for the sibling `test_request_delay_multiplier` field), and rewrote `on_tick`'s
  `LoggedOn` timeout check to compute `f64::from(hb) * self.config.heartbeat_timeout_multiplier
  .max(1.0) + 2.0` and compare against `ticks_since_recv as f64` — `f64` saturates to infinity
  rather than panicking/wrapping on overflow, which is simpler and equally safe as the audit's
  suggested `saturating_mul`/`saturating_add` for this comparison-only use. Updated 2 pre-existing
  test files (`integrity.rs`, `config_switches.rs`) that assigned the field a bare integer literal
  (`1` → `1.0`) — a compile error under the new type, not a behavior change.
  **Verification**: `cargo test -p truefix-session --test heartbeat_timeout_multiplier` — 3/3 pass.
  Bisection: temporarily reverted `on_tick` to truncate the multiplier back to `u32` and use the old
  formula — both the fractional-precision test AND the large-value test failed with concrete
  evidence (disconnected one tick early at threshold 3 instead of 3.4; `attempt to multiply with
  overflow` panic at the old `hb * truncated_multiplier` expression); reverted, reran, passes.
  `cargo fmt -p truefix-session -p truefix-config` applied. Full regression: `cargo test -p
  truefix-session -p truefix-config --all-features` (`/tmp/t087_088_session.log`) — 38 test-result
  blocks, 0 FAILED.
- [X] T089 [P] [US3] Add a failing test: a repeating group's entry missing one of the group's own
  required fields, or a group-aware-decoded message, has its group-entry required-fields and
  field-types/enums recursively validated, in `crates/truefix-dict/tests/
  group_recursive_validation.rs` (new) (`BUG-54`/`55`, FR-034).
  **Completed**: 8 tests against a hand-authored dictionary source declaring a group with a
  required member via the new `req:` group-line syntax (see T090). Covers both reachable code
  paths: (a) BUG-54 — a flat, wire-order-decoded entry missing its required member is rejected
  (`RequiredTagMissing`); present, it's accepted; the check is skippable via
  `check_required_fields`. (b) BUG-55 — a message whose group is already `Member::Group`-structured
  (built directly via `Group`/`FieldMap`, the same shape `decode_with_groups` produces when called
  with a `GroupSpec` that covers body groups) has its entries' required fields *and* type/enum
  checks applied (previously silently skipped entirely for this shape, since
  `FieldMap::fields()`— what the old code walked — skips `Member::Group` members by design).
- [X] T090 [US3] Extend `validate()`/`validate_groups()` (`crates/truefix-dict/src/validate.rs`) to
  recurse into group entries for both required-field and type/enum checks — makes T089 pass (depends
  on T089).
  **Completed**, in two parts, since the underlying dictionary format had no group-level
  required-field concept at all prior to this task:
  1. **Format extension (BUG-54)**: added `GroupDef::required: Vec<u32>` (`model.rs`), and taught
     the DSL's `group` line to accept optional trailing `req:`/`opt:` tokens (mirroring `message`
     lines' existing syntax) in `parser.rs` — absent tokens leave `required` empty (no
     enforcement, identical to pre-existing behavior for not-yet-regenerated dictionaries, rather
     than guessing which members are required). Per Constitution Principle IV's dual-track
     design, `codegen.rs` has its own independent parser of the same text format; verified it
     already silently ignores unrecognized trailing tokens on `group` lines (it only consumes the
     first 4), so no `codegen.rs` change was needed for forward-compatibility. `validate_group`
     (the existing flat/wire-order walker) now tracks each entry's actually-seen tags and checks
     `gdef.required` against them once the entry ends, mirroring QFJ's `checkHasRequired`
     recursing into group entries.
  2. **Structural path (BUG-55)**: `validate_groups` now also iterates every group defined in the
     dictionary (`self.groups.keys()`) and, if `message.body.group(count_tag)` returns `Some`
     (i.e. this message already has that group `Member::Group`-structured), validates it via a new
     `validate_structured_group`/`present_in_structured_entry` pair — type/enum-checking each
     entry's fields via the existing `check_group_field_value`, checking `gdef.required`, and
     recursing into nested groups via the entry's own `FieldMap::group`. This runs *in addition to*
     (not instead of) the existing flat-walk path, since the production transport path never
     structures body groups (only the narrower `HeaderTrailerGroupsOnly` scope, per GAP-26/feature
     006) — the structural path only engages for a caller combining `decode_with_groups` with an
     unrestricted `GroupSpec` (`DataDictionary` itself implements the full trait), which is exactly
     the reachable gap BUG-55 named.
  **Disclosed narrowing**: the 9 bundled `dict-src/normalized/*.fixdict` files were **not**
  regenerated with real `Reqd`-derived group-required data in this task — that would require
  updating `fix_repository.rs`'s codegen tool to emit the new `req:`/`opt:` tokens from the
  `MsgContents.xml` data it already parses (`Reqd`), then re-running the CLI for all 9 versions and
  re-verifying the full AT suite — a separate, larger undertaking than this P3 hardening item's
  scope. Today's bundled dictionaries therefore still have empty `GroupDef::required` (zero
  behavior change for existing callers) until someone does that regeneration; the mechanism itself
  is proven correct via the hand-authored test dictionary above.
  **Verification**: `cargo test -p truefix-dict --test group_recursive_validation` — 8/8 pass.
  Bisection (both fixes independently): (a) disabled the `check_required_fields` loop inside
  `validate_group` — the flat-path test failed with concrete evidence (`unwrap_err() on an Ok
  value`); reverted, reran, passes. (b) disabled the new `self.groups.keys()` structural-group loop
  in `validate_groups` — all 3 structured-group tests failed with the same concrete evidence;
  reverted, reran, passes. `cargo fmt -p truefix-dict` applied. Full regression: `cargo test -p
  truefix-dict --all-features` (`/tmp/t089_090_dict.log`) — 28 test-result blocks, 0 FAILED
  (including the pre-existing `group_validation.rs`'s 12 tests, confirming no regression to the
  existing flat-group-validation behavior).
- [X] T091 [P] [US3] Add a failing test: a multi-character `CHAR` field on a `FIX.4.0`/`4.1` message
  is accepted, in `crates/truefix-dict/tests/char_field_version_leniency.rs` (new) (`BUG-62`,
  FR-035).
  **Completed**: 5 tests against the real bundled `FIX40`/`FIX41`/`FIX44` dictionaries, using two
  real fields that demonstrate the exact quirk: `Account`(1) is `CHAR` in `FIX40.fixdict`/
  `FIX41.fixdict` (multi-char value must be accepted on both) but `STRING` in `FIX44.fixdict`
  (can't show the "later versions still enforce" half); `CxlType`(125) is `CHAR` with no enum
  restriction in `FIX44.fixdict` (multi-char value must still be rejected there, and a
  single-character value still accepted on all three).
- [X] T092 [US3] Skip `CharConverter`-equivalent single-character validation for `FIX.4.0`/`4.1` in
  `FieldType::value_ok`'s `Self::Char` arm (`crates/truefix-dict/src/model.rs`) — makes T091 pass
  (depends on T091).
  **Completed**: `FieldType::value_ok` gained a `legacy_char_lenient: bool` parameter; its
  `Self::Char` arm now has a `if legacy_char_lenient` guard that accepts any valid-UTF-8 value
  (skipping the single-character check) ahead of the strict arm. `DataDictionary` gained a private
  `is_legacy_char_lenient()` helper, called at both of `value_ok`'s call sites in `validate.rs`
  (the top-level field loop and `check_group_field_value`).
  **Discovery mid-implementation**: the natural-seeming approach — checking `self.version_meta`
  (major/minor `Option<VersionMeta>`, already used by the neighboring GAP-32/FR-029 BeginString
  check) — compiled fine but the leniency never actually triggered for the real bundled
  dictionaries. Investigation found `version_meta` is only populated by a separate, optional
  `version-meta` directive that **no** bundled `.fixdict` file (of any of the 10 versions) actually
  declares — it's set only via `version FIX.4.0` (the plain `version` directive, stored as
  `DataDictionary.version: String`). Fixed by reusing the already-existing `parse_fix_begin_string`
  helper (previously only used by the version_meta check) against `self.version` instead — disclosed
  here since it means the pre-existing GAP-32/FR-029 BeginString-mismatch check is *also* silently
  inert for every dictionary shipped today (a pre-existing fact, not something this task introduced
  or is in scope to fix, but worth flagging for whoever next touches that check).
  Updated a dozen pre-existing call sites in `field_types_extended.rs` (mechanical `, false` added
  — a compile error under the new signature, not a behavior change).
  **Verification**: `cargo test -p truefix-dict --test char_field_version_leniency` — 5/5 pass.
  Bisection: added `if false &&` in front of the `legacy_char_lenient` guard — the two FIX.4.0/4.1
  multi-char tests failed with concrete evidence; reverted, reran, passes. `cargo fmt -p
  truefix-dict` applied. Full regression: `cargo test -p truefix-dict --all-features`
  (`/tmp/t091_092_dict.log`) — 29 test-result blocks, 0 FAILED (including
  `field_types_extended.rs`'s 18 tests, confirming the signature-change updates didn't alter any
  other type's behavior).
- [X] T093 [P] [US3] Add a failing test: `TimeStampPrecision=seconds` (lowercase) parses correctly;
  `SocketUseSSL=true` (not `Y`/`N`) is a typed config error rather than silently `false`, in
  `crates/truefix-config/tests/precision_and_bool_parsing.rs` (new) (`BUG-63`/`64`, FR-036).
  **Completed**: 8 tests. BUG-63: lowercase/mixed-case/uppercase `TimeStampPrecision` values all
  parse to the right variant; a genuinely unrecognized value is still a typed `InvalidValue` error.
  BUG-64: `SocketUseSSL=true` is `InvalidValue{key: "SocketUseSSL", ..}`, not a silent `tls: None`;
  `Y` still attempts TLS resolution (proven by getting a *different*, TLS-specific error —
  `MissingRequired` for the key store — rather than being short-circuited); `N` and absent both
  cleanly resolve to no TLS with no error.
- [X] T094 [US3] Make `precision_key` case-insensitive; make `bool_key` (or its Y/N-switch callers,
  starting with `SocketUseSSL`) reject a value that's neither a recognized affirmative nor negative
  form (`crates/truefix-config/src/builder.rs`) — makes T093 pass (depends on T093).
  **Completed**: `precision_key` now upper-cases the raw value before matching (mirroring
  `bool_key`'s existing `eq_ignore_ascii_case` case-insensitivity), so `seconds`/`Millis`/etc. all
  resolve correctly while a genuinely unknown value is still rejected.
  For BUG-64, added a **new** `strict_bool_key` function rather than converting `bool_key` itself
  (which would have required updating ~30 call sites and, more importantly, would have broken a
  deliberate design elsewhere): `resolve_lenient`'s own read of `ContinueInitializationOnError`
  (near the top of this file) is documented as intentionally using "a simple, never-failing boolean
  key" — that flag decides whether to *tolerate* a different key's failure in the same session, so
  making it fallible too would undermine the exact fallback path it exists for. `strict_bool_key`
  returns `Result<bool, ConfigError>`: absent → `default`; `Y`/`N` (case-insensitive) → `true`/
  `false`; anything else present → `InvalidValue`. Wired into `resolve_tls`'s `SocketUseSSL` check
  only (BUG-64's own named example) — every other `bool_key` call site is unchanged, a disclosed
  narrowing consistent with not wanting to silently change behavior for other keys not named by the
  audit finding.
  **Verification**: `cargo test -p truefix-config --test precision_and_bool_parsing` — 8/8 pass.
  Bisection (both fixes independently): (a) reverted `precision_key` to exact-case matching — the
  2 lowercase/mixed-case tests failed with concrete `InvalidValue` evidence; reverted, reran,
  passes. (b) reverted `resolve_tls` to call plain `bool_key` instead of `strict_bool_key` — the
  `SocketUseSSL=true` test failed with concrete evidence (resolved successfully with `tls: None`
  instead of erroring); reverted, reran, passes. `cargo fmt -p truefix-config` applied. Full
  regression: `cargo test -p truefix-config --all-features` (`/tmp/t093_094_config.log`) — 13
  test-result blocks, 0 FAILED.
- [X] T095 [P] [US3] Add a failing test: a resent message's refreshed `SendingTime` uses the
  session's configured timestamp precision, not hardcoded milliseconds, in
  `crates/truefix-session/tests/resend_timestamp_precision.rs` (new) (`BUG-66`, FR-037).
  **Completed**: 3 tests, each driving a real logon → send-app → inbound-ResendRequest → resend
  sequence (reusing `recovery.rs`'s established pattern) at Seconds/Milliseconds/Nanoseconds
  precision, then asserting the resent app message's refreshed SendingTime(52) has exactly the
  expected shape (byte length and fractional-digit count) for each.
- [X] T096 [US3] Use `now_utc_timestamp_prec(self.config.timestamp_precision)` in `prepare_resend`
  (`crates/truefix-session/src/admin.rs`) — makes T095 pass (depends on T095).
  **Completed**: `prepare_resend` gained a `config: &SessionConfig` parameter (its one call site,
  in `state.rs`'s `build_resend`, already has `self.config` in scope) and now calls
  `now_utc_timestamp_prec(config.timestamp_precision)` instead of the hardcoded-milliseconds
  `now_utc_timestamp()`. Since that was `now_utc_timestamp()`'s only production call site, it
  became dead code; removed it from `time_util.rs` along with the unused import in `admin.rs`,
  and repointed its own unit test (`format_has_expected_shape`) at
  `now_utc_timestamp_prec(TimeStampPrecision::Milliseconds)` instead (same coverage, no test
  content lost).
  **Verification**: `cargo test -p truefix-session --test resend_timestamp_precision` — 3/3 pass.
  Bisection: reverted to a hardcoded-milliseconds call (ignoring the new `config` parameter) — the
  Seconds and Nanoseconds tests failed with concrete evidence (both got a 21-char millisecond-shaped
  string regardless of configured precision); reverted, reran, passes. `cargo fmt -p
  truefix-session` applied. Full regression: `cargo test -p truefix-session --all-features`
  (`/tmp/t095_096_session.log`) — 27 test-result blocks, 0 FAILED.
- [X] T097 [P] [US3] Add a failing test: an adopted peer `HeartBtInt` value exceeding TrueFix's
  representable range is clamped, not silently truncated, in `crates/truefix-session/tests/
  heartbeat_int_clamping.rs` (new) (`BUG-67`, FR-038).
  **Completed**: since `Session` exposes no direct accessor for the adopted
  `heartbeat_interval` (config is private), this observes the effect indirectly through
  `on_tick`'s own send cadence (`ticks_since_send >= hb` triggers an outbound Heartbeat).
  `hbi = u32::MAX + 5` truncates via the old buggy `as u32` to exactly `4` (the low 32 bits of
  `0x1_0000_0004`) — a small, easily observable threshold within a handful of ticks — versus
  `u32::MAX` itself when correctly clamped, unreachable by any realistic tick count in a test. A
  second test confirms an in-range value is still adopted exactly (heartbeat fires right on
  schedule), not altered by the new clamping path.
- [X] T098 [US3] Clamp (not `as u32`-truncate) the adopted `HeartBtInt` in `on_logon`'s
  `Role::Acceptor` arm (`crates/truefix-session/src/state.rs`) — makes T097 pass (depends on T097).
  **Completed**: `self.config.heartbeat_interval = hbi as u32;` became
  `u32::try_from(hbi).unwrap_or(u32::MAX)` — `hbi` (already filtered `> 0`) clamps to `u32::MAX`
  when it doesn't fit, instead of wrapping to an arbitrary unrelated small value.
  **Verification**: `cargo test -p truefix-session --test heartbeat_int_clamping` — 2/2 pass.
  Bisection: reverted to the bare `hbi as u32` cast — the clamping test failed with concrete
  evidence (a heartbeat fired within 4 ticks, proving the truncated threshold of 4 rather than
  u32::MAX took effect); reverted, reran, passes. `cargo fmt -p truefix-session` applied. Full
  regression: `cargo test -p truefix-session --all-features` (`/tmp/t097_098_session.log`) — 28
  test-result blocks, 0 FAILED.
- [X] T099 [P] [US3] Add a failing test: `ResetOnError` is honored on the
  low-sequence-without-PossDup rejection path, matching the identity/latency-failure paths, in
  `crates/truefix-session/tests/reset_on_error_low_seq.rs` (new) (`BUG-68`, FR-039).
  **Completed**: 2 tests, reusing `config_switches.rs`'s established `has_reset_store`/
  `has_disconnect` pattern. With `ResetOnError=true`, a too-low-`MsgSeqNum` message without
  `PossDupFlag` must emit `Action::ResetStore` alongside the disconnect; without it, only the
  disconnect happens.
- [X] T100 [US3] Call `reset_on_error()`-equivalent handling on the low-seq-without-PossDup
  rejection path in `on_received` (`crates/truefix-session/src/state.rs`) — makes T099 pass (depends
  on T099).
  **Completed**: the `Ordering::Less` non-PossDup arm now builds its action vec the same way as
  every other hard-disconnect path in this function (latency/identity/validLogonState/dictionary-
  validation, all already calling `reset_on_error()`): `vec![self.send_stored(lo)]`, then
  `actions.extend(self.reset_on_error())`, then `actions.push(Action::Disconnect)` — previously
  this was the one path that skipped it, inconsistent when `ResetOnError` is configured.
  **Verification**: `cargo test -p truefix-session --test reset_on_error_low_seq` — 2/2 pass.
  Bisection: wrapped the `reset_on_error()` call in `if false { }` — the ResetOnError test failed
  with concrete evidence (no `Action::ResetStore` emitted); reverted, reran, passes. `cargo fmt -p
  truefix-session` applied. Full regression: `cargo test -p truefix-session --all-features`
  (`/tmp/t099_100_session.log`) — 29 test-result blocks, 0 FAILED.
- [X] T101 [P] [US3] Add a failing test: heartbeats continue to be sent while `AwaitingLogout` and
  its own timeout hasn't elapsed, in `crates/truefix-session/tests/
  heartbeat_during_awaiting_logout.rs` (new) (`BUG-69`, FR-040).
  **Completed**: 2 tests. With a long `logout_timeout` (30), after `StartLogout` a heartbeat must
  still be sent on the normal cadence before the timeout elapses (and the timeout itself must not
  fire prematurely); with a short `logout_timeout` (2), the timeout still disconnects as before
  (regression guard). Initial test-design bug caught during self-review: the acceptor adopts the
  peer's own Logon `HeartBtInt` as its effective `heartbeat_interval` (`on_logon`), overriding
  whatever `SessionConfig::heartbeat_interval` the test set directly — fixed by embedding the
  intended cadence in the test's own Logon message instead.
- [X] T102 [US3] Send heartbeats from the `AwaitingLogout` (not-yet-timed-out) arm of `on_tick`
  (`crates/truefix-session/src/state.rs`) — makes T101 pass (depends on T101).
  **Completed**: added a new `SessionState::AwaitingLogout => { ... }` arm (guard-less, so it only
  matches once the guarded timeout arm above it has already been tried and failed to match) that
  sends a heartbeat via the same `ticks_since_send >= hb` cadence as `LoggedOn` — previously this
  state fell through to the catch-all `_ => {}` and sent nothing.
  **Verification**: `cargo test -p truefix-session --test heartbeat_during_awaiting_logout` — 2/2
  pass. Bisection: removed the new arm (falling back to the catch-all) — the heartbeat-cadence test
  failed with concrete evidence (no heartbeat sent within the tick budget); reverted, reran, passes.
  `cargo fmt -p truefix-session` applied. Full regression: `cargo test -p truefix-session
  --all-features` (`/tmp/t101_102_session.log`) — 30 test-result blocks, 0 FAILED.
- [X] T103 [P] [US3] Add a failing test: a `ResendRequest`'s `EndSeqNo=999999` on a `FIX.4.2`-or
  -earlier session is treated as "resend through the highest sequence sent," in addition to the
  existing `EndSeqNo=0` convention, in `crates/truefix-session/tests/
  resend_request_infinity_version.rs` (new) (`BUG-97`, FR-041).
  **Completed**: the audit's own text initially cited "FIX ≤ 4.1" but its later 二次核实
  cross-check corrected this to "FIX ≤ 4.2" (matching QFJ's `manageGapFill`/QFGo's
  `sendResendRequest`), which is what this feature's own tasks.md already specifies — confirmed
  consistent before implementing. 3 tests, using `seed_sequences` to construct the one case where
  the existing `end_req > last` fallback does *not* already accidentally cover `999999`: a session
  that has (ostensibly) sent past 999,999 messages, so `999999 < last` and only a genuine
  version-aware infinity check can make `EndSeqNo=999999` resolve correctly rather than silently
  no-op (`begin > end`). Covers FIX.4.0/4.1/4.2 (infinity) and FIX.4.4 (still a literal endpoint,
  regression guard).
- [X] T104 [US3] Extend `on_resend_request`'s infinity-detection (`crates/truefix-session/src/
  state.rs`) to also treat `EndSeqNo=999999` as infinity on `FIX.4.2`-and-earlier sessions — makes
  T103 pass (depends on T103).
  **Completed**: added `is_legacy_infinity_convention` (checks `end_req == 999_999` and the
  session's own `begin_string` parses as FIX major<4, or major==4 with minor<=2), OR'd into the
  existing `end_req == 0 || end_req > last` condition. Reused `admin::fix_version` (previously
  private to `admin.rs`, used by the RefMsgType/SessionRejectReason version-gating from BUG-58/59)
  by promoting it to `pub(crate)`, rather than duplicating the BeginString-parsing logic.
  **Verification**: `cargo test -p truefix-session --test resend_request_infinity_version` — 3/3
  pass. Bisection: hardcoded `is_legacy_infinity_convention = false` — both the FIX.4.2 and
  FIX.4.0/4.1 tests failed with concrete evidence (empty actions, i.e. silent no-op); reverted,
  reran, passes. `cargo fmt -p truefix-session` applied. Full regression: `cargo test -p
  truefix-session --all-features` (`/tmp/t103_104_session.log`) — 31 test-result blocks, 0 FAILED.
- [X] T105 [P] [US3] Add a failing test: a fractional-seconds digit count other than 0/3/6/9 is
  rejected; a leap second (`ss=60`) maps to 59 seconds plus the maximum sub-second fraction, in
  `crates/truefix-core/tests/timestamp_leniency.rs` (new) (`BUG-48`/`78`, FR-046).
  **Completed**: 8 tests covering both `parse_utc_timestamp` and `parse_utc_time_only`: every
  digit count 1-11 except 3/6/9 is exhaustively rejected; 0/3/6/9 are all accepted; a leap second
  maps to 59s + 999_999_999ns (with or without an explicit fraction of its own, which is
  superseded); a genuinely invalid second (61) is still rejected. Also updated 2 pre-existing
  tests in `field_types.rs` that exercised the *old* lenient behavior (a 1-digit fraction and
  12-digit/picosecond truncation, both previously accepted) — these are now expected rejections
  under the new stricter rule, a deliberate behavior change disclosed here, not an incidental
  regression.
- [X] T106 [US3] Add digit-count validation and leap-second mapping to `parse_utc_timestamp`/
  `parse_utc_time_only` (`crates/truefix-core/src/field.rs`) — makes T105 pass (depends on T105).
  **Completed**: both functions' fractional-digit match arm now requires `matches!(f.len(), 3 | 6 |
  9)` (previously any non-empty all-digit string), rejecting anything else via a new `Some(_) =>
  return None` arm. Both functions also now map `sec == 60` to `(59, 999_999_999)` before calling
  `time::Time::from_hms_nano` (which otherwise rejects 60 outright), matching QFJ's explicit
  leap-second handling. TrueFix's own `TimeStampPrecision` enum only goes up to nanoseconds (9
  digits) in the first place, so rejecting would-be picosecond (12-digit) input isn't a capability
  loss.
  **Verification**: `cargo test -p truefix-core --test timestamp_leniency` — 8/8 pass. Bisection
  (4 independent guards, each confirmed necessary): (a) widened `parse_utc_timestamp`'s digit
  check back to "any non-empty digit string" — the exhaustive-rejection test failed with concrete
  evidence; (b) same for `parse_utc_time_only` — failed identically; (c) disabled
  `parse_utc_timestamp`'s leap-second mapping (`if false && sec == 60`) — both leap-second tests
  failed with concrete `NotTimestamp` errors; (d) same for `parse_utc_time_only` — failed
  identically. All 4 reverted individually, reran, pass. `cargo fmt -p truefix-core` applied. Full
  regression: `cargo test -p truefix-core --all-features` (`/tmp/t105_106_core.log`) — 18
  test-result blocks, 0 FAILED.
- [X] T107 [P] [US3] Add a failing test: `frame_length` confirms the bytes at the computed checksum
  position actually resemble a checksum field before accepting the frame, in
  `crates/truefix-core/tests/checksum_position_verification.rs` (new) (`BUG-46`, FR-047).
  **Completed**: 3 tests. A correctly-declared `BodyLength` frames normally (regression guard).
  A `BodyLength` declaring too few bytes (landing `total` mid-field, not on `10=`) is rejected. A
  `BodyLength` declaring too many bytes (landing `total` on trailing garbage past the real
  checksum) is also rejected — both prove the new position check fires rather than trusting a
  numerically-valid-but-wrong `BodyLength`.
- [X] T108 [US3] Add the checksum-position verification to `frame_length`
  (`crates/truefix-core/src/framing.rs`) — makes T107 pass (depends on T107).
  **Completed**: once `buf.len() >= total`, checks `buf.get(total-7..total-4) == Some(b"10=")`
  before returning `Ok(Some(total))`; on mismatch, returns `Err(DecodeError::InvalidBodyLength)`.
  This error variant isn't given special hard-close treatment in `truefix-transport`'s
  `classify_buffered` (unlike `BodyLengthTooLarge`/`ZeroBodyLength`), so it falls into the existing
  generic malformed-frame recovery there — skip-and-resync past the bad prefix — matching QFJ's
  own "verify then rescan on mismatch" behavior, with no new recovery logic needed in this task.
  **Verification**: `cargo test -p truefix-core --test checksum_position_verification` — 3/3 pass.
  Bisection: wrapped the position check in `if false && ...` — both malformed-BodyLength tests
  failed with concrete evidence (frame accepted despite not landing on real `10=` bytes); reverted,
  reran, passes. `cargo fmt -p truefix-core` applied. Given `frame_length` is a widely-shared
  primitive underlying every inbound message across the whole transport/AT-suite, ran a **full
  workspace regression** rather than just the one crate: `cargo test --workspace --all-features`
  (`/tmp/t107_108_full.log`) — 171 test-result blocks, 0 FAILED (including the full 9-version AT
  conformance/scenario suite).
- [X] T109 [P] [US3] Add a failing test: a buffer whose leading bytes don't form a recognizable
  `BeginString` field (`FIX.\d\.\d`/`FIXT.\d\.\d`) is not treated as the start of a frame, in
  `crates/truefix-core/tests/begin_string_format.rs` (new) (`BUG-79`, FR-048).
  **Completed**: 3 tests. Every real bundled version (`FIX.4.0`-`FIX.5.0SP2`, `FIXT.1.1`) frames
  normally. Non-FIX data after `8=` (e.g. `8=NOTFIX`) is rejected, with a genuinely-correct
  `BodyLength`/trailer so the test isolates the BeginString check rather than incidentally
  re-testing BUG-46's already-fixed checksum-position check. A handful of garbled BeginString
  shapes (`FIX`, `FIX.`, `FIX.44`, `FIXX.4.4`, `4.4.4`) are all rejected.
- [X] T110 [US3] Extend the `BeginString` check in `frame_length`
  (`crates/truefix-core/src/framing.rs`) beyond the leading `8=` to the full version pattern —
  makes T109 pass (depends on T109).
  **Completed**: added `looks_like_fix_begin_string`, checking the BeginString *value* (bytes
  between `8=` and the first SOH) against `FIX.<digit>.<digit>`/`FIXT.<digit>.<digit>` (matching
  QFJ's own pattern — only the first 3 characters after the prefix are checked; a trailing suffix
  like `SP2` is accepted as-is). Called right after the existing leading-`8=`-prefix check.
  **Discovery mid-implementation, fixed in the same task**: this broke 42 of 424 AT scenario-runs
  (`server_acceptance_suite_passes`) — every `FIX.Latest`-targeted scenario failed at the Logon
  step. Root cause: `SUITE_VERSIONS`' `"FIX.Latest"` entry (and the bundled `FIXLATEST.fixdict`'s
  own `version` line) is a version-agnostic-testing *label*, never a real wire `BeginString` —
  `truefix-at`'s `client_message`/`start_acceptor` were passing it through literally onto the wire
  and into the acceptor's own `SessionConfig::new`, which "worked" only because both sides used the
  identical non-standard string for exact-match comparison, never validating its format. Fixed by
  adding `truefix_at::runner::wire_begin_string`, translating `"FIX.Latest"` to a real, well-formed
  `"FIX.5.0SP2"` at the 3 places actual wire bytes/session config get constructed
  (`client_message`, `start_acceptor`'s `SessionConfig::new`, and one direct `Field::string(8, ..)`
  construction in `scenarios.rs`'s `missing_msg_seq_num`) — every other `SUITE_VERSIONS` entry is
  untouched (already real, valid version strings).
  **Verification**: `cargo test -p truefix-core --test begin_string_format` — 3/3 pass. Bisection:
  wrapped the new check in `if false && ...` — the two malformed-BeginString tests failed with
  concrete evidence; reverted, reran, passes (also re-verified `checksum_position_verification.rs`/
  `framing_bounds.rs` still pass, confirming no interaction with the earlier BUG-46 fix).
  `cargo test -p truefix-at --test conformance --test coverage` — 4/4 + 3/3 pass post AT-harness
  fix (424/424 scenario-runs, confirmed via the coverage test's own regression-floor check).
  `cargo fmt -p truefix-core -p truefix-at` applied. Given `frame_length` is shared across the
  whole transport/AT suite, ran a **second full workspace regression** after the AT-harness fix:
  `cargo test --workspace --all-features` (`/tmp/t109_110_full2.log`) — 172 test-result blocks, 0
  FAILED.
- [X] T111 [P] [US3] Add a failing test: a decoded message with no `MsgType` field is rejected, in
  `crates/truefix-core/tests/decode_requires_msg_type.rs` (new) (`BUG-80`, FR-049).
  **Completed**: 3 tests. A message with no MsgType(35) anywhere (e.g. a minimal
  BeginString/BodyLength/CheckSum-only frame) is rejected. A well-formed message with MsgType at
  its normal position still decodes. A message with MsgType *present but out of its normal
  third-field position* still decodes (with `fields_out_of_order() == true`) — a pre-existing,
  separately-handled concern (`field_order.rs`'s `third_field_not_msg_type_is_out_of_order`) that
  must not regress into an outright rejection.
- [X] T112 [US3] Require field 35 (`MsgType`) in `tokenize_validated`
  (`crates/truefix-core/src/codec/decode.rs`), alongside the existing BeginString/BodyLength
  requirement — makes T111 pass (depends on T111).
  **Completed**: added a new `DecodeError::MissingMsgType` variant, and a presence check (`!fields
  .iter().any(|f| f.0 == MSG_TYPE)`) in `tokenize_validated`, shared by both `decode()` and
  `decode_with_groups()`. **Deliberately checks presence, not position** — an initial
  position-based implementation (`fields.get(2)` must be tag 35) broke the pre-existing
  `third_field_not_msg_type_is_out_of_order` test, which correctly expects a MsgType-present-but-
  misplaced message to still decode (flagged via `fields_out_of_order`, not rejected outright) —
  caught by running the full crate's existing tests before writing the new one, then corrected to
  a presence check, which satisfies BUG-80's actual concern (MsgType missing *entirely*) without
  re-litigating the separate, already-correct position/ordering concern.
  **Verification**: `cargo test -p truefix-core --test decode_requires_msg_type` — 3/3 pass; full
  crate regression (`cargo test -p truefix-core --all-features`) confirmed no other test broke.
  Bisection: wrapped the presence check in `if false && ...` — the missing-MsgType test failed with
  concrete evidence; reverted, reran, passes. `cargo fmt -p truefix-core` applied. Given `decode`/
  `tokenize_validated` is a widely-shared primitive, ran a full workspace regression: `cargo test
  --workspace --all-features` (`/tmp/t111_112_full.log`) — 173 test-result blocks, 0 FAILED
  (including the full AT conformance suite, `server_acceptance_suite_passes` confirmed green).
- [X] T113 [P] [US3] Add a failing test: the dictionary codegen path (`--features dict-tooling`)
  produces compiling output for a field type outside its explicit type table (e.g. `TIME`,
  `PRICEOFFSET`), an enum-value label colliding with a Rust keyword, and a component/message list
  using the `open` enum modifier — matching the runtime parser's existing correct handling of each,
  in `crates/truefix-dict/tests/codegen_hardening.rs` (new, `--features dict-tooling`) (`BUG-72`/
  `73`/`74`, FR-050).
  **Completed**: 3 tests against a hand-authored dict source (mirroring the existing
  `field_order_production.rs`'s established `truefix_dict::codegen::generate("NAME", src.as_bytes
  ())` pattern). (1) A `TIME` field and a `PRICEOFFSET` field both get the same typed accessor as
  `UTCTIMESTAMP`/`FLOAT` respectively in the generated text, not the generic `&str` fallback. (2)
  An `OrdType` field with enum labels `Type`/`Self` gets sanitized `VType`/`VSelf` variants, not
  bare (uncompilable, for `Self`) ones. (3) **Honest disclosure**: the `open`-modifier fix has *no
  observable effect* on `generate()`'s emitted text for any input — `emit_field_enum` (confirmed by
  inspection to be the only consumer of `FieldDef::values` anywhere in `codegen.rs`) already
  filters to labeled entries only, silently dropping any unlabeled `("open", None)` pseudo-value
  regardless of whether it was stripped first. This is therefore a source-inspection test
  (confirming `parse_dict`'s `"field"` arm strips a leading `open` token), mirroring this session's
  established pattern for hardening whose trigger condition isn't observable via the available
  test surface (e.g. `frame_checksum_overflow_hardening.rs`'s `checked_add` source check).
- [X] T114 [US3] Extend `codegen.rs`'s `type_mapping` (`TIME`/`PRICEOFFSET` and any other
  still-missing explicit types), `sanitize_variant` (Rust-keyword collision handling), and
  `parse_dict` (recognize and strip the `open` modifier, matching `parser.rs`) in
  `crates/truefix-dict/src/codegen.rs` — makes T113 pass (depends on T113).
  **Completed**: (1) `type_mapping` now maps `PRICEOFFSET` alongside `FLOAT`/`PRICE`/`QTY`/`AMT`/
  `PERCENTAGE` (decimal), and `TIME` alongside `UTCTIMESTAMP` (both share `model.rs`'s runtime
  grouping already). (2) `sanitize_variant` now also checks the label case-insensitively against a
  new `RUST_KEYWORDS` list (2018+ strict and reserved keywords), reusing the same `V`-prefix
  strategy as the existing leading-digit case. (3) `parse_dict`'s `"field"` match arm now collects
  remaining tokens into a `Vec` and strips a leading `"open"` token before building `values`,
  mirroring `parser.rs`'s `open_enum` detection — codegen doesn't need the `open_enum` boolean
  itself (no enum-membership-checking codegen exists), just to not corrupt the parsed value list.
  **Verification**: `cargo test -p truefix-dict --test codegen_hardening --features dict-tooling`
  — 3/3 pass. Bisection (all 3 independently): (a) removed `PRICEOFFSET` from the decimal arm —
  the type-mapping test failed with concrete evidence (both fields fell back to `&str`); (b)
  disabled the keyword check (`false && ...`) — the keyword test failed with concrete evidence
  (bare `Type`/`Self` variants emitted); (c) disabled the `open`-stripping — the source-inspection
  test failed with concrete evidence (source reverted to the old unconditional-collect form). All
  3 reverted individually, reran, pass. `cargo fmt -p truefix-dict` applied. Full regression:
  `cargo test -p truefix-dict --all-features` (`/tmp/t113_114_dict.log`) — 30 test-result blocks,
  0 FAILED.

**Checkpoint reached**: all three user stories (US1/US2/US3, T001-T114) are independently
functional.

**Checkpoint**: All three user stories independently functional — `cargo test --workspace
--all-features` green.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Full-gate sweep across all three stories.

- [X] T115 [P] Run `cargo fmt --check` and `cargo clippy --workspace --all-targets --all-features --
  -D warnings`; fix any residual warnings introduced by this feature.
  **Completed**. `cargo fmt --check` was already clean (every fix this feature made had `cargo
  fmt` applied at the time). `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  found 5 residual issues, all introduced by this feature's own changes, fixed one crate at a time
  with a full re-run after each:
  1. `truefix-store/src/file.rs`'s `load_index` (BUG-45, this feature): `header[0..8].try_into()
     .unwrap()`/`header[8..12]...unwrap()` violated `clippy::unwrap_used` (denied crate-wide, per
     Constitution Principle I). Fixed by reading the two header fields into their own
     exactly-sized `[u8; 8]`/`[u8; 4]` buffers via two `read_exact` calls instead of one 12-byte
     buffer sliced afterward — needs no fallible slice-to-array conversion at all.
  2. Same file, unrelated pre-existing line: `write!(f, "{value}\n")` → `writeln!(f, "{value}")`
     (`clippy::write_with_newline`).
  3. Same file, unrelated pre-existing line: `&all_seqs[warm_from..]` (from this feature's BUG-81
     fix) violated `clippy::indexing_slicing` — `warm_from` is computed via `saturating_sub` so
     can't actually panic, but the lint doesn't do that analysis; changed to
     `all_seqs.get(warm_from..).unwrap_or_default()`.
  4. `truefix-core/src/framing.rs`'s `looks_like_fix_begin_string` (T110, this feature):
     `bytes[0]`/`bytes[1]`/`bytes[2]` violated `clippy::indexing_slicing` despite the preceding
     `bytes.len() >= 3` guard (again, a check the lint doesn't statically correlate). Rewritten as
     `matches!(bytes.get(0..3), Some([d1, b'.', d2]) if ...)` — a single fallible slice access with
     pattern-matched destructuring, no indexing.
  5. `truefix-session/src/state.rs`'s `on_tick` (T101/T102, this feature): the guard-less
     `SessionState::AwaitingLogout => { if self.ticks_since_send >= hb { ... } }` arm violated
     `clippy::collapsible_match`; collapsed into `SessionState::AwaitingLogout if self
     .ticks_since_send >= hb => { ... }` (behaviorally identical — the guard failing still falls
     through to the same `_ => {}` catch-all either way).
     Also one pre-existing test-only issue: `truefix-session/tests/valid_logon_state.rs`'s
     `logout_text.iter().any(|t| *t == "...")` → `logout_text.contains(&"...")`
     (`clippy::manual_contains`).
  **Verification**: after each fix, re-ran the affected crate's own test suite (or the specific
  test file the fix's task originally added) to confirm no behavior change — `truefix-store`
  (all-features, full crate), `truefix-core` (`begin_string_format.rs`), `truefix-session`
  (`heartbeat_during_awaiting_logout.rs`, `valid_logon_state.rs`) — all green. Final
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` run: clean, zero
  warnings/errors. `cargo fmt --check`: clean (no diff).
- [X] T116 Run `cargo test --workspace --all-features` and `cargo test -p truefix-at --test
  conformance`/`--test coverage`; confirm zero regressions against T001's baseline (405 AT
  scenario-runs at minimum), and bump the regression floor if T015/T018/T036/T046 landed new AT
  scenarios — disclose the exact before/after numbers.
  **Completed**. `cargo test --workspace --all-features` — 174 test-result blocks, 0 FAILED (run
  after T115's clippy fixes, to confirm those production-code changes in `truefix-store`/
  `truefix-core`/`truefix-session` introduced no regressions). `cargo test -p truefix-at --test
  conformance -- --nocapture` prints `AT report: 424/424 scenario runs passed` — comfortably above
  the 405-run floor `coverage.rs`'s `server_suite_scenario_run_count_does_not_regress` asserted at
  the start of this feature. **Bumped the regression floor from 405 to 424** (the actual, verified
  current count) in that same test, with an updated message extending the existing historical trail
  (373 at 005 closeout → 403 at 006 US1 → 405 at 006 US9 → **424 at 007 Polish/T116**) — the +19
  runs come from new scenarios this feature added across its US1/US2 stories (admin-message-
  dictionary-validation, schedule-enforcement, and several ResetSeqNumFlag/resend/reject-
  correctness scenarios, among others). `cargo test -p truefix-at --test coverage` — 3/3 pass
  against the new floor. `cargo fmt -p truefix-at` applied.
- [X] T117 Run `cargo deny check` — no new dependency was introduced anywhere in this feature
  (plan.md's Technical Context), so this should be a no-op confirmation, not a new check to satisfy.
  **Completed**: `cargo deny check` exits 0 — `advisories ok, bans ok, licenses ok, sources ok`.
  The run prints ~21 `warning[duplicate]` lines (multiple versions of `base64`/`rustls`/
  `socket2`/`tokio-rustls`/`windows-sys`/etc. in the dependency graph) — these are pre-existing,
  transitive version splits (e.g. `mongodb`'s own dependency tree pinning a newer `rustls`/
  `socket2`/`tokio-rustls` than the rest of the workspace), governed by `deny.toml`'s own
  deliberate, pre-existing `multiple-versions = "warn"` policy (not a hard failure by design);
  confirmed a plain redirected run exits `0` (an earlier piped `grep` check had mis-attributed a
  `grep`-exit-status false positive, corrected here). No new dependency, advisory-ignore, or
  license-allowance entries were needed — confirms plan.md's Technical Context claim that this
  feature introduces no new external dependencies.
- [X] T118 Run `quickstart.md`'s full validation-command list end-to-end; correct any test-file-name
  mismatches discovered during implementation (per quickstart.md's own note — every command in it is
  a placeholder as of `/speckit-plan` time).
  **Completed**: checked all ~48 referenced test files for on-disk existence (Python regex
  extraction + existence check); found exactly one mismatch — `cargo test -p truefix-transport
  --test scheduled_retry_backoff` was a `/speckit-plan`-time placeholder name that never
  corresponded to a real file (BUG-94's 200ms-backoff test landed as an added test *inside*
  US1's own `scheduled_reconnect_after_drop.rs` instead). Replaced with a comment explaining this
  plus the correct command
  (`cargo test -p truefix-transport --test scheduled_reconnect_after_drop -- \
  failed_connect_attempts_back_off`), and confirmed that exact command passes (1 passed; 0
  failed). Updated the Prerequisites section with the current (007 Polish closeout) baseline —
  174 test-result blocks / 716 tests passing, AT suite 424/424 — alongside the original
  `/speckit-plan`-time baseline (118 binaries / 578 tests, 405/405 AT), and updated US1's
  "Expected" paragraph to state the final 424 AT scenario-run count instead of "TBD". Ran the
  "Full regression pass" section's full command sequence end-to-end as a final confirmation:
  `cargo fmt --check` (clean), `cargo clippy --workspace --all-targets --all-features -- -D
  warnings` (clean, 0 warnings), `cargo test --workspace --all-features` (174 test-result
  blocks, 0 FAILED, exit 0 — `/tmp/t118_full.log`), `cargo test -p truefix-at --test coverage`
  (3/3 passed, exit 0), `cargo test -p truefix-at --test conformance` (4/4 passed, exit 0),
  `cargo deny check` (exit 0 — `advisories ok, bans ok, licenses ok, sources ok`). Zero
  regressions found.
- [X] T119 Record a completion disclosure in `docs/acceptance-record.md` (mirroring 006's own "006 —
  Audit remediation" section and its "Not addressed by 006" honesty pattern), marking each `BUG-NN`
  item this feature closed, referencing `docs/todo/004.md`, and explicitly listing anything raised
  by `004.md` that this feature did not close (cross-check against spec.md's own Assumptions
  exclusion list — `BUG-37`/`98`/`101`/`104`/`106`/`108`/`110`/`111`/`65`/`73`-partial and the
  Exchange-case retraction) so no future reader assumes this feature closed the *entire* document
  when several items were deliberately, disclosedly out of scope.
  **Completed**: added a "007 — Second audit remediation" section to `docs/acceptance-record.md`
  (mirroring 006's structure), organized by US1 (store durability/crash-safety, handshake/deadlock
  fixes, resource-leak/lifecycle fixes, malformed-input/enforcement gaps, callback-ordering
  restructure), US2 (16 narrower-blast-radius defects), and US3 (20 low-priority hardening items),
  each referencing its FR-IDs; disclosed the AT-suite floor growth (405→424) and the
  `wire_begin_string` cross-cutting fix; recorded the gate status (174 test-result blocks/716 tests,
  424/424 AT, clean fmt/clippy/deny). Added a "Not addressed by 007" section covering every
  `spec.md` Assumptions-section exclusion (`BUG-37`/`98`/`101` — already fixed by 006; the
  Exchange-case retraction; `BUG-108` — intentional default; `BUG-104`/`110`/`111` — design
  choices/documented no-ops; `BUG-106` — duplicate of `BUG-83`; `BUG-65` — negligible-risk overflow)
  plus one disclosed discrepancy found while cross-checking: `BUG-73` is listed in `spec.md`'s
  Assumptions as excluded/negligible-risk, but tasks.md's own T113/T114 (`FR-050`) in fact bundled
  and closed it alongside the in-scope `BUG-72`/`BUG-74` codegen fixes it shares a code path with —
  disclosed as closed-despite-being-listed-deferred (more-thorough direction, not a gap), not
  silently reconciled by rewriting either document. Also updated the pre-existing "Outstanding
  before a v1 release claim" section with 007's own remaining deferrals and reconfirmed the
  `GAP-28`/`GAP-32` version-metadata inertness is still live (independently re-flagged by 007's own
  T091/T092).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **User Stories (Phases 2-4)**: All depend only on Setup. Every story is independently
  implementable/testable in any order; the only sequencing note is *within* US1 itself (T013/T019
  before the tasks that consume them — already the natural order above).
- **Polish (Phase 5)**: Depends on all three user stories being complete.

### Parallel Opportunities

- Every `[P]`-marked task within a story operates on a distinct new test file with no dependency on
  an incomplete task — these can run in parallel.
- Once Setup completes, all three user stories can be worked on in parallel by different
  contributors (no cross-story file conflicts — verified against plan.md's Project Structure: US1
  touches `truefix-store`/`truefix-session`/`truefix-transport`/`truefix-core`/`truefix`; US2/US3
  touch mostly-disjoint functions within the same crates, with the shared files noted below).
- **Same-file caution** (not a hard block, but sequence within-file edits to avoid merge conflicts):
  `crates/truefix-session/src/state.rs` is touched by the large majority of tasks across all three
  stories (it's the session state machine, the natural home for most of `004.md`'s findings) — treat
  same-file tasks as sequential even when marked `[P]` for cross-story independence, or rebase
  frequently if working genuinely in parallel.

---

## Parallel Example: User Story 1

```bash
# Launch the sequence-number-store test-writing tasks together (distinct new test files):
Task: "T002: crash-safety test in crates/truefix-store/tests/seqfile_crash_safety.rs"
Task: "T003: corrupted-file test in crates/truefix-store/tests/seqfile_crash_safety.rs"
Task: "T004: migration test in crates/truefix-store/tests/seqfile_migration.rs"

# Launch every US1 sub-theme's first failing-test task together (all distinct new files):
Task: "T007: SqlStore migration-order test"
Task: "T009: MssqlStore rollback test"
Task: "T011/T012: ResetSeqNumFlag handshake tests"
Task: "T016: ResendRequest deadlock test"
Task: "T027: duplicate-connection test"
Task: "T029: disconnect-event test"
Task: "T032: callback-ordering test"
Task: "T034: admin-dictionary test"
Task: "T037: BodyLength=0 test"
Task: "T040/T041/T044: schedule-enforcement tests"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: User Story 1 (every must-fix-before-shipping item).
3. **STOP and VALIDATE**: `cargo test --workspace --all-features` and `cargo test -p truefix-at
   --test conformance` both green; spec SC-001-SC-006 all exercised.
4. This alone closes every 🔴 "must fix before shipping" item `docs/todo/004.md` identified —
   arguably the more meaningful "ship-readiness" milestone for this feature than a literal
   User-Story-1-only release, since US2/US3 are explicitly lower-priority by the audit's own triage.

### Incremental Delivery

1. Complete Setup → Foundation ready (no dedicated Foundational phase needed — see Phase 1's
   checkpoint note).
2. Add User Story 1 → Test independently → this is the meaningful "production-ready" checkpoint.
3. Add User Story 2 → Test independently.
4. Add User Story 3 → Test independently → the full `docs/todo/004.md` document is now closed
   (modulo spec.md's Assumptions' disclosed exclusions).

---

## Notes

- `[P]` tasks = different files, no dependencies (see the same-file caution above for
  `truefix-session/src/state.rs` specifically).
- `[Story]` label maps task to specific user story for traceability.
- Every implementation task's own test task is listed immediately before it and is expected to FAIL
  before that implementation task runs (test-first, Constitution Principle V).
- Commit after each task or logical group (per this repository's own git conventions — only when the
  user explicitly asks for a commit, per this session's standing instructions).
- Stop at either checkpoint (end of Phase 2, end of Phase 4) to validate independently.
