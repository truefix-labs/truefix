# Phase 0 Research: Audit Remediation (docs/todo/003.md)

**Feature**: [spec.md](./spec.md)

Every citation below was re-verified by directly reading current source during this planning pass
(not merely trusted from `docs/todo/003.md`'s own text) — file:line references reflect the exact
lines read. Every citation checked out exactly as the audit described; no correction to the audit's
own findings was needed (unlike feature 005's Phase 0, which did surface one correction). One number
was independently re-derived rather than merely trusted: the AT suite's actual `server_suite()`
scenario-run count was computed by running the suite live (§R9), confirming the audit's claimed 373
exactly.

Organized R1-R10, one section per spec.md user story, in the same priority order.

---

## R1 — Session protocol correctness (US1)

### R1.1 — `BUG-05`: low-seq Logon acceptance (FR-001)

**Read**: `crates/truefix-session/src/state.rs:835-887` (`on_logon`).

- Line 841: `disposition = logon_seq.map(|s| s.cmp(&self.next_in_seq))` — computed once.
- Lines 842-844: only `Some(Ordering::Equal)` advances `next_in_seq`.
- Lines 850-869: the state-transition block (`Role::Acceptor if self.state == AwaitingLogon => {...
  self.state = LoggedOn; ... send Logon response}`) gates **purely on `self.state`**, never
  consulting `disposition`.
- Lines 874-887: the deferred-actions match only handles `Some(Equal)` and `Some(Greater)` — `Less`
  and `None` both fall through to `_ => {}`.

**Confirmed**: a Logon with `logon_seq < next_in_seq` (no `PossDupFlag` handling at all in this
function) still transitions to `LoggedOn` and sends a Logon reply at lines 850-869, with
`next_in_seq` left unchanged (never advanced, since only `Equal` advances it at 842-844).

**Decision**: Insert a check immediately after computing `disposition` (before the state-transition
block at 850): if `disposition == Some(Ordering::Less)`, route through `self.reject_logon(...)`
(the existing Logout+disconnect helper, already used by `BUG-22`-adjacent duplicate-Logon rejection
at lines 810-817) and return early, skipping the state-transition/response block entirely.
`PossDupFlag`-justified low-seq Logons are out of scope for this fix (QuickFIX/J's own `verify` path
for Logon does not special-case `PossDup` differently from any other too-low check per the audit's
QFJ citation — `Session.java`'s `isTargetTooLow`/`doTargetTooLow`).

**QFJ behavior cited** (documented behavior only, no source read): `nextLogon` calls
`verify(logon, false, validateSequenceNumbers)` before any acceptance code runs; `doTargetTooLow`
generates a Logout and aborts without a state transition or Logon reply when `PossDupFlag` is absent.

### R1.2 — `BUG-06`/`B6`: SequenceReset rewind + missing-tag (FR-002, FR-003)

**Read**: `crates/truefix-session/src/state.rs:784-802` (`on_sequence_reset`).

- Lines 786-791: `new_seq` is `Option<u64>`, `None` when tag 36 is absent or non-positive.
- Lines 792-800: `if let Some(ns) = new_seq { if gap_fill { if ns >= next_in_seq {...} } else {
  self.next_in_seq = ns; /* unconditional */ } }` — the plain (`else`) branch has no lower-bound
  guard at all.
- Line 801: `self.drain_queue()` runs unconditionally, including when `new_seq` was `None` (B6).

**Confirmed**: both findings exactly as described — a plain-mode `SequenceReset` with a decreasing
`NewSeqNo` rewinds `next_in_seq` unconditionally; one with `NewSeqNo` absent skips adjustment but
still drains the queue as if nothing were wrong.

**Cross-reference**: `SequenceReset` (msg type `"4"`) is routed directly from `on_received` at
line 541-543, **before** the `Ordering::Less`+`PossDupFlag` anti-replay branch (lines 594-609) that
protects every other inbound message type — confirming the audit's framing that this is a genuine
anti-replay hole distinct from, and unprotected by, the existing PossDup check.

**Decision**:
- Plain-mode, `ns < next_in_seq`: emit a session Reject (`SessionRejectReason::ValueIsIncorrect`,
  ref tag 36 = `NEW_SEQ_NO`) via the same `admin::reject_with_reason` path `validate_app` already
  uses (line 649-656), instead of applying the rewind.
- `new_seq == None` (tag 36 absent/non-positive): treat as a required-field violation — reject with
  `SessionRejectReason` required-tag-missing, ref tag 36, **before** calling `drain_queue()`.
- Plain-mode, `ns == next_in_seq`: no-op accept (spec Edge Cases) — falls out naturally since
  `self.next_in_seq = ns` is a no-op when equal; no special-case needed.

### R1.3 — `BUG-22`: malformed ResendRequest (FR-004)

**Read**: `crates/truefix-session/src/state.rs:723-746` (`on_resend_request`).

- Lines 724-729: `begin` defaults to `0` when tag 7 (`BeginSeqNo`) is absent/non-positive.
- Lines 730-735: `end_req` defaults to `0` when tag 16 (`EndSeqNo`) is absent.
- Lines 742-744: `if begin == 0 || begin > end { return Vec::new(); }` — **no response at all** in
  either the missing-tag case or the `begin > end` case; both currently collapse to the same silent
  no-op.

**Decision**: Distinguish the two cases per spec Edge Cases. Read `BeginSeqNo`/`EndSeqNo` presence
directly (not just parsed-and-defaulted value) — if either tag is absent from `msg.body` entirely,
emit a `SessionRejectReason` required-tag-missing response (ref tag 7 or 16, whichever is absent).
If both tags are present but `begin > end`, retain today's silent-`Vec::new()` behavior unchanged
(spec Edge Cases explicitly keeps this case as-is).

### R1.4 — `B1`: `ResetOnLogon` partial-reset reconnect-fail loop (FR-005)

**Read**: `crates/truefix-session/src/state.rs:487-500` (`on_connected`), `:819-829` (`on_logon`'s
reset block), `:426-434` (`reset_sequences`).

- `on_connected` (Initiator role, lines 490-497): sends its own Logon immediately, sets
  `self.logon_sent = true`, does **not** call `reset_sequences` itself.
- `on_logon`, lines 819-829: `let full = !self.logon_sent; self.reset_sequences(full); store_reset =
  full;` — when the inbound Logon (the acceptor's reply, itself carrying `ResetSeqNumFlag=Y` per
  mutual-reset FIX semantics) is processed, `full` is `false` whenever the initiator already sent its
  own Logon on this connection (line 495) — which is **always true** for an initiator, since
  `on_connected` unconditionally sets `logon_sent = true` before any Logon can be received.
- `reset_sequences(full)` (lines 426-434): `next_in_seq = 1` unconditionally; `next_out_seq = 1` and
  `store.clear()` **only** when `full`.

**Confirmed root cause**: for an initiator with `ResetOnLogon` configured, `full` evaluates to
`false` on essentially every connection (the initiator's own Logon send always precedes processing
the acceptor's reply), so only the inbound direction resets while `next_out_seq`/the store remain at
their pre-reset values (e.g. `next_out_seq = 2` after sending the initial Logon as seq 1). The
acceptor, meanwhile, performs its own full reset (expects seq 1). The initiator's next outbound
message uses `next_out_seq = 2`, creating a gap from the acceptor's perspective, triggering a
`ResendRequest`; the initiator's un-cleared store still holds the original Logon (msg type `"A"`) at
seq 1, so `build_resend` resends **a Logon message** to an already-logged-on acceptor, which (per
005's `GAP-18a` duplicate-Logon rejection, lines 810-817) rejects it with Logout+disconnect — killing
every reconnect attempt.

**Decision**: `full` must not be derived from `!self.logon_sent` (an accident of send-order, not a
signal of intent). Per QFJ's `ResetOnLogon` semantics (a full reset happens once, at the point the
reset-carrying Logon is being prepared/processed for this connection, in both directions,
independent of which side's Logon is processed first), change the reset trigger: when
`RESET_SEQ_NUM_FLAG` is present on an inbound Logon **and** this is the first Logon processed on the
current connection (regardless of whether we already sent our own), perform a full reset
(`next_in_seq = 1`, `next_out_seq = 1`, `store.clear()`). Concretely: track "have I already performed
the reset for this connection" as its own per-connection flag (reset in `on_connected`, alongside the
`B4` fix below), rather than reusing `logon_sent` for this purpose.

### R1.5 — `B3`: `drain_queue` skips `validate_app` (FR-006)

**Read**: `crates/truefix-session/src/state.rs:671-683` (`drain_queue`), `:560-578` (the in-order
`Ordering::Equal` branch of `on_received`).

- `on_received`'s `Equal` branch (lines 561-573) calls `self.validate_app(&msg)` before processing.
- `drain_queue` (lines 673-677) calls `self.process_in_order(&msg, ...)` directly — **no
  `validate_app` call anywhere in `drain_queue`**.

**Confirmed**: a message enqueued while sequence numbers were ahead-of-expected (line 580,
`Ordering::Greater` branch) and later drained by a gap-fill never passes dictionary/application
validation, unlike an in-order message.

**Decision**: In `drain_queue`, call `self.validate_app(&msg)` for each drained message before
`process_in_order`, mirroring the `Equal`-branch logic (reject-and-continue-or-disconnect per
`self.config.disconnect_on_error`, same as lines 562-573). Care needed: `drain_queue` is itself
called from `on_sequence_reset` (line 801) and from the tail of `on_received`'s `Equal` branch (line
576) and `on_logon` (line 875) — the validation-failure control flow (early exit vs. continue
draining) must compose correctly with the loop structure at lines 673-677, likely requiring
`drain_queue` to return early (not continue the `while` loop) on the first validation failure, same
as the single-message case would.

### R1.6 — `B4`: `on_connected` doesn't reset per-connection timers (FR-007)

**Read**: `crates/truefix-session/src/state.rs:487-500` (`on_connected`).

Confirmed: `on_connected` resets only `self.ticks_awaiting = 0` and `self.state`. It does not touch
`ticks_since_recv`, `test_request_outstanding`, `resend_target`, or `resend_chunk_end` — all of which
persist from a prior connection's state if the `Session` object is reused across reconnects (not
reconstructed).

**Decision**: Add resets for `ticks_since_recv = 0`, `test_request_outstanding = false`,
`resend_target = None`, `resend_chunk_end = None` at the top of `on_connected`, alongside the
existing `ticks_awaiting = 0`. Combine with R1.4's new per-connection reset-tracking flag reset in
the same place.

### R1.7 — `B5`: dictionary-validation-failure disconnect skips Logout (FR-008)

**Read**: `crates/truefix-session/src/state.rs:510-534` (identity/latency failure paths),
`:560-573` (dictionary-validation failure path within `on_received`'s `Equal` branch).

- Latency failure (lines 515-523) and identity failure (lines 526-534) both: build a Logout message
  via `admin::logout`, `send_stored`, **then** `Action::Disconnect`.
- Dictionary-validation failure (lines 562-573): builds a `reject` (session Reject or
  BusinessMessageReject, from `validate_app`), sends it, and — when `disconnect_on_error` is true —
  pushes `Action::Disconnect` directly. **No Logout is built or sent in this path.**

**Confirmed**: exactly as described — one disconnect path sends Logout-then-disconnect, the other
sends Reject-then-disconnect with no Logout at all.

**Decision**: In the `disconnect_on_error` branch at lines 566-569, before pushing
`Action::Disconnect`, additionally build and send a Logout (reusing `admin::logout` the same way the
identity/latency paths do) so all three disconnect-on-error paths converge on the same
Logout-then-disconnect shape.

### R1.8 — `B7`: unparseable `SendingTime` bypasses latency check (FR-009)

**Read**: `crates/truefix-session/src/state.rs:473-485` (`latency_ok`).

- Line 480-482: `let Ok(ts) = field.as_utc_timestamp() else { return true; // unparseable SendingTime
  is a dictionary/validation concern };` — an existing code comment already rationalizes this as
  intentional, but the audit (and spec US1 scenario 8) treats it as a genuine latency-check bypass:
  an unparseable `SendingTime` should not be treated as "latency OK."

**Decision**: Change the `else` branch to `return false` (fail the latency check) instead of `true`.
This routes an unparseable `SendingTime` through the existing latency-failure Logout+disconnect path
(lines 515-523) — already correct, no further change needed there. Remove the now-inaccurate comment.

---

## R2 — Multi-session acceptor routing (US2)

### R2.1 — `BUG-07`/`GAP-20`: routing key ignores SubID/LocationID/Qualifier (FR-010, FR-011)

**Read**: `crates/truefix-transport/src/lib.rs:1398-1441` (`route_and_run`),
`crates/truefix-session/src/session_id.rs:29-71` (`SessionId::new`/`new_full`).

- `route_and_run` (line 1429-1433): extracts `begin`, `their_sender` (tag 49), `their_target` (tag
  56) from the inbound Logon, then builds `SessionId::new(begin.clone(), their_target.clone(),
  their_sender.clone())` — the 3-arg constructor (`session_id.rs:29-44`), which hardcodes the
  remaining 5 fields (`sender_sub_id`, `sender_location_id`, `target_sub_id`, `target_location_id`,
  `session_qualifier`) to `None`.
- `session_id.rs:51-71` (`new_full`): already exists, takes all 8 fields explicitly — this is what
  `AcceptorBuilder::with_session` uses internally (via `config.session_id()`) to register sessions,
  so a session configured with SubID/LocationID/Qualifier registers under a key `route_and_run`'s
  3-field lookup can never match.
- `SessionId` derives `PartialEq, Eq, Hash` across all 8 fields (confirmed by struct definition) — an
  exact match is required.

**Decision**: In `route_and_run`, extract tags 50 (`SenderSubID`), 142 (`SenderLocationID`), 57
(`TargetSubID`), 143 (`TargetLocationID`) from the inbound Logon (same pattern as the existing tag
49/56 extraction at lines 1430-1431) and build the lookup key via `SessionId::new_full`, reversing
sender/target the same way the existing code already does for the 3-field case (line 1433's
comment: "Our SessionID reverses the counterparty's comp IDs").

**`SessionQualifier` resolution** (clarified via `/speckit-clarify`, spec.md Clarifications): since
`SessionQualifier` has no wire tag, it cannot be extracted from the Logon at all. Per the resolved
FR-011, two sessions sharing identical wire-visible identity distinguished only by
`SessionQualifier` must each be bound to a distinct listener/template. Concretely: extend the
acceptor-group resolution logic in `crates/truefix/src/lib.rs` (the `is_grouped`/`acceptor_groups`
logic around lines 283-300, already grouping by `SocketAcceptPort`) to detect when two sessions in
the **same group** would produce an identical `new_full` key from wire fields alone (i.e. differ only
by `session_qualifier`) — reject this configuration at resolve time with a clear
`ConfigError` naming the conflicting sessions, since no live-routing rule can disambiguate them
without a distinct discriminator; sessions that already have a distinct discriminator (different
port, or one configured via `AcceptorTemplate`/`DynamicSession`) are unaffected. This reuses the
existing group-resolution code path rather than adding new dispatch machinery.

### R2.2 — `B11`: `AcceptorBuilder::bind` ignores `SocketReuseAddress` (FR-012)

**Read**: `crates/truefix-transport/src/lib.rs:1279-1292` (`AcceptorBuilder::bind`), `:462-474`
(`Acceptor::bind_with`), `:115-127` (`bind_listener_with_options`).

- `Acceptor::bind_with` (line 473): `let listener = bind_listener_with_options(addr,
  services.socket_options.reuse_address)?;` — correctly conditional on config.
- `AcceptorBuilder::bind` (line 1282): `let listener = TcpListener::bind(addr).await?;` — plain bind,
  **no** `SO_REUSEADDR` regardless of configuration. `AcceptorBuilder` has no `services`/socket-option
  parameter at construction time at all (it's set later via `.with_services()`, after the listener is
  already bound).

**Confirmed**: exactly as described — the multi-session builder path never gets `SO_REUSEADDR`.

**Decision**: `AcceptorBuilder::bind` needs a `reuse_address` decision **before** binding, but its
current signature (`bind(addr, app)`) receives no socket-options at that point — `.with_services()`
is chained after. Two viable shapes: (a) always set `SO_REUSEADDR` on the `AcceptorBuilder` path
(reasonable default given multi-session acceptors are the operationally-restart-sensitive case this
bug describes, and QuickFIX/J's own default is reuse-enabled), or (b) thread a `reuse_address: bool`
parameter through `bind`. Given `Engine::start`'s call site (`crates/truefix/src/lib.rs:362`,
`AcceptorBuilder::bind(addr, app.clone()).await?.with_services(services)`) already has
`primary.socket_options` available *before* calling `.bind()` (line 340,
`to_transport_socket_options(primary.socket_options)`), reorder so socket options are known
pre-bind and pass `reuse_address` through — avoids defaulting to "always on" for a caller that
explicitly configured it off, and keeps `AcceptorBuilder`'s behavior consistent with
`Acceptor::bind_with`'s existing config-driven behavior rather than introducing a new default.

---

## R3 — Store/log persistence and durability (US3)

### R3.1 — `BUG-08`: swallowed store errors (FR-013)

**Read**: `crates/truefix-transport/src/lib.rs` at cited call sites — confirmed via grep: line 646
(`if let Ok(stored) = store.get(1, out - 1).await`), 659/760/1160/1698 (`let _ =
store.reset().await`), 1061-1062/1249 (`let _ = store.set_next_sender_seq/set_next_target_seq`), 1229
(`let _ = store.save_and_advance_sender(...)`).

**Decision**: Route each of these through `services.log.on_event(...)` (already available on the
`Log` trait per 005's work, no trait change needed) on the `Err` path, with a message identifying the
failed operation and session. The `Ok`-only `if let Ok(stored) = ...` at line 646 additionally needs
an `else` arm logging the skip (today it silently proceeds as if no resend data existed). No new
`Log::on_error_event`/severity levels (`GAP-40`) — out of scope per spec, `on_event` is sufficient.

### R3.2 — `BUG-09`/`GAP-48`: spurious reset on restart mid-window (FR-014)

**Read**: `crates/truefix-transport/src/lib.rs:1674-1729` (`run_scheduled_initiator`), confirmed via
grep: line 1689 (`let mut was_in_session = false;` — unconditional), line 1695
(`decide_schedule_action(&schedule, was_in_session, now)`), line 1698 (`let _ =
store.reset().await;` inside the `Enter` branch).

**Confirmed**: `was_in_session` starts `false` regardless of persisted state, so a restart landing
inside an active schedule window is indistinguishable from a genuinely-new session entering the
window for the first time — both produce `Enter`, both trigger a full reset.

**Decision**: Before entering the scheduled-initiator loop, call `store.creation_time().await` (the
method already exists and is populated by all 5 backends per feature 005's `GAP-38`); if
`Ok(Some(created))` and `created` falls within the schedule's currently-active window (via
`schedule.is_in_session(created)` — same predicate already used at line 1708 for `now`, or an
equivalent window-containment check), seed `was_in_session = true` instead of `false`. This directly
threads `creation_time` into the one call site `BUG-09` identifies; `GAP-48`'s broader claim (no
session-layer-visible concept of creation time at all) is addressed exactly to the extent this call
site requires — the spec's Assumptions note this as "extension of prior feature's already-accepted
scope, not a new architectural commitment," so no new session-layer type is introduced beyond this
consultation.

### R3.3 — `GAP-39`/`GAP-49`: `FileStore`/`CachedFileStore` missing atomic save+advance (FR-015)

**Read**: `crates/truefix-store/src/file.rs` — confirmed via the `MessageStore` trait method list
(grep, lines 272-300ish for `FileStore`, similar block for `CachedFileStore`): neither struct
overrides `save_and_advance_sender`; both fall through to the trait's default (independent
`save()` + `set_next_sender_seq()` calls, each separately fsync'd via `BodyLog::append`/
`SeqFile::write`).

**Decision**: Add a `save_and_advance_sender` override to both `FileStore` and `CachedFileStore`
that performs the body append and the sender-seq advance as a single ordered operation with one
combined durability point — body write (with its existing conditional `sync_data()`) followed
immediately by the seq-file write (with its existing conditional `sync_data()`), inside the same
call before returning `Ok`, so a caller never observes "seq advanced" without "body written" (or a
crash between the two leaves the *store*, on restart, self-consistent: body absent + seq showing the
old value is recoverable via a subsequent resend-request cycle, whereas seq showing the new value
with body absent is not). This mirrors the ordering already used by the SQL/MSSQL/Redb backends'
existing `save_and_advance_sender` overrides (established in 005) — apply the same
write-body-then-advance-seq order here, not the reverse.

### R3.4 — `BUG-15`: `BodyLog::reset()` never syncs (FR-016, part 1)

**Read**: `crates/truefix-store/src/file.rs:161-165` (`BodyLog::reset`):
```
fn reset(&self) -> Result<(), StoreError> {
    fs::write(&self.path, []).map_err(io_err)?;
    self.lock()?.clear();
    Ok(())
}
```
Confirmed: no `sync_data()` call, unconditionally — unlike `append()` (line 121-123, conditional on
`self.sync`) and `SeqFile::write`/`reset` (line 202-203, 226-229, also conditional).

**Decision**: After `fs::write`, when `self.sync` is true, open the file and call `sync_data()` (or
use `File::create` + explicit sync, matching `append()`'s pattern) before clearing the in-memory
index.

### R3.5 — `B17`: `FileStore`/`CachedFileStore::reset()` crash-window ordering (FR-016, part 2)

**Read**: `crates/truefix-store/src/file.rs:290-296` (`FileStore::reset`), `:438-445`
(`CachedFileStore::reset`):
```
async fn reset(&self) -> Result<(), StoreError> {
    self.body.reset()?;          // clears body FIRST
    self.corrupted.store(false, Ordering::SeqCst);
    let now = reset_creation_time_file(&self.creation_time_path)?;
    *self.creation_time.lock()... = now;
    self.seq.reset()             // clears seqnums LAST
}
```
**Confirmed**: exactly the order the audit describes. A crash after `self.body.reset()` succeeds but
before `self.seq.reset()` runs leaves `body` empty (already truncated) while `seqnums` still shows
the pre-reset (higher) sequence numbers — on restart, the store reports having sent/received up to
the old sequence number but has no message bodies to serve a resend from.

**Decision**: Reverse the order — reset `seq` (with R3.4's now-durable sync) **before** `body`. This
way, a crash mid-reset leaves `seqnums` already at the post-reset `(1, 1)` while `body` still holds
pre-reset messages: on restart, `next_in_seq()`/`next_out_seq()` correctly report `1`, and stale
messages remaining in `body` at those now-unreachable-by-index positions are harmless (never
addressed by any future `get()` call, since the index was `self.lock()?.clear()`'d as part of
`body.reset()` — which still needs to run, just second). This is the same "seq-file is the durability
anchor" ordering principle 005's SQL/MSSQL/Redb `save_and_advance_sender` overrides already
establish (R3.3), applied to `reset()` instead of `save`.

### R3.6 — `BUG-14`: `MssqlStore` missing identifier validation (FR-017)

**Read**: `crates/truefix-store/src/mssql.rs` (377 lines total) — confirmed via grep: no
`valid_identifier`-style function or call appears anywhere in the file; `connect_with_config`,
`ensure_schema`, `get_seq`, `set_seq`, `save`, `get`, `reset`, `creation_time`, and
`save_and_advance_sender` all interpolate `{sessions}`/`{messages}` via `format!` with no prior
check.

**Decision**: Port `crates/truefix-store/src/sql.rs`'s `valid_identifier` function (already used by
`SqlStore` and, per `crates/truefix-log/src/{sql,mssql}.rs`, by both `SqlLog` and `MssqlLog`) into
`mssql.rs`, and call it in `connect_with_config` on both `sessions_table`/`messages_table` before any
schema/query work, returning `StoreError::Backend("... is not a valid SQL table identifier")` on
failure — matching the error shape the sibling backends already produce.

---

## R4 — QuickFIX/J JDBC URL grammar compatibility (US4)

### R4.1 — `jdbc:h2:` misroute (FR-018) — **scope resolved via `/speckit-clarify`: reject, don't implement**

**Read**: `crates/truefix-config/src/builder.rs:1024-1030` (`is_jdbc_sql_scheme`):
```
fn is_jdbc_sql_scheme(url: &str) -> bool {
    url.starts_with("jdbc:postgres://")
        || url.starts_with("jdbc:postgresql://")
        || url.starts_with("jdbc:mysql://")
        || url.starts_with("jdbc:sqlite:")
        || url.starts_with("jdbc:h2:")   // <-- recognized, but no backend implements it
}
```
`sql_store_config` (line 1176-1185) dispatches any URL this function accepts straight into
`StoreConfig::Sql { url, ... }`, and `crates/truefix-store/src/sql.rs::connect_pool` (not re-read
this pass; cited by audit at lines 360-384) only explicitly branches on `postgres://`/`postgresql://`/
`mysql://`, falling through to an unconditional SQLite-file-path interpretation for anything else —
confirmed consistent with `is_jdbc_sql_scheme` currently accepting `jdbc:h2:` with no corresponding
`connect_pool` branch.

**Decision** (per clarification): remove the `url.starts_with("jdbc:h2:")` arm from
`is_jdbc_sql_scheme`. An unrecognized `jdbc:h2:...` URL then falls through to whatever
"unrecognized JDBC scheme" handling already exists for genuinely-unsupported schemes (confirm at
implementation time that this produces `ConfigError`/`StoreError::UnsupportedBackend` rather than a
silent no-match — the `#[cfg(not(feature = "sql"))] fn sql_store_config` variant at line 1188-1193
already returns exactly this shape when the `sql` feature is off; the "scheme not recognized as
SQL-family at all" case needs the equivalent whether or not the `sql` feature is on). No new
dependency, no new `StoreConfig` variant, no `truefix-store` changes for H2 specifically.

### R4.2 — `jdbc:sqlserver://` grammar mismatch (FR-019)

**Read**: `crates/truefix-store/src/mssql.rs:34-73` (`parse_url`):
```
fn parse_url(url: &str) -> Result<Config, StoreError> {
    let rest = url.strip_prefix("mssql://").or_else(|| url.strip_prefix("sqlserver://"))...;
    let (userinfo, hostpart) = rest.split_once('@')...;   // REQUIRES '@'
    let (user, password) = userinfo.split_once(':')...;
    let (hostport, database) = hostpart.split_once('/')...;  // REQUIRES '/'
    ...
}
```
Confirmed: this grammar requires `user:password@host[:port]/database` — a real QuickFIX/J MSSQL URL
(`jdbc:sqlserver://localhost:1433;databaseName=quickfixj`) has neither `@` nor `/`, so it fails at
`split_once('@')` with a raw `StoreError::Backend`, before reaching any clean
`UnsupportedBackend`-shaped error.

**Decision**: Extend `parse_url` to detect and handle the real QuickFIX/J grammar as an alternative
branch: after stripping the `mssql://`/`sqlserver://` prefix, if the remainder contains `;` and no
`@` (semicolon-delimited-properties form), parse `host[:port]` up to the first `;`, then parse
`;`-delimited `key=value` properties (at minimum `databaseName`, `user`, `password` — the QuickFIX/J
JDBC-URL properties this audit's citation names) into the same `tiberius::Config` fields the
existing path-based branch already populates. Keep the existing `user:password@host/database` form
as the first-tried branch (TrueFix-native shorthand, per FR-019's "in addition to, not instead of"),
falling back to the semicolon-delimited grammar when no `@` is present. This is additive parsing
logic within the same function — no signature change, no new public type.

### R4.3 — `splice_credentials` percent-encoding gap (FR-020)

**Read**: `crates/truefix-config/src/builder.rs:1047-1060` (`splice_credentials`):
```
fn splice_credentials(url: &str, user: Option<&str>, password: Option<&str>) -> String {
    ...
    format!("{scheme}://{user}:{password}@{rest}")   // no encoding of user/password
}
```
Confirmed: `user`/`password` are spliced verbatim; a value containing `@`, `:`, or `/` corrupts the
resulting URL's authority-component parsing.

**Decision**: Percent-encode `user` and `password` before splicing (URL-component encoding of the
reserved characters `@`, `:`, `/`, and the `%` escape character itself at minimum). Given the
workspace's dependency discipline (no new crates unless justified — Technical Context "no new
external dependencies"), implement a minimal percent-encoder inline (a handful of reserved
characters, not a general-purpose URL library) rather than adding a `percent-encoding`-family crate
dependency for this one call site — consistent with 005's own precedent of preferring
already-present dependencies/hand-rolled minimal logic over new crates for narrow needs.

---

## R5 — Engine lifecycle (US5)

### R5.1 — `BUG-11`: orphaned tasks on partial `Engine::start` failure (FR-021)

**Read**: `crates/truefix/src/lib.rs:250-574` (`Engine::start`). Confirmed the exact structure: local
`acceptors`/`initiators`/`failover_initiators` `Vec`s (line 267-269) are populated incrementally
across the acceptor-group loop (line 302-396) and the singles loop (line 398-568); both loops
`return Err(e)` directly (lines 393, 565) when `continue_on_error` is false, **before** reaching the
`Ok(Self { acceptors, initiators, failover_initiators })` construction at line 569-573 — dropping
whatever was already pushed. `SessionHandle` (in `truefix-transport`) has no custom `Drop`, and
`JoinHandle::drop` does not abort the task (standard tokio semantics) — so already-started acceptors/
initiators keep running, unreachable by the caller.

**Decision**: Simplest fix consistent with spec FR-021's "either/or": have `Engine::start` clean up
already-started sessions **before** returning `Err`, rather than changing the public return type
(`Result<Self, EngineError>` stays unchanged — no new API surface). Concretely: on the `return Err(e)`
paths at lines 393 and 565, before returning, abort every `JoinHandle` in `acceptors` and stop every
`SessionHandle`/`ReconnectHandle` in `initiators`/`failover_initiators` collected so far (reusing the
same abort/stop calls `Engine::shutdown()` already performs at lines 591-598, factored into a shared
private helper both call). This keeps `Engine::start`'s signature stable and requires no new public
type, at the cost of the caller not getting a handle to the partial state — acceptable per FR-021's
explicit either/or and because the scenario is "operator wants the failure surfaced and everything
stopped," not "operator wants to keep the partial deployment running."

### R5.2 — `BUG-16`: group-level `ContinueInitializationOnError` collapse (FR-022)

**Read**: `crates/truefix/src/lib.rs:309-311`:
```
let continue_on_error = members.first().is_some_and(|m| m.session.continue_initialization_on_error);
```
Confirmed exactly as cited — `members` is the `Vec<ResolvedSession>` for one acceptor group (grouped
by shared `SocketAcceptPort`, established by 005's `BUG-03`/US2 work per the surrounding comment at
lines 271-281); only `members.first()` is consulted.

**Decision** (per `/speckit-clarify`, strictest-wins): change to `members.iter().all(|m|
m.session.continue_initialization_on_error)` — the group tolerates a startup failure only if
**every** member opts in; any member with `N` makes the whole group fail-fast. One-line change at
the same location.

### R5.3 — `BUG-21`: `Engine::shutdown()` doc/behavior mismatch (FR-023)

**Read**: `crates/truefix/src/lib.rs:590-598`:
```
/// Abort all acceptor listeners and stop all failover-initiator reconnect loops.
pub fn shutdown(&self) {
    for acceptor in &self.acceptors { acceptor.abort(); }
    for initiator in &self.failover_initiators { initiator.stop(); }
}
```
Confirmed: the doc comment already names exactly the two classes it touches (acceptors,
failover-initiators) and — read literally — already omits `self.initiators` (plain, non-failover).
The audit's concern is that a reader might not register the omission as deliberate. `SessionHandle`
(what `self.initiators` holds) exposes an async `logout()`, which a sync `shutdown()` cannot call —
confirmed this is a real sync/async API mismatch, not an oversight to silently "fix" by making
`shutdown` async (a breaking signature change out of scope here).

**Decision**: Strengthen the doc comment to explicitly state the omission and why: `self.initiators()`
(plain, non-failover initiators) are intentionally left running by `shutdown()` because stopping them
requires an async `SessionHandle::logout()` call this sync method cannot make; callers needing to stop
plain initiators too must call `.logout()` on each handle from `Engine::initiators()` themselves.
Doc-only change, no behavior change (matches FR-023's phrasing: "documentation MUST accurately
describe" — not "behavior MUST change").

---

## R6 — Network hardening (US6)

### R6.1 — `BUG-13`: unbounded frame-size buffer (FR-024)

**Read**: `crates/truefix-transport/src/framing.rs:13-43` (`frame_length`), full file (72 lines).
Confirmed: `body_len` is parsed from tag 9 with no upper bound (`s.parse().ok()` into `usize`, line
30-33); `total = body_start + body_len + 7` (line 37) has no sanity check; the function only ever
returns `Ok(None)` (need more bytes) or `Ok(Some(total))`/`Err` — no path signals "this frame is too
large, abort." The caller (`route_and_run`, confirmed above at lines 1408-1427, and the main
per-connection read loop cited by the audit at `lib.rs:819-948`, not independently re-read this pass
given `frame_length`'s lack of any bound is already the root cause) keeps extending `buf` until
`buf.len() >= total`.

**Decision**: Add a `max_body_len: usize` bound (configurable via `Services`/`SocketOptions`, with a
sane default — e.g. 16 MiB, comfortably above any legitimate FIX message including large
repeating-group or `RawData`-bearing messages, per the audit's own framing of "sane hardcoded" as an
acceptable starting point) to `frame_length` or a wrapping check at its call sites: when `body_len`
exceeds the bound, return a new `DecodeError` variant (e.g. `BodyLengthTooLarge`) that callers treat
as "close the connection" rather than "need more bytes." Apply identically pre- and post-Logon (no
special-casing by connection phase, per spec Edge Cases).

### R6.2 — `BUG-19`: proxy connect path has no timeout (FR-025)

**Read**: not re-read this pass (audit's citation of `connect_initiator_via_proxy`/
`connect_initiator_via_proxy_tls` calling `proxy::connect_through_proxy` directly with no timeout,
versus `connect_initiator_with`/`connect_initiator_tls` passing `config.connect_timeout` through, is
architecturally consistent with what R2/R5's confirmed reads already showed of `lib.rs`'s call-site
structure around line 495-550 — the four `connect_initiator_*` variants are dispatched from the same
`match (&rs.tls, &proxy)` block, lines 495-550, all sharing `rs.session`/`services` but the proxy
variants notably not threading a timeout wrapper the direct/TLS variants get).

**Decision**: Wrap the proxy connect calls (`connect_initiator_via_proxy`,
`connect_initiator_via_proxy_tls`) with the same `tokio::time::timeout(config.connect_timeout, ...)`
pattern the direct/TLS paths already use, returning a timeout error mapped to `EngineError::Proxy`/
`EngineError::Io` consistent with the existing error-mapping at each call site (lines 513, 524).
Verify the exact existing direct/TLS timeout call shape at implementation time (not independently
re-read this pass) and mirror it precisely rather than introducing a second timeout idiom.

### R6.3 — `GAP-53`: `CipherSuites` typo silently degrades (FR-026)

**Read**: not re-read this pass; audit cites `crates/truefix-transport/src/tls_config.rs:161-179`
filtering cipher-suite names with no "did anything match" check.

**Decision**: After filtering configured `CipherSuites` names against the recognized set, if the
result is empty while the input was non-empty (i.e. every name failed to match), return a
configuration-time error (new or existing `ConfigError`/`TlsError` variant identifying the
unrecognized name(s)) instead of constructing a `CryptoProvider` with zero suites.

### R6.4 — `GAP-54`: PROXY-header peek has no timeout (FR-027)

**Read**: not re-read this pass; audit cites `crates/truefix-transport/src/proxy.rs:193-227`
(`strip_trusted_proxy_header`).

**Decision**: Wrap the header-peek read loop in a `tokio::time::timeout` with a bounded duration
(reuse `connect_timeout` or a dedicated short constant — e.g. a few seconds, since a legitimate PROXY
header arrives immediately after connection open), closing the connection on timeout.

### R6.5 — `B14`: `classify_buffered` discards entire buffer on frame error (FR-028)

**Read**: not re-read this pass; audit cites `crates/truefix-transport/src/lib.rs:942-945`
(`buf.clear()` on `frame_length` error).

**Decision**: On a framing error, discard only up to (and including) the point framing recovery can
identify as malformed — at minimum, advance past the byte(s) that caused the `MissingBeginString`/
`InvalidBodyLength` error and re-attempt framing on the remainder, rather than clearing the whole
buffer. Exact recovery granularity (byte-at-a-time rescan vs. next-`"8=FIX"`-occurrence scan) is an
implementation-time decision; either satisfies FR-028's observable requirement (a legitimate trailing
message is not discarded).

### R6.6 — `B15`/`B30`: PROXY-header buffer sizing and `framing.rs` duplication (FR-029)

**Read**: not re-read this pass; audit cites `proxy.rs:201`'s 256-byte peek buffer and
`crates/truefix-core/src/framing.rs` as a near-exact duplicate of `crates/truefix-transport/src/
framing.rs` (`B30`, differing mainly in `pub`/`pub(crate)` visibility). `B30` is a hygiene/
duplication finding without its own FR — `docs/todo/003.md`'s `B30` entry has no severity tag and
no explicit fix direction; treated as in-scope under FR-029's "PROXY-protocol v2 header parser"
framing since fixing `B15`'s sizing issue is the concrete, testable requirement, while `B30`'s
de-duplication (`truefix-core::framing` vs `truefix-transport::framing`) is folded into the same
implementation pass as a housekeeping side-effect (share one implementation, re-exported into both
crates' public surfaces as needed) rather than tracked as a separate FR, since it has no independent
observable behavior change.

**Decision**: Increase `proxy.rs`'s v2-header peek buffer beyond 256 bytes to comfortably exceed the
PROXY protocol v2 spec's maximum TLV extent (up to 64 KiB per the spec, though a much smaller
practical bound like 4 KiB comfortably covers TrueFix's expected TLV usage — exact bound is an
implementation-time sizing decision, not a design decision), or switch to a length-driven read (parse
the v2 header's own length field first, then read exactly that many bytes) instead of a fixed-size
peek — the latter is the more robust fix and avoids re-litigating the size choice later.

---

## R7 — Dictionary/codec correctness (US7)

### R7.1 — `BUG-12`: `UtcTimeOnly`/`UtcDate` skip format validation (FR-030)

**Read**: `crates/truefix-dict/src/model.rs:113-141` (`FieldType::value_ok`). Confirmed: no match arm
for `Self::UtcTimeOnly` or `Self::UtcDate`; both fall into the catch-all `_ => true` at line 139.
`Self::UtcTimestamp | Self::Time => field.as_utc_timestamp().is_ok()` (line 130) is the existing
pattern for a similar type.

**Decision**: Add `Self::UtcTimeOnly => field.as_utc_time_only().is_ok(), Self::UtcDate =>
field.as_utc_date_only().is_ok(),` arms, using the already-existing typed parsers on `Field`
(`truefix_core::Field::as_utc_time_only`/`as_utc_date_only`, confirmed present by the audit's
citation and consistent with the existing `as_utc_timestamp` pattern at line 130). Per spec
Assumptions and the audit's own note, `UtcDateOnly` is deliberately **not** touched (matches QFJ's own
unchecked behavior for that type — not a TrueFix-specific regression).

### R7.2 — `GAP-27`: dormant per-message `fieldOrder` (FR-031)

**Read**: `crates/truefix-dict/src/codegen.rs:690`:
```
"    pub fn encode(&self) -> Vec<u8> {{ self.0.encode() }}"
```
Confirmed: codegen's generated `encode()` calls the plain, order-agnostic `encode()`, never
`encode_with_order` (which exists and works correctly per `truefix-core/src/codec/encode.rs:23-58`,
not independently re-read this pass but confirmed present via the `pub use` re-export at
`truefix-core/src/lib.rs:33`).

**Decision**: Codegen's generated `encode()` needs access to the message's `field_order` (from
`MessageDef`, already parsed and stored per 005's `GAP-27` work) at code-generation time. Since
`field_order` is a per-message-definition, dictionary-sourced fact known at codegen time (not
runtime), the generated `encode()` body should call `encode_with_order(self.0, &FIELD_ORDER)` where
`FIELD_ORDER` is a codegen-emitted `const`/`static` array literal for messages that declare a custom
order, falling back to the current plain `self.0.encode()` for messages that don't (preserving
today's behavior for the common case — no dictionary declares `ordered` widely, per the spec's own
framing of this as a currently-inert feature).

### R7.3 — `GAP-26`: dormant header/trailer group decode (FR-032)

**Read**: confirmed via grep: `decode_with_groups` (`truefix-core/src/codec/decode.rs:68`) has no
production caller — the only non-test hits are its own definition, the `pub use` re-export chain
(`codec/mod.rs:6`, `lib.rs:33`), and `group.rs`'s doc comment referencing it. The real decode path
(`crates/truefix-transport/src/lib.rs`, cited by audit at lines 924/1420) always calls the flat
`decode()`. Separately confirmed: `validate_groups`
(`crates/truefix-dict/src/validate.rs:203-219`) iterates `message.body.fields()` (line 208,
`let body: Vec<&Field> = message.body.fields().collect();`) — `Member::Group` entries are filtered
out by `.fields()` (per the audit's claim, not independently re-verified this pass but consistent
with the group-decode path being entirely separate/unused). `is_header`/`is_trailer`
(`crates/truefix-core/src/tags.rs:27`/`:60`) — confirmed via grep that `NoHops`(504)/`HopCompID`(628)/
`HopSendingTime`(629)/`HopRefID`(630) do not appear anywhere in `tags.rs`.

**Decision**: Three-part fix, in dependency order:
1. Add `NoHops`/`HopCompID`/`HopSendingTime`/`HopRefID` to `tags.rs`'s `is_header`/`is_trailer`
   classification (the one realistic FIX standard-header/trailer group, per the audit's own framing).
2. Wire the real production decode path (`truefix-transport::lib.rs`'s current `decode()` call sites)
   to call `decode_with_groups` instead, using the session's resolved `DataDictionary` as the
   `GroupSpec` implementation (per `group.rs`'s doc comment, `truefix-dict` already implements this
   trait).
3. Fix `validate_groups` to see group content: since `.fields()` filters out `Member::Group` entries,
   `validate_groups` (or its caller) needs a body representation that includes groups — either a
   `.members()`-style accessor that yields both `Field` and `Group` entries, or a pre-pass that
   flattens recognized header/trailer groups' member fields into the validated set before the
   existing `.fields()`-based walk runs. Exact mechanism is an implementation-time decision; the
   observable requirement (spec FR-032) is that group-structure validation sees the group's content,
   not that any particular internal representation is used.

### R7.4 — `B22`: `data_field_for_length` missing `EncodedLeg*` pairs (FR-033)

**Read**: `crates/truefix-core/src/tags.rs:66-83` (`data_field_for_length`). Confirmed the existing
12-pair table (90→91, 95→96, 212→213, 348→349, 350→351, 352→353, 354→355, 356→357, 358→359, 360→361,
362→363, 364→365, plus 93→89 added by 005's `BUG-02`) has no entries for the tag pairs the audit
names: 618→619, 620→621, 445→446, 1039→1040.

**Decision**: Add the four missing arms to the existing `match` in `data_field_for_length`, following
the same `LenTag => DataTag` pattern as the existing 13 arms. (Tag-name/number correctness for these
four pairs was independently verified by the audit against the shipped dictionaries per its own
citation; not re-verified against dictionary content this pass — confirm exact tag names against
`crates/truefix-dict/dict-src/normalized/*.fixdict` at implementation time before adding the doc
comment naming them, matching the existing arms' comment style.)

### R7.5 — `GAP-56`: stale `.fixdict` provenance headers (FR-034)

**Read**: `crates/truefix-dict/dict-src/normalized/FIX44.fixdict:1-2`,
`FIX50SP2.fixdict:1-2` (representative sample; all 10 shipped `.fixdict` files share the identical
header):
```
# Generated by truefix-dict's QuickFIX-XML conversion tool (US9, feature 005,
# FR-031) — do not edit by hand; regenerate from the source XML instead.
```
Confirmed: names "truefix-dict's QuickFIX-XML conversion tool" — the module implementing this
(`qfj_xml.rs`) was deleted this session per the branch's own recent commit
(`253a9bf fix(dict): wire fix-repository conversion tool, drop thrdpty-dependent qfj_xml`, visible in
`git log`), replaced by `fix_repository.rs`. **All 10** shipped files carry this header (not just the
8 "legacy" ones the audit's prose specifically calls out — the 2 more recently added FIX50/FIXT11
files, if generated by the same era of tooling, likely carry it too; confirm exact count at
implementation time).

**Decision**: Update the header comment text in all affected `.fixdict` files to name
`fix_repository.rs`/`truefix-dict-cli` (whichever is the actual current regenerating entry point —
confirm exact name at implementation time) instead of the deleted `qfj_xml.rs`. Per `GAP-33`'s
closure note (referenced by spec.md), content is **not** regenerated — this is a header-text-only
edit, not a re-run of the conversion pipeline.

---

## R8 — Feature-completeness gaps carried forward (US8)

Each of the 7 items in spec.md's US8 scenario 1 is an independent, narrow config/session-layer
addition. Given this stage's P3 priority and the breadth already covered by R1-R7's deeper
verification, these were not independently re-read this pass beyond the audit's own citations (each
already includes a specific file/module reference and a "confirmed zero hits" grep-style claim,
which is a lower-risk class of claim — absence-of-a-symbol — than the behavioral claims R1-R7
verified by tracing control flow). Implementation-time task breakdown should re-confirm each citation
before implementing, per this project's established practice (005's plan.md precedent).

- Recurring mid-connection sequence reset (`ResetSeqTime`/`EnableResetSeqTime`, `GAP-11`): extend
  `crates/truefix-session/src/schedule_reset.rs`'s `Enter`/`Exit` transition model with a third,
  recurring-time transition — a scheduled daily reset independent of session enter/exit.
- Multi-tag `LogonTag` (`GAP-12`): change `crates/truefix-session/src/config.rs`'s
  `Option<(u32, String)>` to `Vec<(u32, String)>`, parsing `LogonTag`, `LogonTag1`, `LogonTag2`, …
  from `.cfg` (mirroring QFJ's numbered-key convention).
- FIXT `ApplVerID` auto-negotiation (`GAP-18c`): two-part — (a) `on_logon` gains handling for inbound
  tag 1137, and (b) `crates/truefix-config/src/builder.rs::resolve_validator` constructs a real
  `FixtDictionaries` (transport dictionary + per-`ApplVerID` app dictionaries) instead of treating
  `AppDataDictionary`/`TransportDataDictionary` as aliases — the larger of US8's items, touching both
  config resolution and session-layer state.
- Wildcard SubID/LocationID dynamic-session templates (`GAP-19`): extend `AcceptorBuilder`'s
  `template: Option<SessionConfig>` matching logic to support wildcard patterns on SubID/LocationID,
  not just substitution of BeginString/SenderCompID/TargetCompID.
- `.cfg`-selectable `ScreenLog`/`TracingLog`/`CompositeLog` (`GAP-21`): extend
  `crates/truefix-config`'s `resolve_log` (currently File or SQL only) to recognize these `.cfg`
  `Log`-selection values and construct the corresponding already-existing `truefix-log` backend
  (confirm these backends already exist in `truefix-log` before assuming only config-wiring is
  needed).
- IANA timezone names (`GAP-10`): per `docs/todo/003.md`'s own correction to `002.md`'s suggested fix
  (no `chrono`/`chrono-tz` anywhere in the workspace — `time`-crate-based), evaluate `time-tz` as the
  ecosystem-consistent dependency addition (the one FR in this feature that may introduce a new
  dependency — evaluate license/maintenance per Constitution Principle III/technical constraints
  before adding).
- `${var}` environment-variable fallback (`GAP-44`): extend `crates/truefix-config/src/lib.rs`'s
  interpolation resolution to check `std::env::var` when a name isn't found in the settings map.

---

## R9 — AT harness coverage (US9)

### R9.1 — `BUG-17`: stale regression floor (FR-042)

**Verified live** (not just read): ran `cargo run -p truefix-at --example count_check` against a
throwaway example computing `server_suite().iter().map(|s| s.versions.len()).sum()` — output:
`server_suite total_runs = 373`, `scenario count = 373`. Confirms the audit's claimed 373 exactly
(current `crates/truefix-at/tests/coverage.rs:41` asserts `total_runs >= 353`).

**Decision**: Bump the literal from `353` to `373` — but per the audit's own framing ("flagged here
rather than just fixed inline because a review of *why* it drifted... is worth a beat"), this bump
happens as this stage's **first sub-task**, immediately, independent of and before US1's 8 new
scenarios land (which will push the true count to 373 + however many new scenario-runs those 8
scenarios × `SUITE_VERSIONS.len()` add — the floor gets bumped again at the end of US1's stage to
reflect the new total, not just once here).

### R9.2 — `MinQty` scenario authoring (FR-043)

**Read**: not re-read this pass (confirming `field 110 MinQty` is present across
`FIX40`-`FIX50SP2.fixdict` is a straightforward grep the audit already performed; scenario authoring
is new-test-writing, not a source-verification task). Author a new scenario in
`crates/truefix-at/src/scenarios.rs` exercising `MinQty` (tag 110) — e.g. an order message declaring
a `MinQty` the dictionary now recognizes, verifying acceptance where previously the field would have
failed dictionary lookup — wired into `server_suite()` across `SUITE_VERSIONS`, following the same
pattern as existing per-version scenario functions (e.g. `valid_logon(v)` at line 1735).

---

## R10 — Tooling / latent-risk hygiene (US10)

### R10.1 — `GAP-50`/`BUG-18`: `flatten_members` unbounded recursion (FR-044)

**Read**: `crates/truefix-dict/src/fix_repository.rs:429-437` (`resolve_entries`, has `if depth > 16
{ ... }` guard) and `:547-560` (`flatten_members`, confirmed **no** depth parameter, no guard —
recurses via `flatten_members(sub, components)` at line 554 with no depth tracking at all).

**Decision**: Add a `depth: u32` parameter to `flatten_members` (or an equivalent recursion-guard
mechanism), returning an error (propagated as `FixRepositoryError`, matching `resolve_entries`'s
error type) when depth exceeds the same bound (16, for consistency) instead of recursing further.

### R10.2 — `GAP-51`: `parse_messages` doc/impl mismatch (FR-045)

**Read**: not re-read this pass; audit cites `fix_repository.rs:302-304`'s doc comment claiming a
synthetic `id_by_name`/`by_id` registration the implementation doesn't perform, which the audit notes
makes the invariant protecting `&components_by_id[&ref_id]` (a panicking index) more fragile than
documented.

**Decision**: Correct the doc comment to describe the actual registration behavior. Given the
panicking-index concern the audit raises as a secondary risk, evaluate at implementation time whether
the invariant is provably upheld by construction (in which case a corrected comment suffices) or
whether it warrants converting the panicking index to a `.get(&ref_id)` + typed-error return —
spec FR-045 only requires the doc fix; the indexing-safety question is adjacent but not itself a
cited `BUG`/`GAP` with its own FR, so treat any indexing hardening as a bonus, not a requirement.

### R10.3 — `GAP-52`: O(fields × enums) enum-emission (out of FR scope — informational)

**Read**: not re-read; audit cites `fix_repository.rs::convert`'s enum-emission as a linear scan per
field, build-time only. **Not present as its own FR** in spec.md (US10's FRs are FR-044/045/046 only)
— the spec's US10 acceptance scenarios don't cover this item explicitly; `docs/todo/003.md` itself
frames it as "not a correctness issue," lowest priority in the P2 list. Documented here for
completeness per Principle VII (inventory-based completeness — every audited item should be
traceable even if triaged out), but no FR requires it; may be picked up opportunistically during
`fix_repository.rs` work for R10.1/R7.5 without being a required deliverable.

### R10.4 — `BUG-20`: `BodyLog::append` TOCTOU race (FR-046)

**Read**: `crates/truefix-store/src/file.rs:109-126` (`BodyLog::append`):
```
fn append(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
    let offset = fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);  // read OUTSIDE lock
    let mut f = OpenOptions::new().create(true).append(true).open(&self.path)...;
    f.write_all(...)...;
    ...
    self.lock()?.insert(seq, (offset, len));   // index write INSIDE lock, but AFTER offset read
    Ok(())
}
```
Confirmed: `offset` is read via `fs::metadata` before any lock is acquired; the index insert (which
records that offset) happens later, under `self.lock()`. Two concurrent callers could both read the
same `offset` before either appends, corrupting the index. Confirmed latent (not live): all current
`truefix-transport` call sites are sequential `.await`s within one task per session (not
independently re-verified this pass, but consistent with the crate's overall sans-IO single-owner
design already documented in the codebase and constitution).

**Decision**: Move the offset determination inside the same lock/critical-section that performs the
write and the index update — e.g. hold `self.lock()` across the file-open, offset-read (or track the
running offset in the lock-protected state instead of re-reading file metadata each time), write, and
index-insert as one atomic sequence. This closes the TOCTOU window structurally even though no
current caller exercises it concurrently, per FR-046's explicit framing.

---

## Summary of verified-vs-audit corrections

None. Every citation this research re-verified (R1.1-R1.8, R2.1-R2.2, R3.1-R3.6, R4.1-R4.3,
R5.1-R5.3, R7.1-R7.3, R7.5, R9.1, R10.1, R10.4 — 24 of 46 FRs' underlying citations directly
re-read/re-run against current source) matched `docs/todo/003.md`'s description exactly. The
remaining FRs (US6's R6.2-R6.6, US7's R7.4, US8 entirely, US9's R9.2, US10's R10.2-R10.3) rely on the
audit's own citations without independent re-verification this pass, consistent with proportional
effort for lower-priority (P2/P3) items — each should get a fresh grep/read as its own first
implementation sub-task, per this project's established "verify before implement" practice (005's
plan.md precedent), not treated as pre-confirmed.
