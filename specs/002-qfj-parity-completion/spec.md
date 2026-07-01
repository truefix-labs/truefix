# Feature Specification: Complete Remaining QuickFIX/J Parity (excl. QuickFIX/Go-only)

**Feature Branch**: `002-qfj-parity-completion`

**Created**: 2026-06-30

**Status**: Draft

**Input**: User description: "参考 docs/todo-gap-analysis.md 基于 正确性地基与可用性 看看哪些差异是需要我们实现 还没实现的" — subsequently broadened (user decision) to close **all** remaining QuickFIX/J-parity gaps from the audit **except QuickFIX/Go-only extras**.

## Context

The TrueFix engine (feature `001-fix-engine-parity`) reached structural completeness — a layered
workspace, byte-exact codec, full session semantics, dual-track dictionaries, multi/dynamic
acceptors, and a 48-scenario acceptance-test (AT) suite. A full code audit
(`docs/todo-gap-analysis.md`, 2026-06-30) catalogued 21 remaining gaps versus QuickFIX/J and
QuickFIX/Go.

This feature closes **every remaining QuickFIX/J-parity gap from that audit**, organised so the
highest-value correctness-foundation and usability gaps land first. The **only** category left out is
the set of **QuickFIX/Go-only extras**, which are outside the constitution's QuickFIX/J parity target.

Two of these gaps are not mere parity — they close **partially-met constitution principles**:

- **Principle IV (dual-track data dictionary)** requires build-time codegen of *strongly-typed
  messages* alongside the runtime dictionary. Today `build.rs` emits only MsgType constants and the
  dual-track hash; typed message/field/group structs and a working MessageCracker are missing (US6).
- **Principle I (production-ready, structured observability)** is only partially met: a `Monitor`
  exposes queryable status, but no metrics signals are exported (US9).

Per the constitution, **protocol correctness is the highest-priority principle** and the AT suite is
the release gate. Like feature 001, this feature is delivered in **one pass but internally staged**
with per-stage checkpoints; every correctness requirement MUST be demonstrated by an AT scenario or
session-level test, never merely asserted. No third-party source or test data is copied (Principle
III); dictionaries derive from the normalized FIX source.

## Clarifications

### Session 2026-06-30

- Q: How deep should the strongly-typed codegen go (US6 / FR-020)? → A: **All messages typed** —
  generate per-message, group, and component typed structs (with typed accessors and field-value
  enumerations) for **every message defined in the bundled dictionary subsets**, across targeted
  versions.
- Q: Is a breaking change to the Application callback signatures acceptable for typed outcomes
  (US5 / FR-016)? → A: **Yes** — replace the `Result<(), String>` callbacks with typed
  Reject/DoNotSend/BusinessReject outcomes directly (crates are pre-1.0).
- Q: How should the engine expose metrics (US9 / FR-023)? → A: **Through the `metrics` facade only**
  (gauges/counters); no bundled exporter or HTTP scrape endpoint — operators attach their own.
- Q: Which SQL databases must this feature implement and test (US12 / FR-024)? → A: **PostgreSQL,
  MySQL, and SQLite** (all three via sqlx behind the `sql` feature), for JDBC-equivalent parity.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Stand up a session purely from a configuration file (Priority: P1)

A sell-side/buy-side operator with a QuickFIX-style `.cfg` file (DEFAULT section plus one or more
SESSION sections) wants to bring up a working FIX engine — initiator or acceptor — with **no code
changes**. Today the configuration keys are parsed and classified but nothing maps them into a
runnable engine.

**Why this priority**: The single largest usability blocker; without it the engine is a library
requiring bespoke glue per deployment.

**Independent Test**: Provide a `.cfg` describing one acceptor and one initiator session; start the
engine from that file alone; observe a completed logon-to-heartbeat exchange with store, log, and
schedule all taking effect as configured.

**Acceptance Scenarios**:

1. **Given** a `.cfg` with `[DEFAULT]` and one acceptor `[SESSION]`, **When** the engine starts from
   that file, **Then** an acceptor session is created with the configured CompIDs, heartbeat interval,
   store, log, and schedule, and it accepts a counterparty logon.
