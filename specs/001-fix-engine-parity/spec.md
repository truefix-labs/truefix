# Feature Specification: TrueFix — Full QuickFIX/J-Parity Production FIX Engine

**Feature Branch**: `001-fix-engine-parity`

**Created**: 2026-06-29

**Status**: Draft

**Input**: User description: "构建 TrueFix —— 一台功能完整的、对标 QuickFIX/J 的生产级 FIX 引擎。本规格覆盖整台引擎的全部功能,不是某个里程碑。"

> **Scope note**: This spec describes the **entire engine** at feature-parity with QuickFIX/J, not a
> single milestone. Per the project constitution (v2.0.0), the delivery target is **one-pass full
> parity**, completeness is **inventory-based** (Principle VII), protocol correctness verified by the
> **ported Acceptance Test (AT) suite** is the highest acceptance item (Principles II & V), and the
> reference repos `thrdpty/quickfixj` and `thrdpty/quickfix` were read **only to inventory behavior
> and configuration surface — no source was copied** (Principle III). The two baseline inventories
> required by the user (full config-key set, full AT-case set) are captured in the appendices below
> and were extracted directly from the reference codebases, not from memory.

## Clarifications

### Session 2026-06-29

- Q: Concurrency & public API model (async vs sync vs both)? → A: Async-first core — the public API
  is async (`async` Application callbacks) driven by a single async runtime; a blocking convenience
  layer MAY be layered on later without destabilizing the async core API.
- Q: v1 scope for heavy storage/log backends? → A: All required except the Sleepycat/JE-equivalent KV
  store — memory/file/cached-file/no-op stores, screen/file/composite logs, and the SQL-backed
  (JDBC-equivalent) store+log are in v1 scope; the Sleepycat/JE KV store is deferred-with-reason
  (its Appendix A keys documented as unsupported under FR-I2).
- Q: AT conformance version-coverage gate for release? → A: All targeted versions
  (fix40/41/42/43/44/50/50SP1/50SP2/FIXT 1.1 + fixLatest) are the hard release gate; any individual
  scenario deferral must be explicitly listed with a reason (per FR-M2).
- Q: Performance acceptance for v1? → A: No numeric performance gate — correctness-first. Reproducible
  benchmarks (codec throughput, session round-trip latency) are tracked for visibility and regression
  detection only; no specific number is a release blocker.
- Q: Provenance of the bundled per-version data dictionaries? → A: Derive from the FIX Trading
  Community's official machine-readable specs (FIX Orchestra / Repository), transformed at build time
  into TrueFix's own normalized dictionary format (the single source of truth feeding both codegen and
  runtime validation). QuickFIX/J's XML dictionaries MUST NOT be copied (Constitution Principle III).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Establish and maintain a FIX session (Priority: P1) 🎯 MVP

A buy-side initiator and a sell-side acceptor connect over a socket, complete the FIX Logon
handshake, exchange heartbeats to keep the session alive, respond to TestRequest, and perform an
orderly Logout — for any supported FIX version.

**Why this priority**: Without a live, standards-correct session there is no FIX engine. This is the
minimal slice that delivers value and is independently demonstrable.

**Independent Test**: Start a local acceptor and a local initiator (two processes), observe Logon
exchange, sustained heartbeats over multiple intervals, a successful TestRequest/Heartbeat round
trip, and a clean bilateral Logout — on at least FIX 4.2 and FIX 4.4/FIXT 1.1.

**Acceptance Scenarios**:

1. **Given** an acceptor listening and an initiator configured to it, **When** the initiator connects,
   **Then** both sides exchange Logon with matching HeartBtInt and reach a logged-on state.
2. **Given** a logged-on session with no app traffic, **When** one HeartBtInt elapses, **Then** a
   Heartbeat is sent; **When** ~1.2×HeartBtInt elapses without inbound data, **Then** a TestRequest is
   sent and a Heartbeat with the matching TestReqID is expected.
3. **Given** a logged-on session, **When** either side sends Logout, **Then** the peer replies Logout
   and both disconnect without error.

---

### User Story 2 - Correctly encode and decode FIX messages (Priority: P1) 🎯 MVP

The engine parses raw FIX wire bytes into a typed message model and serializes typed messages back to
the wire, computing BodyLength and CheckSum automatically and preserving header/body/trailer ordering.

**Why this priority**: Session logic and conformance are impossible without a byte-exact codec; it is
co-MVP with US1.

**Independent Test**: Round-trip real messages (NewOrderSingle, ExecutionReport, Logon, Heartbeat,
ResendRequest) and verify BodyLength/CheckSum byte-for-byte against QuickFIX/J output; feed malformed
and truncated input and verify a typed error (never a panic).

**Acceptance Scenarios**:

1. **Given** a valid raw FIX message, **When** decoded then re-encoded, **Then** the output bytes are
   identical (round-trip) and BodyLength(9)/CheckSum(10) match the reference engine exactly.
2. **Given** a message with nested repeating groups, **When** decoded, **Then** group structure and
   delimiter ordering are preserved and re-encoding reproduces the original order.
3. **Given** truncated, garbled, or non-numeric-where-numeric-expected input, **When** decoded,
   **Then** a typed error is returned and no panic/unwrap/expect path is reachable.

---

### User Story 3 - Sequence integrity, recovery, and resend (Priority: P1)

