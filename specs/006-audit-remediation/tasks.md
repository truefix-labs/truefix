# Tasks: Audit Remediation (docs/todo/003.md)

**Input**: Design documents from `specs/006-audit-remediation/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included as first-class tasks throughout (Constitution Principle V mandates test-first
discipline; matches this project's established convention across features 001-005). **US1 also
requires 8 new AT scenario families**, not just unit/integration tests — the only story in this
feature that is genuinely protocol-behavioral (Constitution Principle II). Task file paths below were
grounded by reading the actual current test suite during `/speckit-tasks` (not guessed) — several
existing test files already contain adjacent coverage this feature extends rather than duplicates
(e.g. `crates/truefix/tests/session_qualifier.rs` already tests the *distinct-port* qualifier case;
this feature adds the complementary *same-port-rejected* negative case).

**Organization**: Tasks are grouped by user story (spec.md's 10 stories, in priority order: P1 = US1,
US2, US3, US4; P2 = US5, US6, US7; P3 = US8, US9, US10), mirroring plan.md's R1-R10 staging exactly
(one-to-one with research.md's sections). Stories are independently implementable/testable with two
soft (file-adjacency, not hard-blocking) sequencing notes: US8's FIXT `ApplVerID` items touch the same
`on_logon` function US1 already changes (do US1 first — already true by phase order), and US9's final
regression-floor task depends on US1's AT-scenario task having landed (also already true by phase
order, since US1 = phase 2 and US9 = phase 10).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Maps to spec.md's US1-US10
- File paths are exact; a task without a `[Story]` label is Setup/Polish (applies across stories)

---

## Phase 1: Setup

**Purpose**: Establish and record the starting baseline. No new dependency needed for most of this
feature (plan.md's Technical Context) — the one possible exception (`time-tz`, US8) is evaluated
within its own task, not here.

- [X] T001 [P] Confirm and record the starting baseline: `cargo test --workspace` (expect exit 0, all
  green — confirmed live during `/speckit-plan`), `cargo run -p truefix-at --example count_check` (a
  throwaway example computing `server_suite()`'s actual scenario-run count — expect **373**, matching
  `docs/todo/003.md`'s claim exactly, confirmed live during `/speckit-plan`) — no code change; this
  establishes the exact numbers T022/T086's floor-bump tasks compare against. — **complete**: baseline
  confirmed clean at plan time, no drift since.
- [X] T002 Bump the regression-floor literal in `crates/truefix-at/tests/coverage.rs` from
  `total_runs >= 353` to `total_runs >= 373` (`BUG-17`, research.md §R9.1) — this is `BUG-17`'s own
  first, trivial sub-task, done immediately and independent of US1's later scenario additions (which
  will require a *second* bump at T086, once the true final count is known). — **complete**.

**Checkpoint**: Baseline recorded; regression floor accurate at 373. User story work can begin.

---

## Phase 2: User Story 1 — Session protocol correctness (Priority: P1) 🎯 MVP

**Goal**: Close 8 counterparty-triggerable protocol-correctness holes in `crates/truefix-session/src/
state.rs` (`BUG-05`, `BUG-06`, `BUG-22`, `B1`, `B3`, `B4`, `B5`, `B6`, `B7` — see
`contracts/session-protocol.md`'s before/after table).

**Independent Test**: Drive each scenario against a live two-process session and confirm
protocol-correct rejection/Logout instead of silent acceptance or a wedged reconnect loop; AT suite
scenario-run count grows past 373.

### Tests for User Story 1

- [X] T003 [P] [US1] Add a failing test: an acceptor rejects a Logon with `MsgSeqNum < next_in_seq`
  (no PossDup) via Logout+disconnect — no transition to `LoggedOn`, `next_in_seq` unchanged — in
  `crates/truefix-session/tests/state_machine.rs` (extend, alongside the existing
  `acceptor_logon_reports_seq_state_after_consuming_logon`). — **complete**:
  `logon_with_seq_below_expected_and_no_possdup_is_rejected`.
- [X] T004 [P] [US1] Add a failing test: a plain-mode `SequenceReset` with a decreasing `NewSeqNo` is
  rejected (`SessionRejectReason::ValueIsIncorrect`, ref tag 36) and `next_in_seq` is left unchanged,
  in `crates/truefix-session/tests/recovery.rs` (extend — the file already tests the *increasing*
  case per `sequence_reset_reset_mode_sets_expected`). — **complete**:
  `sequence_reset_reset_mode_with_decreasing_new_seq_no_is_rejected` (plus a no-op-accept edge case
  added: `sequence_reset_reset_mode_with_equal_new_seq_no_is_a_no_op_accept`, per spec Edge Cases).
- [X] T005 [P] [US1] Add a failing test: a plain-mode `SequenceReset` with tag 36 (`NewSeqNo`) absent
  is rejected as required-tag-missing *before* the queue is drained, same file. — **complete**:
  `sequence_reset_reset_mode_missing_new_seq_no_is_rejected_as_required_tag_missing`.
- [X] T006 [P] [US1] Add a failing test: a `ResendRequest` missing `BeginSeqNo`/`EndSeqNo` gets a
  required-tag-missing response; a `ResendRequest` with both tags present but `BeginSeqNo >
  EndSeqNo` still silently no-ops (unchanged, spec Edge Cases), in
  `crates/truefix-session/tests/recovery.rs` (extend). — **complete**: split into
  `resend_request_missing_begin_seq_no_gets_required_tag_missing_response`,
  `resend_request_missing_end_seq_no_gets_required_tag_missing_response`, and
  `resend_request_begin_greater_than_end_with_both_tags_present_still_silently_no_ops`.
- [X] T007 [P] [US1] Add a failing two-process integration test: an initiator configured with
  `ResetOnLogon` reconnects repeatedly without ever hitting the
  gap→resend-stale-Logon→`reject_logon` loop, in `crates/truefix-transport/tests/reconnect.rs`
  (extend, new test fn). — **complete, relocated**: deeper investigation during implementation found
  the bug reproduces on the *very first* connection (not just reconnects) — `on_connected` always
  sets `logon_sent=true` before any reply is processed, so `let full = !self.logon_sent` is false on
  essentially every initiator connection, regardless of connection count. Root-caused this precisely
  as a pure session-state-machine behavior, so the regression test lives in
  `crates/truefix-session/tests/recovery.rs` as
  `reset_on_logon_initiator_sends_its_own_logon_at_a_freshly_reset_seq_not_a_stale_seeded_value`
  (seeds `next_out_seq=7` via `seed_sequences` to distinguish "fresh reset to 1" from "leave
  unchanged," which a same-session single-message test can't otherwise distinguish) — no transport-
  level two-process test needed since the fix is entirely within `on_connected`.
- [X] T008 [P] [US1] Add a failing test: a message drained from the out-of-order queue after a
  gap-fill is validated (rejected when invalid, same disconnect-or-continue semantics) exactly like
  an in-order message, in `crates/truefix-session/tests/recovery.rs` (extend, alongside the existing
  `gap_fills_then_queued_message_processed`). — **complete**:
  `invalid_message_drained_from_queue_after_a_gap_is_rejected_like_an_in_order_one`.
- [X] T009 [P] [US1] Add a failing test: `on_connected` resets `ticks_since_recv`/
  `test_request_outstanding`/`resend_target`/`resend_chunk_end` for a new connection instead of
  carrying over prior-connection values, in `crates/truefix-session/tests/timeouts.rs` (extend). —
  **complete, relocated to recovery.rs**: `ticks_since_recv`/`test_request_outstanding` are already
  reset unconditionally by `on_received`'s first line on every message, so a timers-only test in
  `timeouts.rs` couldn't demonstrate a live failure window for those two specifically. The
  concretely demonstrable bug is `resend_target`/`resend_chunk_end` surviving a reconnect and later
  causing a spurious auto-issued `ResendRequest` on a gap-free fresh connection — test added to
  `recovery.rs` instead:
  `reconnecting_clears_stale_chunked_resend_tracking_so_a_fresh_connection_does_not_spuriously_request_a_resend`.
  All four fields are still reset defensively in the implementation (T016).
- [X] T010 [P] [US1] Add a failing test: a dictionary-validation-failure disconnect
  (`disconnect_on_error=true`) sends a Logout before `Action::Disconnect`, matching the
  identity/latency disconnect paths, in `crates/truefix-session/tests/validation_hook.rs` (extend,
  alongside the existing `invalid_app_message_is_rejected_and_seq_advances`). — **complete**:
  `dictionary_validation_failure_with_disconnect_on_error_sends_logout_before_disconnecting`.
- [X] T011 [P] [US1] Add a failing test: an inbound message with an unparseable `SendingTime` fails
  the latency check (routes through the existing Logout+disconnect path) instead of passing it, in
  `crates/truefix-session/tests/timeouts.rs` (extend, alongside the existing
  `stale_sending_time_aborts_session`). — **complete**: `unparseable_sending_time_fails_the_latency_check`.

### Implementation for User Story 1

- [X] T012 [US1] Fix `on_logon` in `crates/truefix-session/src/state.rs`: when `disposition ==
  Some(Ordering::Less)`, route through `self.reject_logon(...)` before the state-transition block,
  instead of falling through to `_ => {}` (research.md §R1.1) — makes T003 pass. — **complete**:
  computes `logon_seq`/PossDup early (before the reset-flag block) and rejects when
  `Less`-without-PossDup, before any reset/state-transition side effect runs.
- [X] T013 [US1] Fix `on_sequence_reset` in `crates/truefix-session/src/state.rs`: plain-mode
  decreasing `NewSeqNo` is rejected via a session Reject (`ValueIsIncorrect`, ref tag 36) instead of
  applied unconditionally (research.md §R1.2) — makes T004 pass. — **complete**.
- [X] T014 [US1] Same function: the tag-36-absent case is rejected as required-tag-missing *before*
  `drain_queue()` runs (research.md §R1.2) — makes T005 pass. — **complete**.
- [X] T015 [US1] Fix `on_resend_request` in `crates/truefix-session/src/state.rs`: a missing
  `BeginSeqNo`/`EndSeqNo` tag produces a required-tag-missing response, distinct from the retained
  `begin > end` silent-skip (research.md §R1.3) — makes T006 pass. — **complete**.
- [X] T016 [US1] Reset `ticks_since_recv = 0`, `test_request_outstanding = false`, `resend_target =
  None`, `resend_chunk_end = None` in `on_connected`, `crates/truefix-session/src/state.rs`
  (research.md §R1.6) — makes T009 pass. — **complete, simpler than planned**: research.md's
  `connection_reset_done` field turned out unnecessary once T017's fix moved the `ResetOnLogon` reset
  into `on_connected` itself (see T017) — no new `Session` field was needed anywhere in this feature
  (data-model.md's one disclosed addition did not end up required).
- [X] T017 [US1] Fix the `ResetOnLogon` full-reset trigger (research.md §R1.4) — makes T007 pass. —
  **complete, different mechanism than planned**: rather than patching `on_logon`'s reactive
  `!self.logon_sent` check, the fix performs the full reset **proactively in `on_connected`** for
  `Role::Initiator` when `config.reset_on_logon` is set, *before* composing the outbound Logon —
  guaranteeing our own Logon always carries a fresh seq 1 regardless of send/receive order. This
  made `on_logon`'s existing reactive reset-on-inbound-flag logic correct-by-construction for the
  acceptor role (unchanged) and a harmless no-op for the initiator (next_in_seq already 1). Returns
  `Action::ResetStore` (already a generically-handled action) when it fires.
- [X] T018 [US1] Add a `validate_app` call to `drain_queue` in `crates/truefix-session/src/state.rs`,
  matching the in-order `Equal`-branch reject/continue-or-disconnect semantics (research.md §R1.5) —
  makes T008 pass. — **complete**.
- [X] T019 [US1] Add a Logout send before `Action::Disconnect` in the `disconnect_on_error` branch of
  `on_received`'s dictionary-validation-failure path, `crates/truefix-session/src/state.rs`
  (research.md §R1.7) — makes T010 pass. — **complete**.
- [X] T020 [US1] Change `latency_ok`'s unparseable-`SendingTime` branch from `return true` to `return
  false`; remove the now-inaccurate comment, `crates/truefix-session/src/state.rs` (research.md
  §R1.8) — makes T011 pass. — **complete**.

### AT scenario authoring for User Story 1

- [X] T021 [US1] Author new scenario families in `crates/truefix-at/src/scenarios.rs` (Constitution
  Principle II). — **complete, with two disclosed exceptions**: `BUG-05` (low-seq Logon) and `B1`/`B4`
  (ResetOnLogon reconnect, stale chunked-resend tracking) all require either seeded pre-`Connected`
  sequence state or a genuine reconnect within one scenario run — the AT harness's `runner.rs::Step`
  has no such primitive (`docs/todo/003.md`'s own "AT harness follow-ups" section already flags this
  exact limitation for `1b_DuplicateIdentity`/`20_SimultaneousResendRequest`); these three are
  tracked as similarly blocked, not silently skipped, and remain covered by their
  `truefix-session`-level unit/integration tests (T003, T007, T009). The other 6 fixes landed 7 new
  scenarios: `sequence_reset_reset_missing_new_seq_no_rejected`,
  `resend_request_missing_begin_seq_no_rejected`, `resend_request_missing_end_seq_no_rejected`
  (version-agnostic, looped across all 9 `SUITE_VERSIONS`), `gap_fill_drained_message_is_validated`,
  `dictionary_failure_disconnect_sends_logout` (dictionary-dependent, FIX.4.4-only, the latter also
  required adding a new `disconnect_on_error` field to `SessionTweaks`/`start_acceptor`), and
  `unparseable_sending_time_fails_latency` (`check_latency`-tweaked, FIX.4.4-only, added to both
  `server_suite()` and `timestamps_suite()`). **Bonus finding**: the existing `10_MsgSeqNumLess`
  scenario (`seq_reset_new_seq_no_less`) was itself asserting `BUG-06`'s buggy rewind-and-accept
  behavior as correct (its own doc comment said "Reset...has no backward guard") — corrected in
  place to expect the new Reject instead of authoring a redundant duplicate scenario.
- [X] T022 [US1] Bump `crates/truefix-at/tests/coverage.rs`'s regression floor again to reflect the
  new total scenario-run count after T021 lands; disclose the exact before/after numbers in the
  commit (per `BUG-17`'s "disclosed, deliberate bump" requirement, not a silent adjustment). —
  **complete**: 373 → **403** (confirmed via `cargo run -p truefix-at --example count_check`, a
  throwaway example; +30 = 3 new version-agnostic scenarios × 9 versions + 3 new single-version
  scenarios). Will be bumped once more in US9 (T085) after `MinQty`'s scenario lands.

**Checkpoint**: User Story 1 fully functional and testable independently — all tests pass (`cargo
test -p truefix-session`: 61 tests, 0 failed); AT suite grew from 373 to 403 scenario-runs
(`cargo test -p truefix-at --test conformance`: 4/4 suites, all green).

---

## Phase 3: User Story 2 — Multi-session acceptor routing (Priority: P1)

**Goal**: Route inbound connections correctly to SubID/LocationID/SessionQualifier-distinguished
acceptor sessions; fix `SO_REUSEADDR` on the multi-session bind path (`BUG-07`, `GAP-20`, `B11` — see
`contracts/routing-and-lifecycle.md`).

**Independent Test**: Configure two acceptor sessions on one port distinguished by SubID/LocationID
and confirm each routes correctly; configure a SessionQualifier-only-distinguished pair on one port
and confirm config-resolve-time rejection; confirm a released port rebinds immediately.

### Tests for User Story 2

- [X] T023 [P] [US2] Add a failing test: two acceptor sessions sharing one `SocketAcceptPort`,
  distinguished by `SenderSubID`/`SenderLocationID`/`TargetSubID`/`TargetLocationID`, each correctly
  route a live Logon from their respective identity, in `crates/truefix-transport/tests/
  multi_dynamic.rs` (extend, alongside the existing `static_and_dynamic_sessions_route_correctly`).
  — **complete**: `two_sessions_sharing_one_port_distinguished_by_sub_id_each_route_correctly`.
  Uncovered a necessary companion gap while writing it: `admin::base()` never stamped
  SenderSubID/SenderLocationID/TargetSubID/TargetLocationID onto *outbound* messages at all, so
  even a correct inbound-routing fix would have nothing to route on — fixed in the same task (see
  T026).
- [X] T024 [P] [US2] Add a failing test: two sessions sharing one `SocketAcceptPort` distinguished
  only by `SessionQualifier` (no other discriminator) are rejected at config-resolve time with a
  clear `ConfigError`, in `crates/truefix/tests/session_qualifier.rs` (extend — new *negative*-case
  test, complementary to the existing distinct-port *positive*-case
  `two_sessions_differing_only_by_qualifier_both_start_and_log_on`). — **complete**:
  `two_sessions_sharing_one_port_distinguished_only_by_qualifier_is_a_config_error`.
- [X] T025 [P] [US2] Add a failing test: an `AcceptorBuilder`-bound listener rebinds immediately after
  a prior process releases the same port (`SO_REUSEADDR`), in `crates/truefix-transport/tests/
  socket_options.rs` (extend — new test exercising `AcceptorBuilder::bind` specifically, distinct
  from the existing `Acceptor::bind_with`-only `reuse_address_allows_immediate_rebind`). —
  **complete**: `acceptor_builder_bind_allows_immediate_rebind`.

### Implementation for User Story 2

- [X] T026 [US2] Extract tags 50/142/57/143 from the inbound Logon in `route_and_run`
  (`crates/truefix-transport/src/lib.rs`) and build the session lookup key via
  `SessionId::new_full` instead of `SessionId::new` (research.md §R2.1) — makes T023 pass. —
  **complete, plus the outbound-stamping companion fix**: added matching tag stamping to
  `admin::base()` (`crates/truefix-session/src/admin.rs`) so a SubID/LocationID-configured
  session's own Logon/Heartbeat/etc. actually carry those fields on the wire.
- [X] T027 [US2] Add a group-level `SessionQualifier`-uniqueness check to `Engine::start`'s
  acceptor-group resolution (`crates/truefix/src/lib.rs`): reject configurations where two group
  members would produce an identical wire-extractable `SessionId::new_full` key (research.md §R2.1)
  — makes T024 pass. — **complete**: new `ConfigError::AmbiguousSessionQualifier` variant.
- [X] T028 [US2] Thread `reuse_address` through to `AcceptorBuilder::bind`
  (`crates/truefix-transport/src/lib.rs`) — reorder `Engine::start`'s call site so socket options are
  known pre-bind (research.md §R2.2) — makes T025 pass. — **complete, simpler than planned**:
  `AcceptorBuilder::bind` now always binds with `SO_REUSEADDR` (research.md's option (a)) rather
  than threading a parameter through — avoids an undisclosed breaking signature change to a public
  constructor that plan.md's Complexity Tracking didn't account for; multi-session acceptors are
  the operationally restart-sensitive case this bug describes, and `SO_REUSEADDR` is a safe,
  QuickFIX/J-matching default.

**Bonus finding, fixed in the same phase (not separately tracked as its own FR, but a direct,
necessary consequence of T026 + BUG-05's US1 fix)**: full-workspace regression testing surfaced
that `Engine::start`'s grouped-acceptor path shared **one** `MessageStore` instance across every
session in a group (via one group-wide `Services`), corrupting each session's own independent
sequence-number bookkeeping the moment two sessions in a group connected concurrently — previously
masked entirely by BUG-05's own bug (a too-low-seq Logon caused by the resulting desync was
silently *accepted* rather than rejected). Once BUG-05 was fixed (US1), this became a real,
deterministic failure of the pre-existing `crates/truefix/tests/multi_session_acceptor_cfg.rs`
test `two_acceptor_sessions_sharing_one_port_both_start_and_route_correctly`. Root-caused via
bisection (isolating each modified file, then each fix within `state.rs`) and fixed by giving each
statically-registered session in a group its own store: `AcceptorBuilder` gained a
`with_session_store(SessionId, Arc<dyn MessageStore>)` method and a `Registry.session_stores` map
that `route_and_run` consults before falling back to the group-wide store (used only for
dynamic-template-matched connections, which have no fixed identity to key by); `Engine::start`'s
group-resolution loop now builds one store per member instead of only the first member's. This is
a real, previously-undiscovered correctness bug in code that predates 006 — the "shared Services
across a group" limitation was already disclosed for socket-options/TLS/validator (legitimately
group-wide), but store state never should have been included in that sharing, since it is
per-session by construction.

**Checkpoint**: User Story 2 fully functional and testable independently — T023-T025 pass; full
`cargo test --workspace` (118 test binaries) green, including the previously-latent
`multi_session_acceptor_cfg.rs` regression this phase's work exposed and fixed.

---

## Phase 4: User Story 3 — Store/log persistence and durability (Priority: P1)

**Goal**: No silent store-failure signal loss, no spurious mid-window reset, `FileStore`/
`CachedFileStore` atomicity parity, `MssqlStore` identifier validation (`BUG-08`, `BUG-09`,
`GAP-48`, `GAP-39`/`GAP-49`, `BUG-15`, `B17`, `BUG-14` — see `contracts/store-persistence.md`).

**Independent Test**: Inject a store failure and confirm a log event; restart mid-schedule-window and
confirm no spurious reset; simulate a crash mid-write/mid-reset and confirm no desync; configure an
invalid MSSQL identifier and confirm a clean error.

### Tests for User Story 3

- [X] T029 [P] [US3] Add a failing test: a simulated store-operation failure (`save_and_advance_sender`/
  `set_next_sender_seq`/`set_next_target_seq`/`reset`/`get`) produces a log event via
  `services.log.on_event`, in new `crates/truefix-transport/tests/store_error_logging.rs`. —
  **complete**: `a_store_operation_failure_produces_an_operator_visible_log_event`, using a
  `FailingStore` mock that fails every write and a `RecordingLog` capturing `on_event` calls.
- [X] T030 [P] [US3] Add a failing test: a process restart whose store's `creation_time` falls within
  the currently-active schedule window preserves sequence numbers/history (no spurious reset), in
  `crates/truefix-transport/tests/scheduled.rs` (extend, alongside the existing
  `connects_when_in_session`). — **complete**:
  `restart_with_a_store_whose_creation_time_is_already_in_the_active_window_does_not_reset`
  (asserts the store's `creation_time` is unchanged after connecting, since `reset()` always
  stamps a fresh one — more robust than asserting exact sequence-number values, which legitimately
  advance from the Logon exchange itself regardless of whether a reset occurred). Required
  disabling `ResetOnLogon` (defaults `true`) in the test's own config to isolate the schedule-level
  reset path under test from the session-level `ResetOnLogon` reset path US1/T017 added.
- [X] T031 [P] [US3] Add a failing test: `FileStore`/`CachedFileStore`'s `save_and_advance_sender`
  provides the same atomicity guarantee as SQL/MSSQL/Redb (no desync after a simulated crash between
  body-write and seq-advance), in `crates/truefix-store/tests/restart_resend.rs` (extend, alongside
  the existing `file_store_survives_restart`/`cached_file_store_survives_restart`). —
  **complete**: `file_store_save_and_advance_sender_is_atomic_like_the_other_backends`,
  `cached_file_store_save_and_advance_sender_is_atomic_like_the_other_backends`.
- [X] T032 [P] [US3] Add a failing test: `BodyLog::reset()` syncs when `FileStoreSync=Y`, and
  `FileStore`/`CachedFileStore::reset()` clear `seq` before `body` so a simulated crash mid-reset
  leaves `seqnums` at post-reset `(1,1)` with `body` still holding harmless pre-reset data (never the
  reverse), in new `crates/truefix-store/tests/reset_ordering.rs`. — **complete, with a disclosed
  scope note**: a true crash-mid-write can't be simulated from a black-box unit test without OS-level
  fault injection; the added tests verify the observable reset end-state (both files correctly
  reset, sync enabled) as a baseline regression guard — the ordering/durability rationale itself is
  documented at each call site and in research.md §R3.4/§R3.5, not independently proven by a test
  that can interrupt mid-write.
- [X] T033 [P] [US3] Add a failing test: `MssqlStore::connect_with_config` rejects an invalid
  `sessions_table`/`messages_table` identifier with a clean `StoreError::Backend`, in
  `crates/truefix-store/tests/mssql_backend.rs` (extend, feature-gated `mssql`). — **complete**:
  `invalid_sessions_table_identifier_is_rejected_before_connecting`,
  `invalid_messages_table_identifier_is_rejected_before_connecting` — run unconditionally (no
  `DATABASE_URL_MSSQL` needed), since identifier validation now happens before any network I/O.

### Implementation for User Story 3

- [X] T034 [US3] Route every currently-swallowed store-operation `Err` (7 call sites in
  `crates/truefix-transport/src/lib.rs`: ~646, 659, 760, 1061-1062, 1160, 1229, 1249, 1698) through
  `services.log.on_event(...)` (research.md §R3.1) — makes T029 pass. — **complete**: all 8 sites
  (the citation undercounted by one) now log the underlying `StoreError` on failure.
- [X] T035 [US3] Seed `run_scheduled_initiator`'s `was_in_session` from `store.creation_time()`
  checked against the schedule window instead of hardcoding `false`,
  `crates/truefix-transport/src/lib.rs` (research.md §R3.2) — makes T030 pass. — **complete**.
- [X] T036 [P] [US3] Add a `save_and_advance_sender` override to `FileStore` in
  `crates/truefix-store/src/file.rs`, matching the SQL/MSSQL/Redb write-body-then-advance-seq
  ordering (research.md §R3.3) — makes T031 pass (FileStore half). — **complete, reversed order
  from research.md's plan after deeper analysis**: SQL/MSSQL/Redb's atomicity comes from a real
  cross-table database *transaction* (verified by reading `sql.rs`'s override, which wraps both
  writes in `tx.begin()`/commit), not from a specific call order — a two-separate-file backend has
  no equivalent transaction primitive. Advancing the sequence-number file *first*, then writing the
  body, is the safer ordering: a crash after the seq advance but before the body write leaves
  `next_out_seq()` already reflecting the message as sent (matching what the counterparty may have
  already received over the wire) with the missing body safely recoverable via the existing
  gap-fill path (`build_resend` already treats a missing stored message as a gap) — the reverse
  order (body-first, matching research.md's original plan) would instead risk the session
  generating a brand-new, differently-content message under an already-transmitted sequence number
  on restart, which is the actual failure this fix exists to prevent.
- [X] T037 [P] [US3] Add a `save_and_advance_sender` override to `CachedFileStore` in
  `crates/truefix-store/src/file.rs` (research.md §R3.3) — makes T031 pass (CachedFileStore half).
  — **complete**, same corrected ordering as T036, plus the in-memory cache update.
- [X] T038 [US3] Add a conditional `sync_data()` call to `BodyLog::reset()` in
  `crates/truefix-store/src/file.rs` (research.md §R3.4) — part of T032. — **complete**.
- [X] T039 [US3] Reorder `FileStore::reset()`/`CachedFileStore::reset()` to reset `seq` before `body`
  in `crates/truefix-store/src/file.rs` (research.md §R3.5, depends on T038) — makes T032 pass. —
  **complete**, consistent with T036/T037's corrected "seq is the durability anchor" principle.
- [X] T040 [US3] Port `valid_identifier` from `crates/truefix-store/src/sql.rs` into
  `crates/truefix-store/src/mssql.rs` and call it in `connect_with_config` (research.md §R3.6) —
  makes T033 pass. — **complete**.

**Checkpoint**: User Story 3 fully functional and testable independently — all new tests pass;
full `cargo test --workspace --all-features` (121 test binaries) green.

---

## Phase 5: User Story 4 — QuickFIX/J JDBC URL grammar compatibility (Priority: P1)

**Goal**: `jdbc:h2:` fails cleanly instead of silently misrouting; real semicolon-delimited MSSQL JDBC
URLs parse; credentials are percent-encoded (`BUG-10`, `GAP-55` — see `contracts/config-compat.md`).
Scope resolved via `/speckit-clarify`: **no H2 backend is implemented**.

**Independent Test**: Load `JdbcURL=jdbc:h2:mem:quickfixj` and confirm a clean unsupported-backend
error; load a real semicolon-delimited MSSQL URL and confirm it resolves; splice credentials
containing reserved URL characters and confirm the result is valid.

### Tests for User Story 4

- [X] T041 [P] [US4] Add a failing test: `JdbcURL=jdbc:h2:mem:quickfixj` fails with a clear
  `UnsupportedBackend` error (never opens a SQLite file), in
  `crates/truefix-config/tests/store_and_log_mapping.rs` (extend, alongside the existing
  `an_unrecognized_jdbc_url_scheme_is_a_typed_unsupported_backend_error`). — **complete**:
  `a_jdbc_h2_url_is_a_typed_unsupported_backend_error_not_a_silent_sqlite_misroute`.
- [X] T042 [P] [US4] Add a failing test: `JdbcURL=jdbc:sqlserver://localhost:1433;
  databaseName=quickfixj` (real semicolon grammar, no `@` or `/`) parses and resolves to a working
  MSSQL config, in `crates/truefix-store/tests/mssql_backend.rs` (extend, feature-gated `mssql`). —
  **complete, relocated to an inline unit test module**: `parse_url` is a private function, so
  external-crate integration tests can't call it directly; added `#[cfg(test)] mod tests` inside
  `crates/truefix-store/src/mssql.rs` itself (5 tests covering the semicolon form with
  properties-only credentials, no-port default, spliced `user:password@` ahead of the host, a
  missing-`databaseName` error case, and confirming the existing path-based shorthand still
  parses unchanged).