2. **Given** a `.cfg` with an initiator session pointing at a reachable acceptor, **When** the engine
   starts, **Then** the initiator connects, logs on, and exchanges heartbeats.
3. **Given** an unknown or malformed value for a recognised key, **When** the engine starts, **Then**
   startup fails with a typed, human-readable error naming the offending key and session, with no
   partially-started engine left running.
4. **Given** DEFAULT values overridden in a specific SESSION, **When** the engine starts, **Then** the
   per-session value takes precedence.

---

### User Story 2 - Survive a process restart without breaking the sequence contract (Priority: P1)

A counterparty sends a ResendRequest **after** the TrueFix process restarted. The engine must replay
the originally-sent messages (as PossDup) from durable storage. Today sequence numbers persist but the
session's resend path keeps sent messages only in memory, violating the persistence guarantee the
baseline already claims (FR-G2, SC-006).

**Why this priority**: A correctness defect in a *claimed* capability — the most serious kind.

**Independent Test**: Send several messages; restart the engine against the same store; have the
counterparty issue a ResendRequest spanning pre-restart messages; verify identical replays with
`PossDupFlag=Y`.

**Acceptance Scenarios**:

1. **Given** messages 1–5 were sent and persisted, **When** the process restarts and a ResendRequest
   for 2–4 arrives, **Then** 2, 3, 4 are replayed from the store with `PossDupFlag=Y` and the original
   `OrigSendingTime`.
2. **Given** a mix of admin and application messages in range, **When** a post-restart ResendRequest
   arrives, **Then** admin messages collapse to a SequenceReset-GapFill and application messages are
   replayed individually.
3. **Given** `PersistMessages=N`, **When** a ResendRequest arrives, **Then** the engine gap-fills the
   range instead of replaying bodies.

---

### User Story 3 - Correctly handle messages containing repeating groups (Priority: P1)

A counterparty sends messages with repeating groups (and components) — the norm for real order/
market-data flow. The engine must decode these into structured groups using the dictionary and
validate group-level rules. Today the decoder produces a flat field list and validation walks only
flat fields.

**Why this priority**: Repeating groups are unavoidable in real traffic; flat-only handling silently
mis-parses or fails to reject malformed group messages.

**Independent Test**: Send a correct group message, then deliberately malformed variants
(wrong count, missing delimiter, out-of-order members); verify correct acceptance and the right
rejects.

**Acceptance Scenarios**:

1. **Given** a dictionary-defined group, **When** a correctly-formed group message arrives, **Then**
   it is decoded into the structured group and accepted.
2. **Given** a group whose declared count ≠ entry count, **When** it arrives, **Then** the engine
   rejects with the count-mismatch reason.
3. **Given** a group whose first field is not the delimiter and `FirstFieldInGroupIsDelimiter` is on,
   **When** it arrives, **Then** the engine rejects it.
4. **Given** out-of-order group members and `ValidateUnorderedGroupFields` on, **When** the message
   arrives, **Then** the engine rejects it; with the toggle off, it is accepted.

---

### User Story 4 - Reject malformed and invalid inbound messages per FIX rules (Priority: P2)

A counterparty or faulty network sends garbled or structurally-invalid messages: bad checksum,
incorrect body length, wrong BeginString, mismatched CompIDs, out-of-order header fields, repeated
tags, stale SendingTime. The engine must apply FIX message-integrity rules and the **two rejection
layers** (session Reject vs Business Message Reject), including configurable RejectGarbledMessage
behaviour and reverse-routed reject headers.

**Why this priority**: Well-defined FIX correctness behaviours that protect the session; second only
to the P1 defects because today's silent-drop is safe (if lenient) rather than actively wrong.

**Independent Test**: Drive each malformed-message class against an acceptor and verify the FIX-correct
outcome for each.

**Acceptance Scenarios**:

1. **Given** `RejectGarbledMessage=Y`, **When** a bad-checksum or bad-body-length frame arrives,
   **Then** the engine emits a session-level Reject (35=3); **Given** the default (`N`), **Then** the
   frame is dropped without advancing the sequence number.
2. **Given** SenderCompID/TargetCompID not matching the session profile and `CheckCompID` on, **When**
   a message arrives, **Then** the engine rejects/refuses it.
3. **Given** SendingTime outside the accepted range, **When** a message arrives, **Then** the engine
   rejects it with the SendingTime-accuracy reason.
4. **Given** a repeated tag, out-of-order header fields, or first-three-fields out of order, **When**
   a message arrives, **Then** the engine rejects it with the corresponding reason.
5. **Given** an application message the integrator business-rejects, **When** it is processed, **Then**
   a Business Message Reject (35=j) is emitted, distinct from session Rejects.

---

### User Story 5 - Integrate via typed, intention-revealing callback outcomes (Priority: P2)

An integrator wants FIX-meaningful callback outcomes — reject a logon, suppress (do-not-send) an
outbound message, business-reject an inbound application message — through typed results rather than
opaque strings. Today callbacks return `Result<(), String>`, which cannot carry a reason/reference tag
and is ignored on the app path in places.

**Why this priority**: An ergonomics/usability gap with correctness consequences (an integrator
currently cannot make the engine emit a proper Reject/BusinessReject from a callback).

**Independent Test**: Implement callbacks returning each typed outcome and verify the engine acts on
them.

**Acceptance Scenarios**:

1. **Given** a logon-validation callback returning a Reject outcome, **When** a logon arrives,
   **Then** the engine refuses the session.
2. **Given** an outbound callback returning a do-not-send outcome, **When** the engine would send the
   message, **Then** it is suppressed and not stored as sent.
3. **Given** an inbound-application callback returning a business-reject outcome with a reason and
   reference tag, **When** the message is processed, **Then** a Business Message Reject carrying that
   reason and tag is emitted.

---

### User Story 6 - Strongly-typed messages and message cracking (Priority: P2)

A developer integrating against the engine wants the dual-track promise: build-time generated,
strongly-typed per-version message and field structs with typed accessors and field enumerations, and
a MessageCracker that dispatches an incoming message to a typed handler — instead of hand-reading raw
tags. Today codegen emits only MsgType constants and the dual-track hash, and the MessageCracker is an
empty-shell trait.

**Why this priority**: Closes the unmet half of **constitution Principle IV** and is the largest
developer-usability gap; placed at P2 because the engine is functionally correct via the generic API
without it.

**Independent Test**: Generate typed messages for a version, build a known message via the typed API,
encode/decode it, and dispatch it through the MessageCracker to a typed handler; confirm the
dual-track invariant (codegen and runtime dictionary share one source) still holds.

**Acceptance Scenarios**:

1. **Given** a normalized dictionary, **When** the build runs, **Then** strongly-typed message, field,
   group, and component structs (with typed accessors and field-value enumerations) are generated for
   each targeted version.
2. **Given** a generated typed message, **When** it is encoded and decoded, **Then** it round-trips
   byte-identically with the generic codec path.
3. **Given** an incoming message, **When** it is passed to the MessageCracker, **Then** it is
   dispatched to the handler typed for its MsgType/version.
4. **Given** the generated artifacts, **When** the dual-track hash is checked, **Then** it confirms
   codegen and runtime dictionary derive from the same source.

---

### User Story 7 - Configure transport security from the configuration file (Priority: P3)

An operator deploying into a regulated venue needs TLS — often mutual TLS — configured entirely from
the `.cfg` (key/cert/CA, minimum version, SNI, client-auth). Today the transport only accepts a
pre-built TLS configuration in code and uses no client authentication.

**Why this priority**: Important for real deployment but below the core correctness and config-start
work; many internal/test deployments run without TLS.

**Independent Test**: Provide a `.cfg` with TLS material and `NeedClientAuth=Y`; start an acceptor and
connect an initiator presenting a client certificate; verify the mutually-authenticated handshake and
refusal of an un-credentialed client.

**Acceptance Scenarios**:

1. **Given** TLS key/cert/CA referenced in the `.cfg`, **When** the engine starts, **Then** the
   session negotiates TLS without code-level TLS construction.
2. **Given** `NeedClientAuth=Y`, **When** a client without a valid certificate connects, **Then** the
   handshake is refused; with a valid certificate, it succeeds.
3. **Given** a configured minimum TLS version, **When** a peer offers only a lower version, **Then**
   the handshake is refused.

---

### User Story 8 - Reset sessions cleanly at scheduled boundaries, including weekly windows (Priority: P3)

An operator relies on the session schedule to reset sequence numbers at the start of each trading
window, and to define **weekly** windows via StartDay/EndDay (not just daily times). When a session
crosses out of its window it must disconnect, reset sequence numbers, clear stored messages, and (for
initiators) reconnect at the next opening — unless configured non-stop. Today scheduled reset exists
only partially at the transport layer, and weekly StartDay/EndDay windows are unsupported.

**Why this priority**: Correctness/completeness of an operational behaviour; lower urgency than the P1
defects.

**Independent Test**: Configure a short window and a weekly StartDay/EndDay window; advance past
boundaries; verify disconnect → reset → clear-store and reconnect/reset at the next opening, and that
a non-stop session is unaffected.

**Acceptance Scenarios**:

1. **Given** a session past its scheduled end, **When** the boundary is crossed, **Then** the engine
   disconnects, resets sequence numbers, and clears the store.
2. **Given** an initiator whose next window opens, **When** the start boundary is reached, **Then** it
   reconnects with reset sequence state.
3. **Given** a weekly StartDay/EndDay window, **When** the current time is inside/outside that window,
   **Then** "is in session" is computed correctly across day boundaries.
4. **Given** `NonStopSession=Y`, **When** any boundary passes, **Then** no scheduled reset occurs.

---

### User Story 9 - Observe the engine through exported metrics (Priority: P3)

An operator running the engine in production wants queryable metrics — session state, current
sequence numbers, message counts, connection health, reconnect counts — exported through a standard
metrics facade, complementing the existing `Monitor`. Today no metrics are exported.

**Why this priority**: Closes the partially-met **constitution Principle I (structured
observability)**; P3 because basic status is already queryable via `Monitor`.

**Independent Test**: Run a session through logon, message exchange, and a reconnect; scrape the
exported metrics and verify the state gauge, sequence-number gauges, message counters, and
reconnect counter reflect reality.

**Acceptance Scenarios**:

1. **Given** a logged-on session, **When** metrics are read, **Then** a session-state gauge and the
   next sender/target sequence numbers are exposed.
2. **Given** ongoing traffic, **When** metrics are read, **Then** sent/received message counters
   increase accordingly.
3. **Given** a dropped-and-reconnected session, **When** metrics are read, **Then** the reconnect
   counter has incremented.

---

### User Story 10 - Production-grade transport options and endpoint failover (Priority: P3)

An operator needs the full set of socket tuning options (keep-alive, buffer sizes, reuse-address,
linger, etc.) and **multi-endpoint failover** — numbered backup hosts/ports the initiator rotates
through on reconnect. Today only TcpNoDelay is applied and the initiator reconnects to a single
endpoint.

**Why this priority**: Production-readiness/HA; P3 because single-endpoint, default-socket operation
is functionally correct.

**Independent Test**: Configure multiple connect endpoints and socket options; kill the primary
endpoint; verify the initiator rotates to a backup and the configured socket options are applied.

**Acceptance Scenarios**:

1. **Given** configured socket options, **When** a session connects, **Then** each option is applied
   to the socket.
2. **Given** numbered backup endpoints, **When** the primary is unreachable, **Then** the initiator
   connects to the next configured endpoint.

---

### User Story 11 - Reverse-route administrative replies (Priority: P3)

When the engine replies to or rejects a message that carried routing header fields
(`OnBehalfOfCompID`/`DeliverToCompID`, etc.), the reply must reverse those routing fields so it
reaches the originator. Today reverse routing is unimplemented.