Sessions detect sequence gaps and recover them via ResendRequest / SequenceReset (GapFill and Reset),
handle PossDup/PossResend/OrigSendingTime, support ResetSeqNumFlag, and persist sequence numbers and
outbound messages so recovery survives reconnects.

**Why this priority**: Sequence correctness is the heart of FIX session semantics and the bulk of the
AT suite; it is the conformance gate.

**Independent Test**: Drive scripted scenarios where inbound MsgSeqNum is too high (triggers
ResendRequest + queue), too low without PossDup (triggers disconnect), and a SequenceReset-GapFill is
received; verify against the ported AT definitions.

**Acceptance Scenarios**:

1. **Given** a logged-on session, **When** a message arrives with MsgSeqNum higher than expected,
   **Then** a ResendRequest is issued and subsequent in-order messages are queued until the gap fills.
2. **Given** a ResendRequest is received, **When** replaying, **Then** persisted app messages are
   resent with PossDupFlag=Y and original OrigSendingTime, and admin messages are replaced by
   SequenceReset-GapFill.
3. **Given** a message with MsgSeqNum lower than expected and PossDupFlag not set, **Then** the session
   logs the error and disconnects per spec.

---

### User Story 4 - Acceptor as a first-class citizen (Priority: P1)

A single acceptor process listens for and serves multiple sessions concurrently, including **dynamic
sessions** (peers not pre-configured, accepted via an acceptor template), with per-session settings
and optional remote-address allow-listing.

**Why this priority**: First-class acceptor/sell-side support is a core differentiator of TrueFix
(Constitution Principle VI); it must not be a second-class path.

**Independent Test**: Configure one acceptor with two static sessions plus a dynamic-session template;
connect three initiators (two known, one previously unknown but matching the template); verify all
three reach logged-on state independently.

**Acceptance Scenarios**:

1. **Given** an acceptor with multiple `[SESSION]` blocks, **When** several initiators connect, **Then**
   each is routed to its correct session by SessionID and runs independently.
2. **Given** an acceptor with a dynamic-session template, **When** an unconfigured but template-matching
   initiator logs on, **Then** a session is created dynamically and served.
3. **Given** `AllowedRemoteAddresses` is set, **When** a peer connects from a disallowed address,
   **Then** the connection is refused.

---

### User Story 5 - Data dictionary validation, dual-track (Priority: P2)

The engine validates messages at runtime against a DataDictionary (field/type/required/group/version
rules) while also offering build-time codegen of strongly-typed messages — both present, per
Constitution Principle IV. FIXT 1.1 uses separate transport and application dictionaries.

**Why this priority**: Validation correctness underpins the rejection AT cases; codegen ergonomics are
a parity requirement but can follow the runtime path.

**Independent Test**: Load each bundled dictionary; validate conforming and non-conforming messages and
confirm the correct reject/accept outcome for each validation toggle; under FIXT 1.1 confirm transport
vs application dictionary separation and DefaultApplVerID resolution.

**Acceptance Scenarios**:

1. **Given** `UseDataDictionary=Y`, **When** a message violates a dictionary rule (unknown field,
   missing required field, wrong type, bad enum, bad group count), **Then** a session-level Reject (or
   Business Message Reject where applicable) is emitted with the spec-correct reason.
2. **Given** validation toggles (e.g. `ValidateFieldsOutOfOrder`, `ValidateUserDefinedFields`,
   `AllowUnknownMsgFields`, `CheckLatency`/`MaxLatency`), **When** each is set on/off, **Then** the
   engine's accept/reject behavior changes exactly as the corresponding QuickFIX/J behavior.
3. **Given** a FIXT 1.1 session with `TransportDataDictionary` and `AppDataDictionary` configured,
   **When** an application message arrives, **Then** it is validated against the application dictionary
   resolved from DefaultApplVerID/ApplVerID, and session messages against the transport dictionary.

---

### User Story 6 - Session scheduling and reset semantics (Priority: P2)

Sessions start/stop on a schedule (daily StartTime/EndTime, weekly StartDay/EndDay, weekday lists,
time zones) or run 24×7 (NonStopSession). A scheduled reset means: disconnect, reset sequence numbers,
clear stored messages, and reconnect when in-session again.

**Why this priority**: Required for real-world sell-side deployments and matches QuickFIX/J semantics.

**Independent Test**: Configure a short schedule window and a NonStop session; verify the scheduled
session disconnects/zeros/clears at the boundary and the NonStop session never auto-resets on schedule.

**Acceptance Scenarios**:

1. **Given** a daily-scheduled session, **When** EndTime is reached, **Then** the session logs out,
   disconnects, and at the next StartTime reconnects with reset sequence numbers and cleared store.
2. **Given** `NonStopSession=Y`, **When** time passes any wall-clock boundary, **Then** no
   schedule-driven reset occurs.

---

### User Story 7 - Pluggable message stores and logs (Priority: P2)

Sequence numbers and outbound messages persist through a selectable store (in-memory, file, cached
file, SQL-backed, no-op); session/message/event logging routes through selectable logs (screen, file,
SQL-backed, composite) with message vs event streams separated.

**Why this priority**: Persistence enables resend across restarts (depends on US3); logging is required
for production observability (Constitution Principle I).

**Independent Test**: Run a session, restart the process, and confirm sequence numbers and resend
capability survive with the file/cached-file store; confirm each log backend records message and event
streams separately.

**Acceptance Scenarios**:

1. **Given** a persistent store, **When** the process restarts mid-session, **Then** sequence numbers
   and the ability to resend previously sent messages are recovered.