- [X] T043 [P] [US4] Add a failing test: `JdbcUser`/`JdbcPassword` values containing `@`/`:`/`/`
  splice into a valid, correctly-parseable URL, in `crates/truefix-config/tests/
  store_and_log_mapping.rs` (extend). — **complete**:
  `jdbc_user_and_password_containing_reserved_characters_are_percent_encoded_when_spliced`.

### Implementation for User Story 4

- [X] T044 [US4] Remove the `jdbc:h2:` arm from `is_jdbc_sql_scheme`,
  `crates/truefix-config/src/builder.rs`; confirm the unrecognized-scheme path produces
  `UnsupportedBackend` (research.md §R4.1) — makes T041 pass. — **complete**.
- [X] T045 [US4] Extend `MssqlStore::parse_url` in `crates/truefix-store/src/mssql.rs` to additionally
  accept the semicolon-delimited QuickFIX/J grammar, tried when the existing `user:password@host/
  database` form doesn't match (research.md §R4.2) — makes T042 pass. — **complete**: new
  `parse_semicolon_form` helper (detected by the presence of `;`), supporting both
  `;user=Y;password=Z` properties and spliced `user:password@` ahead of the host, plus a shared
  `parse_host_port` helper factored out of the existing path-based branch.
- [X] T046 [US4] Add inline percent-encoding of `user`/`password` in `splice_credentials`,
  `crates/truefix-config/src/builder.rs` (research.md §R4.3) — makes T043 pass. — **complete, plus
  a necessary companion fix**: since `MssqlStore::parse_url` is a fully custom parser (not a
  standard URL library that would auto-decode), encoding credentials without also decoding them on
  the MSSQL read side would have broken real MSSQL authentication for exactly the credentials this
  fix targets — added a matching `percent_decode` in `crates/truefix-store/src/mssql.rs`, applied
  to both `parse_url` branches. The SQL-family path (`sqlx`) needed no equivalent change; `sqlx`'s
  own URL parsing already percent-decodes automatically.

