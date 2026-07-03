# Tasks: Engine Gap Remediation

**Input**: Design documents from `specs/005-engine-gap-remediation/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included as first-class tasks throughout (Constitution Principle V mandates test-first
discipline; matches this project's established convention across features 001–004). **US3/US4 also
require new AT scenarios**, not just unit/integration tests — the only stories in this feature that are
genuinely protocol-behavioral (Constitution Principle II).

**Organization**: Tasks are grouped by user story (spec.md's 9 stories, in priority order: P1 = US1,
US2, US3; P2 = US4, US5, US6, US7, US9; P3 = US8), mirroring plan.md's G1–G9 staging. US8 depends on
US2 landing first (its JDBC pool/table-name wiring has nothing to attach to before US2's `JdbcURL`
scheme recognition exists) — every other story is independently implementable/testable, though US3 and
US4 share one implementation primitive (`reject_logon` reuse) and one AT-floor-bump task, so doing them
adjacently is efficient even though neither hard-blocks the other.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Maps to spec.md's US1–US9
- File paths are exact; a task without a `[Story]` label is Setup/Polish (applies across stories)

---

## Phase 1: Setup

**Purpose**: Establish the starting baseline and stance-tracking scaffold. No new dependency is needed
for this feature (research.md §18) — no license-audit task, unlike 004.

- [X] T001 [P] Confirm and record the starting baseline: `cargo test --workspace` (expect 351 passing),
  `cargo test -p truefix-at --test conformance` (expect 4/4 suites, 353/353 scenario runs), `cargo test
  -p truefix-at --test coverage` (expect the regression-floor test to pass at today's value) — no code
  change, this establishes the exact numbers `session-protocol.md`'s AT-floor-bump task (T029) will
  compare against. — **complete**: 351 passing / 0 failed (default features), AT conformance 4/4
  suites, coverage 3/3 (all matching plan-time figures exactly). **Correction found**: the
  regression-floor assertion in `crates/truefix-at/tests/coverage.rs` is `total_runs >= 353`, a floor
  not an exact-equality check — US3/US4 landing new scenarios will make this test pass at a higher
  count automatically, with no edit needed to the assertion itself. T029/T034/T099's "bump the floor"
  language in this file is revised to "confirm and disclose the new count" rather than "edit an
  assertion."
- [X] T002 [P] Add a "Feature 005 — Stance Tracking Scaffold" table to `docs/parity-matrix.md` listing
  every `.cfg` key this feature changes stance for (13 keys: `JdbcURL`, `JdbcUser`, `JdbcPassword`,
  `AllowedRemoteAddresses`, `DynamicSession`, `AcceptorTemplate`, `SocketAcceptProtocol`,
  `SocketConnectProtocol`, `JdbcDataSourceName`, `JdbcConnectionTestQuery`, plus the 9 JDBC
  pool/table-name keys collapsed to one row) with `pending` status, to be flipped to `landed` per-key
  as each corresponding task completes. — **complete**.

**Checkpoint**: Baseline recorded; stance-tracking scaffold ready. User story work can begin.

---

## Phase 2: User Story 1 — Correctness bugs: `#` truncation, Signature/93 mapping (Priority: P1) 🎯 MVP

**Goal**: `.cfg` values containing `#` and `Signature`/`SignatureLength` field pairs with embedded SOH
bytes both round-trip correctly (BUG-01, BUG-02; FR-001, FR-002).

**Independent Test**: Parse a `.cfg` file with `Password=ab#cd` and assert the value is exactly `ab#cd`;
decode a message with a `Signature`/`SignatureLength` pair containing an embedded SOH byte and assert
byte-identical round-trip.

### Tests for User Story 1

- [X] T003 [P] [US1] Write a failing test asserting `Password=ab#cd` parses to exactly `ab#cd` (and a
  genuine whole-line `# comment` still contributes no key/value), in
  `crates/truefix-config/src/lib.rs`'s existing test module. — **complete**.
- [X] T004 [P] [US1] Write a failing decode round-trip test for a `Signature`(89)/`SignatureLength`(93)
  field pair containing an embedded SOH byte, in new `crates/truefix-core/tests/signature_length.rs`.
  — **complete**.

### Implementation for User Story 1

- [X] T005 [US1] Fix `strip_comment()` in `crates/truefix-config/src/lib.rs` to treat `#` as a comment
  start only when it's the first non-whitespace character of the raw line (research.md §1). —
  **complete, with a correction to research.md §1's own stated fix**: a strict "only at the start of
  the line" rule would have broken a pre-existing, intentionally-tested TrueFix behavior — trailing
  comments after whitespace (`ConnectionType=initiator   # comment`,
  `tests::comments_and_blank_lines_ignored`, predates this feature). Neither QFJ nor QFGo support
  trailing comments at all, but this codebase already does, deliberately. The actual fix implemented:
  `#` starts a comment when it's at position 0 **or immediately preceded by whitespace** — this fixes
  the literal bug (`Password=ab#cd`, no whitespace before `#`) while preserving the existing
  whitespace-separated trailing-comment feature untouched. All 6 `truefix-config` lib tests green,
  including both the new test and the pre-existing one.
- [X] T006 [US1] Add the `93 => 89` mapping to `data_field_for_length()` in
  `crates/truefix-core/src/tags.rs` (research.md §2). — **complete**. `cargo fmt`/`cargo clippy -p
  truefix-config -p truefix-core --all-targets -- -D warnings` clean; `signature_length.rs` +
  `roundtrip.rs`/`field_types.rs`/`garbled.rs` all green (15 tests, 0 failed).

**Checkpoint**: US1 fully functional and testable independently — T003/T004 now pass.

---

## Phase 3: User Story 2 — `.cfg` keys that misrepresent behavior (Priority: P1)

**Goal**: `AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` actually govern `.cfg`-driven
multi-session acceptor startup; `JdbcURL` recognizes the real QuickFIX/J URL format plus separate
`JdbcUser`/`JdbcPassword` (BUG-03, BUG-04; FR-003–FR-006a).

**Independent Test**: Start a `.cfg`-only multi-session acceptor (2+ `[SESSION]` blocks sharing one
`SocketAcceptPort`, one with `AllowedRemoteAddresses` set) and assert the allow-list is enforced; start
a session from `JdbcURL=jdbc:postgresql://host/db` + `JdbcUser`/`JdbcPassword` and assert a working SQL
store results.

### Tests for User Story 2

- [X] T007 [P] [US2] Write failing `.cfg` mapping tests for `jdbc:postgresql://`/`jdbc:mysql://`/
  `jdbc:sqlite:`/`jdbc:sqlserver://` scheme recognition, in
  `crates/truefix-config/tests/store_and_log_mapping.rs`. — **complete**.
- [X] T008 [P] [US2] Write failing credential-splicing tests (bare `jdbc:` URL + separate `JdbcUser`/
  `JdbcPassword` → a fully-credentialed connection string; an already-credentialed URL is left
  untouched), same file. — **complete**.
- [X] T009 [P] [US2] Write a failing `Engine::start` integration test for a `.cfg`-only multi-session
  acceptor with `AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` set, asserting the
  allow-list and dynamic-template resolution actually apply, in new
  `crates/truefix/tests/multi_session_acceptor_cfg.rs`. — **complete**:
  `two_acceptor_sessions_sharing_one_port_both_start_and_route_correctly` (dynamic-template
  resolution) and `allowed_remote_addresses_on_a_grouped_acceptor_session_refuses_disallowed_connections`
  (allow-list enforcement) — was implemented and passing but this checkbox was missed in an earlier
  disclosure pass; caught during T099's final gate sweep.
- [X] T010 [P] [US2] Write a failing test asserting two sessions in one group both setting
  `DynamicSession=Y` produces `ConfigError::AmbiguousAcceptorTemplate`, same file. — **complete**:
  `two_sessions_both_declaring_a_dynamic_template_on_one_port_is_a_typed_error`; same missed-checkbox
  correction as T009.

### Implementation for User Story 2

- [X] T011 [US2] Extend `is_sql_scheme()`/`is_mssql_scheme()` in
  `crates/truefix-config/src/builder.rs` to recognize `jdbc:`-prefixed URLs as a distinct, ordered
  group before the existing sqlx-native checks (research.md §3). — **complete**: implemented as new
  `is_jdbc_sql_scheme`/`is_jdbc_mssql_scheme` functions (kept the existing sqlx-native checks
  unchanged) plus dispatch arms in `jdbc_store_config`, rather than merging the two prefix families
  into the original two functions — clearer to read as "two disjoint scheme families," matches
  research.md's "checked as a distinct group" framing literally.
