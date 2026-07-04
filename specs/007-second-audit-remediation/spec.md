# Feature Specification: Second-Pass Audit Remediation

**Feature Branch**: `007-second-audit-remediation`

**Created**: 2026-07-04

**Status**: Draft

**Input**: User description: "按照 docs/todo/004.md 的内容确认有问题后 修复" (Per the contents of
`docs/todo/004.md`, after confirming the issues, fix them.)

## Clarifications

### Session 2026-07-04

- Q: `BUG-30`/`BUG-99` — should the sequence-number store keep its current single combined
  `seqnums` file (made atomic) or adopt QuickFIX/J-Go's separate sender/target files layout? → A:
  Adopt the separate two-file layout (`senderseqnums`/`targetseqnums`), each deleted and recreated
  wholesale on reset, matching QuickFIX/J/Go.
- Q: Should User Story 3 (the ~20-item P3 low-priority hardening tier) be included in this
  feature's scope, or deferred to a later pass? → A: Include all three user stories (US1+US2+US3)
  in this one pass.
- Q: An existing TrueFix deployment already running with the current single combined `seqnums`
  file — when it upgrades to the new two-file (`senderseqnums`/`targetseqnums`) format, should
  store-open auto-migrate from the legacy file, or require a manual/documented migration step? → A:
  Auto-migrate on open: if the new two files don't exist but a legacy `seqnums` file does, split its
  contents into the new format transparently before proceeding.

## User Scenarios & Testing *(mandatory)*

`docs/todo/004.md` is a second-pass audit (2026-07-03) covering defects **not** already tracked in
`docs/todo/003.md`'s `BUG-05`–`BUG-22`/`GAP-48`–`GAP-56`/`B1`–`B30` (which feature 006 closed). It
was produced by a pure code review (one pass per crate) plus a systematic QuickFIX/J and
QuickFIX/Go source comparison, then went through **two internal re-verification passes** of its
own before this feature started: a per-item "Verification Pass" (confirmed 57/63 of the original
`BUG-23`–`BUG-85` items outright, corrected 3, retracted 3) and a second four-agent cross-check
against the *current* working tree (confirming 3 items — `BUG-37`/`BUG-98`/`BUG-101` — were
already fixed as a side effect of feature 006, narrowing the scope of 3 more, and correcting the
description of 6 more). The document's own final "生产发布评估" (production-readiness assessment)
section is the authoritative triage this spec inherits.

**Numbering**: `BUG-23` onward, continuing from `003.md`'s `BUG-22`.

**Independent Test** (feature-level): For every `BUG-NN` this spec's user stories cover, a targeted
test exists demonstrating the specific previously-broken behavior now works correctly — reusing the
established table-driven-unit-test / two-process-integration-test / AT-scenario pattern from every
prior feature in this repository.

### User Story 1 - Pre-production-blocking correctness, durability, and resource-safety fixes (Priority: P1)

An operator running TrueFix in production hits a class of defects the audit rates
"必须修复才能上线" (must-fix-before-shipping): silent data loss on crash, protocol-breaking
handshake bugs, resource leaks that make the engine impossible to cleanly stop, a remotely-triggerable
malformed-input acceptance, and an acceptor that ignores its own configured trading-hours schedule
entirely.

**Why this priority**: Each item in this tier either loses persisted data, breaks interoperability
with a real counterparty, leaks running connections/tasks with no way to stop them, or lets a
config-declared safety boundary (trading hours) go completely unenforced. These are the audit's own
"must fix before a v1 release claim" set — the same bar features 001-006 have held every prior
audit-derived P0 tier to.

**Independent Test**: Each item has its own targeted test (unit, two-process integration, or AT
scenario) demonstrating the previously-broken behavior now works correctly, run against a live
acceptor/initiator pair or a live store/file backend.

**Acceptance Scenarios**:

1. **Given** a store write that crashes while the sequence-number files are being updated,
   **When** the process restarts, **Then** the prior sequence numbers are recovered intact, not
   silently reset to (1, 1), and a genuinely corrupted file is reported as an error rather than
   silently defaulted. Sender and target sequence numbers are persisted in two separate files
   (matching QuickFIX/J's `senderseqnums`/`targetseqnums` layout), each deleted and recreated
   wholesale on a store reset — resolved via Clarifications (Session 2026-07-04).
2. **Given** an existing deployment's store directory contains only the legacy single `seqnums`
   file (no `senderseqnums`/`targetseqnums` yet), **When** the store is opened for the first time
   after upgrading, **Then** the legacy file's sender/target values are transparently migrated into
   the new two-file layout — the upgrade itself never appears as a sequence reset.
3. **Given** an acceptor and initiator that both configure `ResetOnLogon`, **When** either side logs
   on, **Then** the acceptor echoes `ResetSeqNumFlag=Y` back only when the inbound Logon actually
   requested it, and the initiator verifies the echo (or infers a reset from `MsgSeqNum=1`) before
   treating the reset as complete.
4. **Given** a counterparty sends a `ResendRequest` for a sequence range higher than what has been
   sent yet while we are simultaneously waiting on our own outstanding `ResendRequest` to them,
   **When** both sides are in this state, **Then** the request is answered immediately rather than
   queued, avoiding a `ResendRequest`-vs-`ResendRequest` deadlock.
5. **Given** a `SqlStore` pointed at a database created before the `creation_time` column existed,
   **When** the store opens against that pre-existing schema, **Then** the migration adds the missing
   column before any row is written referencing it, rather than failing.
6. **Given** an `Engine` with running acceptors and initiators, **When** `Engine::shutdown()` is
   called (or the `Engine` value is dropped without an explicit `shutdown()` call), **Then** every
   spawned connection task — including plain (non-failover) initiators — is stopped; no task is left
   running with no way to reach it.
7. **Given** a scheduled initiator whose active connection drops unexpectedly, **When** the drop
   happens, **Then** the initiator reconnects on its next eligible attempt instead of treating the
   connection slot as permanently occupied.
8. **Given** a second inbound connection presents the same session identity as an already-connected,
   logged-on session, **When** the second connection's Logon arrives, **Then** it is refused rather
   than accepted as a second, competing session.
9. **Given** a TCP connection drops out from under an active session (not a graceful Logout),
   **When** the drop is detected, **Then** the session's `ResetOnDisconnect` configuration is honored
   exactly as it would be for a graceful Logout-driven disconnect.
10. **Given** an inbound message declares `BodyLength=0`, **When** it is framed, **Then** it is
    rejected as malformed rather than accepted as a zero-length message body.
11. **Given** an acceptor configured with a trading-hours schedule, **When** a Logon arrives outside
    that window, **Then** the Logon is rejected, and an already-logged-on session that crosses out of
    its window is torn down — matching the initiator-side schedule enforcement this project already
    has.
12. **Given** an inbound admin message (e.g. a malformed Logon) that violates the session's active
    data dictionary, **When** it is received, **Then** it is rejected the same way a dictionary
    violation on an application message would be — admin messages are no longer exempt from
    dictionary validation.
13. **Given** an inbound message, **When** the transport dispatches it to the application's
    `from_admin`/`from_app` callback, **Then** that dispatch happens only after the session layer's
    own sequence/identity/latency/PossDup/dictionary checks have already passed — an application
    never observes a message the session layer is about to reject.

---

### User Story 2 - Real defects with narrower blast radius (Priority: P2)

A second tier of confirmed defects that need an unusual configuration, a specific counterparty
behavior, or a specific input to trigger — real correctness or robustness problems, but ones a
deployment can operate with in the short term while they're being fixed.

**Why this priority**: Every item here is a confirmed, real defect (not a design difference or a
retracted claim) — but each has a narrower trigger condition than User Story 1's items, consistent
with the audit's own "建议修复但可带病上线" (should-fix-but-shippable) tier.

**Independent Test**: Each item has its own targeted test exercising the specific narrow trigger
condition and demonstrating the corrected behavior.

**Acceptance Scenarios**:

1. **Given** a session reconnects after a drop, **When** a new sequence gap appears on the new
   connection, **Then** a `ResendRequest` is issued for it — a stale `resend_requested`/queue state
   from the prior connection no longer suppresses it.
2. **Given** a Logon arrives while the session is in `AwaitingLogon` state, **When** any other
   message type arrives before that Logon completes, **Then** the message is rejected rather than
   processed as if the session were already logged on (no `ResendRequest` issued for a pre-logon
   sequence gap).
3. **Given** an inbound Logon carries `NextExpectedMsgSeqNum(789)` higher than the number of messages
   actually sent, **Given** an inbound Logon is missing `MsgSeqNum(34)` entirely, or **Given** a
   `HeartBtInt` value is negative, **When** any of these arrive, **Then** the Logon is rejected with a
   clear reason instead of silently desyncing the session or accepting a nonsensical value.
4. **Given** a length-prefixed data field (e.g. tag 95/96) is followed by a field that is not its
   matching data tag, or **Given** a length field's value is non-numeric, **When** the message is
   decoded, **Then** decoding fails cleanly rather than silently misparsing subsequent fields.
5. **Given** the plain (non-TLS, non-failover) reconnecting initiator path establishes a connection,
   **When** the socket is set up, **Then** the same socket options (`TcpNoDelay`, `KeepAlive`, buffer
   sizes) already applied on every other connection path are applied here too.
6. **Given** a `MssqlStore::save_and_advance_sender` transaction's final `COMMIT` fails, **When** the
   failure occurs, **Then** the transaction is rolled back rather than left open on a held connection.
7. **Given** an `Engine`-started acceptor group shares one listen port across multiple sessions,
   **When** the group's log/dictionary-validator/socket-options/TLS material is resolved, **Then**
   each session's own configuration is used consistently, not silently pulled from whichever member
   happened to be first (or first with TLS configured).
8. **Given** an outbound `Reject` (35=3) is generated for a `FIX.4.2`-or-later session, **When** it is
   sent, **Then** it includes `RefMsgType(372)`; **Given** the session's `BeginString` is `FIX.4.0`
   or `FIX.4.1`, **When** a `SessionRejectReason(373)` value outside that version's defined code
   range is generated, **Then** the field is omitted rather than sent with an out-of-range code.
9. **Given** a `PossDupFlag=Y` message arrives whose sequence number equals (not just is below) the
   expected next-incoming sequence number, **When** it is processed, **Then** the same
   `OrigSendingTime`-vs-`SendingTime` anti-replay check already applied to the below-expected case is
   applied here too.
10. **Given** a `ResetOnLogon`/`ResetOnLogout`/`ResetOnDisconnect`-configured initiator connects for
    the first time (both sequence numbers still at 1), **When** any one of those three reset
    configurations is set, **Then** the initiator sends `ResetSeqNumFlag=Y` on its Logon — not only
    when `ResetOnLogon` specifically is set.
11. **Given** a scheduled initiator's connection attempt is rejected or fails repeatedly, **When** it
    retries, **Then** the retry interval backs off across a small number of attempts rather than
    polling at a fixed sub-second interval indefinitely.
12. **Given** a `SequenceReset` (gap-fill mode) advances past sequence numbers that were sitting in
    the out-of-order queue, **When** it is processed, **Then** those now-superseded queued messages
    are discarded rather than retained in memory indefinitely.
13. **Given** a stray Logon arrives while the session is in `AwaitingLogout` or `Disconnected` state
    (not `AwaitingLogon`), **When** it arrives, **Then** it is rejected rather than silently ignored.
14. **Given** a TCP connection drops during the Logon handshake itself (before either side reaches
    `LoggedOn`), **When** the drop happens, **Then** the application's logout callback still fires,
    matching the behavior for a drop after a completed logon.
15. **Given** an `MssqlStore`/`MssqlLog` connection URL's credentials portion contains an `@`
    character, **When** the URL is parsed, **Then** the credentials and host are split correctly
    rather than mis-split at the first `@` found anywhere in the URL.
16. **Given** a `FileStore`/`CachedFileStore` opens against a long-running session's accumulated
    message history, **When** it opens, **Then** it does not read every stored message body into
    memory just to rebuild its offset index (or, for `CachedFileStore`, just to warm its cache) —
    peak memory during open is bounded independent of total history size.

---

### User Story 3 - Low-priority hygiene and defensive hardening (Priority: P3)

A long tail of low-probability edge cases, parsing leniency differences from the reference
implementations, and build-tooling robustness issues — individually low-impact, worth closing
opportunistically rather than urgently.

**Why this priority**: None of these block a production deployment on their own; the audit's own
triage places them in "可延后处理" (can-defer). Grouped here so they're tracked to closure rather
than left as an open-ended backlog with no acceptance criteria. Included in this feature's scope
alongside User Stories 1-2 (resolved via Clarifications, Session 2026-07-04) — this whole audit
document closes in one pass, matching feature 006's precedent for `docs/todo/003.md`.

**Independent Test**: Each item has its own targeted test exercising the specific edge case.

**Acceptance Scenarios**:

1. **Given** a message whose total framed length would exceed `usize` bounds under naive addition,
   **When** its frame length is computed, **Then** the arithmetic does not wrap silently (defense in
   depth on top of the existing `MAX_BODY_LEN` bound, which already makes the wrap unreachable via
   normal parsing).
2. **Given** an application constructs an outbound message whose encoded byte length exceeds what a
   `u32` checksum accumulator can sum without wrapping, **When** it is encoded, **Then** the checksum
   computation does not silently wrap to an incorrect value.
3. **Given** `HeartBtIntTimeoutMultiplier` is configured with a fractional value, **When** it is
   parsed, **Then** the fractional precision is preserved (not truncated to an integer), and the
   resulting timeout arithmetic cannot overflow for realistic heartbeat intervals.
4. **Given** a repeating group's entry is missing one of the group's own required fields, or **Given**
   a message was decoded via the group-aware decode path, **When** dictionary validation runs,
   **Then** required-field and field-type/enum checks are applied recursively inside each group entry,
   not only at the message's top level.
5. **Given** an inbound `CHAR` field on a `FIX.4.0`/`FIX.4.1` message carries more than one character,
   **When** it is validated, **Then** it is accepted (matching those versions' treatment of `CHAR` as
   plain text) rather than rejected as an invalid single character.
6. **Given** a `.cfg` sets `TimeStampPrecision=seconds` (lowercase) or `SocketUseSSL=true` (not the
   canonical `Y`/`N`), **When** it is parsed, **Then** the value is recognized case-insensitively
   (precision) or rejected with a clear error rather than silently coerced to a default (`SocketUseSSL`
   and other Y/N switches).
7. **Given** a resent (PossDup-replayed) message's `SendingTime` is refreshed, **When** it is
   refreshed, **Then** it uses the session's configured timestamp precision, not a hardcoded
   millisecond precision.
8. **Given** a peer's Logon declares a `HeartBtInt` value larger than fits in the field TrueFix stores
   it in, **When** it is adopted, **Then** it is clamped to a safe bound rather than silently
   truncated to an arbitrary smaller value.
9. **Given** an inbound message fails the low-sequence-number check without `PossDupFlag`
   justification, **When** it is rejected, **Then** `ResetOnError` is honored the same way it already
   is for the identity- and latency-failure rejection paths.
10. **Given** the session is in `AwaitingLogout` (not yet timed out), **When** its normal heartbeat
    interval elapses, **Then** a heartbeat is still sent, rather than heartbeats stopping the moment
    logout begins.
11. **Given** a `ResendRequest`'s `EndSeqNo` is `999999` on a `FIX.4.2`-or-earlier session, **When** it
    is processed, **Then** it is treated as "resend to the highest sequence number sent," matching
    that version-range's documented infinity convention (not only `EndSeqNo=0`).
12. **Given** the multi-endpoint reconnecting initiator successfully reconnects to a non-primary
    backup endpoint, **When** the connection later drops again, **Then** the next reconnect attempt
    prefers the primary endpoint again rather than continuing to rotate forward.
13. **Given** `ReconnectHandle::stop()` is called while its initiator is mid-connection (not between
    attempts), **When** it is called, **Then** the currently-active connection is torn down rather
    than only suppressing future reconnect attempts.
14. **Given** an acceptor's first inbound message on a new connection is not a Logon, **When** it
    arrives, **Then** the connection is refused rather than routed using that message's identity
    fields.
15. **Given** the transport's internal admin-message channel has no configured bound, **When**
    messages accumulate faster than they're processed, **Then** the channel applies backpressure
    (bounded capacity) rather than growing without limit.
16. **Given** a timestamp field's fractional-seconds portion has a digit count other than 0, 3, 6, or
    9, or **Given** a timestamp's seconds field is exactly 60 (a leap second), **When** it is parsed,
    **Then** the digit-count is rejected as malformed if non-standard, and a leap second is mapped to
    59 seconds plus the maximum sub-second fraction (matching the reference implementations' handling)
    rather than being rejected outright.
17. **Given** a frame's declared checksum position, **When** the frame boundary is computed, **Then**
    the bytes at that position are confirmed to actually look like a checksum field before the frame
    is accepted as complete, narrowing (not replacing) the existing `BodyLength`-driven framing.
18. **Given** a buffer's leading bytes don't form a recognizable `BeginString` field, **When** framing
    is attempted, **Then** those bytes are not treated as the start of a valid frame.
19. **Given** a decoded message has no `MsgType` field at all, **When** it is decoded, **Then** it is
    rejected rather than accepted as a message with an absent type.
20. **Given** `crates/truefix-dict`'s codegen path encounters a field type not in its explicit type
    table (e.g. `TIME`, `PRICEOFFSET`), or an enum-value label that collides with a Rust reserved
    keyword, or a component/message list using the `open` enum modifier, **When** code is generated,
    **Then** the generated code compiles and behaves consistently with the same input processed by
    the runtime (non-codegen) dictionary path.

### Edge Cases

- A store recovery path that finds a genuinely-corrupted (not just missing) sequence-number file
  MUST fail loudly (a typed, catchable error) rather than silently substituting a default — this is
  the deliberate behavior change most likely to surface latent operational issues in existing
  deployments that have been silently running on corrupted state; call this out explicitly during
  rollout rather than treating it as a routine fix.
- The legacy-to-two-file store migration (FR-001a) MUST be distinguished from the corrupted-file
  case immediately above: a *missing* new-format file pair with a *present, parseable* legacy file
  is a migration, not a corruption — it MUST auto-migrate silently (per Clarifications), while a
  legacy file that itself fails to parse MUST still surface as the typed error the corrupted-file
  case requires, not be treated as "nothing to migrate."
- Several User Story 1/2 items change behavior at a session-protocol level (the `ResetSeqNumFlag`
  handshake, admin-message dictionary validation, `validLogonState` enforcement). Each of these MUST
  be verified against **both** roles (acceptor and initiator) per this project's Acceptor/Initiator
  Parity principle, even where the audit's citation names only one role.
- Where this document's citations conflict with each other across its multiple internal passes (the
  original findings, the "Verification Pass," and the "二次交叉验证" cross-check), the **most recent,
  most specific** correction is authoritative — already reconciled into this spec's own acceptance
  scenarios above; implementers MUST NOT re-derive scope from the document's earlier, superseded
  framing of a since-corrected item.

## Requirements *(mandatory)*

### Functional Requirements

**Store/log durability (US1)**

- **FR-001**: Sender and target sequence numbers MUST be persisted in two separate files (matching
  QuickFIX/J's `senderseqnums`/`targetseqnums` layout), each deleted and recreated wholesale on a
  store reset; the persistence path MUST NOT have any window in which a crash can leave a file's
  on-disk sequence-number state fully empty — a write that is interrupted mid-flight MUST leave
  either the prior value or the new value recoverable, never neither.
- **FR-001a**: On store open, if the new two-file layout is absent but a legacy combined `seqnums`
  file is present, the store MUST auto-migrate by reading the legacy file and writing its sender/
  target values into the new two-file layout before proceeding, rather than treating the legacy
  deployment as a fresh store with no history.
- **FR-002**: A sequence-number file that fails to parse as valid recorded state MUST surface as a
  typed error at store-open time, not silently default to (1, 1).
- **FR-003**: `SqlStore::ensure_schema` MUST apply its `creation_time`-column migration before any
  statement that references that column runs, regardless of whether the underlying table already
  existed with an older schema.
- **FR-004**: `MssqlStore::save_and_advance_sender` MUST issue a rollback if its final commit fails,
  matching the rollback behavior already used for a failed statement earlier in the same transaction.

**Session protocol correctness (US1)**

- **FR-005**: An acceptor's Logon response MUST echo `ResetSeqNumFlag=Y` if and only if the inbound
  Logon that triggered it carried `ResetSeqNumFlag=Y`, rather than basing the echoed value on static
  session configuration.
- **FR-006**: An initiator that sent `ResetSeqNumFlag=Y` MUST verify the acceptor's Logon response
  either echoes the flag or is otherwise inferable as an acknowledged reset (e.g. the response's own
  `MsgSeqNum` is 1); when neither holds, the initiator MUST treat this as a handshake failure.
- **FR-007**: A `ResendRequest` whose requested range extends beyond the highest sequence number sent
  so far MUST be answered immediately (available messages resent or gap-filled up to what has been
  sent) rather than queued for later processing once a return-direction gap is filled.
- **FR-008**: A second inbound connection presenting a session identity that already has an active,
  connected session MUST be refused.
- **FR-009**: An unexpected TCP disconnection (not a graceful Logout exchange) MUST dispatch the same
  disconnect-handling logic (including `ResetOnDisconnect` honoring) that a Logout-driven disconnect
  already receives.
- **FR-010**: A message MUST NOT be delivered to the application's `from_admin`/`from_app` callback
  until the session layer's own sequence, identity, latency, PossDup, and dictionary checks for that
  message have already passed.
- **FR-011**: Dictionary validation MUST apply to admin-typed messages (not only application-typed
  messages) when a dictionary is configured for the session.
- **FR-012**: A scheduled or reconnecting initiator whose active connection task ends (drops) MUST
  observe that its connection slot is free and become eligible to reconnect on its next scheduled
  attempt.
- **FR-013**: `Engine::shutdown()` MUST stop every session task it started, including plain
  (non-failover) initiators, not only acceptors and failover initiators; dropping an `Engine` value
  without calling `shutdown()` MUST NOT leave spawned tasks running undetectably.

**Network hardening (US1)**

- **FR-014**: A message declaring `BodyLength=0` MUST be rejected as malformed rather than accepted.

**Schedule enforcement (US1)**

- **FR-015**: An acceptor session with a configured trading-hours schedule MUST reject an inbound
  Logon that arrives outside that schedule's active window.
- **FR-016**: An acceptor session already logged on MUST be disconnected if the current time crosses
  outside its configured schedule window while the connection is active, matching the initiator-side
  schedule-boundary enforcement this project already has.

**Narrower-impact session/protocol fixes (US2)**

- **FR-017**: A new connection MUST reset per-connection resend-request-suppression state and the
  out-of-order message queue, so a gap on the new connection is not suppressed by state left over
  from a prior connection.
- **FR-018**: A non-Logon message arriving while the session has not yet completed its Logon exchange
  MUST be rejected rather than processed as if the session were already established.
- **FR-019**: An inbound Logon MUST be rejected when its `NextExpectedMsgSeqNum` exceeds the number of
  messages actually sent, when it is missing `MsgSeqNum` entirely, or when its `HeartBtInt` is
  negative.
- **FR-020**: Decoding a length-prefixed data field MUST verify the field immediately following it is
  that length field's designated data tag, and MUST treat a non-numeric length value as a decode
  error, rather than silently misparsing subsequent bytes in either case.
- **FR-021**: Every initiator connection path (plain, TLS, failover, and plain-reconnecting) MUST
  apply the session's configured socket options identically.
- **FR-022**: An acceptor group sharing one listen port MUST resolve each connected session's own
  log/dictionary-validator/socket-options/TLS configuration consistently with that session's own
  `.cfg` entry, not implicitly inherited from a different group member.
- **FR-023**: An outbound session-level `Reject` on a `FIX.4.2`-or-later session MUST include
  `RefMsgType`; a `SessionRejectReason` value outside the code range a given `BeginString` defines
  MUST be omitted rather than sent out of range.
- **FR-024**: The PossDup anti-replay check (`OrigSendingTime` not later than `SendingTime`) MUST
  apply to every `PossDupFlag=Y` message, not only ones whose sequence number is below the expected
  value.
- **FR-025**: An initiator's first connection (both sequence numbers still at their initial value)
  MUST send `ResetSeqNumFlag=Y` when any of `ResetOnLogon`/`ResetOnLogout`/`ResetOnDisconnect` is
  configured, not only when `ResetOnLogon` specifically is set.
- **FR-026**: A scheduled initiator's reconnect attempts MUST back off across a small number of
  retries rather than retrying at a fixed sub-second interval indefinitely.
- **FR-027**: Messages superseded by a processed gap-fill `SequenceReset` MUST be discarded from the
  out-of-order queue, not retained indefinitely.
- **FR-028**: A Logon arriving while the session is in a state other than awaiting one (e.g.
  `AwaitingLogout`, `Disconnected`) MUST be rejected rather than silently ignored.
- **FR-029**: A TCP disconnection occurring during the Logon handshake itself (before login
  completes) MUST still trigger the application's logout callback.
- **FR-030**: Parsing an MSSQL store/log connection URL's embedded credentials MUST correctly handle
  a password containing an `@` character.
- **FR-031**: Opening a `FileStore` or `CachedFileStore` MUST NOT require reading every previously
  stored message body into memory; peak memory at open time MUST be bounded independent of the
  store's total message history size.

**Low-priority hardening (US3)**

- **FR-032**: Frame-length and checksum arithmetic MUST NOT silently wrap on inputs that would
  overflow their accumulator type.
- **FR-033**: `HeartBtIntTimeoutMultiplier` configuration MUST preserve fractional precision, and its
  use in timeout-duration arithmetic MUST NOT overflow for realistic configured heartbeat intervals.
- **FR-034**: Dictionary validation MUST recursively check required fields and field types/enums
  within repeating-group entries, not only at the message's top level.
- **FR-035**: `CHAR`-typed field validation MUST accept multi-character values on `FIX.4.0`/`FIX.4.1`
  sessions, consistent with those versions treating `CHAR` as plain text.
- **FR-036**: `.cfg` value parsing for `TimeStampPrecision` MUST be case-insensitive; `.cfg` Y/N-style
  switches (including `SocketUseSSL`) MUST reject a value that is neither a recognized affirmative nor
  negative form, rather than silently defaulting to the negative.
- **FR-037**: A resent message's refreshed `SendingTime` MUST use the session's configured timestamp
  precision.
- **FR-038**: An adopted peer `HeartBtInt` value that exceeds the range TrueFix can represent MUST be
  clamped, not silently truncated to an arbitrary value.
- **FR-039**: `ResetOnError` MUST be honored on the low-sequence-without-PossDup rejection path,
  consistent with the identity- and latency-failure rejection paths.
- **FR-040**: Heartbeats MUST continue to be sent while a session is in `AwaitingLogout` and its
  logout timeout has not yet elapsed.
- **FR-041**: A `ResendRequest`'s `EndSeqNo` of `999999` MUST be treated as "resend through the
  highest sequence sent" on `FIX.4.2`-and-earlier sessions, in addition to the existing
  `EndSeqNo=0` convention.
- **FR-042**: The multi-endpoint reconnecting initiator MUST prefer its primary endpoint again after
  any successful connection, not continue rotating forward from wherever a prior reconnect landed.
- **FR-043**: Stopping a reconnecting initiator MUST tear down a currently-active connection, not only
  suppress future reconnect attempts.
- **FR-044**: An acceptor MUST refuse a connection whose first inbound message is not a Logon, rather
  than routing based on that message's identity fields.
- **FR-045**: The transport's internal admin-message channel MUST apply a bounded capacity with
  backpressure rather than growing without limit.
- **FR-046**: Timestamp parsing MUST reject a fractional-seconds digit count other than 0, 3, 6, or 9,
  and MUST map a leap second (`ss=60`) to 59 seconds plus the maximum sub-second fraction rather than
  rejecting it outright.
- **FR-047**: Frame boundary detection MUST confirm the bytes at the position a declared `BodyLength`
  implies the checksum field starts actually resemble a checksum field before accepting the frame as
  complete.
- **FR-048**: Frame detection MUST require the buffer's leading bytes to form a recognizable
  `BeginString` field before treating them as the start of a frame.
- **FR-049**: A decoded message with no `MsgType` field MUST be rejected.
- **FR-050**: The dictionary codegen path MUST produce compiling, behaviorally-consistent output for
  field types outside its explicit type table, enum-value labels that collide with Rust reserved
  keywords, and dictionaries using the `open` enum modifier — matching how the runtime (non-codegen)
  dictionary path already handles each case.

### Key Entities

- **Sequence-number store record**: The durable on-disk representation of a session's next-outbound
  and next-expected-inbound sequence numbers, split across two separate files (one per direction,
  per Clarifications below); this feature's store-durability items change how and when each file is
  written and how a corrupted record is reported.
- **Session connection lifecycle event**: The transition points (connect, disconnect, schedule-window
  boundary) the session-layer state machine currently only partially observes; several US1/US2 items
  extend which of these transitions the state machine is actually notified of.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every acceptance scenario defined under User Stories 1-3 above passes when exercised
  against a live two-process (or, where noted, single-process/build-time) test, with no regression in
  any test passing before this feature began.
- **SC-002**: A process crash injected between any two adjacent durable-store writes covered by User
  Story 1 recovers to a consistent, non-corrupted state on restart in 100% of exercised crash-point
  cases (or fails loudly with a typed error, per this spec's Edge Cases note — never silently defaults).
- **SC-003**: A counterparty attempting a duplicate/competing connection, a schedule-violating Logon,
  or a malformed `BodyLength=0`/no-`MsgType` message is rejected in 100% of exercised cases.
- **SC-004**: `Engine::shutdown()` leaves zero running session tasks in 100% of exercised
  acceptor/initiator/failover-initiator combinations.
- **SC-005**: The full existing test suite (`cargo test --workspace --all-features`) and the AT
  conformance suite (`cargo test -p truefix-at --test conformance`) both remain 100% green throughout,
  with the AT suite's scenario-run count never decreasing.
- **SC-006**: No test, benchmark, or acceptance scenario passing before this feature begins regresses
  as a result of these changes.

## Assumptions

- **Excluded from this feature, with reasons** (per the source document's own final triage, honored
  rather than silently dropped, consistent with this project's Constitution Principle VII):
  - `BUG-37`, `BUG-98`, `BUG-101` — confirmed already fixed as a side effect of feature 006 (verified
    directly against current source before this spec was written); no action needed.
  - The "Exchange field case-sensitivity" item from the document's "Partially covered" table — fully
    retracted by the document's own second verification pass (neither QuickFIX/J nor QuickFIX/Go
    enforce case on that field; the original claim mischaracterized both references).
  - `BUG-108` (heartbeat-timeout-multiplier default value differs from QuickFIX/J's `1.4` and
    QuickFIX/Go's `1.2`) — the document's own conclusion is this is an intentional difference, not a
    defect; TrueFix's default is more lenient, not incorrect. (The overflow and lost-precision defect
    in the *same area*, `BUG-36`, is still in scope — see US3.)
  - `BUG-104` (no session enabled/disabled concept), `BUG-110` (`ClosedResendInterval` documented
    no-op), `BUG-111` (a new `Session` object per connection rather than one persisting across
    reconnects) — the document itself classifies each as an intentional design choice or a documented,
    accepted no-op, not a correctness defect; `BUG-111` is explicitly noted as the architectural root
    cause of several in-scope items (`BUG-32`/`BUG-42`/`BUG-93`) rather than an independently
    actionable item itself.
  - `BUG-106` — a literal duplicate of `BUG-83` (already covered under US2's `OrigSendingTime`
    scenario); not separately tracked.
  - `BUG-65` (`parse_utc_offset` overflow requiring a `.cfg` offset field value of `hours > ~596523`,
    far beyond any valid UTC offset) and `BUG-73` (codegen enum-label sanitization for a hypothetical
    custom dictionary using a Rust-keyword-named enum value, never observed in any bundled dictionary)
    — both confirmed-correct code-level observations the document itself assesses as having negligible
    practical risk; deferred rather than included, consistent with this project's practice of not
    hardening against inputs with no realistic trigger.
- **`BUG-92`/`BUG-97`/`BUG-60`/`BUG-90`/`BUG-56`/`BUG-24` are included, using the document's own
  corrected (not original) framing** — each had a real, narrower defect underneath an originally
  overstated or misdirected description; this spec's User Story 1/2/3 acceptance scenarios already
  reflect the corrected scope (e.g. `BUG-56` is scoped to `send_app()` only, `BUG-24` to the
  `encode.rs` checksum path only), not the document's first-draft framing.
- Every User Story 1/2 item touching session-protocol behavior applies to **both** acceptor and
  initiator roles, per this project's Acceptor/Initiator Parity principle, even for citations naming
  only one role in the source document.
- No new external dependencies are anticipated; every item is a bug fix within the existing crate
  structure. This assumption should be re-confirmed during `/speckit-plan`.