2. **Given** a composite log over screen+file, **When** messages and events occur, **Then** both
   backends receive them and incoming/outgoing/event streams are distinguishable.

---

### User Story 8 - Configuration via SessionSettings (Priority: P2)

Operators configure the engine with a QuickFIX-style settings source (`[DEFAULT]` + `[SESSION]`
sections), with `${var}` interpolation, or programmatically. **Every** config key in the parity
baseline (Appendix A) is recognized, or explicitly documented as unsupported with a reason.

**Why this priority**: Configuration is the primary operator interface and the user named the full
key-set as a core deliverable.

**Independent Test**: Load a `.cfg` exercising representative keys from every group in Appendix A;
verify each is applied; verify any unsupported key is reported per the documented stance; verify
`${var}` interpolation resolves.

**Acceptance Scenarios**:

1. **Given** a settings file with `[DEFAULT]` and multiple `[SESSION]` blocks, **When** loaded, **Then**
   defaults apply to all sessions and per-session overrides take precedence.
2. **Given** a value containing `${NAME}`, **When** loaded, **Then** the variable is interpolated from
   the defined source.
3. **Given** a key from Appendix A, **When** present, **Then** it is either honored or rejected/ignored
   with a documented, discoverable reason (no silent unknown-key behavior beyond the configured
   `AllowUnknownMsgFields`-style stance).

---

### User Story 9 - Application integration and typed dispatch (Priority: P2)

Integrators implement an Application interface with lifecycle and message callbacks
(onCreate/onLogon/onLogout/toAdmin/fromAdmin/toApp/fromApp) and use a MessageCracker to dispatch
inbound messages to strongly-typed, per-message-type handlers.

**Why this priority**: This is how users actually embed the engine; required for examples and AT
harness.

**Independent Test**: Implement a test Application; verify each callback fires at the right lifecycle
moment and that the cracker routes a NewOrderSingle to its typed handler.

**Acceptance Scenarios**:

1. **Given** a registered Application, **When** a session is created, logs on, exchanges admin and app
   messages, and logs out, **Then** the corresponding callbacks fire in the correct order.
2. **Given** a MessageCracker, **When** a typed message arrives, **Then** it is dispatched to the
   handler for that message type; unhandled types follow the configured fallback.

---

### User Story 10 - Authentication and rejection (Priority: P3)

Sessions support Logon Username/Password and custom authentication via toAdmin/fromAdmin, and emit
session-level Reject and business-level Business Message Reject as the spec requires.

**Independent Test**: Configure Logon credentials; verify a good credential logs on and a bad one is
rejected; trigger a session-level Reject and a Business Message Reject and verify the outbound messages.

**Acceptance Scenarios**:

1. **Given** Logon credentials configured, **When** an initiator presents wrong credentials (validated
   in fromAdmin), **Then** the acceptor rejects/terminates the logon per the application's decision.
2. **Given** an inbound app message that fails business rules, **When** processed, **Then** a Business
   Message Reject is returned; a malformed admin/session-layer message yields a session-level Reject.

---

### User Story 11 - Operational monitoring (Priority: P3)

The engine exposes session state, inbound/outbound sequence numbers, and connection health as
queryable/observable signals and supports basic operational actions (e.g. reset, logout) — the parity
equivalent of QuickFIX/J JMX.

**Independent Test**: While sessions run, query exposed state and confirm it reflects live session
status, sequence numbers, and connectivity; invoke an operational action and confirm its effect.

**Acceptance Scenarios**:

1. **Given** running sessions, **When** monitoring state is queried, **Then** it reports each session's
   logged-on status, next sender/target sequence numbers, and connection health.
2. **Given** an operational reset action, **When** invoked on a session, **Then** sequence numbers reset
   and the action is reflected in monitoring state.

---

### User Story 12 - Acceptance Test (AT) conformance suite passes (Priority: P1) 🚦 GATE

The QuickFIX/J/QuickFIX acceptance-test scenarios (Appendix B) are ported as black-box behavior
contracts and executed against TrueFix; passing them is the highest-priority acceptance item.

**Why this priority**: Per Constitution Principle II/V, AT passage is the objective definition of
protocol correctness and the release gate. It depends on US1–US3, US5.

**Independent Test**: Run the ported AT runner driving scripted client/server message exchanges; every
in-scope scenario in Appendix B must pass for each FIX version it targets.

**Acceptance Scenarios**:

1. **Given** the ported AT definitions, **When** the AT runner executes a scenario, **Then** TrueFix's
   responses (messages sent and disconnect behavior) match the scenario's expected steps exactly.
2. **Given** the full server AT suite per version, **When** run in CI, **Then** all in-scope scenarios
   pass and any intentionally-deferred scenario is explicitly listed with a reason.

---

### User Story 13 - Reference examples (Priority: P3)

Ship runnable examples: an acceptor echo/executor, an initiator client, a multi-session acceptor, and
an order-matching (ordermatch-equivalent) demo.

**Independent Test**: Build and run each example; verify the acceptor examples accept a session and
echo/execute, and the initiator example logs on and sends orders.

**Acceptance Scenarios**:

1. **Given** the executor example running as acceptor, **When** the client example connects and sends a
   NewOrderSingle, **Then** an ExecutionReport is returned.
2. **Given** the ordermatch example, **When** crossing buy/sell orders arrive, **Then** they match and
   ExecutionReports are produced.