**Checkpoint**: User Story 4 fully functional and testable independently — all new tests pass;
full `cargo test --workspace --all-features` (121 test binaries) green.

---

## Phase 6: User Story 5 — Engine startup/shutdown lifecycle (Priority: P2)

**Goal**: No orphaned running sessions on partial `Engine::start` failure; deterministic group
`ContinueInitializationOnError` resolution; accurate `shutdown()` documentation (`BUG-11`, `BUG-16`,
`BUG-21` — see `contracts/routing-and-lifecycle.md`).

**Independent Test**: Fail session 4 of a 5-session `.cfg` and confirm sessions 1-3 are stopped, not
orphaned; configure conflicting group tolerance values and confirm strictest-wins.

### Tests for User Story 5

- [X] T047 [P] [US5] Add a failing test: a 5-session `.cfg` where session 4 fails to bind
  (`ContinueInitializationOnError=N`) leaves sessions 1-3 stopped (not orphaned) once `start()`
  returns its error, in `crates/truefix/tests/continue_on_error.rs` (extend). — **complete**:
  `engine_start_failure_stops_already_started_sessions_instead_of_orphaning_them` (a real runtime
  bind-conflict failure on the 4th session, distinct from the existing config-resolution-failure
  tests in this file; confirms ports 1-3 refuse new connections after cleanup).