**Why this priority**: A defined FIX protocol behaviour exercised by Appendix B `ReverseRoute`
scenarios; P3 because it affects only routed/relayed deployments.

**Independent Test**: Send a message carrying routing fields; verify the engine's reply/reject reverses
them; verify empty routing tags are handled.

**Acceptance Scenarios**:

1. **Given** a message with routing header fields, **When** the engine generates a reply/reject,
   **Then** the routing fields are reversed onto the outbound message.
2. **Given** routing tags present but empty, **When** the engine reverse-routes, **Then** it follows
   the empty-routing-tags rule without error.

---

### User Story 12 - Storage and logging completeness (Priority: P3)

An operator wants storage and logging to match QuickFIX/J behaviour: SQL stores/logs across common
databases (beyond SQLite), a genuinely cached file store with an fsync toggle, and the logging output
switches (heartbeat filtering, millisecond inclusion, event/incoming/outgoing toggles). Today SQL is
SQLite-only, CachedFileStore delegates straight to FileStore, and log switches are recognised but
inert.

**Why this priority**: Production-readiness/operability polish; P3 because the default file/SQLite
stores and current logging are functionally correct.

**Independent Test**: Run the store/log suite against the additional database backends and the cached
store; toggle the log switches and verify the output changes accordingly.

**Acceptance Scenarios**:

1. **Given** a SQL store/log configured for an additional supported database, **When** the engine runs,
   **Then** persistence and logging behave equivalently to the SQLite backend.
2. **Given** a cached file store with a max-cache size and fsync toggle, **When** messages are written,
   **Then** they are cached and flushed per configuration and survive restart.
3. **Given** `FileLogHeartbeats=N` (and related switches), **When** heartbeats flow, **Then** they are
   excluded from the log output.

### Edge Cases

- A `.cfg` references a store/log/TLS file path that is missing or unreadable → startup fails with a
  typed error naming the key and path; no partially-started engine remains.
- A ResendRequest spans both persisted and never-sent ranges after a restart → persisted messages
  replay; the never-sent tail collapses to a GapFill.
- A repeating group is nested, or its declared count is zero → nesting is parsed recursively; a zero
  count is treated as an empty group.
- A validation toggle depends on group parsing (e.g. `ValidateUnorderedGroupFields`) → honoured only
  once group parsing is active; otherwise the unsupported combination is surfaced, not silently passed.
- Timestamp precision is nanoseconds → emitted SendingTime carries nanosecond digits; inbound
  timestamps of any supported precision parse without loss.
- A typed (codegen) message and a generic message for the same logical content → both encode to
  identical bytes (dual-track invariant).
- All configured backup endpoints are unreachable → the initiator keeps retrying per its reconnect
  policy without busy-looping or panicking.
- A reverse-routed reply where the original routing tags are absent → no reversal is attempted and no
  error is raised.

## Requirements *(mandatory)*

Requirements are grouped by theme. IDs are stable within this feature; baseline IDs from feature 001
are noted in parentheses where a requirement continues one.

### Functional Requirements — Correctness Foundation

- **FR-001**: The session MUST satisfy a ResendRequest for messages sent before a process restart by
  replaying them from durable storage with `PossDupFlag=Y` and the original sending time (FR-G2;
  SC-006).
- **FR-002**: The session's sent-message persistence MUST be the source of truth for resends, so
  resend behaviour is identical before and after a restart for the same store contents.
- **FR-003**: When message persistence is disabled, the engine MUST gap-fill resend ranges instead of
  replaying bodies, and MUST NOT claim to persist messages it did not store.
- **FR-004**: The decoder MUST parse repeating groups (including nested groups and components) into
  structured groups using the data dictionary, not only a flat field list (FR-B1/FR-B9/FR-C3).
- **FR-005**: The engine MUST validate group-level rules — declared count vs entries, delimiter
  presence (`FirstFieldInGroupIsDelimiter`), and member ordering (`ValidateUnorderedGroupFields`) —
  rejecting violations with the corresponding FIX reason.
