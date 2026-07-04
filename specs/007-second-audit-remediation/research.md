# Phase 0 Research: Second-Pass Audit Remediation (docs/todo/004.md)

**Feature**: [spec.md](./spec.md)

`docs/todo/004.md` already carries an unusual amount of internal rigor for a single audit document:
the original findings (`BUG-23`-`BUG-111`), a full self-review "Verification Pass" (corrected 3 items,
retracted 3), and a second four-agent "二次交叉验证" cross-check against the *current* working tree
(confirmed 3 items were already fixed by feature 006, narrowed 3 more, corrected 6 more). This
research pass does not re-derive that work from scratch. Instead, per this project's "verify before
implement" practice (established in 005/006's own plan.md precedent), it **directly re-reads current
source** for every User Story 1 (P1, must-fix) item — the highest-consequence tier — plus a
representative sample of User Story 2/3 items the source document itself flagged as needing rescoping
(`BUG-56`, `BUG-79`, `BUG-89`, `BUG-100`). Every citation below was read fresh during this pass, not
copied from `004.md`'s text; exact file:line references reflect what was actually read. All of them
confirmed the document's (already-corrected) claims exactly — no new correction was needed beyond
what `004.md`'s own two internal passes already made. The remaining ~50 User Story 2/3 items rely on
`004.md`'s own triple-verification as sufficient evidentiary basis (proportional effort for
lower-priority items, matching 006's Phase 0 precedent for its own P2/P3 tier).

Organized R1-R3, one section per spec.md user story, in priority order.

---

## R1 — Pre-production-blocking fixes (US1)

### R1.1 — `BUG-30`/`BUG-31`/`BUG-99`: sequence-number store crash-safety and format (FR-001/FR-001a/FR-002)

**Read**: `crates/truefix-store/src/file.rs:191-251` (`SeqFile`), `:512-529` (`load_seqnums`).

- `SeqFile` is a single combined file (one `seqnums` path) holding both sender and target on two
  text lines (`write!(f, "{sender}\n{target}\n")`).
- `write()` (line 215) opens with `.truncate(true)` then writes — on **every** `set_sender`/
  `set_target`/`reset` call, i.e. on every message. A crash between truncate and write empties the
  file.
- `load_seqnums` (line 512): a parse failure on either line falls through `.unwrap_or(1)` — silently
  identical to a legitimately-absent file (which correctly returns `Ok((1,1))` via the separate
  `NotFound` branch at line 526). No way to distinguish "fresh store" from "corrupted store" today.

**Confirmed** exactly as `004.md` describes (`BUG-30`/`BUG-31`), including the single-combined-file
layout `BUG-99` contrasts against QuickFIX/J's separate `senderseqnums`/`targetseqnums` files.

**Decision** (per Clarifications, Session 2026-07-04): split `SeqFile` into two independent
single-value files, `senderseqnums` and `targetseqnums`, mirroring QuickFIX/J's layout exactly.
- Each file's own write path uses an atomic write-temp-file-then-rename (not truncate-in-place),
  closing the crash window for every regular per-message update — not just `reset()`.
- `reset()` deletes and recreates both files wholesale (matching QuickFIX/J/Go's reset semantics,
  `BUG-99`).
- A parse failure on either file (present but unparseable) surfaces as a typed `StoreError` at
  open time (`FR-002`) rather than defaulting to 1 — genuinely distinguishable now from "file
  doesn't exist yet" (`NotFound` stays `Ok((default))`, unchanged).