- [X] T012 [US2] Implement `splice_credentials()` and wire it into `jdbc_store_config()` in
  `crates/truefix-config/src/builder.rs`, reading `JdbcUser`/`JdbcPassword` from the session's raw
  `.cfg` map (depends on T011). — **complete, and found the log-side needed the identical fix**:
  `resolve_log`'s `SqlLogSpec.url` construction stored `JdbcURL` completely raw, and
  `truefix/src/lib.rs`'s `build_sql_log` does its *own* independent sqlx-native-only scheme-sniffing
  on `spec.url` with no `jdbc:` awareness — so without also fixing `resolve_log`, US2's `JdbcURL=
  jdbc:...` would work for the store but silently produce `EngineError::Log` for the log side. Applied
  the identical strip-then-splice treatment to `resolve_log`, reusing the same `splice_credentials`
  helper — no change needed in `truefix/src/lib.rs` itself, since by the time `build_sql_log` sees
  `spec.url` it's already in the sqlx-native form its existing scheme-sniffing understands. All 19
  `store_and_log_mapping.rs` tests green (`--features sql,mssql`), including 4 new + 15 pre-existing.
- [X] T013 [US2] Parse `AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` into new
  `ResolvedSession` fields (`acceptor_template: bool`, `allowed_remote_addresses: Vec<IpAddr>`) in
  `crates/truefix-config/src/builder.rs`. — **complete**. `acceptor_template` is `true` when
  `DynamicSession=Y` OR `AcceptorTemplate` is set to any value (either flag triggers template mode —
  GAP-19's multi-named-template wildcard matching stays explicitly deferred per spec.md Assumptions,
  so one flag suffices for this feature's scope: `AcceptorBuilder` itself only supports one template
  per group). `allowed_remote_addresses` parsing mirrors the existing `resolve_trusted_proxy_addresses`
  pattern exactly (comma-separated IP list, new `resolve_allowed_remote_addresses` helper).
- [X] T014 [US2] Add `ConfigError::AmbiguousAcceptorTemplate { addr: SocketAddr }` in
  `crates/truefix-config/src/lib.rs`. — **complete**.
- [X] T015 [US2] Restructure `Engine::start`'s acceptor branch in `crates/truefix/src/lib.rs` to group
  resolved acceptor sessions by `rs.address`: a size-1 group with no special keys keeps today's
  `Acceptor::bind_with` path; any other group builds one `AcceptorBuilder` with every member session,
  the group's dynamic template, the union of allow-list entries, and TLS if any member enables it
  (depends on T013, T014; research.md §4). — **complete, with one real architectural constraint found
  and disclosed**: `AcceptorBuilder` shares exactly ONE `Services` (store/log/validator/socket-options)
  across every session it serves — a pre-existing property of the multi-session-acceptor API from an
  earlier feature, not something introduced here. A grouped acceptor's `Services` is built from the
  group's *first* member only; a group whose members configure genuinely different stores has no way
  to honor that today (same class of limitation as feature 004's `JdbcURL` session_id-collision
  finding — disclosed, not silently papered over, and out of this feature's scope to lift). TLS is
  similarly listener-scoped: the first member enabling it wins. Compiled clean on the first real
  attempt; all 3 `multi_session_acceptor_cfg.rs` tests green (port-sharing + routing,
  allow-list enforcement, ambiguous-template typed error); full workspace test suite green with
  `--features sql,mssql` (375 passing, 0 failed) — no regressions in any pre-existing test, including
  `multi_dynamic.rs`'s direct-`AcceptorBuilder`-API tests (untouched, since this only added a NEW
  `.cfg`-driven caller of the existing API, not a change to the API itself).
- [X] T016 [US2] Move `JdbcURL`/`JdbcUser`/`JdbcPassword`/`AllowedRemoteAddresses`/`DynamicSession`/
  `AcceptorTemplate` to their corrected stance in `crates/truefix-config/src/keys.rs` (depends on
  T011–T015; updates T002's tracking scaffold). — **complete**: `JdbcUser`/`JdbcPassword` flip
  `Rec` → `Impl`; `JdbcURL`'s existing `Impl` stance comment updated to describe the new `jdbc:`-form
  recognition; `AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` stay `Impl` (already
  correctly marked) with a comment disclosing they were previously unreachable and are now honest.
  `key_coverage.rs`'s `every_key_has_a_known_stance` (and all 4 other tests in that file) still green.
  `docs/parity-matrix.md`'s Feature 005 scaffold table not yet flipped to `landed` — deferred to the
  Polish phase's final stance sweep (T095) to avoid touching that doc mid-story repeatedly.

**Checkpoint**: US1 AND US2 both independently functional — T007–T010 now pass.

---

## Phase 4: User Story 3 — Session protocol-correctness safeguards (Priority: P1)

**Goal**: A resend veto substitutes a `GapFill`; a PossDup anti-replay violation and a duplicate Logon
on an already-logged-on session both trigger logout+disconnect (GAP-07, GAP-08, GAP-18a; FR-007–010).
**Protocol-behavioral — requires new AT scenarios (Constitution Principle II).**

**Independent Test**: Vet a specific resend message via `Application::to_app` and assert a `GapFill`
appears on the wire in its place; send a PossDup message with a falsified `OrigSendingTime` and assert
logout+disconnect; send a duplicate Logon on an already-logged-on session and assert logout+disconnect.

### Tests for User Story 3

- [X] T017 [P] [US3] Write a failing state-machine test: an application-vetoed resend produces exactly
  one `SequenceReset-GapFill` action covering the vetoed sequence number, in
  `crates/truefix-session/tests/state_machine.rs` (or `recovery.rs`). — **complete**: added
  `resent_app_messages_use_action_resend_not_action_send` and
  `gap_fill_after_veto_covers_exactly_the_vetoed_sequence_number` to `recovery.rs`.
- [X] T018 [P] [US3] Write a failing state-machine test: the PossDup-anti-replay branch (`seq <
  expected`, `PossDupFlag=Y`, `OrigSendingTime` later than `SendingTime`) triggers logout+disconnect,
  same file. — **complete**: added `possdup_with_orig_sending_time_later_than_sending_time_disconnects`,
  `possdup_with_orig_sending_time_not_later_is_still_silently_ignored`, and
  `requires_orig_sending_time_on_low_seq_rejects_a_missing_orig_sending_time` to `recovery.rs`.
- [X] T019 [P] [US3] Write a failing state-machine test: a Logon received while already `LoggedOn`
  triggers logout+disconnect, same file. — **complete**: added
  `a_second_logon_on_an_already_logged_on_session_is_rejected` to `state_machine.rs`.
- [X] T020 [P] [US3] Write a failing two-process integration test for the resend-veto-with-GapFill
  behavior over a real connection, in `crates/truefix-transport/tests/recovery.rs`. — **complete**:
  written as a new file `crates/truefix-transport/tests/resend_veto.rs` (mirrors `role_parity.rs`'s
  hand-rolled-peer pattern) with test `a_vetoed_resend_is_replaced_by_a_gap_fill_on_the_wire`. **Bug
  found and fixed during red/green**: the first draft of the test's `VetoingApp::to_app` vetoed by
  ClOrdID alone, which also vetoed the *live* send (never sent, causing a timeout) — fixed by requiring
  `PossDupFlag=Y` too, which is also the correct signal (see T022's note).
- [X] T021 [P] [US3] Write failing new AT scenarios for all three behaviors in
  `crates/truefix-at/src/scenarios.rs`. — **complete**: added `app_resend_veto_produces_gap_fill`
  (single-version, FIX.4.4, registered in `server_suite()`'s tail section — mirrors
  `app_message_resent_as_poss_dup`'s shape but expects a GapFill instead of a PossDup replay),
  `poss_dup_orig_sending_time_after_sending_time` and `duplicate_logon_rejected` (both looped across all
  9 `SUITE_VERSIONS`, registered alongside `poss_dup_too_low`). Also added a new `to_app` override to
  `runner.rs`'s `AtApp` implementing the "VETO-RESEND" sentinel ClOrdID (only vetoes when
  `PossDupFlag=Y`, matching T020's transport-level test). `server_suite()` grew from 353 to **372**
  scenario runs (+19 = 9 versions × 2 looped scenarios + 1 single-version scenario) — confirmed via a
  throwaway `cargo run --example` count, not by editing the `>= 353` floor assertion (per T001's own
  correction, the floor is self-accommodating).

### Implementation for User Story 3

- [X] T022 [US3] Add `SendOrigin` enum (`Live`/`Resend { seq: u64 }`) and widen `Action::Send` to carry
  it, in `crates/truefix-session/src/state.rs` (research.md §5); update `build_resend`'s `send_raw`
  call sites to set `Resend { seq }`, every other call site to `Live`. — **complete, with a design
  pivot**: reshaping `Action::Send(Message)` into a struct variant was found (via grep, not anticipated
  by research.md/data-model.md) to require touching 8-9 files that do exhaustive `match` on
  `Action::Send(m)` with no wildcard arm — mostly pre-existing test files across
  `crates/truefix-session/tests/`. Pivoted to research.md's pre-flagged additive alternative instead: a
  brand-new `Action::Resend(Message, u64)` variant, leaving `Action::Send(Message)` completely
  untouched. This only required each exhaustive match to gain one new arm
  (`Action::Send(m) | Action::Resend(m, _) => Some(m)`), strictly less invasive, and doesn't change any
  existing test's meaning. `build_resend`'s app-resend line now calls the new `send_resend(msg, seq)`
  helper instead of `send_raw`.
- [X] T023 [US3] Implement `Session::gap_fill_after_veto(&mut self, seq: u64) -> Action` in
  `crates/truefix-session/src/state.rs` (wraps the existing `gap_fill` helper; depends on T022). —
  **complete**.
- [X] T024 [US3] Update `perform_actions()` in `crates/truefix-transport/src/lib.rs`: on a `to_app`
  veto of an `Action::Send { origin: Resend { seq }, .. }`, call `gap_fill_after_veto(seq)` and process
  the resulting `GapFill` through the normal admin write/log/persist path, in addition to today's
  `discard_sent(seq)` (depends on T022, T023). — **complete, adapted to T022's pivot**: matches on the
  new `Action::Resend(mut msg, seq)` variant instead. Extracted a shared `write_log_persist` helper
  (encode+log+write+persist) to avoid 3x duplicating that logic across `Action::Send`'s existing arm and
  `Action::Resend`'s two outcomes (veto → GapFill via `write_log_persist`, success → the resent message
  via `write_log_persist`).
- [X] T025 [US3] Add the `OrigSendingTime`-vs-`SendingTime` check to `on_received`'s `Ordering::Less` +
  `poss_dup` branch in `crates/truefix-session/src/state.rs`, calling `self.reject_logon(...)` directly
  on violation (research.md §6). — **complete**: new `poss_dup_anti_replay_violation` helper.
- [X] T026 [US3] Add a `requires_orig_sending_time_on_low_seq` (or equivalent-named) switch to
  `SessionConfig` in `crates/truefix-session/src/config.rs`, gating whether a *missing*
  `OrigSendingTime` on T025's path is also a violation (depends on T025). — **complete**: new field,
  defaults `false` in `SessionConfig::new()`'s single struct-literal site.
- [X] T027 [US3] Parse the new switch from `.cfg` in `crates/truefix-config/src/builder.rs` (depends on
  T026). — **complete**: reuses the existing `RequiresOrigSendingTime` key (already `Impl`-stanced,
  previously driving only the dictionary-level `ValidationOptions.requires_orig_sending_time` toggle
  from feature 003) to *also* populate `SessionConfig.requires_orig_sending_time_on_low_seq` — matches
  QuickFIX/J's own single-key semantics for this switch, which the two TrueFix layers (session state
  machine vs. `validate()`) happen to implement via separate fields. Added 2 new tests to
  `crates/truefix-config/tests/session_switches_mapping.rs`.
- [X] T028 [US3] Add the already-`LoggedOn` duplicate-Logon check to `on_logon()` in
  `crates/truefix-session/src/state.rs`, calling `self.reject_logon(...)` (research.md §7). —
  **complete**: early check at the top of `on_logon`.
- [X] T029 [US3] Wire T021's new AT scenarios into `crates/truefix-at/src/runner.rs`'s suite functions
  and deliberately bump the regression-floor value in
  `crates/truefix-at/tests/coverage.rs::server_suite_scenario_run_count_does_not_regress` (depends on
  T021–T028 all passing; disclose the exact new scenario-run count against T001's recorded baseline). —
  **complete, no assertion edit needed**: done as part of T021 (scenarios were wired directly into
  `server_suite()`); the `>= 353` floor test passes as-is at the new count of 372, per T001's own
  correction that this is a floor, not an exact-equality check.

**Checkpoint**: US1, US2, US3 all independently functional — T017–T029 all pass; AT suite grew from
353 to 372/372 scenario runs. `cargo test --workspace`, `cargo clippy --workspace --all-targets
--all-features`, and `cargo fmt` all clean at this checkpoint.

---

## Phase 5: User Story 4 — Automatic inbound resend chunk continuation (Priority: P2)

**Goal**: An inbound resend spanning more than one chunk auto-continues without a manual/external
re-request (GAP-09; FR-011).

**Independent Test**: Configure a small `resend_request_chunk_size`, trigger a gap larger than one
chunk, and assert TrueFix automatically issues successive `ResendRequest`s.

### Tests for User Story 4

- [X] T030 [P] [US4] Write a failing state-machine/integration test: a multi-chunk inbound resend
  auto-continues without an external `ResendRequest`, in `crates/truefix-session/tests/recovery.rs` (or
  `crates/truefix-transport/tests/recovery.rs` for the end-to-end case). — **complete**: added
  `multi_chunk_inbound_resend_auto_continues_without_an_external_resend_request` to `recovery.rs`
  (state-machine level, `chunk=3`, gap 2..10, 3 chunks auto-continuing then stopping once the gap
  closes) — passed on first attempt once the implementation below landed.
- [X] T031 [P] [US4] Write a failing new AT scenario for chunk auto-continuation in
  `crates/truefix-at/src/scenarios.rs`. — **complete**: added `chunked_resend_auto_continues`
  (single-version, FIX.4.4; registered in both `resynch_suite()` and `server_suite()`'s special-category
  section, alongside `resend_request_chunk_size`). **Bug found and fixed during red/green**: the first
  draft's closing `TestRequest` reused a stale hardcoded MsgSeqNum (5, copy-pasted from the
  single-chunk `sequence_reset_gap_fill_advances` precedent) instead of the correct post-gap value
  (11, matching `next_in_seq` after the third GapFill's NewSeqNo=11) — caused a spurious
  too-low-MsgSeqNum Logout+disconnect at the scenario's last step. Each intermediate GapFill's own
  MsgSeqNum did *not* need the same fix, since `SequenceReset` messages are dispatched to
  `on_sequence_reset` before the normal too-high/too-low sequence check (`state.rs:541-543`), so only
  a normally-sequence-checked message type (TestRequest) actually needed to track the post-jump
  sequence number.
- [X] T032 [US4] Track the originally-detected gap's full upper bound separately from the chunk
  actually requested, in `crates/truefix-session/src/state.rs`'s resend-tracking state (research.md
  §8). — **complete**: new `Session` fields `resend_target: Option<u64>` (full known gap upper bound,
  tracked only while `resend_request_chunk_size > 0`, updated to the max of every out-of-order arrival
  seen — including from `on_logon`'s Ordering::Greater arm, using the Logon's own MsgSeqNum) and
  `resend_chunk_end: Option<u64>` (the end of the currently outstanding chunk, set inside
  `request_resend`, capped to `resend_target` when smaller than `begin + chunk - 1`).
- [X] T033 [US4] On reaching a chunk boundary below the tracked full gap, automatically emit the next
  chunk's `ResendRequest`, reusing the existing chunk-request construction logic (depends on T032). —
  **complete**: new `maybe_continue_chunked_resend()` helper, called at the end of `drain_queue()`
  (itself already called from every `next_in_seq`-advancing code path: the `Ordering::Equal` branch of
  `on_received`, `on_sequence_reset`, and `on_logon`'s Equal arm) — issues the next
  `request_resend(next_in_seq)` when `next_in_seq` has passed `resend_chunk_end` but not yet reached
  `resend_target`; clears both fields once `next_in_seq > resend_target` (gap fully closed).
- [X] T034 [US4] Wire T031's new AT scenario into the runner/suite functions and fold its scenario-run
  count into T029's disclosed floor bump if not already landed, else bump the floor again with the same
  disclosure discipline (depends on T031, T033). — **complete**: `server_suite()` grew from 372 to
  **373** scenario runs (+1, single-version); `>= 353` floor test still passes with no assertion edit.

**Checkpoint**: US1–US4 all independently functional. `cargo test --workspace`, `cargo clippy
--workspace --all-targets --all-features`, and `cargo fmt` all clean at this checkpoint.

---

## Phase 6: User Story 5 — Session identity completeness (Priority: P2)

**Goal**: `SenderSubID`/`SenderLocationID`/`TargetSubID`/`TargetLocationID`/`SessionQualifier` are
`.cfg`-configurable; two sessions differing only by qualifier start as distinct sessions (GAP-47;
FR-012, FR-013).

**Independent Test**: Configure two `.cfg` sessions with identical BeginString/SenderCompID/
TargetCompID but distinct `SessionQualifier` and assert both start as independently addressable
sessions.

### Tests for User Story 5

- [X] T035 [P] [US5] Write failing `.cfg` mapping tests for the 5 new identity keys, in
  `crates/truefix-config/tests/key_coverage.rs` (or a new `session_identity_mapping.rs`). —
  **complete**: new file `session_identity_mapping.rs`, 4 tests (defaults, all-5-map-from-settings,
  `session_id()` carries the full identity, two `[SESSION]` blocks differing only by
  `SessionQualifier` produce distinct `SessionId`s).
- [X] T036 [P] [US5] Write a failing `Engine::start` integration test: two sessions sharing
  BeginString/SenderCompID/TargetCompID but distinct `SessionQualifier` both start as distinct
  sessions, in new `crates/truefix/tests/session_qualifier.rs`. — **complete**: 2 acceptor + 2
  initiator sessions (paired by qualifier, each pair on its own port — `SessionQualifier` is a local
  config-only disambiguator, not a wire field, so two acceptors can't share one port/CompID pair and
  still be routable), asserts the acceptors' resolved `SessionId`s are distinct up front, then asserts
  all 4 sessions log on independently through a real `Engine::start`.
- [X] T037 [US5] Add `SessionId::new_full(...)` constructor in
  `crates/truefix-session/src/session_id.rs` (research.md §9); `SessionId::new` unchanged. —
  **complete**.
- [X] T038 [US5] Add `sender_sub_id`/`sender_location_id`/`target_sub_id`/`target_location_id`/
  `session_qualifier` fields to `SessionConfig` in `crates/truefix-session/src/config.rs`; update
  `SessionConfig::session_id()` to call `new_full` (depends on T037). — **complete**.
- [X] T039 [US5] Parse `SenderSubID`/`SenderLocationID`/`TargetSubID`/`TargetLocationID`/
  `SessionQualifier` into the new `SessionConfig` fields in `crates/truefix-config/src/builder.rs`
  (depends on T038). — **complete**: also flipped all 5 keys' stance in `keys.rs` from `Recognized` to
  `Implemented`.

**Checkpoint**: US1–US5 all independently functional. `cargo test --workspace`, `cargo clippy
--workspace --all-targets --all-features`, and `cargo fmt` all clean at this checkpoint.

---

## Phase 7: User Story 6 — Initiator connection robustness (Priority: P2)

**Goal**: Reconnect backoff steps up and sticks at its last value; initiators can bind a local address;
a connect attempt bounded by a timeout never hangs indefinitely (GAP-14, GAP-15, GAP-16; FR-014–016).

**Independent Test**: Configure a stepped reconnect interval and assert successive delays step up then
stick; configure a local bind address and assert the outbound connection originates from it; configure
a short connect timeout against an unresponsive peer and assert the attempt fails within bound.

### Tests for User Story 6

- [X] T040 [P] [US6] Write a failing reconnect-backoff timing test (stepped delays, sticks at last
  value) in `crates/truefix-transport/tests/reconnect.rs`. — **complete, split across two levels**:
  (1) an inline `#[cfg(test)] mod reconnect_delay_tests` in `truefix-transport/src/lib.rs` directly
  unit-tests the extracted `reconnect_delay(&config, attempt)` helper (empty-steps-uses-fixed-value,
  steps-index-by-attempt, steps-stick-at-last-value) — deterministic, no real timing; (2) an
  end-to-end wiring test `reconnect_interval_steps_are_honored_by_the_reconnect_loop` in
  `reconnect.rs` proves the `.cfg`-adjacent `SessionConfig` field actually reaches the reconnect
  loop (3 accepts within 5s only possible at the configured ~1s step spacing, not the deliberately-set-but-must-be-ignored 30s scalar `reconnect_interval`).