---

### Edge Cases

- Truncated message mid-field, missing SOH terminator, or BodyLength mismatch → typed error / garbled
  handling per `RejectGarbledMessage`, never a panic.
- Logon with MsgSeqNum too high → logon completes then ResendRequest is issued (AT 1a).
- Simultaneous ResendRequests from both sides (AT 20).
- Repeating-group count tag present with value 0 (AT 21); missing group delimiter in nested group
  (QFJ934).
- Negative or missing HeartBtInt / missing MsgSeqNum in Logon (QFJ648, QFJ650).
- Invalid checksum / garbled message rejection toggles (QFJ973, QFJ950).
- Sub-second timestamp precision: accept up to picosecond on input, store truncated to nanosecond
  (QFJ873, QFJ921, `TimeStampPrecision`).
- Corrupted store on startup with `ForceResendWhenCorruptedStore`.
- Clock skew beyond `MaxLatency` with `CheckLatency=Y`.
- Unknown/dynamic acceptor session from a disallowed remote address.

## Requirements *(mandatory)*

Requirements are grouped by the user's domains A–N. Each is independently testable. Domain → primary
user story mapping is noted per group.

### A. Protocol versions & dictionaries *(US2, US5)*

- **FR-A1**: System MUST support FIX 4.0, 4.1, 4.2, 4.3, 4.4, 5.0, 5.0SP1, 5.0SP2 and FIXT 1.1 session
  semantics and message sets.
- **FR-A2**: System MUST bundle a data dictionary for each supported version, **derived from the FIX
  Trading Community's official machine-readable specifications (FIX Orchestra / Repository)** and
  transformed into TrueFix's own normalized dictionary format at build time. QuickFIX/J's XML
  dictionary files MUST NOT be copied (Constitution Principle III). This normalized dictionary is the
  single source of truth feeding both codegen and runtime validation (Principle IV).
- **FR-A3**: Under FIXT 1.1, System MUST support a separate **TransportDataDictionary** (session layer)
  and **ApplicationDataDictionary** (application layer), selectable per session.
- **FR-A4**: System MUST resolve the application version via **DefaultApplVerID** (and per-message
  ApplVerID where present) when validating FIXT 1.1 application messages.

### B. Message model & codec *(US2)*

- **FR-B1**: System MUST model Field, FieldMap, Group (including nested groups), and Message with
  distinct header / body / trailer regions.
- **FR-B2**: System MUST provide a MessageFactory (construct messages by type/version) and a
  MessageCracker for typed dispatch (see US9).
- **FR-B3**: System MUST support all FIX field data types and their value converters (int, float,
  decimal, char, boolean, String, UTCTimestamp, UTCTimeOnly, UTCDateOnly, MonthYear, etc.).
- **FR-B4**: System MUST encode/decode using SOH (0x01) field separators.
- **FR-B5**: System MUST automatically compute and verify BodyLength (tag 9) and CheckSum (tag 10);
  computed values MUST match QuickFIX/J byte-for-byte for identical logical messages.
- **FR-B6**: System MUST emit header fields in FIX-required order and handle RawData/length-prefixed
  fields correctly.
- **FR-B7**: System MUST accept UTCTimestamp input up to picosecond resolution and store/represent it
  truncated to nanosecond resolution.
- **FR-B8**: On illegal, garbled, or truncated input, System MUST return a typed error and MUST NOT
  panic/unwrap/expect on the parse path (Constitution Principle I).
- **FR-B9**: System MUST guarantee round-trip fidelity: decode→encode of a valid message reproduces the
  original wire bytes (including field order within regions and group structure).

### C. Data dictionary & validation *(US5)*

- **FR-C1**: System MUST parse dictionary `fields`, `components`, `groups`, and `messages`.
- **FR-C2**: System MUST provide **both** build-time codegen of strongly-typed messages **and** a
  runtime DataDictionary validator over the same single source of dictionary truth (Constitution IV).
- **FR-C3**: System MUST implement every validation toggle in the parity baseline, including at least:
  `UseDataDictionary`, `ValidateFieldsOutOfOrder`, `ValidateFieldsHaveValues`,
  `ValidateUnorderedGroupFields`, `ValidateUserDefinedFields`, `ValidateIncomingMessage`,
  `ValidateSequenceNumbers`, `ValidateChecksum`, `AllowUnknownMsgFields`, `CheckLatency`, `MaxLatency`,
  `CheckCompID`, `FirstFieldInGroupIsDelimiter` (full list in Appendix A, Validation group).
- **FR-C4**: System MUST support `RejectGarbledMessage` handling and distinguish **two rejection
  layers**: (a) dictionary/validation failure vs (b) FIX protocol-level basic-validity failure.
- **FR-C5**: System MUST support User-Defined Fields (UDF) and loading custom / extended dictionaries.

### D. Session layer *(US1, US3, US10)*

- **FR-D1**: System MUST implement the full session state machine (disconnected, logon in progress,
  logged-on, logout in progress, etc.) for initiator and acceptor roles.
- **FR-D2**: SessionID MUST comprise BeginString, SenderCompID, TargetCompID and optional Sender/Target
  Sub/Location IDs and SessionQualifier.
- **FR-D3**: System MUST manage and persist inbound/outbound sequence numbers.
- **FR-D4**: System MUST perform Logon/Logout handshakes and honor `ResetSeqNumFlag` (141).
- **FR-D5**: System MUST support `ResetOnLogon`, `ResetOnLogout`, `ResetOnDisconnect`, and
  `RefreshOnLogon`.
