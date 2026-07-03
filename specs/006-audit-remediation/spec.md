# Feature Specification: Audit Remediation (docs/todo/003.md)

**Feature Branch**: `006-audit-remediation`

**Created**: 2026-07-03

**Status**: Draft

**Input**: User description: "根据 docs/todo/003.md 实现要做的东西" (implement what's needed per docs/todo/003.md)

**Source document**: `docs/todo/003.md` — a 2026-07-02/03 pre-production audit of TrueFix against
QuickFIX/J and QuickFIX/Go, produced by five independent deep-dive reviews (session/state-machine,
transport+config, codec/dictionary, store/log, Engine facade+AT harness) plus a supplementary
Chinese-language pass (items `B1`–`B30`). Every finding cited below was independently re-verified
against current source as part of that audit, not copied from an earlier doc. Items in the audit's
"Known-accepted, not a gap" section are intentionally excluded from this spec's scope — they were
evaluated and ruled out, not overlooked.

## Clarifications

### Session 2026-07-03

- Q: FR-018 covers the jdbc:h2: scheme, which today silently misroutes to a bogus SQLite file. Should this spec commit to building real H2 backend support, or to rejecting the scheme cleanly? → A: Reject cleanly, no H2 backend — remove `jdbc:h2:` from the recognized-scheme table so it fails fast with a clear `UnsupportedBackend` error; no new database integration or dependency.
- Q: This spec bundles 10 user stories (US1-US10, 46 FRs) spanning P1 protocol-correctness through P3 tooling hygiene — one pass or split into a narrower P1-only spec plus a follow-up? → A: One pass, all 10 user stories — consistent with the project's established pattern (002/003/005 each closed a full audit-tracked backlog in one pass) and the constitution's Inventory-Based Completeness principle; planning stages US1-US10 with checkpoints.
- Q: FR-011 requires deterministic routing to a SessionQualifier-distinguished session, since SessionQualifier has no wire tag. How should routing disambiguate sessions that otherwise share identical wire-visible identity? → A: (No response within timeout; proceeding with the recommended option, revisable at planning time.) Per-listener qualifier binding — each SessionQualifier-distinguished session MUST be bound to its own distinct listen port/socket (or other non-wire discriminator, e.g. a dedicated per-qualifier connection template), so routing never needs to disambiguate purely from wire fields. Matches QuickFIX/J's own documented constraint that SessionQualifier requires a distinguishing transport-level setup.
- Q: Revisiting the above — confirm per-listener/template binding for SessionQualifier routing, or pick a different rule? → A: Confirmed — keep per-listener/dedicated-template binding as FR-011's disambiguation rule (no spec change).
- Q: FR-022 — when two sessions grouped under one listen port have conflicting `ContinueInitializationOnError` values, how should the group's effective tolerance be resolved? → A: Strictest wins — if any member in the group sets `ContinueInitializationOnError=N`, the whole group is treated as `N` (fail-fast), erring toward surfacing failures rather than silently tolerating them (Principle I, Production-Ready First).

## User Scenarios & Testing *(mandatory)*

<!--
  User stories below are grouped by the counterparty-facing or operator-facing capability they
  protect, and ordered by the audit's own severity/triage findings: protocol-correctness breaks
  reachable by any counterparty's input (P1) rank above operational/data-loss risks (P1/P2), which
  rank above hardening and completeness work (P2/P3). Each story is independently testable and
  independently shippable — implementing only US1 already closes the highest-severity holes.
-->

### User Story 1 - Counterparty cannot desync or replay a session via malformed protocol messages (Priority: P1)

A counterparty (malicious, misconfigured, or simply reconnecting with a stale cache) sends a Logon,
SequenceReset, or ResendRequest that is malformed or out of the expected sequence window. Today
several of these inputs are silently accepted instead of rejected, letting an external party corrupt
a session's sequence-number bookkeeping, replay previously-processed traffic, or wedge a session into
a permanent reconnect-fail loop — all without any log line indicating something went wrong.

**Why this priority**: These are observable, externally-triggerable protocol divergences from both
reference engines with no unusual configuration required — the audit's own triage order puts these
first because they are reachable by any counterparty's input and have the smallest fix surface.

**Independent Test**: Drive each scenario (too-low-seqnum Logon without PossDup, decreasing plain
`SequenceReset`, `ResendRequest` missing `BeginSeqNo`/`EndSeqNo`, an initiator configured with
`ResetOnLogon` reconnecting against an acceptor that fully resets) against a live two-process session
and confirm the session rejects/logs-out per FIX semantics instead of silently accepting or wedging.

**Acceptance Scenarios**:

1. **Given** an acceptor in `AwaitingLogon` with `next_in_seq = 5`, **When** it receives a Logon with
   `MsgSeqNum = 2` and no `PossDupFlag`, **Then** the session rejects the Logon (Logout, no state
   transition to `LoggedOn`, `next_in_seq` left unchanged) instead of accepting it and desyncing.
2. **Given** a logged-on session with `next_in_seq = 10`, **When** it receives a plain-mode
   `SequenceReset` (`GapFillFlag` absent/`N`) with `NewSeqNo = 3`, **Then** the session rejects the
   message (`SessionRejectReason::ValueIsIncorrect`, ref tag 36) and `next_in_seq` remains `10`.
2a. **Given** the same session, **When** it receives a plain-mode `SequenceReset` with tag 36 absent
    entirely, **Then** the session treats it as a required-field violation instead of silently
    skipping sequence adjustment while still draining the queue.
3. **Given** a logged-on session, **When** it receives a `ResendRequest` missing `BeginSeqNo` or
   `EndSeqNo`, **Then** the session responds with a required-tag-missing rejection instead of
   silently producing no response.
4. **Given** an initiator configured with `ResetOnLogon=Y` that has already sent its Logon
   (`logon_sent = true`), **When** it reconnects, **Then** both inbound and outbound sequence state
   reset fully and consistently with the acceptor's own reset expectation — no repeating
   gap→resend→reject-duplicate-Logon loop on every connection attempt.
5. **Given** a session with messages queued out of order pending a gap-fill, **When** the gap is
   filled and queued messages are drained, **Then** every drained message passes the same
   application-level validation (`validate_app`) as a message received in order — an invalid message
   enqueued behind a legitimate gap-filler MUST NOT bypass validation.
6. **Given** a session that reconnects after a prior disconnect, **When** the new connection is
   established, **Then** heartbeat/test-request timers (`ticks_since_recv`, `test_request_outstanding`,
   resend-tracking state) reset for the new connection instead of carrying over stale values from the
   previous connection.
7. **Given** an inbound message that fails dictionary validation (as opposed to identity/latency
   checks), **When** the session disconnects it, **Then** it sends a Logout first, consistent with
   the other disconnect-on-error paths, rather than dropping the TCP connection directly.
8. **Given** an inbound message whose `SendingTime` fails to parse, **When** the latency check runs,
   **Then** the message is treated as failing the latency check rather than silently passing it.

---

### User Story 2 - Multi-session acceptor deployments route connections to the correct session (Priority: P1)

An operator runs multiple `[SESSION]` blocks sharing one listen port, distinguished by SubID,
LocationID, or SessionQualifier (a configuration shape the project's own prior work advertised as
supported). Today the live connection-routing path ignores those fields entirely, so every inbound
connection to such a deployment fails to match any registered session regardless of which
counterparty connects — and separately, a listener built through one specific construction path
doesn't enable address reuse, so a routine restart can fail to rebind.

**Why this priority**: This blocks a configuration shape the project has already told users is
supported; shipping release notes claiming that capability without this fix would be actively
misleading.

**Independent Test**: Configure two acceptor `[SESSION]` blocks on one `SocketAcceptPort`,
distinguished only by `SessionQualifier` (or SubID/LocationID), drive a live Logon from each
identity, and confirm each is routed to its own registered session rather than both failing to match.
Separately, restart an acceptor process bound to a port immediately after a prior instance released
it and confirm the bind succeeds instead of failing on address-in-use.

**Acceptance Scenarios**:

1. **Given** two acceptor sessions sharing one port and BeginString/SenderCompID/TargetCompID but
   distinct SubID/LocationID/SessionQualifier, **When** a counterparty logs on with the identity of
   the second session, **Then** the connection is routed to the second session, not rejected as
   unroutable.
2. **Given** two or more `SessionQualifier`-distinguished sessions that would otherwise share
   identical wire-visible identity, **When** they are configured, **Then** each is bound to its own
   distinct non-wire discriminator (e.g. a distinct listen port/socket or a dedicated per-qualifier
   connection template) so an inbound Logon is always routable without needing to guess between them
   from wire fields alone.
3. **Given** an acceptor process that just released a listen port, **When** a new process binds the
   same port immediately afterward via the acceptor-builder path, **Then** the bind succeeds
   (`SO_REUSEADDR`-equivalent behavior) instead of failing.

---

### User Story 3 - Store/log persistence failures and restarts never silently lose or corrupt session state (Priority: P1)

An operator's persistence backend (SQL, MSSQL, file, or otherwise) experiences a transient failure,
or the process simply restarts mid-schedule-window. Today, store write failures are dropped without
any operator-visible signal, a routine restart inside an active schedule window wipes all sequence
numbers and resend history even with no legitimate reset reason, two backends never received an
atomicity fix already applied to their siblings, and a couple of narrower crash-window and
identifier-validation gaps exist in specific backends.

**Why this priority**: These are silent data-loss and silent-failure classes; the spurious-reset case
in particular is production-incident-shaped, not theoretical, and directly undermines the
resend-durability guarantees prior work was meant to provide.

**Independent Test**: (a) Inject a store failure during a send and confirm an operator-visible log
event is emitted. (b) Restart a process mid-schedule-window against a persisted store and confirm
sequence numbers/history are preserved, not wiped. (c) Crash a file-backed store between its
body-append and seqnum-file writes and confirm recovery does not silently duplicate/overwrite a
message slot. (d) Configure an MSSQL store with a malformed table-identifier setting and confirm a
clean configuration-time error instead of a raw driver error.

**Acceptance Scenarios**:

1. **Given** a session backed by a store that returns an error on a persistence call (save-and-advance,
   set-next-seq, reset, or resend re-hydration read), **When** that call fails, **Then** the failure
   is routed through the session's log as an operator-visible event instead of being silently dropped.
2. **Given** a store that persists its creation time, **When** a process restarts and its persisted
   creation time falls within the currently-active schedule window, **Then** the session does not
   perform a full reset (sequence numbers and message history are preserved) — only restarts whose
   persisted creation time falls outside the current window are eligible for a schedule-driven reset.
3. **Given** a file-backed store (`FileStore` or `CachedFileStore`), **When** a send is durably
   persisted, **Then** the sequence-number advance and the message body write are as atomic as the
   SQL/MSSQL/Redb backends already guarantee — a crash between the two MUST NOT produce a state where
   the advertised sequence number has no corresponding stored message.
4. **Given** `FileStoreSync=Y` (or the backend's durability-sync equivalent), **When** a store is
   reset, **Then** the reset is synced to durable storage before being considered complete, matching
   the sync discipline already applied to appends and sequence-file writes — and reset clears
   sequence-number state and message-history state in an order/manner that cannot leave one
   reflecting a reset while the other still reflects pre-reset data after a crash mid-reset.
5. **Given** an `MssqlStore` configured with a table-identifier setting, **When** the identifier is
   invalid, **Then** connection/config resolution fails with a clear, typed configuration error
   (consistent with the other SQL-family backends) rather than surfacing a raw driver-level SQL error.

---

### User Story 4 - QuickFIX/J-authored `.cfg` files using real JDBC URL grammar work as drop-in configuration (Priority: P1)

An operator migrating a QuickFIX/J deployment brings over a real QuickFIX/J-style `JdbcURL` (e.g. an
H2 test-fixture URL, or a semicolon-delimited MSSQL URL). Today one such URL scheme silently succeeds
while writing session data to the wrong backend entirely with no error, and another fails against
grammar TrueFix invented rather than the grammar QuickFIX/J actually uses — both directly undercut a
compatibility guarantee this project has already stated it provides.

**Why this priority**: Directly undercuts an explicit "drop-in QuickFIX/J `.cfg` compatibility" claim
made by prior work; a silent misroute to the wrong storage backend is a data-integrity risk, not just
an inconvenience.

**Independent Test**: Load a `.cfg` with `JdbcURL=jdbc:h2:mem:quickfixj` and confirm it fails loudly at
config-resolution time with a clear unsupported-backend error — never silently opens an unrelated
SQLite file. Load a `.cfg` with a real semicolon-delimited `jdbc:sqlserver://host:1433;databaseName=X`
URL and confirm it parses and connects (or fails with a clear, typed error), not a raw parse failure.

**Acceptance Scenarios**:

1. **Given** a `.cfg` with `JdbcURL=jdbc:h2:mem:quickfixj`, **When** configuration is resolved,
   **Then** the system returns a clear, typed unsupported-backend error — it MUST NOT silently
   create/open a SQLite file using the raw scheme string as a filename, and MUST NOT be advertised as
   a recognized SQL-family scheme when it isn't backed by a real implementation.
2. **Given** a `.cfg` with `JdbcURL=jdbc:sqlserver://localhost:1433;databaseName=quickfixj` (real
   QuickFIX/J MSSQL grammar: semicolon-delimited properties, no `user:password@host/database` path
   form), **When** configuration is resolved, **Then** the URL parses successfully using the real
   grammar (in addition to, not instead of, TrueFix's existing path-based shorthand).
3. **Given** `JdbcUser`/`JdbcPassword` values containing characters with special meaning in a URL
   authority (`@`, `:`, `/`), **When** they are spliced into a JDBC-style connection URL, **Then**
   they are percent-encoded so the resulting URL is not corrupted.

---

### User Story 5 - Multi-session Engine startup failures never leave orphaned, uncontrollable sessions (Priority: P2)

An operator starts an `Engine` configured with multiple sessions. One session fails to start (bind
failure, store failure, etc.) after several others already came up successfully. Today the whole
`Engine::start` call returns an error and the caller never receives a handle to the sessions that did
start — they keep running, connected or listening, with no way to shut them down short of killing the
process. Two related lifecycle gaps compound this: a per-group tolerance flag collapses to only the
first session's preference, and documented shutdown behavior doesn't disclose that it skips one class
of running session.

**Why this priority**: Operational hygiene that matters most for multi-session deployments — not a
counterparty-facing protocol break, but a real resource-leak and operability gap.

**Independent Test**: Configure a 5-session `.cfg` where sessions 1-3 start successfully and session 4
fails to bind with `ContinueInitializationOnError=N` (default); confirm the caller can still obtain
and cleanly shut down the sessions that already started. Configure two grouped sessions with
conflicting `ContinueInitializationOnError` values and confirm the group is treated as `N` (fail-fast)
whenever any member sets `N`, not silently decided by iteration order.

**Acceptance Scenarios**:

1. **Given** a multi-session `Engine::start` call where an earlier session/group starts successfully
   and a later one fails with `continue_on_error == false`, **When** `start` returns its error,
   **Then** the caller retains the ability to cleanly stop every session that already came up (either
   via a returned partial `Engine` or by `start` itself aborting/joining them before returning).
2. **Given** two `[SESSION]` blocks sharing one `SocketAcceptPort` with different
   `ContinueInitializationOnError` values, **When** the group's tolerance is resolved, **Then** the
   group is treated as `ContinueInitializationOnError=N` (strictest member wins) whenever any member
   sets `N`, rather than silently taking the first member's value.
3. **Given** `Engine::shutdown()`'s documentation, **When** it is invoked against an `Engine` with both
   failover and plain (non-failover) initiators running, **Then** the documented behavior accurately
   describes which session classes are stopped and which are not.

---

### User Story 6 - Network-facing inputs cannot exhaust memory or hang a connection indefinitely (Priority: P2)

A connection to an acceptor port (pre- or post-Logon) sends a crafted or stalled input: an
oversized/false `BodyLength`, a byte stream that never completes a proxy handshake, or a PROXY-protocol
header that never finishes. Today several of these paths have no bound and no timeout, letting one
connection pin unbounded memory or hang a connection task indefinitely — a straightforward DoS surface
on any acceptor port not otherwise access-restricted.

**Why this priority**: Real availability/DoS exposure reachable pre-authentication on any open
acceptor port; narrower blast radius than US1-US4 but still a concrete production risk.

**Independent Test**: Send a frame declaring an extreme `BodyLength` followed by a slow/never-completing
byte trickle and confirm the connection is bounded/closed rather than accumulating unbounded memory.
Connect through a proxy path that accepts the TCP connection but never completes its handshake and
confirm the initiator's connect attempt times out. Send a byte stream that never completes a
PROXY-protocol header from a trusted address and confirm the connection task doesn't hang forever.

**Acceptance Scenarios**:

1. **Given** an inbound connection declaring a `BodyLength` far exceeding any reasonable message size,
   **When** the frame is being assembled, **Then** the connection is closed once the declared/actual
   size exceeds a configured or sane maximum, instead of growing an unbounded buffer.
2. **Given** an initiator connecting through a proxy, **When** the proxy accepts the TCP connection but
   never completes the SOCKS/CONNECT handshake, **Then** the connect attempt is bounded by the same
   connect-timeout behavior already applied to direct and TLS connects, instead of hanging
   indefinitely.
3. **Given** a misconfigured `CipherSuites` value that matches zero real cipher suites, **When**
   configuration is resolved, **Then** the system reports a clear configuration-time error instead of
   producing a TLS context that fails opaquely at handshake time.
4. **Given** a trusted-source connection using the PROXY protocol, **When** it opens but never sends a
   complete header, **Then** the read is bounded by a timeout instead of hanging the connection task
   indefinitely.
5. **Given** an inbound byte stream where a legitimate FIX message follows a small run of non-FIX/
   malformed bytes, **When** framing recovers from the malformed prefix, **Then** it discards only the
   malformed prefix, not the entire buffer including the legitimate trailing message.
6. **Given** a PROXY-protocol v2 header whose TLVs approach or exceed the peek buffer's capacity,
   **When** the header is parsed, **Then** it is not truncated in a way that causes remaining
   PROXY-protocol bytes to be misinterpreted as FIX message data.

---

### User Story 7 - Dictionary/codec validation catches malformed values and encodes exactly what the dictionary specifies (Priority: P2)

A message field with a time/date type carries a garbled value, or a dictionary declares custom field
ordering or header/trailer repeating groups. Today certain time/date types skip format validation
entirely, and two previously-implemented dictionary features (custom field ordering, header/trailer
group decoding) are wired correctly in isolation but never actually invoked from any production code
path — so declaring them in a dictionary has zero real effect.

**Why this priority**: Real correctness/validation gaps, but narrower blast radius than session-level
protocol breaks — malformed-value tolerance and dormant features rather than externally-triggerable
session corruption.

**Independent Test**: Send a message with an intentionally malformed `UtcTimeOnly`/`UtcDate` field
value and confirm dictionary validation rejects it. Load a dictionary declaring custom `fieldOrder`
for a message and confirm the encoded wire output actually reflects that order. Load a dictionary
declaring a header/trailer repeating group (e.g. `NoHops`) and confirm it decodes and validates as a
group rather than being invisible to the validator.

**Acceptance Scenarios**:

1. **Given** a field typed `UTCTIMEONLY` or `UTCDATE` with a value that doesn't match the type's
   format, **When** dictionary type-checking runs, **Then** the field fails validation
   (`IncorrectDataFormat`) instead of silently passing.
2. **Given** a message definition with a custom `fieldOrder`, **When** the message is encoded on the
   real production encode path (not just the standalone `encode_with_order` primitive), **Then** the
   wire output reflects the declared order.
3. **Given** a dictionary declaring a header/trailer repeating group such as `NoHops`, **When** a
   message containing that group is decoded on the real production decode path, **Then** the group's
   fields are recognized as header/trailer content and are visible to group-structure validation
   rather than being silently dropped from validation or misrouted to the message body.
4. **Given** a field pair that carries binary/encoded data with a preceding length field (matching the
   pattern of already-recognized pairs like `EncodedText`/`EncodedTextLen`), **When** the dictionary's
   data-field allowlist is consulted, **Then** the remaining undeclared pairs from the same family
   (e.g. `EncodedLeg*` fields) are recognized too, so embedded SOH bytes in their content don't cause
   incorrect message framing.
5. **Given** the shipped `.fixdict` files' provenance header comments, **When** they are read by a
   maintainer, **Then** they name the tooling that actually produced them, not a module that has since
   been deleted.

---

### User Story 8 - Feature-completeness gaps carried forward from the prior audit remain visible and tracked (Priority: P3)

A set of narrower feature-completeness gaps — recurring daily sequence reset, multi-tag `LogonTag`,
version-scoped reject-reason codes, FIXT ApplVerID auto-negotiation from an inbound Logon, wildcard
dynamic-session templates, `.cfg`-selectable log backend variants beyond File/SQL, IANA timezone
names, environment-variable fallback for config interpolation, and similar items — were reconfirmed
still open by this audit but are lower-severity, narrower-scope, or optional-per-reference-engine.

**Why this priority**: Real gaps versus QuickFIX/J/Go feature surface, but none are externally-
triggerable protocol breaks or data-loss risks; appropriate to schedule alongside normal feature work
rather than as urgent remediation.

**Independent Test**: For each item addressed under this story, a targeted test exists demonstrating
the specific previously-missing behavior now works (e.g. a `.cfg` using an IANA timezone name resolves
correctly; a `.cfg` selecting `ScreenLog` actually selects it).

**Acceptance Scenarios**:

1. **Given** the set of feature-completeness gaps reconfirmed open by this audit (recurring
   mid-connection sequence reset, multi-tag `LogonTag`, FIXT `ApplVerID` auto-extraction from inbound
   Logon tag 1137, wildcard SubID/LocationID template matching for dynamic acceptor sessions,
   `.cfg`-selectable `ScreenLog`/`TracingLog`/`CompositeLog`, IANA timezone names, environment-variable
   fallback for `${var}` interpolation), **When** each is addressed, **Then** its own targeted test
   demonstrates the previously-missing behavior now works end-to-end.
2. **Given** an item in this set that remains genuinely out of scope for this pass (e.g. explicitly
   QuickFIX/Go-only optional behavior, or an architectural change flagged as not-recommended
   near-term work), **Then** it is explicitly and individually accepted-or-deferred with a stated
   reason rather than silently dropped from tracking.

---

### User Story 9 - Acceptance-test coverage accurately reflects and exercises current behavior (Priority: P3)

The acceptance-test (AT) suite's regression floor is a stale literal that no longer matches the
suite's actual scenario-run count, and a dictionary blocker that previously prevented authoring a
specific scenario has since been resolved without the scenario itself being written.

**Why this priority**: A one-line fix (regression floor) closes a real coverage-regression blind spot
immediately; the scenario-authoring item is real follow-up work now unblocked.

**Independent Test**: Run the AT suite and confirm the asserted regression-floor threshold matches the
suite's actual current scenario-run count. Confirm a `MinQty`-exercising scenario exists and runs
across the targeted FIX versions.

**Acceptance Scenarios**:

1. **Given** the AT suite's current `server_suite()` scenario-run count, **When** the regression-floor
   test runs, **Then** its asserted threshold matches the actual count (not a stale lower number that
   would silently tolerate losing up to ~20 scenario-runs).
2. **Given** that `MinQty` (tag 110) is now present in every bundled dictionary, **When** the AT suite
   runs, **Then** a scenario exercising `MinQty` behavior runs across the targeted FIX versions.

---

### User Story 10 - Tooling and latent-risk hygiene issues are fixed before they become live (Priority: P3)

Several issues are currently latent (no live trigger against real data/current call patterns) but
would become real bugs under a plausible future input or usage pattern: unbounded recursion on a
malformed future dictionary source, a doc/implementation mismatch that makes a panicking index
appear safer than it is, a build-time algorithmic inefficiency, and a store write path with a
TOCTOU race under concurrent writers (not reachable via any current call site, but would corrupt data
the moment one is added).

**Why this priority**: None of these are live/triggerable today — appropriate to fix opportunistically
rather than urgently, but worth closing before a future change makes them live.

**Independent Test**: For each item, a targeted test or code-level guard demonstrates the fix (e.g. a
self-referencing component chain in a dictionary source is rejected with a depth/cycle error instead
of recursing unboundedly).

**Acceptance Scenarios**:

1. **Given** a malformed dictionary source with a self-referencing component chain, **When** it is
   processed by the header/trailer-resolution path, **Then** processing fails with a depth/cycle error
   instead of recursing unboundedly.
2. **Given** the dictionary-source parsing module's doc comments, **When** compared against its actual
   behavior, **Then** they accurately describe what registration/indexing the implementation performs.
3. **Given** the store's append-path byte-offset bookkeeping, **When** two writers operate concurrently
   against the same store instance, **Then** the offset is read and recorded under the same
   synchronization boundary as the rest of the write, closing the TOCTOU window even though no current
   call site exercises it concurrently.

### Edge Cases

- What happens when a `SequenceReset` (plain mode) arrives with `NewSeqNo` equal to the current
  `next_in_seq`? (Neither an increase nor a decrease — MUST be handled as a no-op accept, not rejected
  as "not greater than.")
- What happens when a `ResendRequest` has `BeginSeqNo > EndSeqNo` but both tags are present and
  non-zero? (Distinct from the missing-tag case in US1 scenario 3 — this remains the existing
  silent-skip behavior, which the audit explicitly flags as acceptable to keep as-is.)
- What happens when a store persistence failure occurs on a backend where the `Log` sink is itself
  unavailable? (The operator-visible signal requirement in US3 must degrade gracefully, not panic or
  cascade.)
- What happens when a `.cfg`'s `JdbcURL` uses a scheme that is neither real H2, real MSSQL, nor any
  currently-supported SQL family? (MUST fail with a clear, typed unsupported-backend error, not a
  silent misroute.)
- What happens when a multi-session acceptor group has members whose `ContinueInitializationOnError`
  values conflict, and the chosen resolution is "reject at config-resolve time"? (MUST produce a clear
  configuration error identifying the conflicting sessions, not a runtime failure.)
- What happens when an oversized `BodyLength` frame arrives on a connection that has already
  successfully completed Logon? (The bound in US6 scenario 1 applies uniformly pre- and post-Logon.)

## Requirements *(mandatory)*

### Functional Requirements

**Session protocol correctness (US1)**

- **FR-001**: The system MUST reject an inbound Logon whose `MsgSeqNum` is lower than the session's
  expected next-incoming sequence number and lacks valid `PossDupFlag` justification, via the same
  rejection path used for other target-too-low conditions, without transitioning session state to
  logged-on and without advancing the expected sequence number.
- **FR-002**: The system MUST reject a plain-mode (non-gap-fill) `SequenceReset` whose `NewSeqNo` is
  less than the session's current expected next-incoming sequence number, leaving the expected
  sequence number unchanged, and MUST accept it (advancing the expected sequence number) only when
  `NewSeqNo` is greater than the current value.
- **FR-003**: The system MUST treat a plain-mode `SequenceReset` missing its `NewSeqNo` field as a
  required-field violation rather than silently skipping sequence adjustment while still processing
  queued messages.
- **FR-004**: The system MUST reject a `ResendRequest` missing `BeginSeqNo` or `EndSeqNo` with a
  required-tag-missing response, distinct from the existing (and retained) silent-skip behavior for a
  `ResendRequest` where both tags are present but `BeginSeqNo > EndSeqNo`.
- **FR-005**: The system MUST perform a full, consistent sequence-number reset (both inbound and
  outbound) on the initiator side when `ResetOnLogon` is configured and a Logon has already been sent
  for the current connection attempt, matching the acceptor's own full-reset expectation for the same
  configuration.
- **FR-006**: The system MUST apply the same application-level validation to a message drained from
  the out-of-order queue after a gap is filled as it applies to a message received in the expected
  order — no drained message bypasses validation.
- **FR-007**: The system MUST reset per-connection heartbeat/test-request/resend timers and tracking
  state at the start of each new connection, not carry them over from a prior connection.
- **FR-008**: The system MUST send a Logout before disconnecting when an inbound message fails
  dictionary validation, consistent with the Logout-then-disconnect behavior already used for
  identity/latency validation failures.
- **FR-009**: The system MUST treat an inbound message with an unparseable `SendingTime` as failing
  the latency check rather than passing it.

**Multi-session acceptor routing (US2)**

- **FR-010**: The system MUST build its live connection-routing lookup key from the full session
  identity (BeginString, SenderCompID, TargetCompID, and SenderSubID/SenderLocationID/TargetSubID/
  TargetLocationID extracted from the inbound Logon) rather than a partial key that ignores SubID/
  LocationID, so a session registered with those fields populated is routable.
- **FR-011**: The system MUST require every `SessionQualifier`-distinguished session to be bound to
  its own distinct non-wire discriminator (a distinct listen port/socket, or a dedicated per-qualifier
  connection template) rather than relying on wire fields alone — since `SessionQualifier` has no wire
  representation, two sessions sharing identical wire-visible identity and no other discriminator MUST
  be rejected as an ambiguous configuration at resolve time rather than left silently unroutable at
  connection time.
- **FR-012**: The acceptor-builder bind path MUST enable listen-address reuse (equivalent to
  `SocketReuseAddress`/`SO_REUSEADDR`) consistent with the other bind path that already does so, so a
  routine restart does not fail to rebind a just-released port.

**Store/log persistence and durability (US3)**

- **FR-013**: The system MUST route every currently-swallowed store-operation failure (save-and-advance,
  set-next-sender/target-seq, reset, resend re-hydration read) through the session's log as an
  operator-visible event.
- **FR-014**: The system MUST consult a store's persisted creation time on startup and MUST NOT perform
  a schedule-driven full reset when that creation time already falls within the currently-active
  schedule window — only a creation time outside the current window makes a schedule-driven reset
  eligible.
- **FR-015**: File-backed stores (`FileStore` and `CachedFileStore`) MUST provide the same
  save-and-advance atomicity guarantee already provided by the SQL, MSSQL, and Redb-backed stores, so
  a crash between the sequence-advance and the message-body write cannot leave the advertised sequence
  number without a corresponding stored message.
- **FR-016**: A store's reset operation MUST be synced to durable storage before completing when the
  backend's durability-sync option is enabled, matching the sync discipline already applied to appends
  and sequence-file writes, and MUST clear sequence-number and message-history state such that a crash
  mid-reset cannot leave one reflecting post-reset state while the other still reflects pre-reset data.
- **FR-017**: `MssqlStore` MUST validate any configuration-sourced table-identifier setting before use
  and MUST fail with a clear, typed configuration error on an invalid identifier, consistent with the
  identifier validation already performed by the other SQL-family store and log backends.

**QuickFIX/J `.cfg` drop-in compatibility (US4)**

- **FR-018**: The system MUST NOT silently misroute a `JdbcURL` scheme it advertises as SQL-family
  support to an unrelated backend (e.g. treating an unrecognized scheme's raw string as a SQLite
  filename) — an unsupported scheme MUST fail with a clear, typed unsupported-backend error, and any
  scheme that is genuinely supported MUST connect to the correct backend. `jdbc:h2:` specifically MUST
  be removed from the recognized-SQL-family-scheme table (rejected with a clear unsupported-backend
  error) rather than gaining a new H2-compatible backend implementation — this feature does not add H2
  database support.
- **FR-019**: The system MUST parse the real QuickFIX/J MSSQL JDBC URL grammar (semicolon-delimited
  properties, no path segment) in addition to TrueFix's existing path-based shorthand form.
- **FR-020**: `JdbcUser` and `JdbcPassword` configuration values MUST be percent-encoded before being
  spliced into a JDBC-style connection URL's authority component.

**Engine lifecycle (US5)**

- **FR-021**: When a multi-session `Engine::start` call fails partway through startup, the caller MUST
  retain the ability to cleanly stop every session that already started successfully before the
  failure — either via a returned partial engine/handle set or by the start call itself
  aborting/joining already-started sessions before returning the error.
- **FR-022**: When multiple sessions grouped under one listen configuration specify conflicting
  `ContinueInitializationOnError` values, the system MUST resolve the group's effective tolerance by
  strictest-member-wins: the group is treated as `ContinueInitializationOnError=N` whenever any member
  sets `N`, rather than silently using the first member's value.
- **FR-023**: `Engine::shutdown()`'s documentation MUST accurately describe which classes of running
  session it stops and which (if any) it intentionally does not, so its behavior is not misread as a
  full stop.

**Network hardening (US6)**

- **FR-024**: The system MUST bound the size of an in-progress inbound frame being assembled and close
  the connection once the declared or accumulated size exceeds a configured or sane maximum, applying
  uniformly before and after Logon.
- **FR-025**: The connect-timeout behavior already applied to direct and TLS initiator connects MUST
  also apply to the proxy connect path, so a proxy that accepts a TCP connection but never completes
  its handshake does not hang the connect attempt indefinitely.
- **FR-026**: A `CipherSuites` configuration value that matches zero recognized cipher suites MUST
  produce a clear configuration-time error rather than silently producing a TLS context with no usable
  suites.
- **FR-027**: The PROXY-protocol trusted-source header-peek path MUST be bounded by a timeout so a
  connection that opens but never completes its header cannot hang its connection task indefinitely.
- **FR-028**: When frame recovery discards a malformed/non-FIX prefix from the read buffer, it MUST
  discard only the malformed prefix, preserving any legitimate message data that follows it in the same
  buffer.
- **FR-029**: The PROXY-protocol v2 header parser's peek buffer MUST be sized (or its parsing logic
  adjusted) so that headers with TLVs near or exceeding the current buffer capacity are not truncated
  in a way that causes trailing PROXY-protocol bytes to be misread as FIX message data.

**Dictionary/codec correctness (US7)**

- **FR-030**: Dictionary type-checking MUST format-validate `UtcTimeOnly` and `UtcDate` field values
  using the existing typed parsers, rejecting malformed values instead of passing them unconditionally.
- **FR-031**: A message definition's declared `fieldOrder` MUST be honored by the production message
  encode path actually used by generated/codegen'd message types, not only by the standalone
  order-aware encode primitive.
- **FR-032**: Header/trailer repeating groups declared in a dictionary (including at minimum the
  standard `NoHops`/`HopCompID`/`HopSendingTime`/`HopRefID` group) MUST be decoded and made visible to
  group-structure validation on the production decode path actually used by live message handling.
- **FR-033**: The data-field-length allowlist used to protect encoded/binary-content fields from
  SOH-based frame corruption MUST include the remaining undeclared members of already-recognized field
  families (at minimum the `EncodedLeg*` pairs).
- **FR-034**: Shipped `.fixdict` provenance header comments MUST name the tooling that actually
  produced them.

**Feature-completeness gaps carried forward (US8)**

- **FR-035**: The system MUST support recurring, schedule-driven mid-connection sequence resets
  (equivalent to `ResetSeqTime`/`EnableResetSeqTime`) in addition to the existing boundary
  enter/exit reset transitions.
- **FR-036**: `LogonTag` configuration MUST support multiple `(tag, value)` pairs (equivalent to
  `LogonTag`, `LogonTag1`, `LogonTag2`, …), not exactly one.
- **FR-037**: The system MUST auto-extract `DefaultApplVerID` from an inbound Logon's tag 1137 when
  present, and the `.cfg`-driven engine-start path MUST construct real per-transport/per-application
  FIXT dictionary separation rather than treating both as aliases of one dictionary.
- **FR-038**: Dynamic-session acceptor templates MUST support wildcard matching on SubID/LocationID in
  addition to the currently-supported BeginString/SenderCompID/TargetCompID substitution.
- **FR-039**: The `.cfg`-driven log-backend resolution path MUST support selecting `ScreenLog`,
  `TracingLog`, and `CompositeLog` in addition to the currently-supported File and SQL log specs.
- **FR-040**: `TimeZone` configuration values MUST accept IANA timezone names in addition to the
  currently-supported numeric `+HH:MM`/`-HH:MM` offset form.
- **FR-041**: `${var}` configuration interpolation MUST fall back to environment variables when a
  referenced name is not present in the settings map itself.

**AT harness coverage (US9)**

- **FR-042**: The AT suite's asserted regression-floor threshold MUST match the suite's actual current
  total scenario-run count.
- **FR-043**: The AT suite MUST include a scenario exercising `MinQty` (tag 110) behavior, run across
  the suite's targeted FIX versions.

**Tooling / latent-risk hygiene (US10)**

- **FR-044**: The dictionary header/trailer-resolution path MUST guard against unbounded recursion on a
  self-referencing component chain, consistent with the depth/cycle guard already present in its
  sibling resolution path.
- **FR-045**: The dictionary-source parsing module's doc comments MUST accurately describe the
  registration/indexing behavior the implementation actually performs.
- **FR-046**: The store append path's byte-offset bookkeeping MUST be read and recorded within the same
  synchronization boundary as the rest of the write, so concurrent writers to one store instance cannot
  race on the recorded offset.

### Key Entities

- **SessionId (routing key)**: The full 8-field session identity (BeginString, SenderCompID,
  TargetCompID, SenderSubID, SenderLocationID, TargetSubID, TargetLocationID, SessionQualifier) used
  both to register a session and to route a live inbound connection to it — today registration uses
  the full identity while live routing uses only a 3-field subset, the root cause of US2.
- **Store creation time**: A persisted timestamp recording when a store's data was first created,
  written by every backend but consulted by no session/transport/engine logic today (US3) — intended
  to distinguish "restart within the same schedule window" (preserve state) from "restart in a new
  window" (eligible for reset).
- **JdbcURL scheme**: The connection-string scheme in a `.cfg`'s `JdbcURL`/store-config setting that
  determines which storage backend and URL grammar to use — must map 1:1 to an actually-implemented
  backend and grammar (US4), never to a plausible-looking but unimplemented one.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every acceptance scenario defined under User Stories 1-10 above passes when exercised
  against a live two-process (or, for US5/US9/US10, single-process/build-time) test, with no
  regression in any existing passing test.
- **SC-002**: A counterparty sending any of the malformed/out-of-window inputs covered by US1 (low-seq
  Logon, decreasing SequenceReset, malformed ResendRequest) receives a protocol-correct
  rejection/Logout in 100% of exercised cases, with zero cases of silent acceptance or session-state
  desync.
- **SC-003**: A multi-session acceptor deployment distinguished by SubID/LocationID/SessionQualifier
  routes 100% of correctly-identified inbound connections to their intended session.
- **SC-004**: A simulated store-operation failure produces an operator-visible log event in 100% of
  the previously-silent call sites identified by this spec, with zero silent failures remaining at
  those sites.
- **SC-005**: A process restart landing inside an active schedule window preserves persisted sequence
  numbers and message history in 100% of exercised cases (zero spurious full resets).
- **SC-006**: A `.cfg` file using real QuickFIX/J JDBC URL grammar (H2 or semicolon-delimited MSSQL)
  either connects to the correct backend or fails with a clear, typed error — zero cases of silent
  misroute to an unintended backend.
- **SC-007**: The AT suite's regression-floor assertion matches its actual scenario-run count exactly,
  and the suite's total scenario-run count does not decrease as a result of this feature's changes.
- **SC-008**: No test, benchmark, or acceptance scenario passing before this feature begins regresses
  as a result of these changes.

## Assumptions

- This feature delivers all 10 user stories (US1-US10) in one pass, internally staged with
  per-story/per-priority-tier checkpoints — not split across multiple specs — consistent with how
  002-qfj-parity-completion, 003-qfj-full-parity-closure, and 005-engine-gap-remediation each closed
  a full audit-tracked backlog in a single feature.
- This spec's scope is exactly the items tracked as open findings in `docs/todo/003.md` (the P0/P1/P2
  bugs, the "problems found in previously-closed items," the reconfirmed-open feature-completeness
  gaps, the AT-harness follow-ups, and the supplementary `B1`-`B30` findings) — items explicitly listed
  under that document's "Known-accepted, not a gap" section are out of scope by design, not oversight.
- Per the governing constitution's Principle VII (Inventory-Based Completeness) and Principle III
  (License & Provenance Discipline), remediation work MUST be implemented from the FIX protocol
  specification and independently-authored logic, using `thrdpty/quickfixj`/`thrdpty/quickfix` only to
  understand referenced behavior — never by copying source, comments, or test implementations.
- Per Principle VI (Acceptor/Initiator Parity), every session-layer fix in US1/US2 MUST be verified on
  both acceptor and initiator sides where the underlying behavior applies to both roles.
- `GAP-49` (FileStore/CachedFileStore save-and-advance atomicity) is treated as in-scope under FR-015
  as an extension of the prior feature's already-accepted scope, not a new architectural commitment.
- Items noted in `docs/todo/003.md` as "latent, not live" (US10) are included in scope because the
  audit explicitly recommends closing them before a future change makes them live, but they carry
  lower urgency than US1-US7 and may be sequenced last.
- The supplementary Chinese-language findings (`B1`-`B30`, appended without full audit-style writeups)
  are held to the same fix-direction/testability bar as the main findings; where a `B`-item's terse
  description leaves an implementation detail ambiguous, the fix MUST match the referenced FIX
  protocol behavior (or the corresponding QuickFIX/J/Go behavior) rather than the shortest possible
  code change.