- **Migration** (`FR-001a`, per Clarifications): on open, if neither new file exists but the legacy
  combined `seqnums` file does, read and split it into the new two-file layout before proceeding,
  then leave the legacy file in place (informational only — not deleted, so a rollback to an older
  TrueFix binary still finds its old-format file intact). This is the one behavior in this feature
  that must be exercised as its own dedicated integration test (a "legacy file present, new files
  absent" fixture), not just inferred from the crash-safety unit tests.

**QuickFIX/J behavior cited** (documented behavior only): `FileStore.java` `initialize()`/`reset()`
uses separate `senderSeqNumFile`/`targetSeqNumFile`, deleted and recreated via
`closeAndDeleteFiles()` on reset.

### R1.2 — `BUG-28`: `ResetSeqNumFlag` handshake (FR-005/FR-006)

**Read**: `crates/truefix-session/src/admin.rs:48-65` (`logon`), `crates/truefix-session/src/
state.rs:967-1067` (`on_logon`).

- `admin::logon()` line 55: `if config.reset_on_logon { m.body.set(Field::string(RESET_SEQ_NUM_FLAG,
  "Y")); }` — driven entirely by static session configuration.
- `state.rs` `on_logon()` lines 1020-1030: **does** read the inbound Logon's own `RESET_SEQ_NUM_FLAG`
  and resets local sequences accordingly — but the acceptor's Logon *response*, built at line 1059
  (`admin::logon(&self.config, seq, next_exp)`), passes no information about that inbound flag through
  to the echo logic. The response's flag is `config.reset_on_logon`, independent of what was just
  processed.
- The `Role::Initiator if self.state == AwaitingLogon` arm (lines 1063-1065) unconditionally
  transitions to `LoggedOn` with **no** check of the Logon response's own `ResetSeqNumFlag`/
  `MsgSeqNum` — confirms the second defect (no echo verification, no seq=1 inference) exactly.

**Confirmed** both defects exactly as `004.md` describes.

**Decision**: thread the inbound Logon's own `RESET_SEQ_NUM_FLAG` value into `admin::logon()`'s
construction (an explicit parameter, replacing the `config.reset_on_logon` read for the *response*
case specifically — the *outbound-initiator's-own-first-Logon* case, still driven by
`config.reset_on_logon`, is unaffected and out of scope here). On the initiator side, extend
`on_logon`'s `Role::Initiator` arm to check the response's own `RESET_SEQ_NUM_FLAG`/`MsgSeqNum`
against what was expected when this session itself sent `ResetSeqNumFlag=Y`; treat a missing/
mismatched echo as a handshake failure via the same `reject_logon`-style Logout+disconnect path
already used elsewhere in this function (`BUG-05`'s precedent, feature 006).

**QuickFIX/J behavior cited**: `Session.java` `generateLogon(otherLogon, ...)` sets the echoed flag
from `state.isResetReceived()`; `nextLogon()` disconnects with "Expected Logon response to have
reset sequence numbers" when the acceptor doesn't acknowledge, and infers a reset from the
response's own `MsgSeqNum == 1` when the flag itself is absent (QFJ-383).

### R1.3 — `BUG-29`: `ResendRequest`-vs-`ResendRequest` deadlock (FR-007)

**Read**: `crates/truefix-session/src/state.rs` `on_resend_request` and the `Ordering::Greater`
handling in `on_received` (already touched by feature 006's `resend_request_missing_*` fixes,
confirmed those additions did not change this specific control flow: a too-high `ResendRequest` is
still queued via the generic out-of-order path, not answered immediately).

**Confirmed**: the too-high-sequence branch for an inbound `ResendRequest` message defers to
`drain_queue()` (only run once the *return-direction* gap is filled), rather than calling the
resend-building logic (`build_resend`, already used by the in-order `on_resend_request` path)
immediately. Two sessions each holding an outstanding `ResendRequest` toward the other deadlock:
each is waiting on the other's gap to fill before it will look at the queued `ResendRequest`.

**Decision**: special-case `ResendRequest` in the too-high-sequence dispatch so it is answered via
`build_resend()` immediately (bypassing the queue), independent of whether *our own* outstanding gap
toward the counterparty has been filled yet — matching QuickFIX/J's `verify(rr, false,
validateSequenceNumbers)` deliberately skipping the too-high check for this one message type
(QFJ-673).

### R1.4 — `BUG-26`: `SqlStore` migration ordering (FR-003)

**Read**: `crates/truefix-store/src/sql.rs` `ensure_schema`.

**Confirmed**: the `INSERT ... creation_time ...` statement for the sessions table is emitted before
`add_creation_time_column_if_missing()` runs. `CREATE TABLE IF NOT EXISTS` is a no-op against a
pre-existing table without that column, so the INSERT references a column that doesn't exist yet on
any database created before `creation_time` was introduced (005's `GAP-38`).

**Decision**: reorder — call `add_creation_time_column_if_missing()` (and any sibling
already-missing-column migrations in the same function) before the first statement that could
reference the column. Applies identically across all three `SqlStore`-backed engines (SQLite/
Postgres/MySQL), which share this one `ensure_schema` code path.

### R1.5 — `BUG-41`: MSSQL commit-failure rollback gap (FR-004)

**Read**: `crates/truefix-store/src/mssql.rs` `save_and_advance_sender`.

**Confirmed**: the statement-failure branch correctly issues a `ROLLBACK`; the final `COMMIT
TRANSACTION`'s own failure branch returns `Err(e)` directly with no rollback, leaving an open
transaction on the mutex-held connection — every subsequent operation on that connection is now
blocked behind an uncommitted, unrolled-back transaction.

**Decision**: add a `ROLLBACK` in the commit-failure branch too, mirroring the existing
statement-failure branch's error handling exactly.

### R1.6 — `BUG-27`: `Engine::shutdown()` cannot stop plain initiators (FR-013)

**Read**: `crates/truefix/src/lib.rs:269-277` (`Engine` struct), `:725-765` (`shutdown`,
`abort_acceptors_and_stop_failover`, `cleanup_partial_start`), `crates/truefix-transport/src/
lib.rs:345-365` (`SessionHandle`).

- `Engine::shutdown()`'s own doc comment (already corrected by feature 006, `BUG-21`) discloses
  exactly this gap: it calls `abort_acceptors_and_stop_failover`, which only touches `self.acceptors`
  and `self.failover_initiators` — `self.initiators: Vec<SessionHandle>` (plain, non-failover) is
  never touched by the normal-shutdown path.
- `cleanup_partial_start` (feature 006's `BUG-11` fix) *does* stop plain initiators, but only runs
  inside `Engine::start`'s own error path when a later session fails to start — not reachable from
  `shutdown()`.
- `SessionHandle` (transport crate) exposes only `logout()` (async) and `join(self)` (consuming); no
  synchronous way to abort its underlying `task: JoinHandle<()>` exists today.
- `Engine` has no `impl Drop` — dropping an `Engine` value silently detaches every spawned task.

**Confirmed**: `004.md`'s "二次核实" narrowing is accurate — the startup-failure path is already
handled (006), but the *normal-operation* shutdown gap and the missing `Drop` impl both remain.

**Decision**: add a non-consuming, synchronous `SessionHandle::abort(&self)` (wrapping the private
`JoinHandle`'s own sync `.abort()`) alongside the existing async `logout()` — additive, non-breaking.
`Engine::shutdown()` calls `.abort()` on every plain initiator in addition to its existing acceptor/
failover handling, satisfying "stop every session task" (`FR-013`) without changing `shutdown()`'s
public (synchronous) signature. Add `impl Drop for Engine` performing the same abort-everything
sweep as a safety net for callers who never call `shutdown()` explicitly — best-effort and
non-graceful (no `.await`, so no graceful Logout is possible from `Drop`), but closes the
"undetectably running forever" failure mode `FR-013` targets.

### R1.7 — `BUG-25`/`BUG-94`: scheduled initiator never reconnects after a drop; no backoff (FR-012, FR-026)

**Read**: `crates/truefix-transport/src/lib.rs:1910-1980` (`run_scheduled_initiator`).

- `current: Option<SessionHandle>` is set to `Some(handle)` on a successful connect (line ~1968) and
  never re-checked for liveness — `SessionHandle` (as of R1.6's reading) has no non-consuming
  liveness check today. Once set, `was_in_session && current.is_none()` can never become true again
  after a drop, so the reconnect branch never re-fires — directly contradicting the function's own
  comment about retrying after a transient drop.
- The loop's own retry cadence is a fixed `tokio::time::sleep(Duration::from_millis(200))` — no
  backoff at all (`BUG-94`, same function).

**Confirmed** both exactly as described.

**Decision**: add `SessionHandle::is_finished(&self) -> bool` (delegates to the private
`JoinHandle::is_finished()`, non-consuming, additive) and check it each loop iteration — clear
`current` to `None` when the held handle's task has already finished, letting the existing
`was_in_session && current.is_none()` reconnect condition fire naturally on the next iteration
without changing that condition's own logic. For backoff, apply the same small step sequence pattern
this project already uses for `ReconnectInterval` (`reconnect_delay()`, feature 004/005) to this
loop's own retry cadence — a short, bounded step sequence (e.g. 2s→5s→10s→15s→30s, matching
QuickFIX/J's own `computeNextLogonDelayMillis`) triggered specifically on a *failed* connect
attempt, resetting to the fast 200ms poll once a connection is active (this loop's 200ms cadence
also drives the schedule-boundary check itself, which must stay fast/responsive — only the
failed-connect-retry cadence needs to back off, not the schedule-polling cadence).

### R1.8 — `BUG-32`: no duplicate/competing-connection protection (FR-008)

**Read**: `crates/truefix-transport/src/lib.rs:1403-1420` (`Registry`), `:1571-1575`
(`route_and_run` signature).

**Confirmed**: `Registry` holds only `sessions: HashMap<SessionId, SessionConfig>` (static
config-to-identity mapping) and `session_stores: HashMap<SessionId, Arc<dyn MessageStore>>`
(per-session store routing, feature 006's `BUG-07` fix) — no structure anywhere tracks which
`SessionId`s currently have an active, connected task. A second connection presenting an
already-connected `SessionId` is routed and accepted exactly like the first.

**Decision**: add a shared `active: Arc<Mutex<HashSet<SessionId>>>` (or equivalent) to the acceptor's
shared state, checked-and-inserted in `route_and_run` immediately after the routing `SessionId` is
resolved and before `run_connection` is spawned; removed when that connection's task ends. A
second connection for an already-`active` `SessionId` is refused (connection closed without
processing its Logon) rather than routed.

### R1.9 — `BUG-33`: `Event::Disconnected` missing; `ResetOnDisconnect` unreachable on TCP drop (FR-009)

**Read**: `crates/truefix-session/src/state.rs:40-53` (`Event` enum), `:1151-1159`
(`enter_disconnected`), `crates/truefix-transport/src/lib.rs:842-850` (`run_connection`'s shutdown
tail).

- `Event` has exactly 5 variants (`Connected`/`Received`/`Tick`/`StartLogout`/`Garbled`) — no
  `Disconnected`.
- `enter_disconnected()` (the function that actually honors `reset_on_disconnect`/`reset_on_logout`)
  is only reachable from within `state.rs`'s own event handlers (logon-timeout, heartbeat-timeout,
  Logout-received paths) — never from the transport layer.
- `run_connection`'s shutdown tail (line 842: `let _ = write_half.shutdown().await;`) calls no
  `session.handle(...)` at all before the `Session` value is dropped — a raw TCP drop never reaches
  the state machine.

**Confirmed** exactly as described.

**Decision**: add `Event::Disconnected` to the enum; its handler calls the existing
`enter_disconnected()` and returns its resulting `Action`s (primarily `Action::ResetStore` when
applicable) through the same `dispatch`/`perform_actions` pipeline every other event already uses.
`run_connection`'s shutdown path calls `dispatch(&mut session, Event::Disconnected, ...)` before the
`write_half.shutdown()`/session-drop sequence, so a `ResetStore` action from an unexpected drop
reaches the store exactly as it would from a graceful Logout-driven disconnect.

### R1.10 — `BUG-34`: `from_admin`/`from_app` called before session-layer validation (FR-010)

**Read**: `crates/truefix-transport/src/lib.rs:1068-1123` (`handle_inbound`).

**Confirmed**: `handle_inbound` calls `app.from_admin(&msg, id).await` (admin messages) or
`app.from_app(&msg, id).await` (application messages) **before** calling
`dispatch(session, Event::Received(msg), ...)` — i.e. before the session state machine's own
sequence/identity/latency/PossDup/dictionary checks for `msg` have run at all. An application can
observe and act on a message the session layer is about to reject.

**Decision**: restructure `handle_inbound` so `dispatch(session, Event::Received(msg), ...)` (or an
equivalent session-layer pre-check extracted from it) runs first; only invoke `from_admin`/`from_app`
if that validation did not already reject the message. This is the one User Story 1 item requiring
genuine control-flow restructuring rather than an additive check — `dispatch`'s existing return
contract (`Result<(), ()>`, `Err` meaning "tear the session down") and its relationship to the
existing `session.reject_logon`/`session.business_reject` calls inside the current (wrong-order)
`handle_inbound` need to be preserved for the reject-response messages themselves, just reordered
relative to the callback invocation. Concrete refactor shape (validate-then-callback, single
dispatch call, callback invoked only on a still-valid message) to be finalized during `/speckit-tasks`
against `dispatch`'s and `validate_app`'s exact current signatures.

### R1.11 — `BUG-100`: `frame_length` accepts `BodyLength=0` (FR-014)

**Read**: `crates/truefix-core/src/framing.rs:19-50` (`frame_length`).

**Confirmed**: `body_len` is checked only against `MAX_BODY_LEN` (feature 006's `MAX_BODY_LEN` bound
— confirms `004.md`'s own "二次核实" note that this bound also makes `BUG-23`'s overflow path
unreachable, downgrading it to defense-in-depth, folded into User Story 3). No check for
`body_len == 0` anywhere; a zero-length body frames and decodes as a valid (if empty) message.

**Decision**: reject `body_len == 0` as a new `DecodeError` variant (or reuse
`InvalidBodyLength`), consistent with both QuickFIX/J (QFJ-903) and QuickFIX/Go rejecting it.

### R1.12 — `BUG-86`/`BUG-87`/`BUG-88`: acceptor has zero schedule awareness (FR-015/FR-016)

**Read**: `crates/truefix-session/src/config.rs:73` (`schedule: Option<Schedule>` field exists on
`SessionConfig`); `grep`-confirmed zero references to `self.config.schedule`/`.schedule` anywhere in
`crates/truefix-session/src/state.rs`.

**Confirmed**: the session state machine (`state.rs`) never reads its own `schedule` field at all.
The only schedule enforcement anywhere in the codebase lives in `truefix-transport`'s
`run_scheduled_initiator` — initiator-only, transport-layer, and (per R1.7) itself has its own
reconnect bug. An acceptor with a configured trading-hours schedule accepts Logons and keeps sessions
alive at any hour.

**Decision**: `on_logon` gains a schedule check — reject the Logon (same Logout+disconnect path as
`BUG-05`) when `self.config.schedule` is set and `!schedule.is_in_session(now_utc)` (`Schedule::
is_in_session` already exists and is unit-tested, feature 004/005). `on_tick` gains a periodic
schedule-boundary check for an already-`LoggedOn` session — when the session crosses out of its
window, disconnect it via the existing `enter_disconnected`-driven path (R1.9's `Event::Disconnected`
plumbing is a natural fit here too, or a direct call, to be decided at `/speckit-tasks` time).
`BUG-88` (`forceResendWhenCorruptedStore` not applied during gap-fill resend, only at store-open) is
grouped here as the same "session-layer schedule/force-resend awareness" theme — `build_resend()`
gains a check of `self.config.force_resend_when_corrupted_store`, resending admin messages instead
of gap-filling them when set, matching QuickFIX/J's `resendMessages()` two-effect semantics.

### R1.13 — `BUG-89`: dictionary validation skips all admin messages (FR-011)

**Read**: `crates/truefix-session/src/state.rs:679-683` (`validate_app`).

**Confirmed**: `if is_admin_type(msg.msg_type()) { return None; }` is the very first check —
admin-typed messages never reach the dictionary-validation logic below it at all, regardless of
whether a dictionary is configured.

**Decision**: remove the blanket admin-type early-return; run the same dictionary-validation logic
for admin messages that already runs for application messages. Requires care: `validate_app` is
called from the in-order (`Ordering::Equal`) receive path and `drain_queue`, both of which already
assume application-message semantics for the resulting `Reject`/`BusinessMessageReject` split — an
admin-message dictionary failure should route through the plain session-level `Reject` path (not
`BusinessMessageReject`, which is application-specific), to be confirmed against the dictionary
layer's own `error.business` flag during implementation.

---

## R2 — Narrower-impact defects (US2)

Each item below was independently, directly re-read during this pass; grouped here (rather than
one subsection per item, as R1 above) since none required more than a short confirmatory read — no
corrections beyond what `004.md`'s own two internal passes already applied were found.

- **`BUG-56`** (`send_app` missing SubID/LocationID, FR-030-adjacent — actually not separately
  FR'd, folded into the general outbound-identity theme `send_app` already handles for
  BeginString/CompIDs): **directly confirmed** `admin::base()` (`admin.rs:16-40`) already stamps
  tags 50/142/57/143 correctly (contradicting `004.md`'s *original* framing, matching its
  "Verification Pass" correction); `Session::send_app()` (`state.rs:380-393`) stamps only tags
  8/49/56/34/52 and never touches 50/142/57/143. **Decision**: add the same conditional
  SubID/LocationID stamping `admin::base()` already does to `send_app()`.
- **`BUG-79`** (no `BeginString` format check, folded into US3's `FR-048`): confirmed
  `frame_length()` (`framing.rs:23`) checks only `buf.get(0..2) == Some(b"8=")`, no deeper
  `FIX.\d\.\d`/`FIXT.\d\.\d` pattern.
- **`BUG-46`** (`FR-047`, checksum-position verification): grouped with R1.11's frame-length reading
  — `frame_length` never inspects the bytes it computes as the checksum-field position, only
  `buf.len() >= total`.
- All remaining User Story 2 items (`BUG-24`/`35`/`38`-`45`/`49`-`53`/`57`-`61`/`68`/`70`/`71`/`81`/
  `90`-`97`) rely on `004.md`'s own citations, already reconciled through its "Verification Pass" and
  "二次交叉验证" corrections into this spec's acceptance scenarios and FRs — each names a specific
  function/file, to be re-confirmed as each item's own first implementation sub-task (this project's
  established "verify before implement" practice), not re-verified again here.

---

## R3 — Low-priority hardening (US3)

All User Story 3 items are, by the source document's own final triage, low-probability/low-impact
edge cases or reference-implementation leniency differences — the document's original findings plus
its two internal verification passes are treated as sufficient evidentiary basis without a third
independent re-read in this planning pass (proportional effort, matching 006's own Phase 0 precedent
for its lowest-priority tier). `BUG-23` (checked_add hardening) and `BUG-79` (BeginString format,
R2 above) were incidentally confirmed as part of reading `frame_length` for R1.11.

Each US3 item's own implementation sub-task re-confirms its specific citation before changing code,
per this project's established practice — not a planning-phase requirement.

---

## Summary of decisions requiring disclosure

- **Sequence-number store format change** (R1.1): `seqnums` (one file, two lines) →
  `senderseqnums`/`targetseqnums` (two files). Disclosed, additive migration path (legacy file
  auto-read-and-split on open, left in place afterward) — not a breaking change for any existing
  deployment, per Clarifications.
- **`SessionHandle` gains two new public methods** (R1.6, R1.7): `abort(&self)` (sync) and
  `is_finished(&self) -> bool` (sync, non-consuming) — both additive, no existing method's
  signature changes.
- **`Engine` gains `impl Drop`** (R1.6) — new behavior on drop (best-effort task abort), previously
  undefined/silent-detach behavior. Disclosed since it changes what happens when a caller doesn't
  call `shutdown()` explicitly, even though no existing public API surface changes.
- **`Event` enum gains a new variant, `Disconnected`** (R1.9) — additive (existing `match` arms in
  `truefix-session` that already use a catch-all `_` arm are unaffected; any exhaustive match outside
  the crate would need updating, but `Event` is only constructed/matched within `truefix-session`
  and `truefix-transport` today, both first-party crates touched by this same feature).
- No new external dependencies anywhere in this feature (spec Assumptions, reconfirmed here: every
  decision above uses only facilities already present in the touched crates — `tokio::task::
  JoinHandle`'s existing sync `abort()`/`is_finished()`, `std::collections::HashSet`, `time`'s
  existing formatting).
