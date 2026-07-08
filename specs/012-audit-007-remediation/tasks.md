---

description: "Task list for implementation plan"
---

# Tasks: Audit 007 Remediation

**Input**: Design documents from `/specs/012-audit-007-remediation/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md,
data-model.md, contracts/public-api-changes.md, quickstart.md

**Tests**: Included and REQUIRED — `spec.md`'s SC-005 explicitly requires every accepted finding
(`NEW-156`/`NEW-157`/`NEW-158`) to have at least one automated test that fails before the
corresponding fix and passes after it, and Constitution Principle V mandates test-first table-driven
and integration coverage for this crate.

**Organization**: Tasks are grouped by user story (matching `spec.md`'s Story 1/2/3, priority
P1/P2/P3) to enable independent implementation and testing of each story.

**Revision note**: T007, T023, and T044 were added by a `/speckit-analyze` remediation pass to close
two gaps found in the original task list: `FR-008` (acceptor/initiator parity) had no test for User
Story 1/2 (only User Story 3 had one), and `SC-004` had no task that actually exercises the
documented custom-backend example under automated test. All task IDs from the original list shift by
the corresponding amount from this point on; see the git history of this file for the prior
numbering if needed.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- File paths are exact and relative to the repository root

## Path Conventions

Existing Cargo workspace (single project, `crates/*` members) — no new crate. Paths are under
`crates/truefix-log/`, `crates/truefix-store/`, `crates/truefix-config/`, `crates/truefix/`, plus
`README.md` at the repository root, per `plan.md`'s Project Structure section.

---

## Phase 1: Setup

**Purpose**: Confirm a clean baseline before any change, per this feature's Assumption that no new
crate or external dependency is needed.

- [X] T001 Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and
      `cargo test --workspace --all-features` at the repository root to confirm a clean, green
      baseline before any change in this feature begins.

---

## Phase 2: Foundational

**Purpose**: Blocking prerequisites shared by all user stories.

**Note**: This feature's three findings are scoped to disjoint files/crates — `NEW-156`/`NEW-157`
live entirely in `crates/truefix-log/src/file.rs`; `NEW-158` lives in `crates/truefix-store/src/
lib.rs`, `crates/truefix-config/src/builder.rs`, and `crates/truefix/src/lib.rs`. No shared
code-level infrastructure is needed beyond the Phase 1 baseline check. **User Story 3 (P3) may
start immediately after Phase 1**, in parallel with User Stories 1-2. **User Story 2 (P2) has a
documented dependency on User Story 1 (P1)** — see Dependencies & Execution Order below — so within
`truefix-log`, US1 must complete before US2 begins.

**Checkpoint**: Phase 1 complete → User Story 3 may start in parallel with User Story 1; User Story
2 waits on User Story 1's checkpoint.

---

## Phase 3: User Story 1 - Non-blocking file log writes (Priority: P1) 🎯 MVP

**Goal**: `FileLog` writes never block the tokio worker thread processing other sessions' work,
even under slow/contended disk I/O (`NEW-156`).

**Independent Test**: Under simulated slow-disk conditions, `FileLog` writes execute off the async
runtime thread and other concurrent session tasks continue making progress without being stalled;
verifiable via `cargo test -p truefix-log --test file_log_non_blocking` and
`cargo test -p truefix --test file_log_role_parity` without any US2/US3 code.

### Tests for User Story 1 ⚠️

> Write these tests FIRST; confirm they FAIL against today's synchronous `FileLog` before starting
> the implementation tasks below.

- [X] T002 [P] [US1] Write a test proving `FileLog` writes currently block the calling async task
      under a simulated slow/blocked disk write (must fail pre-fix; passes once writes are moved
      off the executor thread) in `crates/truefix-log/tests/file_log_non_blocking.rs` (`NEW-156`,
      FR-001, SC-001, SC-005)
- [X] T003 [P] [US1] Write a test proving other concurrently scheduled work (simulated
      heartbeat-timer-equivalent task) still fires within its configured tolerance while a slow
      `FileLog` write is in flight, in `crates/truefix-log/tests/file_log_non_blocking.rs` (SC-001)
- [X] T004 [P] [US1] Write a test proving sustained write pressure applies bounded
      buffering/backpressure rather than unbounded queue growth, in
      `crates/truefix-log/tests/file_log_non_blocking.rs` (FR-003)
- [X] T005 [P] [US1] Write a test proving a background write/flush I/O failure (e.g. simulated disk
      full) is surfaced via a structured `tracing` error event and a `metrics` counter increment,
      while the triggering `on_incoming`/`on_outgoing`/`on_event` call itself does not panic or
      return an error, in `crates/truefix-log/tests/file_log_non_blocking.rs` (FR-009)
- [X] T006 [P] [US1] Extend `crates/truefix-log/tests/async_writer_flush_on_shutdown.rs` with a
      `FileLog` case (mirroring the existing `RedbLog` case): queue N entries, call
      `Log::shutdown()`, and assert all N lines are present on disk with no sleep/delay before the
      assertion (FR-002, SC-002)
- [X] T007 [P] [US1] Write a test proving `FileLog`'s non-blocking write and `shutdown()`-drain
      behavior (T002-T006) is identical whether the owning session is configured as an acceptor or
      an initiator — start a minimal `Engine` with one `FileLog`-backed acceptor session and one
      `FileLog`-backed initiator session and assert both exhibit the same non-blocking/flush
      behavior, in `crates/truefix/tests/file_log_role_parity.rs` (`NEW-156`, FR-008; closes the
      `/speckit-analyze` D1 gap — Constitution Principle VI requires both roles be tested, not just
      designed for)

### Implementation for User Story 1

- [X] T008 [US1] Define an `Entry` enum (`Message { direction: &'static str, text: String }` |
      `Event { text: String }`) in `crates/truefix-log/src/file.rs`, mirroring `SqlLog`'s/
      `RedbLog`'s existing `Entry` shape (depends on T002-T007 existing and failing)
- [X] T009 [US1] Add a `FileLogWriter`-equivalent internal shape to `FileLog`
      (`tx: Mutex<Option<mpsc::Sender<Entry>>>`, `task: Mutex<Option<JoinHandle<()>>>`) in
      `crates/truefix-log/src/file.rs`, replacing the current `Mutex<RotatingFile>` fields with
      channel senders plus a background-task handle (depends on T008)
- [X] T010 [US1] Spawn a `tokio::spawn`ed background task owning the `messages`/`events`
      `RotatingFile` handles, receiving `Entry` values from the bounded `mpsc` channel and writing
      each one via `tokio::task::spawn_blocking` (mirroring `RedbLog`'s
      `crates/truefix-log/src/redb.rs:145-168` pattern exactly) in `crates/truefix-log/src/file.rs`
      (depends on T009)
- [X] T011 [US1] Rewire `FileLog::on_incoming`/`on_outgoing`/`on_event` to send an `Entry` through
      the bounded channel (best-effort: drop on a saturated channel, matching `Log`'s existing
      infallible/fire-and-forget contract) instead of calling `write_line` inline, in
      `crates/truefix-log/src/file.rs` (depends on T010)
- [X] T012 [US1] Implement `Log::shutdown()` for `FileLog`: take the sender (dropping it to close
      the channel) and await the background task's `JoinHandle`, in
      `crates/truefix-log/src/file.rs` (depends on T010)
- [X] T013 [US1] Emit a `tracing::error!` event and increment a `metrics` counter inside the
      background task on a `RotatingFile::write_line` I/O failure, replacing/supplementing the
      existing `eprintln!`-based NEW-124 fallback, in `crates/truefix-log/src/file.rs` (FR-009)
      (depends on T010)
- [X] T014 [US1] Update the crate-level doc comment in `crates/truefix-log/src/lib.rs` describing
      `FileLog`'s new channel-backed/async behavior and its `shutdown()` override, matching the
      existing doc style for `SqlLog`/`RedbLog` (depends on T008-T013)
- [X] T015 [US1] Run `cargo test -p truefix-log -p truefix` and confirm T002-T007 now pass and that
      `crates/truefix-log/tests/audit006_file_log.rs` / `audit006_file_growth.rs` /
      `crates/truefix-log/tests/switches.rs` still pass unmodified (regression check; depends on
      T008-T014)

**Checkpoint**: User Story 1 is fully functional and independently testable — `FileLog` no longer
blocks the executor, has bounded backpressure, a real `shutdown()` drain, structured failure
observability, and verified acceptor/initiator parity.

---

## Phase 4: User Story 2 - Configurable file log/store rotation retention (Priority: P2)

**Goal**: Operators can configure generation-count and/or time-based retention for file-based logs,
composable, defaulting to today's unchanged single-backup-overwrite behavior when unset
(`NEW-157`).

**Depends on**: User Story 1's checkpoint (Phase 3) — per `spec.md`'s own priority rationale, this
story's rotation rewrite is built on Story 1's non-blocking write path rather than being migrated
onto it later; both stories modify the same `RotatingFile`/`FileLog` internals in
`crates/truefix-log/src/file.rs`.

**Independent Test**: Configure a retention policy (generation count and/or roll interval) and
verify, across multiple simulated rotation events, that old generations are retained/pruned per
policy and/or dated files roll at the configured boundary; verifiable via
`cargo test -p truefix-log --test file_log_retention` and
`cargo test -p truefix --test file_log_role_parity` without any US3 code.

### Tests for User Story 2 ⚠️

> Write these tests FIRST; confirm they FAIL against today's single-fixed-`.1`-backup rotation
> before starting the implementation tasks below.

- [X] T016 [P] [US2] Write a test proving a configured generation count `N` retains at most `N`
      prior generations after repeated rotations, pruning older ones, in
      `crates/truefix-log/tests/file_log_retention.rs` (FR-004, SC-003, SC-005)
- [X] T017 [P] [US2] Write a test proving a configured time-based roll interval creates a new
      dated file at the interval boundary while preserving prior files, in
      `crates/truefix-log/tests/file_log_retention.rs` (FR-004)
- [X] T018 [P] [US2] Write a test proving generation-count and time-based rotation are composable
      (both configured together, e.g. daily rolling capped at N retained files), in
      `crates/truefix-log/tests/file_log_retention.rs` (FR-004 clarification)
- [X] T019 [P] [US2] Write a test proving no log content already accepted before a rotation is lost
      or corrupted, under either policy, in `crates/truefix-log/tests/file_log_retention.rs`
      (FR-005)
- [X] T020 [P] [US2] Write a test simulating a crash mid-rotation (filesystem rename observed, new
      file handle not yet reopened) and asserting the next `RotatingFile::open()` resumes exactly
      one unambiguous active file with no content loss, in
      `crates/truefix-log/tests/file_log_retention.rs` (FR-011)
- [X] T021 [P] [US2] Write a test simulating a time-based roll interval that elapsed while the
      process was stopped (e.g. via a fake/injected clock or manipulated file mtime), asserting the
      first write after restart rolls exactly once to the current interval with no fabricated gap
      files and no continued writes to the stale-named file, in
      `crates/truefix-log/tests/file_log_retention.rs` (FR-013)
- [X] T022 [P] [US2] Write a test proving that when no retention policy is configured, rotation
      behavior is byte-for-byte identical to today's single-backup-overwrite behavior, in
      `crates/truefix-log/tests/file_log_retention.rs` (FR-004 default/compatibility clarification)
- [X] T023 [P] [US2] Write a test proving the retention policy (generation-count and/or time-based
      rotation) behaves identically whether the owning session is configured as an acceptor or an
      initiator, extending the `Engine`-level fixture from T007, in
      `crates/truefix/tests/file_log_role_parity.rs` (`NEW-157`, FR-008; closes the
      `/speckit-analyze` D1 gap for this story)

### Implementation for User Story 2

- [X] T024 [US2] Add a `RetentionPolicy { generations: Option<u32>, roll_interval:
      Option<RollInterval> }` struct and a `RollInterval` enum to
      `crates/truefix-log/src/file.rs`, rejecting `generations = Some(0)` as invalid at
      construction (depends on T016-T023 existing and failing)
- [X] T025 [US2] Add a `retention: Option<RetentionPolicy>` field to `FileLogOptions` in
      `crates/truefix-log/src/file.rs`, defaulting to `None` (depends on T024)
- [X] T026 [US2] Generalize `RotatingFile::rotate()` from a single fixed `.1` backup to shifting
      `.1`..`.N` generations, driven by `RetentionPolicy::generations` when set, in
      `crates/truefix-log/src/file.rs` (depends on T025)
- [X] T027 [US2] Add a time-based roll trigger (date-stamped active-file naming) alongside the
      existing size-based (`MaxFileLogSize`) trigger in `RotatingFile`'s write/rotate path, driven
      by `RetentionPolicy::roll_interval` when set, in `crates/truefix-log/src/file.rs` (depends on
      T025)
- [X] T028 [US2] Restructure `RotatingFile::open()`/`rotate()` so all filesystem-mutating
      operations complete before any in-memory state (size counter, current generation, current
      interval) is updated, and so `open()` re-derives current generation/interval from what's
      actually on disk rather than an in-memory default, guaranteeing the crash-mid-rotation
      recovery invariant, in `crates/truefix-log/src/file.rs` (FR-011; depends on T026, T027)
- [X] T029 [US2] Implement the roll-once-on-restart-if-stale-interval check on
      `RotatingFile::open()`/first subsequent write, in `crates/truefix-log/src/file.rs` (FR-013;
      depends on T027, T028)
- [X] T030 [US2] Add new `.cfg` keys (e.g. `FileLogMaxGenerations`/`FileLogRollInterval`) to
      `LogSpec` and `resolve_log` in `crates/truefix-config/src/builder.rs`, mirroring the existing
      `MaxFileLogSize` parsing pattern (`builder.rs:912-914`) (depends on T024, T025)
- [X] T031 [US2] Thread the new `LogSpec` retention fields through `build_log`/`build_log_kind` in
      `crates/truefix/src/lib.rs` into `FileLogOptions::retention`, mirroring how
      `max_size_bytes` is already threaded (depends on T025, T030)
- [X] T032 [US2] Run `cargo test -p truefix-log -p truefix-config -p truefix` and confirm
      T016-T023 now pass and that existing `MaxFileLogSize`/`audit006_file_growth.rs` /
      `config_start.rs` tests still pass unmodified (regression check; depends on T026-T031)

**Checkpoint**: User Stories 1 AND 2 both work independently — file logs now support composable,
crash-safe, restart-safe retention, opt-in only, with verified acceptor/initiator parity.

---

## Phase 5: User Story 3 - Discoverable custom store/log injection via `Engine::start` (Priority: P3)

**Goal**: A `.cfg`-driven `Engine::start` caller can supply a custom `MessageStore`/`Log`
implementation through a documented mechanism, without dropping to
`truefix::transport::Services` (`NEW-158`).

**Independent Test**: Configure `Engine::start` (or its new override entry point) with a custom
`MessageStore`/`Log` and verify session persistence/logging uses it instead of a built-in backend,
for both acceptor and initiator roles; verifiable via
`cargo test -p truefix --test custom_backend_injection` without any US1/US2 code — this story
touches `truefix-store`/`truefix-config`/`truefix` only, not `truefix-log`'s internals.

### Tests for User Story 3 ⚠️

> Write these tests FIRST; confirm they FAIL against today's closed `StoreConfig`/`ResolvedSession`
> surface before starting the implementation tasks below.

- [X] T033 [P] [US3] Write a test proving an `Engine` started through the `.cfg`-driven path with
      `StoreConfig::Custom(...)` uses the supplied `MessageStore` instead of any built-in store, in
      `crates/truefix/tests/custom_backend_injection.rs` (`NEW-158`, FR-006, SC-005)
- [X] T034 [P] [US3] Write a test proving an `Engine` started through the `.cfg`-driven path with a
      custom `Log` (via the new `ResolvedSession` custom-log slot) uses the supplied `Log` instead
      of any built-in log, in `crates/truefix/tests/custom_backend_injection.rs` (FR-006)
- [X] T035 [P] [US3] Write a test proving that when both `Custom(...)`/the custom-log slot and a
      builder-level override are set for the same store/log slot, the builder-level override is
      the one actually used, in `crates/truefix/tests/custom_backend_injection.rs` (FR-012)
- [X] T036 [P] [US3] Write a test proving a session combining `Custom(...)`/the custom-log slot
      with a built-in-only setting (e.g. `MaxFileLogSize`) fails `Engine::start` with a clear,
      typed `ConfigError` rather than starting, in
      `crates/truefix/tests/custom_backend_injection.rs` (FR-010)
- [X] T037 [US3] Write a test proving custom store/log injection (including builder-override
      precedence and fail-fast validation) behaves identically for an acceptor session and an
      initiator session, in `crates/truefix/tests/custom_backend_injection.rs` (FR-008)

### Implementation for User Story 3

- [X] T038 [P] [US3] Add a `Custom(Arc<dyn MessageStore>)` variant to `StoreConfig` in
      `crates/truefix-store/src/lib.rs`; hand-write the `Debug` impl arm (`dyn MessageStore` isn't
      `Debug`) since the variant now breaks the derived impl; add a pass-through arm to
      `build_store` (`lib.rs:188`) (depends on T033-T037 existing and failing)
- [X] T039 [P] [US3] Add a `custom_log: Option<Arc<dyn Log>>` field to `ResolvedSession` in
      `crates/truefix-config/src/builder.rs`, populated only via programmatic construction (never
      parsed from `.cfg` text) (depends on T033-T037 existing and failing)
- [X] T040 [US3] Add a new `ConfigError` variant (e.g.
      `CustomBackendWithBuiltinOnlySetting { session, setting }`) and validate it in
      `resolve_store`/`resolve_log` in `crates/truefix-config/src/builder.rs`, rejecting
      `StoreConfig::Custom`/`custom_log` combined with a built-in-only setting (`MaxFileLogSize` and
      — once Phase 4 has landed — the new retention keys from T030) (FR-010; depends on T038, T039)
- [X] T041 [US3] Update the log/store resolution branch in `Engine::start`
      (`crates/truefix/src/lib.rs:486-495`-equivalent) to give `custom_log`/
      `StoreConfig::Custom` precedence over `log_kind`/`sql_log`/`log` and the built-in
      store-building path (FR-006; depends on T038-T040)
- [X] T042 [US3] Add an `Engine` entry point (e.g. `Engine::start_with_overrides`) accepting a
      caller-supplied `Services`-shaped override, applied per session (or globally) on top of
      whatever `.cfg` resolution already produced, with builder-override-wins precedence, in
      `crates/truefix/src/lib.rs` (FR-006, FR-012; depends on T041)
- [X] T043 [US3] Update `README.md` to document `StoreConfig::Custom`/the custom-log slot and the
      new `Engine` override entry point as the supported way to inject a custom backend through
      the `.cfg`-driven path (FR-007, SC-004; depends on T038-T042)
- [X] T044 [US3] Add an automated test that exercises the exact custom-backend example just
      documented in `README.md` (T043) — e.g. a doctest on the new `Engine` override entry point, or
      a runnable `examples/` binary invoked from a test — so `SC-004`'s "verified by a documented
      example that is exercised by an automated test" is actually satisfied rather than just
      documented in prose, in `crates/truefix/tests/custom_backend_injection.rs` or as a doc-test
      (FR-007, SC-004; closes the `/speckit-analyze` E1 gap; depends on T043)
- [X] T045 [US3] Run `cargo test -p truefix-store -p truefix-config -p truefix` and confirm
      T033-T037 now pass and that existing `config_start.rs`/`audit006_engine_api.rs`/
      `multi_session_acceptor_cfg.rs` tests still pass unmodified (regression check; depends on
      T038-T044)

**Checkpoint**: All three user stories are independently functional — non-blocking writes, retention
policy, and custom-backend injection are each complete and separately testable, with SC-004's
documented example actually exercised by a test.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Reconciliation between stories and final feature-wide verification.

- [X] T046 If both Phase 4 (User Story 2) and Phase 5 (User Story 3) have landed, extend T040's
      fail-fast validation to also cover the new retention `.cfg` keys added in T030 (not just
      `MaxFileLogSize`), in `crates/truefix-config/src/builder.rs` (FR-010 completeness across both
      stories)
- [X] T047 [P] Add/verify doc comments on every new public item — `FileLogOptions::retention`,
      `RetentionPolicy`, `RollInterval`, `StoreConfig::Custom`, `ResolvedSession::custom_log`, the
      new `ConfigError` variant, and the new `Engine` entry point — per Constitution Principle I
      (public API MUST be documented)
- [X] T048 [P] Record the additive-but-match-breaking migration notes from
      `contracts/public-api-changes.md` (new enum variant / new struct field on non-`#[non_exhaustive]`
      public types) in `CHANGELOG.md` if the repository maintains one, or in the relevant crate's
      doc comments otherwise
- [X] T049 Run the full `quickstart.md` validation: `cargo fmt --check`,
      `cargo clippy --all-targets --all-features -- -D warnings`,
      `cargo test --workspace --all-features` at the repository root
- [X] T050 Verify SC-005 directly: confirm each of `NEW-156` (T002-T005), `NEW-157` (T016-T021),
      and `NEW-158` (T033-T036) has at least one test that was observed failing before its
      corresponding implementation task and passing after

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — run first.
- **Foundational (Phase 2)**: Depends on Setup; no code-level blocking tasks of its own (see note
  in Phase 2).
- **User Story 1 (Phase 3, P1)**: Depends on Phase 1 only.
- **User Story 2 (Phase 4, P2)**: Depends on Phase 1 **and** User Story 1's checkpoint (shared file:
  `crates/truefix-log/src/file.rs`; see Phase 4's "Depends on" note).
- **User Story 3 (Phase 5, P3)**: Depends on Phase 1 only — independent of User Stories 1 and 2
  (disjoint crates/files).
- **Polish (Phase 6)**: Depends on whichever of Phase 3/4/5 are in scope for a given delivery; T046
  specifically depends on both Phase 4 and Phase 5.

### User Story Dependencies

- **User Story 1 (P1)**: No dependencies on other stories. MVP candidate on its own.
- **User Story 2 (P2)**: Hard dependency on User Story 1 (both modify
  `crates/truefix-log/src/file.rs`'s `RotatingFile`/`FileLog` internals; Story 2's rotation logic is
  built on Story 1's async-writer/background-task shape).
- **User Story 3 (P3)**: No dependency on User Story 1 or 2 — different crates
  (`truefix-store`/`truefix-config`/`truefix`) — can be implemented in parallel with Phase 3-4 by a
  second contributor/session.

### Within Each User Story

- Tests (T002-T007, T016-T023, T033-T037) MUST be written and observed FAILING before their
  corresponding implementation tasks begin.
- Within US1: `Entry`/writer-shape (T008-T009) → background task (T010) → rewire `on_*`/`shutdown`
  (T011-T012) → observability (T013) → docs (T014) → regression run (T015).
- Within US2: policy types (T024-T025) → rotation logic (T026-T029) → `.cfg` surface (T030-T031) →
  regression run (T032).
- Within US3: `StoreConfig::Custom`/`custom_log` (T038-T039, parallel) → validation (T040) →
  resolution precedence (T041) → override entry point (T042) → docs (T043) → automated example
  test (T044) → regression run (T045).

### Parallel Opportunities

- T002-T007 (US1 tests) can all run in parallel — T002-T006 are independent assertions in the same
  new test file, and T007 is a separate file (`crates/truefix/tests/file_log_role_parity.rs`); no
  code dependencies between any of them.
- T016-T023 (US2 tests) can all run in parallel, same reasoning (T023 extends T007's separate file).
- T033-T036 (US3 tests) can run in parallel; T037 depends on the injection mechanism existing
  conceptually but can be drafted in parallel and filled in once T041-T042 land.
- T038 and T039 (US3 implementation) touch different crates (`truefix-store` vs. `truefix-config`)
  and can run in parallel.
- **User Story 3 (Phase 5) as a whole can run in parallel with User Stories 1-2 (Phases 3-4)** —
  distinct crates, no shared files.
- T047 and T048 (Polish) can run in parallel with each other.

---

## Parallel Example: User Story 1

```bash
# Launch all US1 tests together (T002-T006 share one file, T007 is a separate crate/file):
Task: "Write a test proving FileLog writes currently block the calling async task under a
       simulated slow/blocked disk write in crates/truefix-log/tests/file_log_non_blocking.rs"
Task: "Write a test proving other concurrently scheduled work still fires within its configured
       tolerance while a slow FileLog write is in flight"
Task: "Write a test proving sustained write pressure applies bounded backpressure rather than
       unbounded queue growth"
Task: "Write a test proving a background write/flush I/O failure is surfaced via structured
       tracing/metrics"
Task: "Extend async_writer_flush_on_shutdown.rs with a FileLog case"
Task: "Write a test proving non-blocking/shutdown-drain behavior is identical for an acceptor and
       an initiator session in crates/truefix/tests/file_log_role_parity.rs"
```

## Parallel Example: User Story 3 (runs alongside User Stories 1-2)

```bash
# Different crates — safe to run as a fully separate workstream:
Task: "Add a Custom(Arc<dyn MessageStore>) variant to StoreConfig in crates/truefix-store/src/lib.rs"
Task: "Add a custom_log: Option<Arc<dyn Log>> field to ResolvedSession in crates/truefix-config/src/builder.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001).
2. Complete Phase 3: User Story 1 (T002-T015).
3. **STOP and VALIDATE**: `cargo test -p truefix-log --test file_log_non_blocking`, the extended
   `async_writer_flush_on_shutdown.rs` case, and `cargo test -p truefix --test
   file_log_role_parity` all pass; `FileLog` no longer blocks the executor, for either session
   role. This alone closes `NEW-156`, the audit's P2-severity/highest-suggested-priority finding.

### Incremental Delivery

1. Setup (Phase 1) → baseline confirmed.
2. User Story 1 (Phase 3) → non-blocking `FileLog` writes, role-parity verified → independently
   testable/deployable.
3. User Story 2 (Phase 4) → adds retention on top of Story 1, role-parity verified →
   independently testable/deployable.
4. User Story 3 (Phase 5) → adds custom-backend injection, independent of 1-2, with its documented
   example actually exercised by a test → independently testable/deployable, can land before,
   after, or interleaved with 1-2.
5. Polish (Phase 6) → cross-story reconciliation (T046) and feature-wide verification.

### Parallel Team Strategy

With two contributors/sessions:

1. Both complete Phase 1 (Setup) together.
2. Contributor A: Phase 3 (US1) then Phase 4 (US2), sequentially (US2 depends on US1).
3. Contributor B: Phase 5 (US3), in parallel with A — no shared files.
4. Both converge on Phase 6 (Polish), where T046 explicitly reconciles US2's new `.cfg` keys with
   US3's fail-fast validation.

---

## Notes

- [P] tasks touch different files or independent assertions within one new test file, with no
  ordering dependency between them.
- [Story] labels map every Phase 3-5 task to `spec.md`'s Story 1/2/3 for traceability back to
  `NEW-156`/`NEW-157`/`NEW-158`.
- Every implementation task in Phase 3-5 has a corresponding test task written first (Constitution
  Principle V, SC-005) — confirm each test fails against pre-change code before starting its
  implementation task.
- Commit after each task or logical group, per repository convention.
- Regression-check tasks (T015, T032, T045, T049) exist specifically to catch any accidental
  behavior change to `audit006_file_log.rs`/`audit006_file_growth.rs`/`config_start.rs`/
  `audit006_engine_api.rs` and similar pre-existing tests, which MUST keep passing unmodified.
- Avoid: skipping the "observe the test fail first" step, editing `RotatingFile` in Phase 4 before
  Phase 3's channel/background-task refactor lands, and conflating US2's `.cfg`-key task (T030)
  with US3's `ConfigError` validation (T040) — they're written by different phases and reconciled
  only in Polish (T046).