- **FR-D6**: System MUST implement Heartbeat and TestRequest behavior driven by HeartBtInt (with
  `HeartBeatTimeoutMultiplier` / `TestRequestDelayMultiplier` and `DisableHeartBeatCheck`).
- **FR-D7**: System MUST implement ResendRequest including `ResendRequestChunkSize` chunked resend
  (0 = unbounded single request), and gap fill.
- **FR-D8**: System MUST implement SequenceReset in both **GapFill** and **Reset** modes.
- **FR-D9**: System MUST handle PossDupFlag, PossResend, and OrigSendingTime (incl.
  `RequiresOrigSendingTime`, `AllowPosDup`).
- **FR-D10**: For FIX ≥ 4.4, System MUST support `NextExpectedMsgSeqNum` (789) logon synchronization and
  `EnableLastMsgSeqNumProcessed` (369).
- **FR-D11**: System MUST enforce `LogonTimeout` / `LogoutTimeout` and, with `CheckLatency`, `MaxLatency`
  validation of SendingTime.
- **FR-D12**: System MUST handle received higher-than-expected sequence numbers (ResendRequest + queue)
  and lower-than-expected (PossDup check, else disconnect), and recover queued messages in order.

### E. Session scheduling *(US6)*

- **FR-E1**: System MUST support `StartTime`/`EndTime` daily windows, `StartDay`/`EndDay` weekly
  windows, `Weekdays` lists, and `TimeZone`.
- **FR-E2**: System MUST support `NonStopSession` (24×7) with no schedule-driven reset.
- **FR-E3**: A scheduled reset MUST mean: disconnect → reset sequence numbers → clear stored messages →
  reconnect when next in-session.

### F. Transport / connectors *(US1, US4)*

- **FR-F1**: System MUST provide an Initiator (outbound connect) and an Acceptor (inbound listen), each
  a first-class, independently usable connector (Constitution VI).
- **FR-F2**: System MUST support single- and multi-session operation and concurrent sessions.
- **FR-F3**: System MUST support **dynamic sessions** (accept peers not pre-configured) via an acceptor
  template (`AcceptorTemplate`, `DynamicSession`) with optional `AllowedRemoteAddresses`.
- **FR-F4**: Initiator MUST reconnect on disconnect per `ReconnectInterval`.
- **FR-F5**: System MUST support socket options in the parity baseline (keep-alive, TCP nodelay, reuse
  address, linger, buffer sizes, etc.) and `SocketAcceptPort`/`SocketConnectHost`/`SocketConnectPort`
  and related binding keys.
- **FR-F6**: System MUST support SSL/TLS for both initiator and acceptor, including the SSL key-set in
  Appendix A (keystore/truststore, protocols, cipher suites, client auth, SNI).
- **FR-F7**: The public API MUST be **async-first** — Application callbacks are async and all sessions
  are driven by a single async runtime. A synchronous/blocking convenience layer MAY be added later but
  MUST NOT require breaking the async core API (Constitution Principle I: stable public API).

### G. Message stores *(US7)*

- **FR-G1**: System MUST provide MemoryStore, FileStore, CachedFileStore, a SQL-backed store
  (JDBC-equivalent), and a NoopStore, selected via a MessageStoreFactory. The Sleepycat/JE-equivalent
  embedded-KV store is **deferred for v1** and documented as unsupported-with-reason under FR-I2.
- **FR-G2**: Stores MUST persist sequence numbers and outbound messages sufficient to satisfy resend.

### H. Logging *(US7)*

- **FR-H1**: System MUST provide ScreenLog, FileLog, a SQL-backed log (JDBC-equivalent), a unified
  logging facade, and a CompositeLog, selected via a LogFactory.
- **FR-H2**: System MUST separate message logging from event logging.

### I. Configuration *(US8)*

- **FR-I1**: System MUST load a SessionSettings source in `.cfg` format with `[DEFAULT]` and `[SESSION]`
  sections (defaults inherited, per-session overrides win).
- **FR-I2**: System MUST recognize **every** configuration key in Appendix A, or explicitly document the
  key as unsupported with a reason (Constitution VII; total-acceptance criterion).
- **FR-I3**: System MUST support `${name}` variable interpolation and programmatic configuration.

### J. Application interface *(US9)*

- **FR-J1**: System MUST expose an Application interface with `onCreate`, `onLogon`, `onLogout`,
  `toAdmin`, `fromAdmin`, `toApp`, `fromApp` callbacks at the correct lifecycle points.
- **FR-J2**: System MUST provide a MessageCracker for typed, per-message-type dispatch with a
  configurable fallback for unhandled types.

### K. Authentication & rejection *(US10)*

- **FR-K1**: System MUST support Logon Username/Password and custom authentication via toAdmin/fromAdmin.
- **FR-K2**: System MUST emit session-level Reject and business-level Business Message Reject per spec,
  with `RejectInvalidMessage` / `RejectMessageOnUnhandledException` behavior toggles.

### L. Monitoring *(US11)*

- **FR-L1**: System MUST expose session state, sequence numbers, and connection health as structured,
  queryable/observable signals (Constitution Principle I), the parity equivalent of QuickFIX/J JMX.
- **FR-L2**: System MUST support basic operational actions (e.g. reset sequence numbers, force logout)
  reflected in monitoring state.