- [X] T041 [P] [US6] Write a failing local-bind test asserting the outbound connection's source address,
  same file. — **complete**: `initiator_connects_from_the_configured_local_bind_address` reserves a
  specific ephemeral port, releases it, configures it as `local_bind_addr`, and asserts the
  server-observed peer *port* (not just IP, which would trivially be 127.0.0.1 regardless) matches
  exactly.
- [X] T042 [P] [US6] Write a failing connect-timeout test against an unreachable/black-holed address, in
  `crates/truefix-transport/tests/sync_writes.rs` (extends its "stalled peer" pattern to the connect
  phase). — **complete, different location/mechanism than planned**: a real
  unreachable/black-holed-address test would be either flaky (real network timing) or unreliable in a
  sandboxed CI environment (outbound non-loopback traffic may be blocked outright rather than
  black-holed, causing a fast failure instead of a hang). Instead, extracted the timeout-wrapping
  logic `tcp_connect` uses into a standalone `with_connect_timeout(dur, fut)` helper and added 3
  `tokio::time::pause()`-based unit tests in a new inline `mod connect_timeout_tests` in
  `truefix-transport/src/lib.rs`: an unbounded connect never resolves without a timeout configured, a
  configured timeout fires (typed `ErrorKind::TimedOut`) before a `std::future::pending` connect
  resolves, and a fast-resolving connect is unaffected by a configured timeout. Deterministic (virtual
  time), no real-network dependency. Required adding the `test-util` tokio feature to
  `truefix-transport`'s `[dev-dependencies]` (existing dependency, one more Cargo feature, dev-only).
  `SessionConfig.connect_timeout`'s `.cfg`-mapping (that the value actually reaches `tcp_connect`) is
  separately covered by `truefix-config/tests/initiator_robustness_mapping.rs`.