- **FR-006**: The engine MUST handle garbled frames (bad checksum, incorrect body length) per
  `RejectGarbledMessage`: a session-level Reject when enabled, or a drop without advancing the
  sequence number when disabled (FR-C4/FR-B8).
- **FR-007**: The engine MUST apply inbound message-integrity validation covering at minimum
  BeginString correctness, BodyLength correctness, CheckSum validity, CompID matching (`CheckCompID`),
  SendingTime range, first-three-fields ordering, header/body field ordering
  (`ValidateFieldsOutOfOrder`), and repeated-tag detection — each producing the FIX-correct outcome.
- **FR-008**: The engine MUST maintain the two distinct rejection layers — session Reject (35=3) and
  Business Message Reject (35=j) — routing each failure to the correct layer.
- **FR-009**: The engine MUST support configurable timestamp precision (seconds, milliseconds,
  microseconds, nanoseconds) for emitted timestamps and MUST parse inbound timestamps of any supported
  precision without loss.
- **FR-010**: The engine MUST honour the correctness-affecting session toggles `PersistMessages`,
  `HeartBeatTimeoutMultiplier`, and `TestRequestDelayMultiplier` rather than fixed constants.
- **FR-011**: The engine MUST reverse routing header fields on replies/rejects to messages that
  carried them, including the empty-routing-tags case (Appendix B `ReverseRoute`).
- **FR-012**: Every correctness requirement MUST be demonstrated by an AT scenario or session-level
  test (AT is the release gate), including the previously-uncovered integrity and repeating-group
  scenarios these behaviours enable.

### Functional Requirements — Usability & Configuration

- **FR-013**: The engine MUST construct a complete, runnable configuration — session parameters, store,
  log, schedule, transport, and service wiring — from a parsed settings file, applying DEFAULT values
  with per-SESSION overrides (FR-I1/FR-I3).
- **FR-014**: The engine MUST route each session to initiator or acceptor construction based on its
  `ConnectionType`, and MUST start all configured sessions from a single settings source.
- **FR-015**: Configuration mapping MUST fail with a typed, human-readable error naming the offending
  key and session when a recognised key has an invalid value, a required key is missing, or a
  referenced resource is unusable — and MUST NOT leave a partially-started engine running.
- **FR-016**: The application-integration callbacks MUST return typed outcomes — a logon/admin
  rejection, a do-not-send suppression, and a business rejection carrying a reason code and reference
  tag — replacing opaque string results, and the engine MUST act on each. This is a deliberate
  breaking change to the callback signatures (acceptable pre-1.0); back-compat string variants are NOT
  retained.
- **FR-017**: The engine MUST allow TLS — including client material, minimum version, server name
  (SNI), and mutual-TLS client authentication (`NeedClientAuth`) — to be configured from the settings
  file without code-level TLS construction (FR-F6).
- **FR-018**: The engine MUST perform scheduled-reset semantics — disconnect, reset sequence numbers,
  clear the store, reconnect/reset at the next window — at schedule boundaries, skipping the reset when
  the session is non-stop, and MUST support weekly `StartDay`/`EndDay` windows in addition to daily
  times (FR-E3/FR-E1).
- **FR-019**: The engine MUST apply the full set of configured socket options to the underlying
  connection, and MUST support numbered backup connect endpoints that an initiator rotates through on
  reconnect.

### Functional Requirements — Dual-Track Typed Messages (Principle IV)

- **FR-020**: The build MUST generate strongly-typed per-version message, field, group, and component
  structs (with typed accessors and field-value enumerations) for **every message defined in the
  bundled dictionary subsets**, from the normalized dictionary source (FR-C2).
- **FR-021**: A generated typed message MUST encode/decode byte-identically with the generic codec
  path, and the dual-track hash MUST continue to prove codegen and runtime dictionary share one source.
- **FR-022**: The engine MUST provide a MessageCracker that dispatches an incoming message to a typed
  handler by MsgType/version, replacing the current empty-shell trait.

### Functional Requirements — Observability & Storage/Logging (Principle I + parity)