### M. Acceptance tests *(US12 — highest verification priority)*

- **FR-M1**: System MUST port the AT scenarios enumerated in Appendix B as black-box behavior contracts
  (independently authored fixtures, no copied source — Constitution III) and execute them in CI.
- **FR-M2**: Every in-scope AT scenario MUST pass for each FIX version it targets; any deferred scenario
  MUST be explicitly listed with a reason.
- **FR-M3**: The release gate is **all targeted FIX versions** passing their AT scenarios; no version is
  "best-effort." Per-scenario deferrals are the only permitted exception and MUST be enumerated with
  justification (FR-M2).
  - **AT scenario availability mapping**: the QuickFIX/J AT corpus provides version-specific scenario
    sets only for `fix40/41/42/43/44/50` and `fixLatest` (Appendix B). Accordingly, the AT gate runs the
    `fix50` set for FIX 5.0 and the `fixLatest` set for the 5.0SP1/5.0SP2/FIXT 1.1 application layer.
    FIX 5.0SP1 and 5.0SP2 therefore have **no distinct AT scenario set**; their conformance is gated by
    (a) the `fixLatest` AT set plus (b) per-version codec round-trip and dictionary validation (FR-A1,
    SC-002). This avoids claiming AT coverage for versions the corpus does not enumerate.

### N. Examples *(US13)*

- **FR-N1**: System MUST ship runnable examples: an acceptor echo/executor (`executor`), an initiator
  client (`banzai`-equivalent), a multi-session acceptor (`multi_acceptor`), and an ordermatch-equivalent
  demo (`ordermatch`).

### Key Entities

- **Message / Field / FieldMap / Group**: the typed in-memory representation of a FIX message and its
  components, including nested repeating groups and header/body/trailer regions.
- **DataDictionary**: parsed field/component/group/message definitions plus validation rules for a
  version; single source feeding both codegen and runtime validation.
- **SessionID**: BeginString + Sender/Target (Comp/Sub/Location) IDs + optional Qualifier.
- **Session**: state machine holding sequence numbers, schedule, store, log, settings, and peer link.
- **SessionSettings**: parsed configuration (DEFAULT + per-session) keyed by Appendix A keys.
- **MessageStore**: persistence of sequence numbers and outbound messages.
- **Log**: message + event sinks.
- **Initiator / Acceptor**: connectors managing sockets and session lifecycle.
- **Application**: integrator callback surface.
- **AT Scenario**: a scripted client/server step sequence (Appendix B) with expected messages and
  disconnect outcomes.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every domain (A–N) has corresponding passing tests, and the full in-scope AT suite
  (Appendix B) passes in CI for every targeted FIX version.
- **SC-002**: For the canonical message set (NewOrderSingle, ExecutionReport, Logon, Heartbeat,
  ResendRequest, SequenceReset, Reject), decode→encode round-trips are byte-identical and
  BodyLength/CheckSum match QuickFIX/J output exactly (100% of the canonical set).
- **SC-003**: Initiator and Acceptor each run independently, with multiple static sessions and at least
  one dynamic session, in a two-process integration test.
- **SC-004**: 100% of the Appendix A config keys are either implemented or explicitly documented as
  unsupported with a stated reason (no key is silently unrecognized).
- **SC-005**: No reachable `panic!`/`unwrap`/`expect` exists on the codec, session state machine, I/O,
  or timer paths (verified by lint/audit); all recoverable errors are typed.
- **SC-006**: A session survives a process restart with a persistent store and can still satisfy a
  ResendRequest for messages sent before the restart.
- **SC-007**: Session state, sequence numbers, and connection health are observable for every running
  session via the monitoring surface.
- **SC-008**: Reproducible benchmarks exist for codec throughput and session round-trip latency and run
  in CI for regression visibility; no specific numeric threshold is a release blocker for v1
  (performance is observed, not gated — correctness-first per constitution).

## Assumptions

- "JDBC-equivalent" store/log means a SQL-database-backed implementation through a Rust database
  abstraction; exact driver/runtime choices are deferred to planning (implementation detail). Parity is
  measured by capability (persist sequence numbers/messages/logs in a relational DB), not by using JDBC.
- "JMX-equivalent" monitoring means an idiomatic structured observability + control surface; the exact
  transport (metrics exporter, admin API) is a planning decision, not a spec requirement.
- BerkeleyDB/Sleepycat JE store keys appear in QuickFIX/J; the Sleepycat-style embedded-KV store is
  **deferred for v1** (clarified 2026-06-29): its config keys are catalogued in Appendix A and marked
  "unsupported with reason" under FR-I2. The SQL-backed (JDBC-equivalent) store/log remains in v1 scope.
- "FIX Latest" AT variants are treated as the newest FIX 5.0SP2/FIXT application layer for porting
  purposes.
- Dictionary content is derived from the FIX Trading Community's official machine-readable specs (FIX
  Orchestra / Repository) and normalized into TrueFix's own format at build time (clarified
  2026-06-29), never copied from reference-implementation data files (Constitution III/IV).
- The constitution (v2.0.0) governs all trade-offs; on conflict, protocol correctness > production
  readiness > feature count, with License discipline as an absolute precondition.

---

## Appendix A — Configuration Key Full Set (Parity Baseline)

> Extracted from `thrdpty/quickfixj` (`Session.java`, `SessionSettings.java`, transport/store/log/SSL
> setting classes) and cross-checked against `thrdpty/quickfix`. This is the FR-I2 acceptance baseline:
> every key MUST be implemented or documented as unsupported-with-reason. Behavior catalogued only — no
> source copied.