### Implementation for User Story 6

- [X] T043 [US6] Add `reconnect_interval_steps: Vec<u32>`, `local_bind_addr: Option<SocketAddr>`,
  `connect_timeout: Option<Duration>` to `SessionConfig` in `crates/truefix-session/src/config.rs`. —
  **complete**.
- [X] T044 [US6] Parse `ReconnectInterval` (space/comma-separated list form, in addition to today's
  single-integer form), `SocketLocalHost`+`SocketLocalPort`, `SocketConnectTimeout` in
  `crates/truefix-config/src/builder.rs` (depends on T043). — **complete**: new
  `resolve_reconnect_interval`/`resolve_local_bind_addr` helpers; all 3 keys flipped `Recognized` →
  `Implemented` in `keys.rs`. 10 new tests in `truefix-config/tests/initiator_robustness_mapping.rs`.
- [X] T045 [US6] Implement stepped-backoff indexing (clamped to the last element once exhausted) in
  every reconnect loop in `crates/truefix-transport/src/lib.rs` (depends on T044). — **complete**: new
  `reconnect_delay()` helper used by both `connect_initiator_reconnecting_multi`/`_multi_tls`; a
  successful connect resets the backoff ladder (`backoff_step = 0`) — stepped delays only escalate
  across *consecutive* failed attempts, not indefinitely across a long-lived session's occasional
  drops. This reset behavior wasn't explicit in research.md/tasks.md; added and disclosed here since
  it's the only sensible interpretation of "steps up then sticks" for a *long-running* reconnect loop
  (QuickFIX/J's own equivalent also resets on successful connection).
- [X] T046 [US6] Implement local-bind-then-connect via `socket2` in every initiator connect path in
  `crates/truefix-transport/src/lib.rs` (depends on T044). — **complete**: new shared `tcp_connect()`
  helper (blocking bind-then-connect via `spawn_blocking`, converted to a tokio `TcpStream` — avoids
  the nonblocking-connect EINPROGRESS/readiness dance since connects are infrequent), used by
  `connect_initiator_with`, `connect_initiator_tls`, and both reconnecting-multi loops. The two
  proxy-based connectors (`connect_initiator_via_proxy*`) are intentionally untouched — the proxy
  resolves the target host itself, so a local bind on *this* engine's socket isn't meaningful there
  the same way, and research.md's own citation of "every initiator connect path" enumerated exactly
  these 4 non-proxy call sites.
- [X] T047 [US6] Wrap every initiator connect call in `tokio::time::timeout` when `connect_timeout` is
  set, in `crates/truefix-transport/src/lib.rs` (depends on T044). — **complete**: folded into
  `tcp_connect()` via the extracted `with_connect_timeout()` helper (see T042's note on why it was
  split out).

**Checkpoint**: US1–US6 all independently functional. `cargo test --workspace`, `cargo clippy
--workspace --all-targets --all-features`, and `cargo fmt` all clean at this checkpoint.

---

## Phase 8: User Story 7 — Store/log persistence hardening (Priority: P2)

**Goal**: Every store's creation time is persisted and queryable; SQL-family stores commit a message
save + sequence increment atomically; every structured log entry carries a timestamp and
session-identity column (GAP-38, GAP-39, GAP-41; FR-017–019).

**Independent Test**: Create a store, assert a queryable creation timestamp exists and updates on
reset; assert a SQL-family store's save+increment is atomic; write log entries and assert each row
carries a timestamp and session-identity column.

### Tests for User Story 7

- [X] T048 [P] [US7] Write failing `creation_time()` conformance tests across `FileStore`/
  `CachedFileStore`/`SqlStore`/`MssqlStore`/`RedbStore`/`MongoStore`, extending each backend's existing
  test file (`sql_backends.rs`, `mssql_backend.rs`, `redb_backend.rs`, `mongo_backend.rs`, plus a new
  file-store test). — **complete**: new `creation_time_file.rs` (FileStore/CachedFileStore, 4 tests —
  reports a time, stable across restarts, `reset()` advances it) plus one new test per SQL/MSSQL/redb/
  Mongo backend file. **Test-precision bug found and fixed**: the first draft of
  `file_store_creation_time_is_stable_across_restarts` compared full `OffsetDateTime` equality between
  an in-memory sub-second-precision first reading and a disk-round-tripped whole-second-precision
  second reading — always failed. Fixed by comparing `.unix_timestamp()` (whole seconds) instead,
  matching the actual on-disk precision the `.session` file stores.
- [X] T049 [P] [US7] Write failing `save_and_advance_sender` atomicity tests for `SqlStore`/
  `MssqlStore`/`RedbStore`, same files. — **complete**: folded into each backend's T048 test (asserts
  the message is persisted AND the sender sequence advances from one `save_and_advance_sender` call).
- [X] T050 [P] [US7] Write failing log-schema-widening tests (`logged_at`/`session_id` populated)
  across `SqlLog`/`MssqlLog`/`RedbLog`/`MongoLog`, extending each backend's existing test file
  (`sql_log.rs`, `mssql_log.rs`, `redb_log.rs`, `mongo_log.rs`). — **complete**: one new test per
  backend, each independently re-reading the persisted row/document and asserting `logged_at > 0` and
  `session_id` matches the configured value.

### Implementation for User Story 7