- **FR-023**: The engine MUST export metrics signals — session state, next sender/target sequence
  numbers, messages sent/received, and reconnect count — through the `metrics` facade only (no bundled
  exporter or HTTP scrape endpoint; operators attach their own), complementing the existing `Monitor`
  (FR-L1).
- **FR-024**: The SQL store and SQL log MUST implement and test PostgreSQL, MySQL, and SQLite (via
  sqlx behind the `sql` feature), with configurable table names and connection-pool settings as
  registered.
- **FR-025**: The cached file store MUST maintain an in-memory cache with a configurable max size and
  an fsync (`FileStoreSync`) toggle, rather than delegating directly to the file store.
- **FR-026**: The logging implementations MUST honour the registered output switches (heartbeat
  filtering, millisecond inclusion, event/incoming/outgoing visibility, session-ID prefixing).
- **FR-027**: Configuration keys consumed by this feature that move from "recognised" to "implemented"
  MUST update their registered stance so the Appendix A coverage record stays accurate (SC-004).
- **FR-028**: The breaking change to the Application callback signatures (FR-016) MUST be accompanied by
  migration documentation (a `MIGRATION.md` describing the `Result<(), String>` → typed-outcome change)
  and a semantic-version bump, per Constitution Principle I (stable, documented public API).

### Key Entities

- **Engine Configuration**: The fully-resolved in-memory configuration from a settings file — the set
  of session configurations plus their store, log, schedule, transport, and security settings, with
  DEFAULT/per-SESSION precedence applied.
- **Persistent Sent-Message Record**: The durable record of a sent message, keyed by sequence number,
  sufficient to replay it byte-for-byte as a PossDup after a restart.
- **Repeating Group**: A dictionary-defined, ordered, counted collection of field sets within a
  message, possibly nested, with a designated delimiter field.
- **Typed Message**: A generated, strongly-typed representation of a per-version message with typed
  field accessors and field-value enumerations, equivalent on the wire to its generic form.
- **Rejection Outcome**: A typed result of validating/handling a message — session reject, business
  reject, or do-not-send — carrying a reason code and (where applicable) a reference tag.
- **Schedule Window**: The active period during which a session may be logged on (daily and/or weekly),
  with boundary behaviour (reset/reconnect or non-stop).
- **Metrics Signal**: An exported, queryable measurement of session state, sequence numbers, message
  counts, or connection health.
- **Connect Endpoint Set**: The ordered set of host/port targets an initiator may use, enabling
  failover on reconnect.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator can bring up a working initiator or acceptor session entirely from a `.cfg`
  file with zero code changes, completing a logon-to-heartbeat exchange.
- **SC-002**: After an unplanned restart against the same store, 100% of messages within a
  ResendRequest's persisted range are replayed with identical content and `PossDupFlag=Y`.
- **SC-003**: 100% of the targeted repeating-group AT scenarios (correct, wrong-count,
  missing-delimiter, out-of-order, nested, zero-count) reach their FIX-correct outcome.
- **SC-004**: 100% of the targeted inbound message-integrity AT scenarios (bad checksum, wrong body
  length, wrong BeginString, CompID mismatch, sending-time out of range, field-order violations,
  repeated tag, garbled-with-RejectGarbledMessage, reverse-route) reach their FIX-correct outcome.
- **SC-005**: Emitted timestamps can be produced at second/millisecond/microsecond/nanosecond
  precision by configuration, with no precision loss on round-trip.
- **SC-006**: An integrator can, through typed callback outcomes alone, cause the engine to refuse a
  logon, suppress an outbound message, and emit a Business Message Reject with a chosen reason and
  reference tag.
- **SC-007**: A developer can build, encode/decode, and crack a message via the generated typed API,
  and the dual-track hash confirms a single source — satisfying Principle IV.
- **SC-008**: A TLS (and mutual-TLS) session can be established using only configuration-file settings,
  and an un-credentialed client is refused when client authentication is required.
- **SC-009**: A scheduled session performs disconnect → reset → clear-store at the window boundary
  (daily and weekly windows) and resumes at the next window without manual intervention; a non-stop
  session performs no scheduled reset.