**Session identity & type**: `BeginString`, `SenderCompID`, `SenderSubID`, `SenderLocationID`,
`TargetCompID`, `TargetSubID`, `TargetLocationID`, `SessionQualifier`, `ConnectionType`, `Description`,
`DefaultApplVerID`.

**Dictionary & validation**: `UseDataDictionary`, `DataDictionary`, `AppDataDictionary`,
`TransportDataDictionary`, `ValidateFieldsOutOfOrder`, `ValidateFieldsHaveValues`,
`ValidateUnorderedGroupFields`, `ValidateUserDefinedFields`, `ValidateIncomingMessage`,
`ValidateSequenceNumbers`, `ValidateChecksum`, `AllowUnknownMsgFields`, `CheckLatency`, `MaxLatency`,
`CheckCompID`, `RejectGarbledMessage`, `RejectInvalidMessage`, `RejectMessageOnUnhandledException`,
`FirstFieldInGroupIsDelimiter`.

**Session behavior**: `HeartBtInt`, `HeartBeatTimeoutMultiplier`, `DisableHeartBeatCheck`,
`TestRequestDelayMultiplier`, `LogonTimeout`, `LogoutTimeout`, `LogonTag`, `ResetOnLogon`,
`ResetOnLogout`, `ResetOnDisconnect`, `ResetOnError`, `RefreshOnLogon`, `PersistMessages`,
`ResendRequestChunkSize`, `SendRedundantResendRequests`, `ClosedResendInterval` (UseClosedResendInterval),
`EnableLastMsgSeqNumProcessed`, `EnableNextExpectedMsgSeqNum`, `RequiresOrigSendingTime`, `AllowPosDup`
(AllowPosDupMessages), `ForceResendWhenCorruptedStore`, `DisconnectOnError`, `TimeStampPrecision`,
`MaxScheduledWriteRequests`, `ContinueInitializationOnError`, `LogMessageWhenSessionNotFound`.

**Scheduling**: `StartTime`, `EndTime`, `StartDay`, `EndDay`, `Weekdays`, `TimeZone`, `NonStopSession`.

**Acceptor / dynamic session**: `SocketAcceptAddress`, `SocketAcceptPort`, `SocketAcceptProtocol`,
`AcceptorTemplate`, `DynamicSession`, `AllowedRemoteAddresses`.

**Initiator**: `SocketConnectHost`, `SocketConnectPort`, `SocketConnectProtocol`, `SocketConnectTimeout`,
`SocketLocalHost`, `SocketLocalPort`, `ReconnectInterval`.

**Socket options**: `SocketKeepAlive`, `SocketTcpNoDelay`, `SocketReuseAddress`, `SocketLinger`,
`SocketOobInline`, `SocketReceiveBufferSize`, `SocketSendBufferSize`, `SocketTrafficClass`,
`SocketSynchronousWrites`, `SocketSynchronousWriteTimeout`.

**SSL/TLS**: `SocketUseSSL`, `EnabledProtocols`, `CipherSuites`, `KeyStoreType`,
`KeyManagerFactoryAlgorithm`, `TrustManagerFactoryAlgorithm`, `TrustStoreType`, `SocketKeyStore`,
`SocketKeyStorePassword`, `SocketTrustStore`, `SocketTrustStorePassword`, `NeedClientAuth`,
`EndpointIdentificationAlgorithm`, `UseSNI`, `SNIHostName`.

**Proxy**: `ProxyType`, `ProxyVersion`, `ProxyHost`, `ProxyPort`, `ProxyUser`, `ProxyPassword`,
`ProxyDomain`, `ProxyWorkstation`.

**File store**: `FileStorePath`, `FileStoreSync`, `FileStoreMaxCachedMsgs`.

**File log**: `FileLogPath`, `FileLogHeartbeats`, `FileIncludeMilliseconds`,
`FileIncludeTimeStampForMessages`.

**Screen log**: `ScreenLogShowEvents`, `ScreenLogShowHeartBeats`, `ScreenLogShowIncoming`,
`ScreenLogShowOutgoing`, `ScreenIncludeMilliseconds`.

**Facade (SLF4J-equivalent) log**: `SLF4JLogEventCategory`, `SLF4JLogErrorEventCategory`,
`SLF4JLogIncomingMessageCategory`, `SLF4JLogOutgoingMessageCategory`, `SLF4JLogPrependSessionID`,
`SLF4JLogHeartbeats`.

**SQL (JDBC-equivalent) store/log**: `JdbcDriver`, `JdbcURL`, `JdbcUser`, `JdbcPassword`,
`JdbcDataSourceName`, `JdbcStoreMessagesTableName`, `JdbcStoreSessionsTableName`, `JdbcLogIncomingTable`,
`JdbcLogOutgoingTable`, `JdbcLogEventTable`, `JdbcLogHeartBeats`, `JdbcMaxActiveConnection`,
`JdbcMaxConnectionLifeTime`, `JdbcMinIdleConnection`, `JdbcConnectionTimeout`, `JdbcConnectionIdleTimeout`,
`JdbcConnectionKeepaliveTime`, `JdbcConnectionTestQuery`, `JdbcSessionIdDefaultPropertyValue`,
`JndiContextFactory`, `JndiProviderURL`.