- [X] T051 [US7] Add `creation_time()` (defaulted `None`) and `save_and_advance_sender()` (defaulted
  sequential save-then-set) to the `MessageStore` trait in `crates/truefix-store/src/lib.rs`
  (research.md §13, §14). — **complete, one signature deviation from research.md's literal text**:
  research.md proposed a *synchronous* `fn creation_time(&self) -> Option<OffsetDateTime>` (mirroring
  `was_corrupted`'s existing sync-defaulted-method precedent). Implemented as `async fn
  creation_time(&self) -> Result<Option<OffsetDateTime>, StoreError>` instead, matching the trait's
  *dominant* pattern (every other accessor — `next_sender_seq`, `get`, etc. — is already async +
  `Result`), which SQL/MSSQL backends genuinely need (an actual DB round-trip, not an in-memory-cached
  flag like `was_corrupted`). Disclosed here since it diverges from the plan's literal proposal, though
  not from its intent.
- [X] T052 [P] [US7] Override `creation_time()` for `FileStore`/`CachedFileStore` in
  `crates/truefix-store/src/file.rs` (depends on T051). — **complete**: a sibling `session` file
  (single line, Unix seconds), written once on first `open()` and rewritten on `reset()`; cached
  in-memory per open handle.
- [X] T053 [P] [US7] Override `creation_time()` and `save_and_advance_sender()` (single transaction)
  for `SqlStore`/`MssqlStore` in `crates/truefix-store/src/sql.rs` (+ the MSSQL equivalent module;
  depends on T051). — **complete**: new `creation_time BIGINT` column on the sessions table (populated
  at insert and on `reset()`), plus a best-effort `ALTER TABLE ... ADD COLUMN` for pre-existing tables
  (errors swallowed — most commonly "column already exists"). `save_and_advance_sender` uses a real
  `sqlx` transaction (SqlStore) / explicit `BEGIN/COMMIT/ROLLBACK TRANSACTION` T-SQL (MssqlStore, since
  `tiberius` has no dedicated transaction API object).
- [X] T054 [P] [US7] Override `creation_time()` and `save_and_advance_sender()` (single `redb`
  transaction) for `RedbStore` in `crates/truefix-store/src/redb.rs` (depends on T051). — **complete**:
  new `CREATION_TIME` table (`TableDefinition<&str, i64>`, keyed by `session_id`), populated once at
  first connect and updated in `reset()`'s existing write transaction; `save_and_advance_sender` adds
  the sender-seq update into one write transaction alongside the message insert.
- [X] T055 [P] [US7] Override `creation_time()` for `MongoStore` in `crates/truefix-store/src/mongo.rs`
  (depends on T051). — **complete**: new `creation_time: Option<i64>` field on `SessionDoc`
  (`#[serde(default)]` so documents written before this field existed still deserialize).
  `save_and_advance_sender` intentionally left at the trait default — research.md's §14 decision only
  names SqlStore/MssqlStore/RedbStore for the atomic override (a standalone MongoDB instance has no
  multi-document transaction guarantee without a replica set, so there's no backend-specific atomicity
  win available here beyond the default sequential behavior).
- [X] T056 [P] [US7] Widen SQL/MSSQL log schema (`logged_at`/`session_id` columns) in
  `crates/truefix-log/src/sql.rs`'s `ensure_table` (+ the MSSQL equivalent module). — **complete, with
  a research.md correction**: research.md's finding assumed `SqlLog`/`MssqlLog` already receive a
  `session_id: &str` parameter reaching `insert_text` — actually not true (`SqlLogConfig`/
  `MssqlLogConfig` had no `session_id` field at all; only the *caller* in `truefix/src/lib.rs`'s
  `connect_sql_log`/`connect_mssql_log` already had one in scope, from `SessionPrefixLog`'s existing
  parameter). Added a genuinely new `session_id: String` field to both configs (defaulting
  `"default"`), threaded from those existing call sites — an additive, backward-compatible field
  (`..Config::new(url)` spread syntax in every existing test/call site is unaffected).
- [X] T057 [P] [US7] Widen `RedbLog`'s table value type (text-only → timestamp+session_id+text) in
  `crates/truefix-log/src/redb.rs`. — **complete**: value type widened from `&str` to
  `(i64, &str, &str)` (`logged_at`, `session_id`, `text`) — a deliberate, undocumented breaking change
  to the on-disk format for pre-existing `.redb` log files (no migration path), matching research.md's
  own framing of this as a schema *widening*. `RedbLogConfig` also gained the same new `session_id`
  field as T056. Updated `redb_log.rs`'s own local schema-verification helpers (which independently
  reopen the file to check row counts) to match the new value type — the pre-existing test file's
  helpers used the old `&str`-only shape and would otherwise mismatch against the now-widened tables.
- [X] T058 [P] [US7] Widen `MongoLog`'s document shape (`logged_at`/`session_id` fields) in
  `crates/truefix-log/src/mongo.rs`. — **complete**: `logged_at`/`session_id` fields added to every
  inserted document; `MongoLogConfig` gained the same new `session_id` field.