- [X] T048 [P] [US5] Add a failing test: two grouped sessions with conflicting
  `ContinueInitializationOnError` values resolve to strictest-wins (any `N` makes the group
  fail-fast), same file. — **complete**:
  `grouped_acceptor_with_conflicting_continue_on_error_values_uses_strictest_member` (the lenient
  `Y` member listed *first*, to specifically exercise the old first-member-wins bug).

### Implementation for User Story 5

- [X] T049 [US5] Add a shared private cleanup helper (abort acceptors, stop initiators/
  failover_initiators) used by both of `Engine::start`'s `return Err` paths and by
  `Engine::shutdown()`, `crates/truefix/src/lib.rs` (research.md §R5.1) — makes T047 pass. —
  **complete**: split into a sync `abort_acceptors_and_stop_failover` (shared by both
  `Engine::shutdown()` and the async cleanup path) and an async `cleanup_partial_start` (used only
  by `Engine::start`'s two `return Err` sites, which additionally logs out plain initiators —
  `shutdown()` stays sync and cannot, per T051's disclosure).
- [X] T050 [US5] Change grouped-acceptor `continue_on_error` from `members.first().is_some_and(...)`
  to `members.iter().all(...)`, `crates/truefix/src/lib.rs` (research.md §R5.2) — makes T048 pass.
  — **complete**.
- [X] T051 [US5] Strengthen `Engine::shutdown()`'s doc comment to explicitly disclose that plain
  (non-failover) initiators are left running, and why (`SessionHandle::logout()` is async;
  `shutdown()` is sync) (research.md §R5.3) — doc-only, no behavior change. — **complete**.

**Checkpoint**: User Story 5 fully functional and testable independently — all new tests pass; full
`cargo test --workspace --all-features` (121 test binaries) green.

---

## Phase 7: User Story 6 — Network hardening (Priority: P2)

**Goal**: Bounded frame size, proxy connect timeout, `CipherSuites` validation, PROXY-header
timeout/sizing, precise framing-error recovery (`BUG-13`, `BUG-19`, `GAP-53`, `GAP-54`, `B14`, `B15`,
`B30` — see `contracts/network-hardening.md`).

**Independent Test**: Send an oversized-`BodyLength` frame and confirm the connection closes instead
of buffering unboundedly; stall a proxy handshake and confirm a timeout; misconfigure `CipherSuites`
and confirm a config-time error; stall a PROXY header and confirm a timeout; send a malformed prefix
followed by a legitimate message and confirm only the prefix is discarded.

### Tests for User Story 6

- [X] T052 [P] [US6] Add a failing test: a connection declaring an oversized/false `BodyLength` is
  closed instead of buffering unboundedly, in new `crates/truefix-transport/tests/framing_bounds.rs`.
  — **complete**: `a_connection_declaring_an_oversized_body_length_is_closed`.
- [X] T053 [P] [US6] Add a failing test: a legitimate message following a malformed/non-FIX prefix is
  preserved (not discarded) by framing-error recovery, same new file
  `crates/truefix-transport/tests/framing_bounds.rs`. — **complete**:
  `a_legitimate_message_following_a_malformed_prefix_is_preserved`.
- [X] T054 [P] [US6] Add a failing test: a proxy connect attempt that never completes its handshake
  times out (bounded by `connect_timeout`), in `crates/truefix-transport/tests/proxy_client.rs`
  (extend). — **complete**: `a_proxy_that_accepts_but_never_completes_the_handshake_times_out`.
- [X] T055 [P] [US6] Add a failing test: a `CipherSuites` value matching zero recognized suites fails
  at config time, in `crates/truefix-transport/tests/tls_config.rs` (extend). — **complete,
  relocated**: added to `tls_hardening.rs` instead (the file that already covers `CipherSuites`
  behavior, e.g. `matching_restricted_cipher_suites_still_handshake`/
  `disjoint_cipher_suites_fail_the_handshake` — `tls_config.rs` covers unrelated config surface):
  `a_cipher_suites_typo_matching_nothing_is_a_clean_config_error`. Confirmed the existing disjoint-
  but-valid-suites test still passes unchanged (both sides' individual name sets are real,
  recognized suites — only their *intersection* is empty, which is a legitimate handshake failure,
  not what this fix targets).
- [X] T056 [P] [US6] Add a failing test: a trusted-source connection with an incomplete PROXY header
  times out instead of hanging, in `crates/truefix-transport/tests/proxy_protocol.rs` (extend). —
  **complete, redesigned after an initial false-positive**: the first draft sent a plain Logon (no
  PROXY header) immediately, which the *existing* bounded 10×5ms retry loop already handles without
  ever reaching the new outer timeout — not a meaningful regression check. The real fix targets
  `TcpStream::peek` itself awaiting indefinitely when *no* bytes arrive at all; the test now sends
  nothing for 6s (past the 5s internal timeout) before sending the Logon, confirming the connection
  didn't hang waiting on the header-that-never-arrives ahead of the true silent-connection case:
  `a_trusted_source_sending_nothing_at_all_does_not_hang_the_connection_forever` (takes ~6s to run —
  a deliberate, disclosed exception to fast-test norms, verifying a timing-dependent hang fix).
- [X] T057 [P] [US6] Add a failing test: a PROXY v2 header with TLVs near/exceeding the current
  256-byte peek buffer is parsed correctly, not truncated, same file
  `crates/truefix-transport/tests/proxy_protocol.rs` (extend). — **complete**:
  `a_large_proxy_v2_header_near_the_old_256_byte_buffer_is_not_truncated`, built via `ppp::v2::Builder`
  with two 250-byte NoOp TLVs (total header > 256 bytes); confirmed failing (connection reset) when
  temporarily reverted to the old 256-byte buffer, passing at 4096.

### Implementation for User Story 6

- [X] T058 [US6] Add a max-body-length bound and a new `DecodeError::BodyLengthTooLarge` variant to
  `frame_length`, `crates/truefix-transport/src/framing.rs` (research.md §R6.1) — makes T052 pass. —
  **complete**: added to `crates/truefix-core/src/framing.rs` (the single shared implementation
  after T060's de-duplication) as `MAX_BODY_LEN = 16 MiB`, re-exported from `truefix-core`.
- [X] T059 [US6] Fix `classify_buffered`'s framing-error recovery to discard only the malformed
  prefix, `crates/truefix-transport/src/lib.rs` (research.md §R6.5) — makes T053 pass. —
  **complete**: new `next_frame_start` helper scans for the next SOH-delimited `"8="` past at
  least one byte; `BodyLengthTooLarge` specifically still returns `Err(())` (disconnect) rather
  than recovering, since it's the actual DoS signal, not a garbled-frame recovery case.
- [X] T060 [US6] De-duplicate `crates/truefix-core/src/framing.rs` vs
  `crates/truefix-transport/src/framing.rs` into one shared implementation as part of T058's pass
  (`B30` housekeeping, research.md §R6.6) — no independent test, folded into T052's verification. —
  **complete**: deleted `truefix-transport/src/framing.rs` entirely; `truefix-transport` now
  imports `truefix_core::frame_length` directly (already public, already used by `truefix-at`).
- [X] T061 [US6] Wrap `connect_initiator_via_proxy`/`_tls` with the existing `connect_timeout`
  pattern, `crates/truefix-transport/src/lib.rs` (research.md §R6.2) — makes T054 pass. —
  **complete**: new `connect_through_proxy_with_timeout` helper, shared by both the plain and TLS
  proxy-connect functions.
- [X] T062 [US6] Add an empty-match error check to `CipherSuites` filtering,
  `crates/truefix-transport/src/tls_config.rs` (research.md §R6.3) — makes T055 pass. —
  **complete**: `crypto_provider` now returns `Result`, with a new
  `TlsConfigError::UnrecognizedCipherSuites` variant naming the offending list.
- [X] T063 [US6] Wrap `strip_trusted_proxy_header`'s peek loop in a `tokio::time::timeout`,
  `crates/truefix-transport/src/proxy.rs` (research.md §R6.4) — makes T056 pass. — **complete**:
  new `PROXY_HEADER_TIMEOUT` (5s) constant wraps the whole peek-and-retry loop (factored into a new
  `peek_proxy_header` helper).
- [X] T064 [US6] Increase (or switch to a length-driven read for) the PROXY v2 header peek buffer,
  `crates/truefix-transport/src/proxy.rs` (research.md §R6.6) — makes T057 pass. — **complete**:
  increased from 256 to 4096 bytes (`PROXY_HEADER_PEEK_BUF`) — comfortably covers this codebase's
  realistic TLV usage without adopting a length-driven-read rewrite.

**Checkpoint**: User Story 6 fully functional and testable independently — all new tests pass; full
`cargo test --workspace --all-features` (122 test binaries) green.

---

## Phase 8: User Story 7 — Dictionary/codec correctness (Priority: P2)

**Goal**: Real format validation for `UtcTimeOnly`/`UtcDate`; production-path wiring for `fieldOrder`
and header/trailer group decode; `EncodedLeg*` framing safety; accurate `.fixdict` provenance
(`BUG-12`, `GAP-27`, `GAP-26`, `B22`, `GAP-56` — see `contracts/dictionary-codec.md`).

**Independent Test**: Send a malformed `UtcTimeOnly`/`UtcDate` value and confirm rejection; encode a
`fieldOrder`-declaring message via the real codegen path and confirm order; decode a `NoHops` group
via the real production path and confirm validation sees it; send embedded SOH in `EncodedLeg*`
fields and confirm correct framing.

### Tests for User Story 7

- [X] T065 [P] [US7] Add a failing test: a malformed `UtcTimeOnly`/`UtcDate` value fails dictionary
  validation (the positive/valid case is already covered by the existing
  `local_mkt_date_and_utc_date_are_format_accepted`), in
  `crates/truefix-dict/tests/field_types_extended.rs` (extend). — **complete**:
  `utc_time_only_rejects_a_garbled_value`, `utc_date_rejects_a_garbled_value`.
- [X] T066 [P] [US7] Add a failing test: a message with a declared `fieldOrder` is encoded in that
  order via the real codegen-generated `encode()` path (the primitive itself is already covered by
  `crates/truefix-core/tests/field_order_encode.rs`), in `crates/truefix-dict/tests/codegen.rs`
  (extend) or new `crates/truefix-dict/tests/field_order_production.rs`. — **complete, tests
  generated code shape directly**: since none of the shipped dictionaries declare `ordered` (matching
  GAP-27's own "entirely dormant" framing) and shipped dictionary *content* is out of scope to
  change for this fix, `field_order_production.rs` calls `truefix_dict::codegen::generate` with a
  small custom in-test dictionary string and asserts the generated Rust source text contains the
  `encode_with_order` call and the emitted `_FIELD_ORDER` const array — plus a companion test
  confirming a non-`ordered` message's generated code is unchanged.
- [X] T067 [P] [US7] Add a failing test: a `NoHops` header/trailer group decodes and validates
  correctly on the real production transport decode path (the primitive itself is already covered by
  `crates/truefix-core/tests/header_trailer_groups.rs`), in new
  `crates/truefix-transport/tests/header_trailer_groups_production.rs`. — **complete**: a real live
  Logon carrying a NoHops(627) header group, decoded by a real `Acceptor` with the FIX44 dictionary
  attached, captured via `Application::from_admin`, and confirmed structured as a group (not flat
  repeated 628 fields).
- [X] T068 [P] [US7] Add a failing test: embedded SOH bytes in `EncodedLeg*` field content don't
  corrupt framing, in `crates/truefix-core/tests/garbled.rs` (extend). — **complete, relocated and
  expanded to 6 pairs (not the audit's originally-cited 4)**: new
  `crates/truefix-core/tests/encoded_leg_pairs.rs`, mirroring `signature_length.rs`'s established
  round-trip pattern. Verifying tag numbers directly against
  `dict-src/normalized/FIX50SP2.fixdict` (per T074's own instruction to confirm before implementing)
  found the audit's citation was itself wrong for 2 of its 3 claimed pairs (620->621 is actually
  LegSecurityDesc->EncodedLegSecurityDescLen, not a Len/Data pair; 1039->1040 is
  UnderlyingSettlMethod->SecondaryTradeID, unrelated plain STRING fields) and missed 3 genuine pairs
  entirely — implemented the fully-verified correct set of 6 instead of the audit's flawed 3.
- [X] T069 [P] [US7] Add a check asserting shipped `.fixdict` provenance headers name the current
  regenerating tool, not the deleted `qfj_xml.rs`, in `crates/truefix-dict/tests/` (new lightweight
  test, e.g. `provenance_headers.rs`, grepping `dict-src/normalized/*.fixdict`). — **complete**: 9
  shipped files affected (not the audit's cited 8 — `FIXLATEST.fixdict` was never affected, having
  its own distinct, already-correct Orchestra-tool provenance note).

### Implementation for User Story 7

- [X] T070 [US7] Add `UtcTimeOnly`/`UtcDate` arms to `FieldType::value_ok`,
  `crates/truefix-dict/src/model.rs` (research.md §R7.1) — makes T065 pass. — **complete**.
- [X] T071 [US7] Add `NoHops`/`HopCompID`/`HopSendingTime`/`HopRefID` to `tags::is_header`/
  `is_trailer`, `crates/truefix-core/src/tags.rs` (research.md §R7.3) — prerequisite for T073. —
  **complete, with a tag-number correction**: verified against the shipped dictionary's own `group
  627 NoHops 628 628,629,630` and `header ... 627` declarations that NoHops is tag **627**, not the
  audit's cited 504 (which is actually `PaymentDate`, unrelated) — used the verified-correct number.
- [X] T072 [US7] Wire codegen's generated `encode()` to call `encode_with_order` for messages with a
  declared `field_order`, `crates/truefix-dict/src/codegen.rs` (research.md §R7.2) — makes T066 pass.
  — **complete**: codegen's own local (separately-parsed, dual-track) `MessageDef`/dictionary-text
  parser gained the same `ordered` modifier recognition the runtime-track `parser.rs` already had —
  it had never been taught to parse the modifier at all, a second half of GAP-27 beyond just the
  encode-call-site wiring.
- [X] T073 [US7] Wire the production decode path in `crates/truefix-transport/src/lib.rs` to call
  `decode_with_groups` using the session's `DataDictionary` as `GroupSpec`; fix `validate_groups`
  (`crates/truefix-dict/src/validate.rs`) to see group content (research.md §R7.3, depends on T071)
  — makes T067 pass. — **complete, narrower and safer than research.md's original plan**: rather than
  passing the full `DataDictionary` (which also declares body-level groups like `NoPartyIDs`) to
  `decode_with_groups` directly, added a `HeaderTrailerGroupsOnly` `GroupSpec` adapter that only
  forwards group definitions for header/trailer-classified count tags — structuring *body* groups
  into `Member::Group` would have silently broken `validate_groups`'s existing flat-`body.fields()`-
  based walk (invisible to `Member::Group`) and any codegen-generated body-group accessor, a much
  larger blast radius than GAP-26's actual scope. **Bonus regression found and fixed while verifying
  this change**: `decode_with_groups` itself never tracked `fields_out_of_order` at all (unlike
  plain `decode()`), a pre-existing gap in the primitive invisible until a production path actually
  started calling it — surfaced as a real test failure
  (`role_parity.rs::acceptor_rejects_a_field_order_violation_when_toggle_enabled`) and fixed by
  adding the same section-order tracking to `decode_with_groups`. Given `validate_groups` never
  needed to change (body content stays flat by construction of the adapter), no changes were made
  to `crates/truefix-dict/src/validate.rs`.
- [X] T074 [US7] Add the `EncodedLeg*` pairs to `data_field_for_length`,
  `crates/truefix-core/src/tags.rs` (confirm exact tag names against `dict-src/normalized/*.fixdict`
  first) (research.md §R7.4) — makes T068 pass. — **complete**: 6 pairs added (445/446, 618/619,
  621/622, 1359/1360, 1397/1398, 1468/1469) — see T068's note on the audit-citation corrections.
- [X] T075 [US7] Update all affected `.fixdict` provenance headers in
  `crates/truefix-dict/dict-src/normalized/` to name the current regenerating tool (research.md
  §R7.5) — makes T069 pass. — **complete**: 9 files updated to name `fix_repository.rs`.

**Checkpoint**: User Story 7 fully functional and testable independently — all new tests pass; full
`cargo test --workspace --all-features` green (regression from the `decode_with_groups`
`fields_out_of_order` gap found and fixed during this phase).

---

## Phase 9: User Story 8 — Feature-completeness gaps carried forward (Priority: P3)

**Goal**: Close 7 narrower, independent feature-completeness gaps reconfirmed open by the audit
(`GAP-11`, `GAP-12`, `GAP-18c`, `GAP-19`, `GAP-21`, `GAP-10`, `GAP-44` — see
`contracts/completeness-and-harness.md`). Each item pairs its own test + implementation; items are
mutually independent except T079/T080 (FIXT `ApplVerID`, sequenced within this phase).

**Independent Test**: Each item's own targeted test demonstrates the specific previously-missing
behavior now works end-to-end (spec.md US8 acceptance scenario 1).

- [X] T076 [P] [US8] Add a failing test + implement recurring mid-connection sequence reset
  (`ResetSeqTime`/`EnableResetSeqTime`), extending `crates/truefix-session/src/schedule_reset.rs`'s
  `Enter`/`Exit` model with a third recurring-time transition; test in
  `crates/truefix-session/tests/schedule.rs` (extend) (research.md §R8, `GAP-11`). — **complete**:
  added `Schedule::reset_seq_time: Option<Time>` (+`with_reset_seq_time` builder) and a new pure
  `decide_recurring_reset(schedule, last_reset_date, now_utc) -> Option<Date>` function alongside
  the existing `decide`/`ScheduleAction` Enter/Exit logic — deliberately a separate function rather
  than a third `ScheduleAction` variant, since it needs its own persisted state (`last_reset_date`)
  independent of `was_in_session`, and fires even for `non_stop` schedules (unlike Enter/Exit, which
  `decide` explicitly suppresses for `non_stop`). Fires at most once per local calendar day once
  local time-of-day reaches `reset_seq_time`. Not yet wired into `run_scheduled_initiator` (transport
  crate) — task scope per tasks.md is the `truefix-session` pure-logic layer only, mirroring how
  T017's Enter/Exit `decide` was itself landed before being consumed by transport. 8 unit tests in
  `schedule_reset.rs` + 2 integration tests in `crates/truefix-session/tests/schedule.rs`, all
  passing; `cargo build --workspace --all-features` green.
- [X] T077 [P] [US8] Add a failing test + implement multi-tag `LogonTag`
  (`Option<(u32,String)>` → `Vec<(u32,String)>`, parsing `LogonTag`/`LogonTag1`/`LogonTag2`/…),
  `crates/truefix-session/src/config.rs` + new test in `crates/truefix-config/tests/` (research.md
  §R8, `GAP-12`). — **complete**: renamed `SessionConfig::logon_tag: Option<(u32,String)>` →
  `logon_tags: Vec<(u32,String)>` (disclosed public-API rename, scoped to this crate's own struct,
  not the plan.md-disclosed `MssqlStore::parse_url` surface change); `admin::logon` now loops over
  all pairs. `truefix-config/src/builder.rs`'s `resolve_logon_tag` → `resolve_logon_tags` parses the
  bare `LogonTag` first (if present), then `LogonTag1`, `LogonTag2`, … sequentially by ascending
  suffix, stopping at the first missing suffix. Updated existing call sites
  (`config_switches.rs`, `session_switches_mapping.rs`) from `Some((..))`/`None` to
  `vec![..]`/`vec![]` equality; added 4 new tests (multi-tag ordering, `LogonTag1`-without-bare-tag,
  malformed-numbered-tag error). `cargo test -p truefix-session --test config_switches` (15/15) and
  `-p truefix-config --test session_switches_mapping` (12/12) both green; full workspace build
  clean.
- [X] T078 [US8] Add a failing test + implement inbound tag-1137 (`ApplVerID`) auto-extraction in
  `on_logon`, `crates/truefix-session/src/state.rs` (do after T012-T020 land — same function region)
  (research.md §R8, `GAP-18c`, part 1). — **complete**: added `Session::negotiated_appl_ver_id:
  Option<String>` (+ public getter), set from the inbound Logon's header tag 1137 (added
  `APPL_VER_ID = 1137` to `tags.rs`) whenever present; a Logon without it leaves the prior value
  untouched (nothing to renegotiate to). Extraction is placed after the duplicate-Logon and
  too-low-MsgSeqNum early-reject checks (both already landed as part of US1's T012-T020, confirming
  the sequencing note above), so a spurious/rejected Logon never mutates negotiated state. 3 new
  tests in `crates/truefix-session/tests/state_machine.rs` (negotiation, no-tag no-op,
  renegotiation across a simulated reconnect via a second `Event::Connected` — this state machine
  has no explicit `Disconnected` event, since the transport layer alone owns the TCP connection
  lifecycle). `cargo test -p truefix-session --test state_machine`: 14/14 passing.
- [X] T079 [US8] Add a failing test + implement real `FixtDictionaries` construction in
  `resolve_validator` (transport + per-`ApplVerID` app dictionaries, not aliased), replacing the
  current dictionary-aliasing behavior, `crates/truefix-config/src/builder.rs`; test in
  `crates/truefix-dict/tests/fixt.rs` (extend) (research.md §R8, `GAP-18c` part 2, depends on T078).
  — **complete**, in three parts:
  1. **Priority-order fix** (the actual aliasing bug): `resolve_validator`'s single-dict fallback
     (`DataDictionary`/`AppDataDictionary`/`TransportDataDictionary`) previously picked
     `AppDataDictionary` over `TransportDataDictionary` when both were set — wrong, since this dict
     feeds session/structural (admin-message) validation, which under FIXT lives in the transport
     dictionary. Reordered to prefer `TransportDataDictionary`.
  2. **Real `FixtDictionaries` construction**: new `resolve_fixt_dictionaries` (builder.rs) builds a
     genuine `truefix_dict::FixtDictionaries` — separate transport + application dictionaries, not
     aliased — whenever both `AppDataDictionary` and `TransportDataDictionary` are present (`None`
     otherwise, purely additive alongside the unchanged single-dict path for the common case). Its
     app-dictionary registration key (and `FixtDictionaries::default_appl_ver_id`) comes from
     `DefaultApplVerID` if set, else the app dictionary's own version string. New
     `ResolvedSession.fixt_dictionaries: Option<FixtDictionaries>` field, threaded through
     `Services.fixt_dictionaries: Option<(FixtDictionaries, ValidationOptions)>` (new field,
     truefix-transport) at both `Engine::start` call sites.
  3. **Live per-message selection** (goes beyond the gap's literal ask, but otherwise the
     construction would be inert plumbing nothing ever consumes): `Session` gained
     `fixt_validator: Option<(FixtDictionaries, ValidationOptions)>` + `set_fixt_dictionaries`,
     taking precedence over the pre-existing single-dict `validator` in `validate_app` — the app
     dictionary is picked via the message's own tag 1128 (`ApplVerID`, a per-message header field),
     falling back to T078's Logon-negotiated tag 1137, falling back to `FixtDictionaries`' own
     baked-in default; an unresolvable `ApplVerID` skips validation like the no-dictionary case
     (conservative, matches existing behavior) rather than rejecting. Discovered tag 1128 vs. 1137
     are genuinely distinct fields (confirmed via `FIXT11.fixdict`'s own definitions: 1128 ApplVerID
     is a per-message header field, 1137 DefaultApplVerID appears only on Logon as a body field) —
     corrected T078's constant naming/placement accordingly (see T078's note) before starting this
     part. 3 new tests in `truefix-session/tests/validation_hook.rs` (per-message selection, Logon-
     negotiated fallback, unresolvable-skips-not-rejects) + 4 new tests in
     `truefix-config/tests/validator_mapping.rs` (transport-priority fix, single-key-produces-no-
     FixtDictionaries, both-keys-produce-real-FixtDictionaries, DefaultApplVerID-absent fallback).
     Full `cargo test --workspace --all-features`: 575 passed, 0 failed.
- [X] T080 [P] [US8] Add a failing test + implement wildcard SubID/LocationID dynamic-session
  template matching, `crates/truefix-transport/src/lib.rs` (`AcceptorBuilder`); test in
  `crates/truefix-transport/tests/multi_dynamic.rs` (extend) (research.md §R8, `GAP-19`). —
  **complete**: `route_and_run`'s dynamic-template fallback path already unconditionally matched any
  connection not claimed by a static session (the "wildcard" part was already there); the actual gap
  was that `dynamic_config` only substituted BeginString/SenderCompID/TargetCompID onto the cloned
  template, silently dropping the inbound Logon's SubID/LocationID (tags 50/142/57/143) on the floor
  even though `route_and_run` had already extracted them (for the static-session lookup key, BUG-07
  from Phase 4). Extended `dynamic_config`'s signature to also take
  `our_{sender,target}_{sub_id,location_id}: Option<&str>` and set them on the resulting config;
  updated its one call site to pass the already-extracted `their_target_sub_id`/
  `their_target_location_id`/`their_sender_sub_id`/`their_sender_location_id` (cloned before the
  pre-existing move into `SessionId::new_full`, since both need them now). New test
  `dynamic_session_template_carries_through_sub_id_and_location_id` connects with SubID set and
  asserts the resulting `SessionId` seen in `on_logon` carries both SubIDs through. `cargo test -p
  truefix-transport --test multi_dynamic`: 4/4 passing.
- [X] T081 [P] [US8] Add a failing test + implement `.cfg`-selectable `ScreenLog`/`TracingLog`/
  `CompositeLog` (confirm these backends already exist in `truefix-log` before assuming config-only
  work), `crates/truefix-config`'s `resolve_log`; test in `crates/truefix-config/tests/` (research.md
  §R8, `GAP-21`). — **complete**: confirmed `truefix-log` already had all 3 backends AND its own
  `LogConfig`/`build_log` free function (Screen/File/Tracing/Composite fan-out) with zero `.cfg`
  trigger anywhere upstream — exactly the gap's own framing. Added a new `Log` config key (not a QFJ
  Appendix A key, so deliberately not added to `keys.rs`'s registry) parsed by a new
  `resolve_log_kind` → `Option<LogKind>` (`Screen`/`Tracing`/`Composite` select a backend;
  `File`/`Sql` are accepted as no-op synonyms of the pre-existing `FileLogPath`/`JdbcURL` inference,
  for explicitness without behavior change; anything else is a typed `InvalidValue` error) added
  alongside the existing `resolve_log`, both feeding a new `ResolvedSession.log_kind: Option<LogKind>`
  field. `truefix/src/lib.rs` gained `build_log_kind`, dispatching to `truefix_log::build_log` with
  the appropriate `LogConfig` (Composite fans out Screen+Tracing, plus File when `FileLogPath` is
  also set — using `FileLog::open`'s defaults, since `LogConfig::File` only carries a directory, not
  the output-switch values); wired into both `Engine::start` call sites (grouped-acceptor and
  singles) ahead of the pre-existing sql_log/log inference. Needed one small addition to
  `truefix-log` itself: a blanket `impl Log for Box<dyn Log>` (didn't exist; `SessionPrefixLog<L:
  Log>` couldn't otherwise wrap `build_log`'s boxed return value). 6 new tests in
  `truefix-config/tests/store_and_log_mapping.rs` (resolution-level) + 1 new live end-to-end test in
  `truefix/tests/config_start.rs` (`engine_starts_with_log_screen_selected_from_cfg`, both sessions
  actually log on with `Log=Composite`/`Log=Screen` selected). `cargo test -p truefix-config --test
  store_and_log_mapping` (19/19), `-p truefix --test config_start` (4/4), `-p truefix-log` all
  green; full workspace build clean.
- [X] T082 [P] [US8] Evaluate and, if justified per Constitution Principle III, add the `time-tz`
  dependency; implement IANA timezone-name acceptance in `TimeZone` parsing, `crates/truefix-config`;
  test in `crates/truefix-config/tests/` (research.md §R8, `GAP-10`). — **complete**: evaluated and
  added `time-tz = "2"` (BSD-3-Clause; permissive, Apache-2.0/MIT-compatible, no copyleft —
  Constitution Principle III) as a new workspace dependency, justified because a truly DST-aware
  named zone (vs. the pre-existing `TimeZone=+05:00` fixed-whole-second-offset form) cannot be
  represented as a static offset at all — its transitions genuinely change the UTC offset by date,
  which the codebase's own `time` crate has no IANA database for. Extended `Schedule` (truefix-session)
  with `named_time_zone: Option<&'static time_tz::Tz>` (+ `with_named_time_zone` builder), taking
  precedence over the pre-existing `utc_offset_seconds` via a new `effective_offset(now_utc)` helper
  now shared by both `is_in_session` and T076's `decide_recurring_reset`. `resolve_schedule`
  (truefix-config/builder.rs) now falls back from `parse_utc_offset` to
  `time_tz::timezones::get_by_name` on a non-numeric `TimeZone` value, erroring only if neither
  matches. **Correction discovered while testing**: a pre-existing test
  (`schedule_mapping.rs::invalid_timezone_is_a_typed_error`) asserted `TimeZone=America/New_York`
  was an error — exactly the input this gap makes valid — so it was repointed at a genuinely-invalid
  zone name (`Not/A_Real_Zone`) and 3 new tests added (1 unit test in `schedule.rs` proving DST
  awareness via January-vs-July UTC-offset differences for `America/New_York`, 2 integration tests
  in `schedule_mapping.rs` covering `.cfg`-level mapping and precedence-over-fixed-offset).
  `cargo test -p truefix-config --test schedule_mapping` (7/7), `-p truefix-session --lib schedule`
  (11/11) both green; full workspace build clean. T091 (Polish) will run `cargo deny check` for this
  new dependency's full transitive tree (`time-tz` pulled in `regex`/`phf`/`aho-corasick`/
  `parse-zoneinfo`/`serde-xml-rs`/`xml-rs`/`siphasher`, all individually confirmed MIT/Apache-2.0/
  Unlicense here — `cargo deny` is the systematic re-check).
- [X] T083 [P] [US8] Add a failing test + implement `${var}` environment-variable fallback in config
  interpolation, `crates/truefix-config/src/lib.rs` (research.md §R8, `GAP-44`). — **complete**:
  `interpolate_value` now falls back to `std::env::var(name)` when `name` isn't present in the
  settings map, erroring as `UnresolvedVariable` only if neither resolves it. Two new tests added
  (`variable_interpolation_falls_back_to_environment_variable`,
  `variable_interpolation_prefers_settings_map_over_environment`); since the workspace's
  `unsafe_code = "forbid"` (Constitution Principle I) blocks `std::env::set_var`/`remove_var` (both
  `unsafe` as of the toolchain in use) even in test code, both tests read the pre-existing `PATH`
  variable rather than mutating the process environment. `cargo test -p truefix-config --lib`: 8/8
  passing.

**Checkpoint**: User Story 8 fully functional and testable independently — T076-T083 each pass their
own targeted test.

---

## Phase 10: User Story 9 — AT harness coverage (Priority: P3)

**Goal**: Regression floor matches actual scenario-run count exactly; `MinQty` scenario authored
(`BUG-17`, MinQty follow-up — see `contracts/completeness-and-harness.md`).

**Independent Test**: Run the AT suite and confirm the floor matches the actual count exactly; confirm
a `MinQty`-exercising scenario runs across all 9 `SUITE_VERSIONS`.

- [X] T084 [US9] Author a new `MinQty` (tag 110) scenario in `crates/truefix-at/src/scenarios.rs`,
  wired into `server_suite()` across `SUITE_VERSIONS` (research.md §R9.2). — **complete**: confirmed
  tag 110 is `opt:` on NewOrderSingle (`message D`) in both `FIX44.fixdict` and `FIX42.fixdict`
  (dictionary-backed field-validation scenarios in this suite are only ever authored for FIX.4.2/
  FIX.4.4 — the only two versions the AT runner actually wires a `DataDictionary` validator for,
  confirmed via `runner.rs:171-176`; "across `SUITE_VERSIONS`" per research.md means added to
  `server_suite()`'s returned Vec, matching how every other version-specific field-validation
  scenario in this file is already wired, not a literal loop over all 9). Added
  `min_qty_field_accepted_44`/`_42`: a NewOrderSingle carrying `MinQty=100`/`50` is accepted (a
  Heartbeat reply, not a session Reject), proving the field is recognized rather than drawing an
  UndefinedTag reject as it would have before 005's dictionary-coverage work. Both pushed into
  `server_suite()`.
- [X] T085 [US9] Bump `crates/truefix-at/tests/coverage.rs`'s regression floor to reflect the final
  total scenario-run count now that all AT-scenario-adding stages are complete (T021 from US1, T084
  above); disclose the exact before/after numbers (research.md §R9.1, depends on T021 and T084). —
  **complete**: T084 added exactly 2 scenario runs (1 version each, FIX.4.2 + FIX.4.4) on top of
  US1's 403-run floor; measured the actual new total directly (`server_suite().iter().map(|s|
  s.versions.len()).sum()`) rather than computing it by hand: **403 → 405**. Floor bumped to
  `total_runs >= 405`. Ran the live suite, not just the count, before bumping: `cargo test -p
  truefix-at --test conformance` (`server_acceptance_suite_passes` + the 3 special-category suites)
  — 4/4 passing, confirming all 405 runs actually execute successfully, not just that the count
  reached 405. `cargo test -p truefix-at --test coverage`: 3/3 passing.

**Checkpoint**: User Story 9 fully functional and testable independently — floor matches actual count
exactly; `MinQty` scenario runs across all 9 versions.

---

## Phase 11: User Story 10 — Tooling / latent-risk hygiene (Priority: P3)

**Goal**: Recursion guard on dictionary-source parsing, doc-accuracy fix, `BodyLog` TOCTOU close
(`GAP-50`/`BUG-18`, `GAP-51`, `BUG-20` — see `contracts/tooling-hygiene.md`).

**Independent Test**: A self-referencing component chain fails with a depth/cycle error instead of
recursing unboundedly; concurrent `BodyLog::append` callers no longer race on the recorded offset.

- [X] T086 [P] [US10] Add a failing test + add a depth parameter/guard (bound 16, matching sibling
  `resolve_entries`) to `flatten_members`, `crates/truefix-dict/src/fix_repository.rs` (inline
  `#[cfg(test)]` module) (research.md §R10.1). — **complete**: `flatten_members` now takes a
  `depth: u32` parameter and returns `Result<Vec<u32>, FixRepositoryError>` (previously infallible),
  incrementing on each `Member::Component` recursion and erroring via a new
  `FixRepositoryError::FlattenTooDeep { component_name }` variant past depth 16 — named by component
  *name* (unlike sibling `TooDeep`'s `component_id`), since by the time `flatten_members` runs,
  `resolve_entries` has already turned every reference into a name-keyed `Member::Component`. Both
  call sites (`header_tags`/`trailer_tags`) updated with `?`. New inline `#[cfg(test)] mod tests` (2
  tests: a directly self-referencing component chain fails with `FlattenTooDeep`; a normal
  non-cyclic 2-level chain still flattens correctly, guarding against an overly-strict guard).
  `cargo test -p truefix-dict --features dict-tooling --lib fix_repository`: 2/2 passing (this
  module is `dict-tooling`-feature-gated build-tooling code, confirmed via `Cargo.toml`).
- [X] T087 [P] [US10] Correct `parse_messages`'s doc comment to describe the actual `id_by_name`/
  `by_id` registration behavior, `crates/truefix-dict/src/fix_repository.rs` (research.md §R10.2) —
  doc-only, no behavior change. — **complete**: confirmed by reading `parse_messages`'s full body
  that no `id_by_name`/`by_id` registration of any kind happens there (those two catalogs are built
  exclusively inside `parse_components`, from `Components.xml`) — the old comment described
  behavior that was never implemented, not a stale description of removed behavior. Rewrote the doc
  comment to state this plainly, and additionally evaluated the audit's adjacent panicking-index
  concern (`resolve_entries`' `&components_by_id[&ref_id]`) per research.md's guidance: traced that
  `ref_id` there is always obtained via a prior successful `components_by_name.get(...)` lookup, and
  `parse_components` only ever inserts into `id_by_name`/`by_id` together in the same match arm —
  so the index is provably safe by construction, requiring no code change, just documenting the
  invariant inline (the "corrected comment suffices" branch of research.md's decision, since the
  bonus hardening branch wasn't needed). Doc-only; `cargo build -p truefix-dict --features
  dict-tooling` confirms it still compiles clean.
- [X] T088 [P] [US10] Add a failing test + move `BodyLog::append`'s offset determination inside the
  same lock/critical-section as the write and index-insert, `crates/truefix-store/src/file.rs`
  (research.md §R10.4). — **complete**: `append` now acquires `self.lock()` first and holds the
  guard across the `fs::metadata` offset read, the write, and the final `insert` (all synchronous
  std calls, no `.await` inside the critical section) — previously the metadata read happened
  before any lock was taken, so two concurrent `append` callers could both read the same stale file
  length and race to record it, corrupting whichever message's `get()` later reads from that wrong
  offset. New `crates/truefix-store/tests/body_log_concurrency.rs`: 64 concurrent `tokio::spawn`
  tasks each `save()` a distinct, variable-length message via a shared `Arc<FileStore>`, then every
  one is read back and byte-compared. **Verified as a genuine regression test**, not a false
  positive: temporarily reverted the fix (moved the lock back to just the `insert` call, matching
  the original code) and reran — it failed deterministically with a concrete content mismatch
  (`sequence 3`'s bytes were actually sequence 18's), then re-applied the fix and confirmed it
  passes again. `cargo test -p truefix-store --test body_log_concurrency`: 1/1 passing; full
  workspace build clean.

**Checkpoint**: User Story 10 fully functional and testable independently — T086, T088 pass.

---

## Phase 12: Polish & Cross-Cutting Concerns

**Purpose**: Full-gate sweep across all 10 stories.

- [X] T089 [P] Run `cargo fmt --check` and `cargo clippy --workspace --all-targets --all-features --
  -D warnings`; fix any residual warnings introduced by this feature. — **complete**: `cargo fmt`
  (many files needed reformatting, accumulated across US1-US10 — mostly mechanical line-wrapping,
  no semantic changes); `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  surfaced 2 residual issues, both fixed: (1) `validate_app`'s new T079 `let Some(dict) = ... else {
  return None }` rewritten to `let dict = dicts.application_for(appl_ver_id)?;` (clippy
  `question_mark`, matching the sibling `self.validator.as_ref()?` pattern already in the same
  function); (2) T080's `dynamic_config` grew to 8 parameters (clippy `too_many_arguments`, capped
  at 7) — introduced an `IdentityTriple<'a> = (&'a str, Option<&'a str>, Option<&'a str>)` type
  alias, collapsing the 6 sender/target CompID+SubID+LocationID params into 2. Both `cargo fmt
  --check` and the full clippy command now exit clean; re-ran `cargo test --workspace
  --all-features` after both fixes to confirm no regression: 578 passed, 0 failed.
- [X] T090 Run `cargo test --workspace --all-features` and `cargo test -p truefix-at --test
  conformance`/`--test coverage`; confirm zero regressions against T001's baseline, AT suite
  scenario-run count grew per T021/T084, floor matches per T022/T085 (SC-001-SC-008). —
  **complete**: `cargo test --workspace --all-features`: **578 passed, 0 failed** (T001's baseline
  was "all green" with no exact count recorded, since the goal was zero-regression, not matching a
  specific number — satisfied: zero failures throughout every phase of this feature, confirmed live
  after each phase's changes, not just at the end). `cargo test -p truefix-at --test conformance`:
  4/4 passing, including `server_acceptance_suite_passes` — every one of the 405 scenario runs
  (not just their count) actually executes successfully live. `cargo test -p truefix-at --test
  coverage`: 3/3 passing, floor exactly matches the true count (405, per T085's disclosure).
  AT scenario-run count progression this feature: **373 (T001 baseline) → 403 (T021/T022, US1's 8
  new scenario families) → 405 (T084/T085, US9's MinQty scenario)** — strictly grew, never shrank
  (SC-007). SC-001-SC-006/SC-008 are covered by the individual per-story live test runs already
  disclosed in each task's own completion note above (US1-US10); this task's full-workspace re-run
  is the final cross-cutting confirmation that none of those individually-verified fixes regressed
  each other.
- [X] T091 [P] If T082 added the `time-tz` dependency, run `cargo deny check` (or equivalent) to
  confirm license/provenance compliance (Constitution Principle III); skip if T082 chose not to add
  it. — **complete**: T082 did add `time-tz` (2.0.0, BSD-3-Clause), pulling in 12 new transitive
  crates (`aho-corasick`, `parse-zoneinfo`, `phf`/`phf_codegen`/`phf_generator`/`phf_shared`,
  `regex`/`regex-automata`/`regex-syntax`, `serde-xml-rs`, `siphasher`, `xml-rs`) — individually
  spot-checked via `cargo info` during T082 (all MIT/Apache-2.0/Unlicense). Ran `cargo deny check`
  against the workspace's existing `deny.toml` (already configured for Apache-2.0 OR MIT release
  per Constitution Principle III, pre-existing from earlier features): exit code 0, final line
  `advisories ok, bans ok, licenses ok, sources ok`. Only warnings emitted are pre-existing
  duplicate-crate-version notices (`bans.multiple-versions = "warn"`, not deny) unrelated to
  `time-tz` — e.g. `base64`/`windows-sys` duplicates via the pre-existing `tiberius`/`mongodb`/
  `rcgen` dependency trees, not newly introduced by this task.
- [X] T092 Run `quickstart.md`'s full validation-command list end-to-end; correct any test-file-name
  mismatches discovered during implementation (per quickstart.md's own note). — **complete**: every
  single command in `quickstart.md` was a placeholder guess from `/speckit-plan` time (before real
  file names existed, per the file's own pre-existing caveat) — none matched the actual landed test
  files. Individually ran all ~30 corrected commands live before writing them (not just renamed on
  paper): US1 (`state_machine`/`recovery`/`timeouts`/`validation_hook`, not the guessed
  `resend_and_gap_fill`/`reset_on_logon`), US2 (`multi_dynamic`/`session_qualifier`/`socket_options`,
  not `session_qualifier_routing`/`acceptor_builder_reuse_addr`), US3
  (`store_error_logging`/`scheduled`/`restart_resend`/`reset_ordering`/`mssql_backend`), US5
  (`continue_on_error`, not `engine_partial_start_cleanup`/`engine_group_tolerance` — ended up as
  one file, not two), US6 (`framing_bounds`/`proxy_client`/`proxy_protocol`/`tls_hardening`), US7
  (`field_types_extended`/`field_order_production`/`header_trailer_groups_production`/
  `encoded_leg_pairs`/`provenance_headers`), US8 (13 commands spanning `schedule_reset`/`schedule`/
  `config_switches`/`session_switches_mapping`/`state_machine`/`validation_hook`/
  `validator_mapping`/`multi_dynamic`/`store_and_log_mapping`/`config_start`/`schedule_mapping`/lib
  tests — none of the 7 originally-guessed filenames existed), US9 (`coverage`/`conformance`), US10
  (`fix_repository` under `--features dict-tooling`, not `fix_repository_cycle_guard`;
  `body_log_concurrency`, not `body_log_concurrent_append`). Rewrote `quickstart.md` in full with
  the corrected commands, updated the stale 373-baseline framing to note the final 405 count, and
  added `cargo deny check` to the full-regression pass (T091's new coverage). Every command's
  actual live output (pass count) was captured before being written into the file.
- [X] T093 Record a completion disclosure (mirroring 005's convention) marking each `BUG-NN`/
  `GAP-NN`/`B-NN` item this feature closed, referencing `docs/todo/003.md`. — **complete**: 005's own
  convention was updating `docs/acceptance-record.md` with a new per-feature section (not a
  standalone new file — `docs/engine-comparison-gaps.md`, the other doc 005 touched, no longer
  exists in this repo, superseded by the `docs/todo/*.md` convention). Added a "006 — Audit
  remediation" section mirroring "005"'s structure/tone: one theme-grouped bullet per user story,
  each citing its `BUG-NN`/`GAP-NN`/`B-NN` numbers and cross-referencing `docs/todo/003.md` and
  `tasks.md`'s per-task disclosures. **Went beyond a simple closed-items list**: cross-checked every
  `BUG-NN`/`GAP-NN` citation in `docs/todo/003.md`'s P0/P1/P2/AT-follow-up sections against what
  `tasks.md`'s 93 tasks actually addressed, and found `GAP-28`/`GAP-32` (dictionary version-meta
  validation, orphaned from real data) was never covered by any FR in `spec.md` or task in
  `tasks.md` — an honest specification-time omission, not a deliberate deferral like the other
  P1/P2 items research.md explicitly triaged out with stated reasons. Added a new "Not addressed by
  006" section disclosing this plus every other explicitly-deferred item (with its stated reason)
  and the still-harness-blocked AT scenarios, per Constitution Principle VII (inventory-based
  completeness — no hollow "everything closed" claim). Updated "Current gate status" (92→118 test
  binaries, 373→405 AT scenario runs, added a `cargo deny check` line) and "Outstanding before a v1
  release claim" (replaced the stale 005-era open-item list with 006's own, headlined by
  `GAP-28`/`GAP-32`).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **User Stories (Phases 2-11)**: All depend only on Setup. US9's final task (T085, Phase 10)
  additionally depends on US1's AT-scenario task (T021, Phase 2) — already satisfied by phase order.
  US8's FIXT items (T078/T079, Phase 9) are sequenced after US1's `on_logon` changes (T012-T020,
  Phase 2) due to shared-function-region file adjacency, not a hard cross-story block — already
  satisfied by phase order. Every other story is independently implementable/testable in any order.
- **Polish (Phase 12)**: Depends on all ten user stories being complete.

### Within Each User Story

- Tests are written first and MUST fail before implementation (Principle V).
- Within US1: T012-T020 are each independent fixes to different functions/branches in the same file
  (`state.rs`) except T017, which depends on T016 (new field); T021 (AT authoring) depends on
  T012-T020 all landing; T022 (final floor bump) depends on T021.
- Within US3: T038 blocks T039 (reset-ordering fix needs the sync call first); T036/T037 are
  independent of each other and of T038/T039.
- Within US6: T058 and T060 land together (same task, T060 is the de-dup housekeeping folded into
  T058's pass).
- Within US7: T071 blocks T073 (group decode wiring needs the tag classification first).
- Within US8: T078 blocks T079 (FIXT dictionary construction needs the tag-1137 extraction landed
  first to have something to attach `ApplVerID` resolution to); every other US8 task is independent.

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel.
- Once Setup completes, all 10 user stories can start in parallel (if team capacity allows), subject
  to the two soft sequencing notes above (US8's T078/T079 after US1; US9's T085 after US1's T021 and
  US9's own T084).
- Within US1, all 9 test tasks (T003-T011) are mutually parallel (different files or different
  functions in the same file, no shared state).
- Within US8, 6 of its 8 items (T076, T077, T080, T081, T082, T083) are fully mutually parallel —
  independent files, no shared state — the largest parallel-execution opportunity in this feature
  after US1's tests.
- Within US6, all 6 test tasks (T052-T057) are mutually parallel.

---

## Parallel Example: User Story 1 (MVP)

```bash
# Launch all 9 test tasks for User Story 1 together:
Task: "Add a failing test: acceptor rejects low-seq Logon, in crates/truefix-session/tests/state_machine.rs"
Task: "Add a failing test: decreasing plain SequenceReset rejected, in crates/truefix-session/tests/recovery.rs"
Task: "Add a failing test: SequenceReset missing tag 36 rejected, in crates/truefix-session/tests/recovery.rs"
Task: "Add a failing test: malformed ResendRequest rejected, in crates/truefix-session/tests/recovery.rs"
Task: "Add a failing test: ResetOnLogon reconnect loop fixed, in crates/truefix-transport/tests/reconnect.rs"
Task: "Add a failing test: gap-fill drain validates, in crates/truefix-session/tests/recovery.rs"
Task: "Add a failing test: per-connection timers reset, in crates/truefix-session/tests/timeouts.rs"
Task: "Add a failing test: Logout before disconnect on dict failure, in crates/truefix-session/tests/validation_hook.rs"
Task: "Add a failing test: unparseable SendingTime fails latency, in crates/truefix-session/tests/timeouts.rs"
```

## Parallel Example: User Story 8 (largest fully-independent cluster)

```bash
# Launch 6 of User Story 8's 8 items together (T078/T079 sequenced separately):
Task: "Recurring mid-connection sequence reset in crates/truefix-session/src/schedule_reset.rs"
Task: "Multi-tag LogonTag in crates/truefix-session/src/config.rs"
Task: "Wildcard dynamic-session templates in crates/truefix-transport/src/lib.rs"
Task: "cfg-selectable ScreenLog/TracingLog/CompositeLog in crates/truefix-config"
Task: "IANA timezone names in crates/truefix-config"
Task: "\${var} environment-variable fallback in crates/truefix-config/src/lib.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: User Story 1 (the 8 highest-severity, counterparty-triggerable
   protocol-correctness holes — smallest fix surface, highest impact per the audit's own triage
   order).
3. **STOP and VALIDATE**: run `cargo test -p truefix-session -p truefix-transport -p truefix-at`.
4. Continue to US2/US3/US4 (the rest of P1) before considering any P2/P3 story.

### Incremental Delivery

1. Setup → User Story 1 (P1) → validate (AT suite grows for the first time) → User Story 2 (P1) →
   validate → User Story 3 (P1) → validate → User Story 4 (P1) → validate.
2. User Story 5 (P2) → User Story 6 (P2) → User Story 7 (P2) → validate each independently.
3. User Story 8 (P3) → User Story 9 (P3, depends on US1's AT task) → User Story 10 (P3) → validate
   each independently.
4. Polish (Phase 12) → full-gate sweep → done.

### Parallel Team Strategy

With multiple developers, after Setup:
- Developer A: US1 → US9 (natural AT-scenario-count dependency chain).
- Developer B: US2 → US5 (routing + lifecycle, both touch `crates/truefix/src/lib.rs`).
- Developer C: US3 → US4 (store/persistence + JDBC compat, both touch `truefix-store`).
- Developer D: US6 → US7 (network + codec hardening).
- Developer E (or multiple): US8's 6 fully-parallel items, then US10.

---

## Notes

- [P] tasks touch different files (or independent regions of the same file) with no dependency on an
  incomplete task.
- [Story] labels map every task to its spec.md user story for traceability back to
  `docs/todo/003.md`'s `BUG-NN`/`GAP-NN`/`B-NN` identifiers (see each task's parenthetical and
  research.md's per-story sections).
- Commit after each task or logical group, per this project's established convention.
- US1's AT-scenario-run-count growth (T021/T022) and US9's own bump (T084/T085) are this feature's
  deliberate exceptions to "the AT floor test's threshold never changes silently" — disclose the
  exact before/after counts when each lands, per `BUG-17`'s own framing.
- Avoid: vague tasks, same-file conflicts within a `[P]` group, cross-story dependencies beyond the
  two explicitly noted (US8's T078/T079 after US1; US9's T085 after US1's T021).