**Embedded-KV (Sleepycat/JE-equivalent) store — optional parity**: `SleepycatDatabaseDir`,
`SleepycatMessageDbName`, `SleepycatSequenceDbName`.

> Total catalogued keys: ~150. Each maps to a domain requirement above; FR-I2 governs the
> implemented-or-documented stance for every one.

## Appendix B — Acceptance Test (AT) Case Full Set (Parity Baseline)

> Enumerated from `thrdpty/quickfixj` acceptance-test definitions
> (`quickfixj-core/.../test/acceptance/definitions/`). The **server** suite (the classic QuickFIX AT
> scenarios) carries the bulk of conformance and is replicated per FIX version. Ported as black-box
> behavior contracts (Constitution III). FR-M governs passage.

**Server AT scenarios (73 distinct, replicated across fix40/41/42/43/44/50/fixLatest, ~60–67 per
version)**:

`1a_ValidLogonWithCorrectMsgSeqNum`, `1a_ValidLogonMsgSeqNumTooHigh`, `1b_DuplicateIdentity`,
`1c_InvalidSenderCompID`, `1c_InvalidTargetCompID`, `1d_InvalidLogonBadSendingTime`,
`1d_InvalidLogonLengthInvalid`, `1d_InvalidLogonNoDefaultApplVerID`, `1d_InvalidLogonWrongBeginString`,
`1e_NotLogonMessage`, `2a_MsgSeqNumCorrect`, `2b_MsgSeqNumTooHigh`, `2c_MsgSeqNumTooLow`,
`2d_GarbledMessage`, `2e_PossDupAlreadyReceived`, `2e_PossDupNotReceived`,
`2f_PossDupOrigSendingTimeTooHigh`, `2g_PossDupNoOrigSendingTime`, `2i_BeginStringValueUnexpected`,
`2k_CompIDDoesNotMatchProfile`, `2m_BodyLengthValueNotCorrect`, `2o_SendingTimeValueOutOfRange`,
`2q_MsgTypeNotValid`, `2r_UnregisteredMsgType`, `2t_FirstThreeFieldsOutOfOrder`, `3b_InvalidChecksum`,
`3c_GarbledMessage`, `4a_NoDataSentDuringHeartBtInt`, `4b_ReceivedTestRequest`, `6_SendTestRequest`,
`7_ReceiveRejectMessage`, `8_AdminAndApplicationMessages`, `8_AdminAndApplicationMessages-FIX50SP2`,
`8_OnlyAdminMessages`, `8_OnlyApplicationMessages`, `10_MsgSeqNumEqual`, `10_MsgSeqNumGreater`,
`10_MsgSeqNumLess`, `11a_NewSeqNoGreater`, `11b_NewSeqNoEqual`, `11c_NewSeqNoLess`,
`13b_UnsolicitedLogoutMessage`, `14a_BadField`, `14b_RequiredFieldMissing`,
`14c_TagNotDefinedForMsgType`, `14d_TagSpecifiedWithoutValue`, `14e_IncorrectEnumValue`,
`14f_IncorrectDataFormat`, `14g_HeaderBodyTrailerFieldsOutOfOrder`, `14h_RepeatedTag`,
`14i_RepeatingGroupCountNotEqual`, `14j_OutOfOrderRepeatingGroupMembers`,
`15_HeaderAndBodyFieldsOrderedDifferently`, `19a_PossResendMessageThatHasAlreadyBeenSent`,
`19b_PossResendMessageThatHasNotBeenSent`, `20_SimultaneousResendRequest`,
`21_RepeatingGroupSpecifierWithValueOfZero`, `AlreadyLoggedOn`,
`bugfix_QFJ634_ResendRequestAndSequenceReset`, `LogonUnknownDefaultApplVerID`,
`MinQty40`/`41`/`42`/`43`/`44`/`50`, `QFJ648_NegativeHeartBtInt`, `QFJ650_MissingMsgSeqNum`,
`QFJ934_MissingDelimiterNestedRepeatingGroup`, `RejectResentMessage`, `ReverseRoute`,
`ReverseRouteWithEmptyRoutingTags`, `SessionReset`.

**Special-category AT suites (per applicable versions)**:

- *nextExpectedMsgSeqNum* (fix44/50/fixLatest): `2a_NextExpectedMsgSeqNumCorrect`,
  `2b_NextExpectedMsgSeqNumTooHigh`, `2c_NextExpectedMsgSeqNumTooLow`,
  `ResetSeqNumFlagAndNextExpectedMsgSeqNum`.
- *lastMsgSeqNumProcessed* (fix42/43/44/50/fixLatest): `LastProcessedMsgSeqNum`.
- *resendRequestChunkSize* (fix40–50/fixLatest): `SequenceGapFollowedByMessageResent`,
  `SequenceGapFollowedBySequenceResetWithGapFill`.
- *validateChecksum* (fix50/fixLatest): `QFJ973-ProcessMessageWithInvalidChecksum`.
- *rejectGarbledMessages* (fix50/fixLatest): `QFJ950-RejectGarbledMessages`.
- *timestamps* (fix44): `QFJ873_timestamps`, `QFJ921_HigherResolutionTimestamps`.
- *resynch*: session resynchronization scenario (Java-driven AT).

> Approx. 480 concrete definition files across all versions/categories; the distinct-scenario sets above
> are the porting checklist. FR-M2 requires every in-scope scenario to pass per targeted version, with
> any deferral explicitly listed and justified.