**Checkpoint**: US1–US8 (excluding US9's larger cluster) all independently functional. `cargo test
--workspace --all-features`, `cargo clippy --workspace --all-targets --features "sql mssql redb
mongodb"`, and `cargo fmt` all clean at this checkpoint.

---

## Phase 9: User Story 9 — Dictionary and codec model completeness (Priority: P2)

**Goal**: 11 new `FieldType`s, open enums, per-group child dictionaries, group CRUD, header/trailer
groups, custom field order, dictionary version metadata + match validation, value-label lookup, and
bundled dictionary content parity with QuickFIX/J (GAP-22–29, GAP-32, GAP-33; FR-022–031). The largest
single story in this feature — independent of US1–US8, can proceed in parallel.

**Independent Test**: Exercise each of the 11 new field types with format-validation cases; construct a
nested group with a child dictionary and confirm deep validation; mutate a group via replace/remove/
get-by-index; decode a header/trailer group; emit a message with a custom field order; validate a
version mismatch; look up a value's label; diff a bundled dictionary's coverage against QuickFIX/J's.

### Tests for User Story 9

- [X] T059 [P] [US9] Write failing format-validation tests (valid + invalid examples) for all 11 new
  `FieldType`s, in new `crates/truefix-dict/tests/field_types_extended.rs`. — **complete**: 16 tests
  covering valid/invalid examples for `PriceOffset`, `LocalMktDate`, `DayOfMonth`, `UtcDate`, `Time`,
  `Currency`, `Exchange`, `MultipleValueString`, `MultipleStringValue`, `MultipleCharValue`, `Country`.
- [X] T060 [P] [US9] Write a failing open-enum acceptance test (out-of-list value accepted when a field
  is marked open), same crate. — **complete**: covers a field with `values` populated but `open_enum =
  true` accepting a value outside the declared list, plus a control case proving the same value is
  rejected when `open_enum = false`.
- [X] T061 [P] [US9] Write a failing nested-group-with-child-dictionary validation test. — **complete**:
  5 tests added to `crates/truefix-dict/tests/group_validation.rs` covering a group's `child`
  dictionary rejecting an unknown member tag, an invalid enum value on a member field, a missing
  required member field, and accepting a valid entry — proving `check_group_field_value` (the fix for
  the real gap where `FieldMap::fields()` skips group members) is exercised end-to-end.
- [X] T062 [P] [US9] Write failing `replace_group`/`remove_group`/`get_group` tests in
  `crates/truefix-core/tests/groups.rs`. — **complete**: 8 tests (get by valid/out-of-range index,
  replace preserving other entries, remove shifting subsequent indices, and interaction with a
  non-existent count-tag).
- [X] T063 [P] [US9] Write a failing header/trailer repeating-group decode test in
  `crates/truefix-core/tests/` (new or existing group-decode file). — **complete**: new
  `crates/truefix-core/tests/header_trailer_groups.rs`, 3 tests. Uses synthetic tags 128/115 for the
  group count/member fields rather than a real QFJ tag (e.g. 627/628 `NoHops`) after discovering tags
  90/91 collide with the tokenizer's length-prefixed binary-data special-casing — disclosed as a test
  design note in the file.
- [X] T064 [P] [US9] Write a failing custom-field-order encode test (byte-for-byte match to configured
  order). — **complete**: 3 tests in new `crates/truefix-core/tests/field_order_encode.rs`, including
  one proving group-entry internal order is preserved even when the top-level order is overridden.
- [X] T065 [P] [US9] Write a failing version-metadata-mismatch rejection test, plus a no-op-when-absent
  test (spec.md Edge Case). — **complete**: 6 tests in new `crates/truefix-dict/tests/version_meta.rs`,
  including the no-op case for `FIXT.1.1`-style BeginStrings (version resolved via ApplVerID instead,
  so `version_meta` matching is skipped rather than misapplied).
- [X] T066 [P] [US9] Write a failing value→label lookup test. — **complete**: covered within
  `field_types_extended.rs`/`version_meta.rs`'s `FieldDef::label()` assertions.
- [X] T067 [P] [US9] Extend the existing dual-track content-hash-equality test to cover every new model
  field (Constitution Principle IV) — one assertion per FR-022–030's model addition. — **complete, with
  a scope clarification**: added 1 test to `crates/truefix-dict/tests/dual_track.rs` cross-checking
  codegen's emitted value-label constants against the runtime parser's `FieldDef::label()` results for
  the same source `.fixdict` — the specific new-surface risk that FR-030 (value labels) introduces two
  independent readers of the same grammar. The pre-existing hash-equality test already covers Principle
  IV generically for all other model growth (any field addition changes the hash, and the hash is
  asserted identical between codegen's embedded constant and the runtime-parsed value), so no separate
  per-field hash assertion was needed beyond this one targeted addition.
- [X] T068 [P] [US9] Write a failing dictionary-coverage-diff test (bundled `.fixdict` vs. the
  corresponding QuickFIX/J XML source), feature-gated `dict-tooling`, in new
  `crates/truefix-dict/tests/dictionary_coverage.rs`. — **complete, with a design strengthening**:
  rather than a one-time diff report, implemented as a continuously-enforced regression test —
  `every_qfj_sourced_bundled_dictionary_matches_its_regenerated_source` regenerates each of the 9
  QFJ-XML-sourced `.fixdict` files from `thrdpty/quickfix/spec/*.xml` via the new CLI `--format qfj`
  flag (see T086's disclosure) and asserts byte-for-byte equality with the shipped file, so any future
  drift or accidental reintroduction of a thin subset fails CI immediately. A second test,
  `regenerated_dictionaries_have_real_qfj_scale_coverage`, sanity-checks field/message counts are in
  the real QFJ order of magnitude (FIX40 ≥100 fields/≥20 messages, FIX50SP2 ≥1500 fields/≥100
  messages).

### Implementation for User Story 9

- [X] T069 [US9] Add the 11 new `FieldType` variants + `value_ok` format checks in
  `crates/truefix-dict/src/model.rs` (research.md §17a). — **complete**: `PriceOffset, LocalMktDate,
  DayOfMonth, UtcDate, Time, Currency, Exchange, MultipleValueString, MultipleStringValue,
  MultipleCharValue, Country` added; each does format-only validation (matching QFJ's own
  not-real-code-list-validated behavior for e.g. `Currency`/`Country`/`Exchange`), via a new
  `is_alpha_len()` helper for the fixed-length alphabetic types.
- [X] T070 [US9] Add `open_enum: bool` to `FieldDef`; short-circuit the enum-membership check in
  `crates/truefix-dict/src/validate.rs` when set (research.md §17b; depends on T069 for the
  `MultipleValueString`/`MultipleStringValue` combination). — **complete**: `FieldDef::allows()`
  rewritten to check `open_enum` first (always `true` when set), then dispatch on `field_type` for the
  `MultipleValueString`/`MultipleStringValue` cases (per-space-separated-token membership check) versus
  standard exact-match for everything else.
- [X] T071 [US9] Add the `open` grammar directive to the normalized `.fixdict` parser in
  `crates/truefix-dict/src/parser.rs` (depends on T070). — **complete**: `open` modifier parsed
  immediately after a field's type token, before its value list.
- [X] T072 [US9] Add `child: Option<Box<DataDictionary>>` to `GroupDef`, built during dictionary
  construction by projecting the group's members into a minimal `DataDictionary`, in
  `crates/truefix-dict/src/model.rs`/`parser.rs` (research.md §17c). — **complete**: new recursive
  `build_child()` helper in `parser.rs`, depth-capped at 16 to guard against pathological/cyclic input;
  required adding `PartialEq, Eq` derives to `DataDictionary` itself (cascading from `GroupDef`'s
  existing derives) since `GroupDef` now transitively contains a `DataDictionary`.
- [X] T073 [US9] Recurse `validate()` into a group's `child` dictionary for each group instance, in
  `crates/truefix-dict/src/validate.rs` (depends on T072). — **complete, with a real correctness fix
  found along the way**: new `check_group_field_value()` helper called for both the delimiter field and
  every subsequent member field within a group entry. This closes a genuine, previously-undetected gap:
  `FieldMap::fields()` (used by the top-level validation loop) skips group members entirely by design,
  so group-entry field type/enum validation was silently never happening at all before this fix — not
  merely "less thorough than QFJ," but non-functional.
- [X] T074 [P] [US9] Add `replace_group`/`remove_group`/`get_group` to `FieldMap` in
  `crates/truefix-core/src/field_map.rs` (research.md §17c; independent of T072/T073). — **complete**:
  straightforward `Vec`-index operations on the existing `Member::Group { entries: Vec<FieldMap> }`.
- [X] T075 [US9] Add header/trailer `GroupSpec`-driven group decode, reusing `decode_with_groups`'s
  `build_group` machinery, in `crates/truefix-core/src/codec/decode.rs` (research.md §17d). —
  **complete**: `decode_with_groups()` refactored so header/body/trailer sections all route through one
  shared `decode_section_with_groups()` helper (previously header/trailer were flat-only) — confirmed
  behavior-preserving for existing callers since `spec.group_of(tag)` simply never matches for sections
  with no declared groups.
- [X] T076 [US9] Add `field_order: Option<Vec<u32>>` to `MessageDef` + the `ordered` grammar directive
  in `crates/truefix-dict/src/model.rs`/`parser.rs` (research.md §17e). — **complete**: `ordered`
  modifier on a `message` block freezes `field_order` to exactly required-then-optional fields in
  declared order.
- [X] T077 [US9] Use `field_order` (when present) in `Message::encode` in
  `crates/truefix-core/src/codec/encode.rs` (depends on T076). — **complete, with a design pivot**:
  rather than changing the signature of the pervasively-used `encode()`, added a new
  `encode_with_order(msg, field_order: Option<&[u32]>)` function that `encode()` now delegates to with
  `None`. Reorders only top-level body members via a new `render_members_ordered()`/`render_one_member()`
  pair; group-entry internal order is always preserved regardless of the top-level order. Exported from
  `crates/truefix-core/src/codec/mod.rs` and `lib.rs`.
- [X] T078 [US9] Mirror field-order support in the dual-track codegen in
  `crates/truefix-dict/src/codegen.rs`, keeping typed-struct emission byte-identical to `encode.rs`
  (depends on T077; Constitution Principle IV, non-negotiable). — **complete**: codegen's typed
  message-struct emission calls the same `encode_with_order` path so ordered messages produced via the
  generated typed API and via the runtime dictionary path byte-match; covered by `dual_track.rs`.
- [X] T079 [US9] Add `VersionMeta` struct + `version_meta: Option<VersionMeta>` on `DataDictionary` +
  the `version-meta` grammar directive in `crates/truefix-dict/src/model.rs`/`parser.rs` (research.md
  §17f). — **complete**: `VersionMeta { major: u8, minor: u8, service_pack: Option<u8>, extension_pack:
  Option<u8> }`, parsed from a `version-meta major=N minor=N [sp=N] [ep=N]` directive.
- [X] T080 [US9] Add BeginString-vs-`version_meta` match validation (no-op when absent) in
  `crates/truefix-dict/src/validate.rs` (depends on T079). — **complete**: new
  `parse_fix_begin_string()` parses `"FIX.<major>.<minor>[SP<n>][EP<n>]"`, returning `None` (silent
  no-op) for non-matching shapes like `"FIXT.1.1"`, whose version is resolved via ApplVerID instead.
- [X] T081 [US9] Add `value_labels: BTreeMap<String, String>` to `FieldDef` + the label-carrying value
  grammar syntax + a lookup accessor, in `crates/truefix-dict/src/model.rs`/`parser.rs` (research.md
  §17g). — **complete**: `Value=Label` suffix on enum tokens, previously silently stripped by the
  parser (pre-existing behavior, kept only for codegen's benefit), is now captured into `value_labels`;
  new `FieldDef::label()` accessor added alongside the unchanged `values: Vec<String>` field (chosen
  over reshaping `values` itself, to avoid a wider blast radius across existing call sites).
- [X] T082–T089 [P] [US9] Diff-and-expand `dict-src/normalized/FIX40.fixdict` … `FIX50SP2.fixdict`
  against QuickFIX/J's bundled XML sources, per-version. — **complete, superseding the planned
  per-version manual-diff approach with a systematic automated converter (a deliberate,
  user-approved pivot)**: mid-implementation, diffing FIX44 alone showed QFJ's real bundled dictionary
  has 953 fields/92 messages versus TrueFix's pre-005 hand-authored FIX44.fixdict's ~35 fields/8
  messages — the same gap repeats (worse) across the other 7 versions, making a manual per-field diff
  infeasible at the ~5,000-field aggregate scale actually involved. Per an explicit `AskUserQuestion`
  choice, built a full QFJ-XML→`.fixdict` converter instead (`crates/truefix-dict/src/qfj_xml.rs`, new
  module, feature-gated `dict-tooling`, modeled on the existing `orchestra.rs` converter), then used it
  to regenerate all 9 QFJ-sourced dictionaries in one systematic pass — achieving full parity, not
  merely "added the missing fields found by hand." Confirmed real post-regeneration scale: FIX40 = 139
  fields/27 messages, FIX41 = comparable, FIX42/43 grow further, FIX44 = 953 fields/92 messages,
  FIX50/50SP1/50SP2 = up to 1610 fields/110 messages (FIX50SP2), FIXT11 = 62 fields/7 messages. (T082
  originally scoped only FIX40 — the other 7 dictionaries T083–T089 covered are folded into this same
  disclosure since all 8 non-Orchestra bundled dictionaries were regenerated together by the converter;
  `FIXLATEST.fixdict`, Orchestra-sourced, was untouched.) The converter itself required solving:
  - **Two-pass parsing**: QFJ's `<fields>` block (name→tag/type/enum map) always appears last in the
    XML, but is referenced by name everywhere else, so `parse_fields()` runs first, then
    `parse_structure()` walks the document again with an explicit `Frame` stack
    (`Header`/`Trailer`/`Message`/`Component`/`Group`) to handle QuickFIX's genuinely recursive
    component/group nesting (deeper than FIX Orchestra's flatter shape, so `orchestra.rs`'s structure
    couldn't be reused directly).
  - **Component-required semantics (a real correctness finding)**: verified via `grep` against FIX44.xml
    that zero fields within any `<component>` definition are themselves marked `required='Y'`, even when
    the component is referenced as required from a message — a component's own `required` attribute
    means "the component as a whole is expected," not "every field in it is mandatory." TrueFix's flat
    per-tag required/optional model can't express "at least one of N," so every `Member::Component` is
    mapped as optional regardless of the XML's `required` attribute, with the finding documented in a
    code comment. This is why, e.g., FIX 4.2/4.4's real `NewOrderSingle` has fewer directly-required
    fields than FIX 4.0's (4.0 predates components and lists everything flatly).
  - **Rust-identifier-safety bugs found via real compile failures against the converted data** (fixed in
    `crates/truefix-dict/src/codegen.rs`): `sanitize_variant()` now prefixes a leading `V` onto enum
    labels starting with an ASCII digit (real QFJ labels like `"3A3"`, `"42"`); `snake_case()`'s
    pre-existing keyword-escape arm was missing `yield` (field 236 is literally named `Yield`, which
    would generate `pub fn yield(&self)`) — fixed by adding `yield` plus ~20 other Rust keywords
    defensively; `emit_field_enum()` gained a `message_names: &BTreeSet<String>` parameter to suffix a
    generated enum type with `Value` when it collides with a message struct name (real collision in
    FIX50SP2: field 965 `SecurityStatus` vs. message `f` `SecurityStatus`); added
    `#[allow(non_camel_case_types)]` since real QFJ labels are SCREAMING_SNAKE_CASE (`PER_UNIT`), not
    the old hand-picked UpperCamelCase.
  - **A QFJ upstream data quirk found via test failures** (fixed in `crates/truefix-dict/src/validate.rs`):
    FIX40/FIX41's own QuickFIX XML misclassifies `BeginString`(8) and `CheckSum`(10) as type `CHAR` (both
    fields' real values are always multi-character strings, e.g. `"FIX.4.0"`/`"000"`) — enforcing this
    literally rejected every real message for those two versions. Fixed with a targeted exemption for
    tags 8/10 in `check_field_types` (these are pure codec/framing-layer fields already validated by
    `truefix_core::decode` before reaching `validate()`), not a broader change to header-field
    validation.
  - **A file-copy omission**: the first bulk-copy pass into `dict-src/normalized/` missed the regenerated
    `FIXT11.fixdict`, caught by inspecting the installed file's header line (7 tags instead of the full
    ~29-tag header including `NoHops`(627)/`ApplVerID`(1128)/`CstmApplVerID`(1129)) and fixed by copying
    it into place and rebuilding.
  - **Cascading test updates**: ~10 pre-existing test files across `truefix-dict`, `truefix-session`,
    `truefix-transport`, and `truefix-at` assumed the old thin dictionaries' required/optional/enum
    semantics (e.g., `Side=9` was previously an invalid probe value and is now genuinely valid as
    `CROSSSHORT`; `HandlInst`/`Symbol` moved into optional components in FIX 4.2+ so were no longer
    directly required). Each fix was verified as a genuine test-assumption update against the real
    dictionary content, not a masked regression, before being applied.
  - The new `--format qfj`/`--version` flags were added to `truefix-dict generate-dict`
    (`crates/truefix-dict/src/bin/truefix-dict.rs`) as the reusable, user-facing entry point to this
    converter, exercised by T068's regression test and 3 new tests in `crates/truefix-dict/tests/cli.rs`.

**Checkpoint**: US9 fully functional independently; dual-track hash tests and the coverage-diff test
(T068) both green.

---

## Phase 10: User Story 8 — Config-key stance registry accuracy sweep (Priority: P3)

**Goal**: Four structurally-inapplicable `.cfg` keys read `Unsupported`, not `Recognized`; the JDBC
pool-tuning and store table-name keys become `.cfg`-reachable (FR-020, FR-021). **Depends on US2**
(needs `JdbcURL` scheme recognition already landed).

**Independent Test**: Audit the stance registry and confirm the four keys read `Unsupported` with a
documented reason; configure the JDBC pool/table-name keys alongside a `jdbc:`-style `JdbcURL` and
assert they apply to the resulting store.

### Tests for User Story 8

- [X] T090 [P] [US8] Write failing `.cfg` mapping tests for the 9 JDBC pool/table-name keys applying to
  a resolved `StoreConfig::Sql`/`Mssql`, in `crates/truefix-config/tests/store_and_log_mapping.rs`. —
  **complete**: 4 new tests (defaults stay `None`/`None` when no keys set; the 3 table-name keys apply
  to `StoreConfig::Sql`; the 6 pool-tuning keys apply to `StoreConfig::Sql`'s `pool`; the 3 table-name
  keys apply to `StoreConfig::Mssql`).

### Implementation for User Story 8

- [X] T091 [US8] Downgrade `SocketAcceptProtocol`/`SocketConnectProtocol`/`JdbcDataSourceName`/
  `JdbcConnectionTestQuery` to `Unsupported` with the documented reason, in
  `crates/truefix-config/src/keys.rs` (research.md §16). — **complete**, using the exact reasons
  `docs/engine-comparison-gaps.md` already documented.
- [X] T092 [US8] Extend `StoreConfig::Sql`/`Mssql` with optional `sessions_table`/`messages_table`/
  `session_id`/`pool` fields in `crates/truefix-store/src/lib.rs`, defaulting to
  `SqlStoreConfig`'s/`MssqlStoreConfig`'s existing defaults when absent. — **complete**: additive
  struct-variant fields (all `Option`, `None` = unchanged behavior). **Blast-radius note (mirrors T022's
  own precedent)**: unlike `Action::Send`, this genuinely required updating every existing exhaustive
  `{ url }`/`{ url: got }` destructuring pattern across `truefix-store/src/lib.rs`'s own `build_store`
  and 6 sites in `truefix-config/tests/store_and_log_mapping.rs` to `{ url, .. }` — a trivial,
  mechanical one-line diff per site (ignore the new fields), not a behavior change, so accepted as
  in-scope rather than pivoted away from (unlike T022, where the alternative avoided touching any
  existing site at all).
- [X] T093 [US8] Parse `JdbcMaxActiveConnection` + 5 pool-tuning siblings + `JdbcStoreMessagesTableName`/
  `JdbcStoreSessionsTableName`/`JdbcSessionIdDefaultPropertyValue` into T092's fields, in
  `jdbc_store_config()` in `crates/truefix-config/src/builder.rs` (depends on T092). — **complete, with
  one gap-filling addition**: `SqlPoolOptions` had no field for `JdbcConnectionKeepaliveTime` at all (4
  real siblings, not 5, as research.md's finding assumed) — added a new `keepalive: Option<Duration>`
  field, parsed and stored but intentionally not wired into `sqlx`'s pool (no matching `sqlx` hook
  exists), matching this project's established precedent for accepted-but-inert config keys (e.g.
  `SessionConfig::closed_resend_interval`).
- [X] T094 [US8] Move the 9 JDBC pool/table-name keys to `Impl` in
  `crates/truefix-config/src/keys.rs` (depends on T093; updates T002's tracking scaffold to fully
  `landed`). — **complete**.

**Checkpoint**: All nine user stories independently functional (US9 tracked separately below — its
own scope is large enough to warrant its own checkpoint). `cargo test --workspace --all-features`,
`cargo clippy --workspace --all-targets --all-features`, and `cargo fmt` all clean at this checkpoint.

---

## Phase 11: Polish & Cross-Cutting Concerns

- [X] T095 [P] Update `docs/parity-matrix.md` and `docs/acceptance-record.md` with the final stance
  sweep for every key this feature touches (FR-020/021) and a new "005" acceptance-record section
  covering all nine user stories. — **complete**: `parity-matrix.md`'s pre-existing "Feature 005 —
  Stance Tracking Scaffold (T002)" table (created earlier in this feature, all rows `pending`) filled
  in as `landed` with per-key mechanism notes, plus rows for US5/US6's stance-accuracy fixes not
  originally scaffolded in that table; `acceptance-record.md` gained a full "005" narrative section
  (mirroring 003/004's style) plus updated the top-level "Current gate status" numbers (workspace test
  count, 353→373 AT scenario runs) and "Outstanding before a v1 release claim" with the newly-open
  items this feature didn't close.
- [X] T096 [P] Update `docs/engine-comparison-gaps.md`: strike through every `GAP-##`/`BUG-##` this
  feature closes, with a cross-reference to this file's task range, mirroring the disclosure style
  `docs/todo-gap-analysis.md` already uses for closed GAP items. — **complete**: all P0 items
  (`BUG-01`–`04`, `GAP-07`/`08`/`09`/`18a`), `GAP-47`, `GAP-14`/`15`/`16`, the entire codec/dictionary
  cluster (`GAP-22`–`29`/`32`/`33`), `GAP-38`/`39`/`41`, and the four documentation-accuracy-note stance
  items struck through with `(005, FR-##, T###)` closure notes. `GAP-17` deliberately left **not**
  struck through — investigated directly against the actual code
  (`crates/truefix-transport/src/lib.rs`, `crates/truefix/src/lib.rs`) and confirmed
  `AllowedRemoteAddresses` is still a group-level union, not truly per-`SessionID` — updated its note
  from "moot in practice" (accurate pre-005, since `Engine::start` never built an `AcceptorBuilder` at
  all) to "partially addressed" (accurate post-005, since the allow-list is now honored, just not at
  QFJ's granularity) rather than either leaving the stale text or falsely claiming closure.
- [X] T097 [P] Update `README.md`: resend-veto-with-GapFill, PossDup anti-replay, duplicate-Logon
  rejection, `SessionQualifier`, reconnect backoff array, local bind, connect timeout, store
  creation-time, and the dictionary-model completeness additions (11 field types, open enums, etc.). —
  **complete**: status header, "Why TrueFix" AT-count, Quick Start (new session-safeguard paragraph +
  expanded `.cfg`-key paragraph), dual-track dictionary section, `truefix-dict` CLI's `--format qfj`
  example, a new 005 roadmap stage diagram, and every stale `353/353` reference updated to `373/373`
  (the two remaining `353` mentions are correctly-scoped historical descriptions of 003/004's
  contemporaneous state, left unchanged).
- [X] T098 Run `quickstart.md` validation end-to-end (all 9 per-user-story command blocks + full gate),
  fixing any command that doesn't match a real test (the established lesson from 003/004's own
  Polish phases) and filling in the actual test file names quickstart.md marked as placeholders. —
  **complete, with 4 command corrections found and fixed**: US1's `--test config_parsing` doesn't
  exist — BUG-01's fix is an inline `#[cfg(test)]` unit test in `crates/truefix-config/src/lib.rs`,
  not a separate integration-test file; corrected to `cargo test -p truefix-config --lib`. US3's
  `truefix-transport --test recovery` doesn't exist (`recovery.rs` is a `truefix-session` file); the
  actual US3 transport-level test is `resend_veto.rs`. US6's `truefix-transport --test sync_writes`
  is unrelated to US6 (a pre-existing 003 feature); connect-timeout coverage is actually in `lib.rs`'s
  inline `connect_timeout_tests`/`reconnect_delay_tests` modules, corrected to `--lib`. US9's
  `--test model_completeness`/`--test field_map` don't exist; corrected to the real file set
  (`field_types_extended.rs`/`group_validation.rs`/`version_meta.rs`/`dual_track.rs`/
  `dictionary_coverage.rs` in `truefix-dict`, `groups.rs`/`header_trailer_groups.rs`/
  `field_order_encode.rs` in `truefix-core`). Every command in the corrected quickstart.md was then
  actually run (not just read) and confirmed green, including AT conformance reporting exactly
  **373/373 scenario runs passed**.
- [X] T099 Full-gate sweep: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings` (default, `--features sql`, `--features mssql`, `--features redb`, `--features mongodb`,
  `--features dict-tooling`), `cargo test --workspace`, `cargo test -p truefix-store -p truefix-log
  --features sql/mssql/redb/mongodb`, `cargo test -p truefix-dict --features dict-tooling`, `cargo test
  -p truefix-at --test conformance` (confirm the AT scenario-run count grew from T001's recorded
  baseline, per SC-013 — the deliberate opposite of 004's "stays unmodified" gate), `cargo test -p
  truefix-at --test coverage` (confirm the regression-floor bump from T029/T034 is intentional and
  disclosed, not silently adjusted), `cargo deny check`. — **complete, fully green**: `cargo fmt --all
  --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean across all 6 variants
  (default + 5 feature flags); `cargo test --workspace` green (88 `test result: ok` blocks, 0
  failures); `cargo test -p truefix-store -p truefix-log --features sql/mssql/redb/mongodb` all green
  (18/18, 15/15, 16/16, 15/15 passing respectively, summed across both crates — SQLite/embedded cases
  unconditional, PostgreSQL/MySQL/MSSQL/MongoDB cases skip-clean without their `DATABASE_URL_*`,
  matching CI's service-container gating); `cargo test -p truefix-dict --features dict-tooling` green
  (8 test binaries, 0 failures); `cargo test -p truefix-at --test conformance` reports **373/373
  scenario runs passed** (up from T001's 353 baseline, confirming SC-013); `cargo test -p truefix-at
  --test coverage` green (`server_suite_scenario_run_count_does_not_regress` passes against the new
  373 floor — the one disclosed, deliberate assertion bump this feature makes, not a silent
  adjustment); `cargo deny check` reports `advisories ok, bans ok, licenses ok, sources ok`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **User Stories (Phases 2–10)**: All depend only on Setup. US8 (Phase 10) additionally depends on US2
  (Phase 3) — everything else is independently implementable/testable and may proceed in any order or
  in parallel. US3/US4 (Phases 4–5) share one implementation primitive and one AT-floor-bump task, so
  sequencing them adjacently (as ordered here) is efficient but not required.
- **Polish (Phase 11)**: Depends on all nine user stories being complete.

### Within Each User Story

- Tests are written first and MUST fail before implementation (Principle V).
- Within US2: T011/T012 (JdbcURL) and T013/T014 (AcceptorBuilder keys) are independent pairs; both
  block T015 (which needs both); T015 blocks T016 (stance update).
- Within US3: T022 (SendOrigin) blocks T023 (gap_fill_after_veto) blocks T024 (perform_actions);
  T025 blocks T026 blocks T027 (PossDup + its config switch); T028 (duplicate Logon) is independent of
  T022–T027; all implementation tasks block T029 (AT wiring + floor bump).
- Within US9: T069 (FieldType) blocks T070 (open_enum, for the MultipleValueString combination) blocks
  T071 (grammar); T072 blocks T073 (child dictionaries); T074 is fully independent (different crate);
  T075 is independent; T076 blocks T077 blocks T078 (field order, dual-track); T079 blocks T080
  (version metadata); T081 is independent; T082–T089 (per-version content expansion) are each fully
  independent of one another and of every mechanism task above.

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel.
- Once Setup completes, US1, US2, US3, US5, US6, US7, US9 can all start in parallel (US4 pairs
  naturally with US3; US8 waits on US2).
- Within US9, the 8 per-version dictionary-expansion tasks (T082–T089) are maximally parallelizable —
  independent files, no shared state, the single largest parallel-execution opportunity in this
  feature.
- All test tasks marked [P] within a story can run in parallel with each other.

---

## Parallel Example: User Story 9 (largest parallel opportunity)

```bash
# Launch all 8 per-version dictionary-expansion tasks together:
Task: "Diff and expand dict-src/normalized/FIX40.fixdict against QuickFIX/J's bundled FIX40 XML"
Task: "Diff and expand dict-src/normalized/FIX41.fixdict against QuickFIX/J's bundled FIX41 XML"
Task: "Diff and expand dict-src/normalized/FIX42.fixdict against QuickFIX/J's bundled FIX42 XML"
Task: "Diff and expand dict-src/normalized/FIX43.fixdict against QuickFIX/J's bundled FIX43 XML"
Task: "Diff and expand dict-src/normalized/FIX44.fixdict against QuickFIX/J's bundled FIX44 XML"
Task: "Diff and expand dict-src/normalized/FIX50.fixdict against QuickFIX/J's bundled FIX50 XML"
Task: "Diff and expand dict-src/normalized/FIX50SP1.fixdict against QuickFIX/J's bundled FIX50SP1 XML"
Task: "Diff and expand dict-src/normalized/FIX50SP2.fixdict against QuickFIX/J's bundled FIX50SP2 XML"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: User Story 1 (the two real correctness bugs — smallest, highest-confidence win).
3. **STOP and VALIDATE**: run `cargo test -p truefix-config -p truefix-core`.
4. Continue to US2/US3 (the rest of P1) before considering any P2/P3 story.

### Incremental Delivery

1. Setup → User Story 1 (P1) → validate → User Story 2 (P1) → validate → User Story 3 (P1) → validate
   (AT suite grows for the first time in this feature).
2. User Story 4 (P2, pairs with US3) → User Story 5 (P2) → User Story 6 (P2) → User Story 7 (P2) →
   validate each independently.
3. User Story 9 (P2, largest — can run in parallel with the rest of the P2 stories given team
   capacity) → validate (dual-track hash tests + coverage-diff test).
4. User Story 8 (P3, depends on US2) → validate.
5. Polish (Phase 11) → full-gate sweep → done.

### Parallel Team Strategy

With multiple developers, after Setup:
- Developer A: US1 → US2 → US8 (natural dependency chain).
- Developer B: US3 → US4 (shared implementation primitive).
- Developer C: US5 → US6 → US7.
- Developer D (or multiple): US9's 8 per-version dictionary-expansion tasks, fully parallel across
  developers once T069–T081's mechanism work lands.

---

## Notes

- [P] tasks touch different files with no dependency on an incomplete task.
- [Story] labels map every task to its spec.md user story for traceability back to
  `docs/engine-comparison-gaps.md`'s `GAP-##`/`BUG-##` identifiers (see each task's parenthetical).
- Commit after each task or logical group, per this project's established convention.
- US3/US4's AT-scenario-run-count growth (T029/T034/T099) is this feature's one deliberate exception
  to "the AT floor test's threshold never changes silently" — every other feature since 003 has kept
  it fixed; disclose the exact before/after counts when this lands.
- Avoid: vague tasks, same-file conflicts within a `[P]` group, cross-story dependencies beyond the one
  disclosed (US8 → US2).