- **SC-010**: Exported metrics reflect real session state, sequence numbers, message counts, and
  reconnect count during a logon-traffic-reconnect cycle — satisfying Principle I observability.
- **SC-011**: Configured socket options are applied, and an initiator fails over to a configured backup
  endpoint when the primary is unreachable.
- **SC-012**: The SQL store/log operates equivalently across the supported databases, and the cached
  file store caches/flushes per configuration and survives restart.
- **SC-013**: The full workspace gate (format, lints with warnings-as-errors, all tests, AT conformance
  across targeted versions) remains green after every internal stage.
- **SC-014**: Every configuration key implemented by this feature is recorded as "implemented" in the
  Appendix A coverage record, keeping that record accurate.

## Assumptions

- **Settings model**: The settings file follows the QuickFIX convention of `[DEFAULT]` plus one or more
  `[SESSION]` sections, with per-session overrides; the existing parser output (key→value map per
  section) is the input to configuration mapping.
- **Targeted FIX versions**: Correctness scenarios are exercised across the versions the AT suite
  already targets (FIX.4.2 and FIX.4.4); broadening to all versions remains tracked under baseline
  FR-M3 and is not a gate for this feature.
- **Typed codegen depth**: Codegen generates typed structs for **every** message/group/component in
  the bundled dictionary subsets (per Clarifications); coverage therefore scales with those subsets,
  and no third-party dictionary data is copied (Principle III).
- **Timestamp default**: Millisecond precision remains the default (QuickFIX/J parity); other
  precisions are opt-in.
- **TLS material**: Key/cert/CA are supplied as file paths in configuration (inline PEM-bytes is a
  QuickFIX/Go-only convenience and stays out of scope).
- **SQL backends**: PostgreSQL, MySQL, and SQLite are all implemented and tested via sqlx (per
  Clarifications); database drivers outside that family are out of scope. CI gains Postgres/MySQL
  service containers; tests requiring an external database are gated on its availability.
- **Metrics facade**: Observability emits through the `metrics` facade only (per Clarifications); no
  exporter (e.g. a Prometheus HTTP endpoint) is bundled — operators attach their own.
- **Verification discipline**: Correctness items are gated by AT/session tests; the AT scenarios these
  behaviours unblock are authored as part of this feature's "done".
- **Delivery**: One pass, internally staged with per-stage checkpoints (mirroring feature 001); the AT
  suite is the release gate.

## Out of Scope (Deferred)

Only the **QuickFIX/Go-only** extras (beyond the constitution's QuickFIX/J parity target) are excluded:

- Inline PEM-bytes TLS material (`SocketPrivateKeyBytes`/`CertificateBytes`/`CABytes`).
- TCP PROXY protocol support (`UseTCPProxy`, HAProxy/ELB).
- MongoDB store/log backends.
- `DynamicQualifier` dynamic session qualifiers.
- `InChanCapacity` bounded inbound buffering, `ConnectionValidator`/`NewListenerCallback` acceptor
  hooks, and other QuickFIX/Go-only runtime knobs.
- `ResetSeqTime` mid-connection scheduled sequence reset and `HeartBtIntOverride` (QuickFIX/Go-only).
- A standalone `generate-fix` CLI (codegen is delivered as part of the build, not a separate tool).

## Dependencies

- Builds on feature `001-fix-engine-parity` and its constitution (`.specify/memory/constitution.md`,
  v2.0.0).
- Repeating-group validation toggles (FR-005) depend on dictionary-driven group parsing (FR-004).
- Typed codegen (FR-020) depends on the dictionary group/component model and benefits from group
  parsing (FR-004); the MessageCracker (FR-022) depends on the generated types (FR-020).
- Config-driven TLS (FR-017), socket options/failover (FR-019), and scheduled windows (FR-018) depend
  on the configuration-mapping layer (FR-013).
- The Appendix A coverage record and the AT conformance gate from feature 001 are the verification
  surfaces for SC-004/SC-013/SC-014.
