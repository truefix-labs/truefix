# Feature Specification: Close Remaining QuickFIX/J Parity Gaps (Post-002 Audit)

**Feature Branch**: `003-qfj-full-parity-closure`

**Created**: 2026-07-01

**Status**: Draft

**Input**: User description: "参考 docs/todo-gap-analysis.md 看哪些能实现" — scoped by user decision to cover
**all three priority tiers (P0 + P1 + P2)** of that audit, i.e. every remaining gap except
QuickFIX/Go-only extras and the items the audit itself marks "不适合" (not suitable for a Rust engine).

## Context

Features `001-fix-engine-parity` and `002-qfj-parity-completion` brought TrueFix to a
structurally-complete, config-driven, dual-track-typed FIX engine with a growing acceptance-test (AT)
suite. A fresh full-code audit on 2026-07-01 (`docs/todo-gap-analysis.md`, cross-referenced against
`thrdpty/quickfixj` and `thrdpty/quickfix`) found **14 remaining gaps** versus QuickFIX/J, grouped into
three tiers:

- **P0 — release-blocking**: AT scenario coverage is still only ~30% of the QuickFIX/J server suite and
  only two of the nine targeted FIX versions are exercised (TODO-01); the session's resend path still replays from an
  in-memory map instead of the durable `MessageStore` that 002 already wired up at the transport layer
  (TODO-02).
- **P1 — feature completeness**: top-level field-order validation (TODO-03), a dozen recognised-but-inert
  session config switches (TODO-04), a dictionary `component` model (TODO-05), runtime-loadable custom
  dictionaries (TODO-06), field-type coverage — Data/UtcDateOnly/UtcTimeOnly (TODO-08; the audit's
  optional `Double`/f64 accessor is excluded, see Assumptions), four
  extra validation toggles (TODO-09), and FIX Latest support (TODO-10).
- **P2 — benchmark & tooling**: a session round-trip latency benchmark (TODO-07), network hardening —
  PROXY protocol, SOCKS proxy, inline PEM bytes, configurable cipher suites, synchronous writes
  (TODO-11), a standalone dictionary CLI (TODO-12), bounded inbound backpressure (TODO-13), and MSSQL/
  Oracle SQL backends (TODO-14).

Two QuickFIX/J-only capabilities the audit marks "适合实现" (suitable for TrueFix despite having no
QuickFIX/Go equivalent) are folded into this feature as well: an `ApplicationExtended`-style
pre-logon predicate and post-reset hook, and a `RejectLogon`-equivalent carrying a `SessionStatus`
(tag 573) reason.

Per the constitution, **protocol correctness is the highest-priority principle** and the AT suite is the
release gate (Principle II, VII). This feature is delivered **in one pass but internally staged**,
mirroring 001 and 002, with per-stage checkpoints; every correctness requirement MUST be demonstrated by
an AT scenario or session-level test. No third-party source, test data, or dictionary content is copied
(Principle III) — FIX Latest content derives from FIX Orchestra, converted, not vendored QuickFIX/J
sources.

## Clarifications

### Session 2026-07-01

- Q: US12/FR-015 adds PROXY protocol support so an acceptor recovers the real client IP from a
  PROXY header sent by an upstream load balancer (HAProxy/ELB). That header is self-declared by
  whoever opens the TCP connection, so trusting it unconditionally would let a directly-connecting
  client forge it and bypass the IP allow-list. How should trust be bounded? → A: **Trust only
  configured trusted upstreams** — the acceptor parses and accepts the PROXY header only when the
  physical connection's source IP matches an operator-configured list of trusted proxy/LB addresses;
  otherwise the connection is refused, or the header is ignored and the physical source IP is used
  for logging/allow-listing instead.
- Q: US12/FR-016 requires an initiator to connect through a configured proxy. QuickFIX/J's `Proxy*`
  settings support multiple proxy types (SOCKS4, SOCKS5, HTTP CONNECT), and SOCKS5 can carry username/
  password authentication. Which proxy types must this feature support? → A: **SOCKS4, SOCKS5 (with
  optional username/password auth), and HTTP CONNECT** — full parity with QuickFIX/J's `Proxy*` type
  options.
- Q: US11/FR-014/SC-010 add a session round-trip latency benchmark requiring only a "stable,
  reproducible" distribution, with no numeric threshold. Should it be a hard CI gate (numeric SLO
  failure fails the build) or an observation/regression-comparison tool with no hard threshold? → A:
  **Observation tool only, no hard threshold** — consistent with QuickFIX/J and QuickFIX/Go, which don't
  gate releases on a fixed latency SLO; hardware variance would make a fixed numeric gate unreliable
  across CI runners. Results are for manual regression comparison and do not block release.
- Q: FR-019's bounded inbound backpressure must not starve session-level administrative messages
  (heartbeat/TestRequest/ResendRequest) when the application channel is full. How should this be
  architected? → A: **Separate admin and application channels** — administrative/session-level messages
  travel on their own unbounded (or separately, small-bounded) channel, while application messages
  travel on the `in_chan_capacity`-bounded channel; the session processing loop prioritizes draining the
  admin channel, mirroring QuickFIX/Go's `InChanCapacity` design.
- Q: TODO-14 requires `SqlStore`/`SqlLog` to support Oracle. sqlx has no official Oracle driver;
  options include a third-party sqlx-compatible Oracle extension or the `oracle` crate (which links the
  closed-source, separately-licensed Oracle Instant Client). Given this project's strict license/
  provenance discipline (Principle III), how should this be captured in the spec? → A: **Don't
  prescribe a driver — defer the choice to `/speckit-plan`** — the spec states only the behavioral
  requirement (parity with the other supported SQL backends); the planning phase evaluates the sqlx
  ecosystem and license implications at that time, and MAY downgrade Oracle support to
  documented-interface-only (deferred) if no license-compatible mature option exists.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Prove conformance across the full QuickFIX/J acceptance suite (Priority: P1)

A protocol-conformance reviewer needs confidence that TrueFix behaves identically to QuickFIX/J across
its **entire** published Acceptance Test catalogue, not a partial subset. Today only ~22 of 73 server
scenarios run, across two of the nine targeted FIX versions, and three special-category suites
(`validateChecksum`, `timestamps`, `resynch`) aren't run at all.

**Why this priority**: The AT suite is the constitution's release gate; an unexercised majority of the
official scenario catalogue is the single biggest correctness-confidence gap left in the project.

**Independent Test**: Run the full AT suite (all server scenarios plus the three special-category
suites) against every targeted FIX version and confirm each scenario reaches its documented outcome.

**Acceptance Scenarios**:

1. **Given** the full set of 73 published server scenarios, **When** the AT suite runs, **Then** every
   scenario passes for each FIX version it targets.
2. **Given** the `validateChecksum`, `timestamps`, and `resynch` special-category suites, **When** they
   run, **Then** each reaches its documented pass condition.
3. **Given** a scenario defined against fix40/fix41/fix43/fix50/fix50SP1/fix50SP2/fixLatest, **When** it
   is run against TrueFix configured for that version, **Then** it passes without version-specific
   workarounds in the test harness.

---

### User Story 2 - Replay resends from the same durable store after a restart (Priority: P1)

An operator restarts the engine process. A counterparty then issues a ResendRequest spanning messages
sent before the restart. Feature 002 persisted sequence numbers and sent messages at the transport
layer, but the `Session`'s own resend/reset logic still reads and clears an in-memory `BTreeMap`,
so a session rebuilt after a crash (not a graceful reconnect) cannot correctly replay pre-crash
messages from the store, and `reset()` doesn't clear the store consistently with in-memory state.

**Why this priority**: A correctness defect in a capability the project already claims to have — the
same class of defect the constitution treats as highest priority.

**Independent Test**: Send messages, kill and restart the process (not just the socket) against the same
store, have the counterparty issue a ResendRequest spanning pre-restart sequence numbers, and verify
byte-identical replay with `PossDupFlag=Y`; then trigger a full reset and confirm the store is cleared
consistently with the in-memory session state.

**Acceptance Scenarios**:

1. **Given** a `Session` constructed with a durable store handle (`Session::with_store`), **When** a
   ResendRequest arrives for a range sent in a prior process lifetime, **Then** `build_resend()` reads
   those messages from the store rather than an in-memory map.
2. **Given** `reset()` is invoked (logon-time reset or scheduled reset), **When** it completes, **Then**
   the store's persisted sequence/message state is reset consistently with the in-memory session state.
3. **Given** no store is configured, **When** the session runs, **Then** behaviour is unchanged from
   today's in-memory-only resend (no regression for store-less deployments).

---

### User Story 3 - Reject messages whose fields are out of dictionary order (Priority: P2)

A counterparty (or a fuzzer) sends a message whose header, body, or trailer fields are correctly-typed
and present, but ordered differently than the data dictionary declares. `ValidationOptions` already has
nine toggles but no top-level field-order check, so such messages are silently accepted.

**Why this priority**: A well-defined, narrowly-scoped FIX correctness rule with existing AT scenarios
(`14g`, `15`, `2t`) already named in the audit waiting to exercise it.

**Independent Test**: Send a structurally-valid message with header/body/trailer fields reordered;
verify rejection when the toggle is on and acceptance when it is off.

**Acceptance Scenarios**:

1. **Given** `validate_fields_out_of_order = true`, **When** a message with out-of-order header, body,
   or trailer fields arrives, **Then** the engine rejects it with the field-order-violation reason.
2. **Given** the toggle is `false` (default, matching today's behaviour), **When** the same message
   arrives, **Then** it is accepted as before.
3. **Given** AT scenarios `14g_HeaderBodyTrailerFieldsOutOfOrder`, `15_HeaderAndBodyFieldsOrderedDifferently`,
   and `2t_FirstThreeFieldsOutOfOrder`, **When** they run, **Then** each reaches its documented outcome.

---

### User Story 4 - Configure the remaining recognised session switches (Priority: P2)

An operator sets any of the twelve config keys the engine currently parses and classifies as
"recognised" but does not act on (`SendRedundantResendRequests`, `ClosedResendInterval`,
`ResetOnError`, `DisconnectOnError`, `DisableHeartBeatCheck`, `RejectMessageOnUnhandledException`,
`LogonTag`, `MaxScheduledWriteRequests`, `ContinueInitializationOnError`,
`LogMessageWhenSessionNotFound`, `RefreshOnLogon`, `ForceResendWhenCorruptedStore`) and expects the
engine's behaviour to change accordingly.

**Why this priority**: Twelve distinct QuickFIX/J-parity behaviours bundled under one coherent theme
(session runtime policy); each is individually small but collectively a large completeness gap.

**Independent Test**: For each switch, configure it to its non-default value, drive the corresponding
condition, and observe the documented behavioural change versus the switch left at default.

**Acceptance Scenarios**:

1. **Given** `ResetOnError=Y` or `DisconnectOnError=Y`, **When** a processing error occurs, **Then** the
   session resets or disconnects accordingly, rather than continuing silently.
2. **Given** `RefreshOnLogon=Y`, **When** logon completes, **Then** in-memory session state is refreshed
   from the store as declared (today the field exists but neither the builder nor session state acts on
   it).
3. **Given** `ForceResendWhenCorruptedStore=Y` and a detected store corruption, **When** the condition is
   detected, **Then** the engine forces a resend rather than only detecting and logging the condition.
4. **Given** each remaining switch (`SendRedundantResendRequests`, `ClosedResendInterval`,
   `DisableHeartBeatCheck`, `RejectMessageOnUnhandledException`, `LogonTag`,
   `MaxScheduledWriteRequests`, `ContinueInitializationOnError`, `LogMessageWhenSessionNotFound`) in its
   documented triggering condition, **When** that condition occurs, **Then** the engine's behaviour
   matches the QuickFIX/J-documented effect of that switch.

---

### User Story 5 - Define and reuse dictionary components (Priority: P2)

A dictionary author wants to define a reusable `component` (a named group of fields/groups) once and
reference it from multiple messages, matching the FIX repository's own component model. Today the
normalized `.fixdict` format has `field`/`message`/`group` directives only, so components must be
inlined by hand in every message that uses them.

**Why this priority**: Structural dictionary completeness that several downstream items (typed codegen
fidelity, FIX Latest conversion) benefit from having in place, but no correctness behaviour depends on
it today.

**Independent Test**: Define a component with fields and a nested group, reference it from two
messages, and verify both messages decode/validate/encode as if the component's contents were inlined.

**Acceptance Scenarios**:

1. **Given** a normalized `.fixdict` with a `component` directive, **When** it is parsed, **Then** a
   `ComponentDef` is produced in the `DataDictionary`.
2. **Given** a message definition referencing a component, **When** a message using it is decoded,
   **Then** the component's fields/groups are expanded and validated exactly as if inlined.
3. **Given** the same component referenced from two different messages, **When** both are validated,
   **Then** the component's validation rules apply identically in both contexts.

---

### User Story 6 - Load and extend dictionaries at runtime (Priority: P2)

An integrator has a custom or extended dictionary file (e.g. a house dictionary adding custom tags to a
standard FIX version) and wants to load it from disk at startup and merge it with a bundled dictionary,
without recompiling.

**Why this priority**: Enables custom-field deployments without a code change or rebuild — a common
integration need `parse(&str)` alone doesn't serve well.

**Independent Test**: Load a bundled dictionary, load a second file defining extension fields, merge
them, and verify a message using the extension fields validates correctly.

**Acceptance Scenarios**:

1. **Given** a `.fixdict` file path, **When** `DataDictionary::load_from_file(path)` is called, **Then**
   it returns a parsed `DataDictionary` or a typed error naming the file and problem.
2. **Given** a base dictionary and an extension dictionary, **When**
   `DataDictionary::extend(&other)` is called, **Then** the extension's fields/messages/components/
   groups are merged into the base, with conflicting definitions surfaced as a typed error rather than
   silently overwritten.

---

### User Story 7 - Exchange fields of every dictionary-declared type (Priority: P2)

An integrator builds or reads a message containing a `Data` field (e.g. `RawData`/tag 96), a
`UtcDateOnly` field, or a `UtcTimeOnly` field. Today `truefix-core::Field` implements six constructors/
accessors (String, Int, Decimal, Char, Bool, UTCTimestamp); the `FieldType` enum already declares
17 variants but most have no runtime conversion, so these fields can only be handled as raw strings.

**Why this priority**: Real-world messages (market data, mass quotes, encrypted fields) use these types;
falling back to raw-string handling loses type safety exactly where QuickFIX/J provides it.

**Independent Test**: Construct and round-trip a message containing a Data field and UtcDateOnly/
UtcTimeOnly fields through the typed constructor/accessor pair for each, and verify byte-identical
encode/decode.

**Acceptance Scenarios**:

1. **Given** a `Data` field (tag 95/96/212/348/352/445 pattern: length-tag + data-tag), **When** it is
   set via `Field::bytes()` and read via `as_bytes()`, **Then** the raw bytes round-trip exactly,
   including embedded SOH bytes.
2. **Given** a `UtcDateOnly` field, **When** set via `Field::utc_date_only()` and read via
   `as_utc_date_only()`, **Then** it round-trips to the FIX `YYYYMMDD` wire format exactly.
3. **Given** a `UtcTimeOnly` field, **When** set via `Field::utc_time_only()` and read via
   `as_utc_time_only()`, **Then** it round-trips to the FIX `HH:MM:SS[.sss...]` wire format exactly.

---

### User Story 8 - Apply the remaining checksum/validation policy toggles (Priority: P2)

An operator wants independent control over four QuickFIX/J validation policies `ValidationOptions`
doesn't yet expose: an independent checksum-validation toggle, a master switch to skip all dictionary
validation, a PossDup-acceptance policy, and a requirement that PossDup messages carry
`OrigSendingTime`.

**Why this priority**: Four small, well-defined QuickFIX/J-parity switches with existing AT coverage
expectations (`validateChecksum` suite, PossDup scenarios).

**Independent Test**: Toggle each of the four options independently and verify the corresponding
message class is accepted/rejected as documented.

**Acceptance Scenarios**:

1. **Given** `validate_checksum = true` and a message with an incorrect checksum that would otherwise
   decode, **When** it arrives, **Then** it is rejected on checksum grounds independent of framing-level
   checks.
2. **Given** `validate_incoming_message = false`, **When** a message that would normally fail dictionary
   validation arrives, **Then** dictionary validation is skipped entirely and only session-level checks
   apply.
3. **Given** `allow_pos_dup = false` and a message with `PossDupFlag=Y`, **When** it arrives, **Then**
   it is rejected per the configured PossDup policy.
4. **Given** `requires_orig_sending_time = true` and a PossDup message missing `OrigSendingTime`,
   **When** it arrives, **Then** it is rejected for the missing required field.

---

### User Story 9 - Run the engine against the FIX Latest dictionary (Priority: P2)

An integrator targeting the unified FIX Latest (Orchestra-based) specification wants to configure a
session for `fixLatest` the same way they would for FIX.4.4 or FIX 5.0 SP2. Today nine bundled
dictionary sources cover FIX40 through FIX50SP2 plus FIXT11, with no FIX Latest source or loader.

**Why this priority**: The last targeted-version gap in the audit; placed after the P1 correctness/
completeness items because no current deployment requires it, but it completes the version matrix
FR-M3 (baseline) calls for.

**Independent Test**: Load the FIX Latest dictionary, generate its typed messages, run the `fixLatest`
AT scenarios, and confirm a session configured for it exchanges messages correctly.

**Acceptance Scenarios**:

1. **Given** a normalized `.fixdict` generated from FIX Orchestra (not vendored QuickFIX/J XML), **When**
   `DataDictionary::load_fixlatest()` is called, **Then** it returns a fully-parsed dictionary.
2. **Given** the FIX Latest dictionary, **When** the build runs, **Then** typed message/field/group/
   component structs and a `crack_fixlatest` cracker are generated, matching the pattern used for other
   versions.
3. **Given** the `fixLatest`-targeted AT scenarios, **When** they run against a session configured for
   FIX Latest, **Then** each passes.

---

### User Story 10 - Gate logons and observe session resets through extended application hooks (Priority: P2)

An integrator wants to run an arbitrary predicate before accepting a logon (beyond CompID/dictionary
checks) and to be notified just before a session resets, and — when refusing a logon — wants to carry a
FIX `SessionStatus` (tag 573) reason rather than a generic reject.

**Why this priority**: The audit marks these QuickFIX/J-only capabilities as suitable for TrueFix; they
extend the existing typed-callback-outcome work from feature 002 rather than introducing a new
mechanism.

**Independent Test**: Register a logon predicate that refuses a specific CompID with a chosen
`SessionStatus` value, and a pre-reset hook that records invocation; verify both fire at the right point
and the refusal includes the configured status.

**Acceptance Scenarios**:

1. **Given** an application-supplied logon predicate returning refusal, **When** a matching logon
   arrives, **Then** the session is refused before normal logon processing completes.
2. **Given** a refusal carrying a chosen `SessionStatus` (tag 573) value, **When** the logon is refused,
   **Then** the outbound Logout/Reject carries that `SessionStatus` value.
3. **Given** an application-supplied pre-reset hook, **When** the session resets (logon-time or
   scheduled), **Then** the hook is invoked before the reset takes effect.

---

### User Story 11 - Measure session round-trip latency (Priority: P3)

A performance-conscious integrator wants a repeatable benchmark of session-level round-trip latency
(message in → processed → response out) to track regressions across changes.

**Why this priority**: Tooling for ongoing quality, not a functional gap; useful once the correctness/
completeness work above lands so it measures the shipped behaviour.

**Independent Test**: Run the benchmark and confirm it produces a stable, reproducible latency
distribution for a fixed message mix.

**Acceptance Scenarios**:

1. **Given** the benchmark suite, **When** `cargo bench` runs `benches/session.rs`, **Then** it reports
   a round-trip latency distribution for a representative message mix.
2. **Given** two runs on unchanged code, **When** results are compared, **Then** the distributions are
   consistent within normal benchmarking noise (no crashes, no unbounded growth).

---

### User Story 12 - Harden network connectivity for regulated/proxied environments (Priority: P3)

An operator deploying behind a load balancer (HAProxy/ELB) needs the acceptor to recover the real client
IP from a PROXY protocol header; an operator whose initiator must egress through a forward proxy (SOCKS4, SOCKS5, or
HTTP CONNECT) needs that configurable; an operator wants inline PEM bytes (no filesystem cert), a configurable cipher-suite list,
and synchronous-write semantics with a timeout — matching capabilities present in QuickFIX/J and/or
QuickFIX/Go but missing in TrueFix today.

**Why this priority**: Deployment-environment hardening; below the correctness/completeness tiers
because default (non-proxied, file-based-cert) deployments already work.

**Independent Test**: For each capability, configure it and verify the documented network behaviour
(PROXY header parsed into the reported client IP; SOCKS-proxied connection succeeds; inline PEM bytes
establish TLS without a file path; a restricted cipher suite list is honoured in the handshake;
synchronous writes block/time out as configured).

**Acceptance Scenarios**:

1. **Given** `UseTCPProxy=Y` and a connection from a configured trusted upstream sending a PROXY
   protocol header, **When** the acceptor accepts the connection, **Then** it records the header's
   original client IP rather than the proxy's; **Given** the same header arriving from a source IP not
   in the trusted-upstream list, **When** the connection is accepted, **Then** the header is rejected (or
   ignored in favor of the physical source IP) rather than trusted.
2. **Given** SOCKS4, SOCKS5 (optionally with username/password credentials), or HTTP CONNECT proxy
   settings, **When** an initiator connects, **Then** the TCP connection is established through the
   configured proxy of the corresponding type.
3. **Given** inline PEM bytes for private key/certificate/CA, **When** TLS is configured, **Then** the
   handshake succeeds without any of those being read from a file path.
4. **Given** a configured cipher-suite list, **When** a TLS handshake occurs, **Then** only suites from
   that list are offered/accepted.
5. **Given** `SocketSynchronousWrites=Y` with a configured timeout, **When** an outbound write would
   block past the timeout, **Then** the engine surfaces a typed timeout error rather than blocking
   indefinitely.

---

### User Story 13 - Generate and validate dictionaries from the command line (Priority: P3)

A dictionary maintainer wants a standalone CLI to convert a FIX Orchestra/FPL repository source into the
normalized `.fixdict` format, generate typed Rust code from an existing `.fixdict` outside of a
`build.rs` run, and validate a dictionary file's syntax while printing its dual-track hash — without
writing a throwaway Rust program.

**Why this priority**: Developer-experience tooling; the underlying codegen and parsing already exist
inside `build.rs`, so this wraps existing capability rather than adding new correctness behaviour.

**Independent Test**: Run each CLI subcommand against a sample source/dictionary and verify the expected
output artifact or validation result.

**Acceptance Scenarios**:

1. **Given** a FIX Orchestra or FPL repository source, **When** the CLI's generate-dictionary subcommand
   runs, **Then** it emits a normalized `.fixdict` file.
2. **Given** a `.fixdict` file, **When** the CLI's generate-code subcommand runs, **Then** it emits the
   same typed Rust structs `build.rs` would generate for that dictionary.
3. **Given** a `.fixdict` file, **When** the CLI's validate subcommand runs, **Then** it reports syntax
   errors (if any) or success plus the dual-track hash.

---

### User Story 14 - Apply inbound backpressure and reach additional SQL backends (Priority: P3)

An operator whose downstream processing is slower than inbound message arrival wants a bounded inbound
channel that applies backpressure (blocks the reader) rather than growing unbounded or dropping
messages; separately, an operator whose infrastructure standardizes on MSSQL or Oracle wants `SqlStore`/
`SqlLog` to support those databases, matching QuickFIX/J's JDBC-based reach.

**Why this priority**: Both are infrastructure-completeness items for specific deployment profiles, not
needed by the default file/SQLite-based, unbounded-channel path.

**Independent Test**: Configure a small `in_chan_capacity`, saturate it with a slow consumer, and verify
the reader blocks rather than drops or grows unbounded; separately, run the store/log conformance suite
against MSSQL and Oracle instances and confirm equivalent behaviour to the SQLite backend.

**Acceptance Scenarios**:

1. **Given** `in_chan_capacity = Some(n)`, **When** more than `n` inbound messages are pending and the
   consumer is slow, **Then** the transport's read side blocks (backpressure) instead of dropping
   messages or growing the channel unbounded.
2. **Given** `in_chan_capacity = None` (default), **When** the session runs, **Then** behaviour is
   unchanged from today's unbounded channel.
3. **Given** `SqlStore`/`SqlLog` configured for MSSQL or Oracle, **When** the store/log conformance suite
   runs, **Then** persistence and logging behave equivalently to the existing PostgreSQL/MySQL/SQLite
   backends.

### Edge Cases

- An AT scenario targets a FIX version whose dictionary lacks a field/message the scenario references →
  the gap is treated as a dictionary-completeness defect to fix, not a reason to skip the scenario.
- A process crash (not graceful shutdown) leaves the in-memory resend map empty but the store populated
  → post-restart resend MUST still succeed from the store (this is precisely the scenario 002's
  transport-level persistence could not by itself guarantee).
- `validate_fields_out_of_order` is enabled together with a message containing a dictionary-undefined
  custom field in an unusual position → the check applies only to dictionary-known field ordering
  constraints, not to placement of undefined fields.
- A component is referenced by another component (nested components) → nesting resolves recursively, and
  a circular component reference is rejected as a typed dictionary-load error, not an infinite loop or
  stack overflow.
- `DataDictionary::extend()` merges a dictionary that redefines an existing tag with a different type →
  surfaced as a typed conflict error, not silently overwritten.
- A `Data` field's declared length field is present but the byte length doesn't match → rejected with a
  length-mismatch reason, not silently truncated/padded.
- FIX Latest conversion encounters an Orchestra construct with no FIX40-50SP2 equivalent (e.g. a
  post-5.0SP2-only message) → the normalized dictionary represents it faithfully; codegen and AT scope
  cover it like any other message.
- A `RejectLogon`-style refusal with a `SessionStatus` fires while `DisconnectOnError`/`ResetOnError`
  (US4) are also configured → the logon refusal path and the error-handling switches compose without
  double-disconnecting or leaving an inconsistent session state.
- `in_chan_capacity` is configured smaller than the burst size QuickFIX-standard heartbeats/test requests
  need → session-level administrative traffic is unaffected, since it travels on its own channel
  separate from the bounded application-message channel, and is prioritized ahead of it in the session
  processing loop.
- A client not in the configured trusted-upstream list connects directly and sends a forged PROXY
  protocol header → the header MUST NOT be trusted; the connection is refused or the physical source IP
  is used for IP-allow-list/logging purposes instead, so the forged header cannot bypass the allow-list.
- MSSQL/Oracle connectivity is unavailable in a given environment → tests requiring that backend are
  gated on its availability (matching the PostgreSQL/MySQL precedent from feature 002), not a hard build
  failure.

## Requirements *(mandatory)*

Requirements are grouped by theme, following the P0/P1/P2 tiers of the audit. Baseline IDs from features
001/002 are noted in parentheses where a requirement continues or completes one.

### Functional Requirements — Conformance Coverage & Session Correctness (P0)

- **FR-001**: The AT suite MUST cover all 73 published QuickFIX/J server scenarios plus the
  `validateChecksum`, `timestamps`, and `resynch` special-category suites (FR-M3).
- **FR-002**: The AT suite MUST exercise its scenarios across all nine targeted FIX versions
  (fix40/41/42/43/44/50/50SP1/50SP2 dictionaries plus fixLatest once US9 lands), not only FIX.4.2/FIX.4.4.
  FIX50SP1 is included even though it wasn't named in the originating audit's uncovered-version list,
  because a normalized dictionary already exists for it (per `research.md`) and full audit-based
  completeness (Principle VII) should exercise every already-bundled version.
- **FR-003**: `Session` MUST support construction with a durable store handle
  (`Session::with_store(config, store)`) whose `build_resend()` reads sent-message bodies from that
  store rather than an in-memory map.
- **FR-004**: `Session::reset()` MUST reset the durable store's persisted state consistently with the
  in-memory session state whenever a store is attached.
- **FR-005**: Every scenario and correctness behaviour added under FR-001–FR-004 MUST be demonstrated by
  a passing AT scenario or session-level test (AT remains the release gate; Principle II, VII).

### Functional Requirements — Validation & Dictionary Completeness (P1)

- **FR-006**: `ValidationOptions` MUST add `validate_fields_out_of_order`, and `validate()` MUST reject
  messages whose header/body/trailer fields violate dictionary-declared ordering when it is enabled.
- **FR-007**: `ValidationOptions` MUST add `validate_checksum`, `validate_incoming_message`,
  `allow_pos_dup`, and `requires_orig_sending_time`, each independently controlling its documented
  QuickFIX/J-equivalent behaviour.
- **FR-008**: The engine MUST implement the twelve currently-recognised-but-inert session config
  switches (`SendRedundantResendRequests`, `ClosedResendInterval`, `ResetOnError`, `DisconnectOnError`,
  `DisableHeartBeatCheck`, `RejectMessageOnUnhandledException`, `LogonTag`,
  `MaxScheduledWriteRequests`, `ContinueInitializationOnError`, `LogMessageWhenSessionNotFound`,
  `RefreshOnLogon`, `ForceResendWhenCorruptedStore`), each changing session behaviour per its
  QuickFIX/J-documented effect.
- **FR-009**: The normalized `.fixdict` format MUST support a `component` directive, and
  `DataDictionary` MUST represent it as a `ComponentDef` referenced by one or more messages, expanded
  during decode/validate/encode exactly as if inlined.
- **FR-010**: `DataDictionary` MUST provide `load_from_file(path)` to load a dictionary from disk and
  `extend(&other)` to merge an extension dictionary into a base, surfacing conflicting redefinitions as
  a typed error.
- **FR-011**: `truefix-core::Field` MUST add `bytes()`/`as_bytes()` (Data fields), `utc_date_only()`/
  `as_utc_date_only()`, and `utc_time_only()`/`as_utc_time_only()` constructor/accessor pairs, each
  round-tripping to the exact FIX wire format.
- **FR-012**: The engine MUST support the FIX Latest dictionary — loaded via
  `DataDictionary::load_fixlatest()` from a normalized source derived from FIX Orchestra (not vendored
  QuickFIX/J data), with typed codegen (`crack_fixlatest`) generated the same way as other versions.
- **FR-013**: The application-integration surface MUST support an extended logon predicate (arbitrary
  refusal condition beyond CompID/dictionary checks) and a pre-reset hook invoked before a session reset
  takes effect, and a logon refusal MUST be able to carry a `SessionStatus` (tag 573) reason on the
  outbound Logout/Reject.

### Functional Requirements — Benchmark & Infrastructure Reach (P2)

- **FR-014**: A `benches/session.rs` benchmark MUST measure session round-trip latency (message in →
  processed → response out) for a representative message mix, runnable via `cargo bench`. It is an
  observation/regression-comparison tool only — it MUST NOT enforce a fixed numeric latency threshold
  as a CI gate.
- **FR-015**: The transport MUST support the PROXY protocol (v1/v2) on acceptor connections
  (`UseTCPProxy`), recovering the original client IP from the header for logging/IP-allow-list purposes
  **only** when the physical connection's source IP matches an operator-configured set of trusted
  upstream (proxy/LB) addresses; a PROXY header arriving from a non-trusted source MUST be rejected
  (or ignored in favor of the physical source IP) rather than trusted, preventing IP-allow-list bypass
  by a directly-connecting client forging the header.
- **FR-016**: The transport MUST support connecting an initiator through a configured forward proxy,
  supporting SOCKS4, SOCKS5 (with optional username/password authentication), and HTTP CONNECT proxy
  types, matching the scope of QuickFIX/J's `Proxy*` settings.
- **FR-017**: TLS configuration MUST accept inline PEM bytes for private key, certificate, and CA (in
  addition to existing file-path configuration), a configurable cipher-suite list, and
  `SocketSynchronousWrites` with a configurable write timeout that surfaces a typed error rather than
  blocking indefinitely.
- **FR-018**: A `truefix-dict` CLI MUST provide subcommands to generate a normalized `.fixdict` from a
  FIX Orchestra/FPL repository source, generate typed Rust code from a `.fixdict` outside `build.rs`, and
  validate a `.fixdict`'s syntax while printing its dual-track hash.
- **FR-019**: `SessionConfig` MUST add `in_chan_capacity: Option<usize>`; when set, the transport's
  inbound **application**-message channel MUST be bounded to that capacity and apply backpressure
  (block the reader) rather than drop messages when full. Session-level administrative messages
  (heartbeat, TestRequest, ResendRequest, and other admin traffic) MUST travel on a separate channel
  from the bounded application channel, and the session processing loop MUST prioritize draining that
  admin channel, so administrative processing is never starved by application-channel backpressure.
- **FR-020**: `SqlStore` and `SqlLog` MUST additionally support MSSQL, behaving equivalently to the
  existing PostgreSQL/MySQL/SQLite backends, with tests gated on backend availability. `SqlStore` and
  `SqlLog` MUST also support Oracle to the same behavioral standard, but this specification does NOT
  prescribe the driver/crate; the planning phase MUST evaluate license-compatible options (Principle
  III) and MAY scope Oracle support down to a documented, deferred interface if no mature
  license-compatible driver exists at that time.
- **FR-021**: Configuration keys and toggles implemented by this feature MUST update their registered
  stance from "recognised" to "implemented" so the Appendix A coverage record stays accurate.

### Key Entities

- **AT Scenario Catalogue**: The full set of QuickFIX/J-published server scenarios and special-category
  suites this feature exercises, keyed by scenario ID and targeted FIX version.
- **Durable Resend Source**: The `MessageStore` handle a `Session` reads from during `build_resend()` and
  resets during `reset()`, replacing the in-memory-only resend map.
- **Component Definition**: A named, reusable group of field/group definitions referenced by one or more
  message definitions in the data dictionary.
- **Extension Dictionary**: A dictionary loaded at runtime and merged into a base dictionary via
  `extend()`, potentially adding custom fields/messages/components.
- **Typed Field Value**: A dictionary-declared field type (Data, UtcDateOnly, UtcTimeOnly) with a
  constructor/accessor pair that round-trips to its exact FIX wire representation.
- **FIX Latest Dictionary**: The normalized dictionary derived from FIX Orchestra representing the
  unified FIX Latest specification, loaded and code-generated like any other targeted version.
- **Extended Application Hook**: The logon-predicate and pre-reset-hook surface an integrator can
  register, plus the `SessionStatus`-carrying logon refusal outcome.
- **Bounded Inbound Channel**: The capacity-limited transport channel that applies backpressure instead
  of dropping messages once full.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the 73 published server scenarios plus the three special-category suites pass,
  across every FIX version each scenario targets.
- **SC-002**: After a full process restart (not a graceful reconnect) against the same store, 100% of
  messages within a ResendRequest's persisted range replay byte-identically with `PossDupFlag=Y`.
- **SC-003**: 100% of the field-order AT scenarios (`14g`, `15`, `2t`) and the four extra-validation
  scenarios reach their FIX-correct outcome.
- **SC-004**: All twelve previously-inert session config switches produce their documented behavioural
  change when enabled, verified by a dedicated test per switch.
- **SC-005**: A dictionary using `component` definitions decodes/validates/encodes identically to an
  equivalent hand-inlined dictionary.
- **SC-006**: A custom dictionary loaded via `load_from_file`/`extend` at runtime is usable to validate
  and decode a message containing its custom fields, with no rebuild required.
- **SC-007**: Data/UtcDateOnly/UtcTimeOnly fields round-trip byte-identically through their typed
  constructor/accessor pairs.
- **SC-008**: A session configured for FIX Latest completes a logon-to-heartbeat exchange and passes its
  targeted AT scenarios.
- **SC-009**: An extended logon predicate can refuse a logon with a chosen `SessionStatus` value, and a
  pre-reset hook is observably invoked before every reset.
- **SC-010**: The session round-trip latency benchmark runs reproducibly and reports a stable
  distribution across unchanged-code runs, used for manual regression comparison — it is not a
  CI-gating pass/fail threshold.
- **SC-011**: An acceptor behind a PROXY-protocol load balancer reports the real client IP; an initiator
  connects successfully through a configured SOCKS proxy; TLS establishes from inline PEM bytes; a
  restricted cipher-suite list is honoured; a stalled synchronous write times out with a typed error.
- **SC-012**: The `truefix-dict` CLI generates a `.fixdict` from an Orchestra/FPL source, generates
  matching typed Rust code from an existing `.fixdict`, and validates a dictionary's syntax with its
  dual-track hash.
- **SC-013**: A bounded inbound channel applies backpressure under a slow consumer without dropping
  messages or starving administrative traffic; an unbounded (default) channel is unaffected.
- **SC-014**: The SQL store/log suite passes equivalently against MSSQL and Oracle wherever those
  backends are available in CI/test environments.
- **SC-015**: The full workspace gate (format, lints with warnings-as-errors, all tests, AT conformance
  across all targeted versions) remains green after every internal stage.
- **SC-016**: Every configuration key/toggle implemented by this feature is recorded as "implemented" in
  the Appendix A coverage record.

## Assumptions

- **FIX version target set**: The full targeted-version matrix is fix40/41/42/43/44/50/50SP1/50SP2 (all
  8 already-bundled non-fixLatest normalized dictionaries) plus fixLatest once US9 lands — 9 versions
  total (resolved via `/speckit-analyze`; FIX50SP1 is included even though the originating audit's
  TODO-01 uncovered-version list didn't name it, since a normalized dictionary already exists for it and
  full audit-based completeness, Principle VII, should exercise every bundled version).
- **`Double`/f64 field type excluded**: `Field::double()`/`as_double()` (the audit's optional Float/
  Price/Qty compatibility accessor under TODO-08) is out of scope for this feature — `rust_decimal`
  already covers Price/Qty representation, and the audit itself marks this accessor optional.
- **Scope boundary**: Per user decision, this feature covers all three tiers (P0+P1+P2) of the
  2026-07-01 audit, i.e. TODO-01 through TODO-14, plus the two QuickFIX/J-only items the audit marks
  "适合实现" (`ApplicationExtended`-style hooks, `RejectLogon`/`SessionStatus`). QuickFIX/Go-only extras
  and the audit's "不适合" items remain out of scope, unchanged from feature 002's boundary.
- **FIX Latest provenance**: The FIX Latest normalized dictionary is generated from FIX Orchestra
  content, not converted from vendored QuickFIX/J `.def`/XML files, preserving Principle III
  (license/provenance discipline).
- **MSSQL/Oracle availability**: Like the PostgreSQL/MySQL precedent from feature 002, MSSQL/Oracle
  conformance tests are gated on the availability of a service instance in CI/local environments rather
  than being a hard build/test dependency.
- **Oracle driver choice deferred**: Per Clarifications, this spec does not prescribe an Oracle
  driver/crate; `/speckit-plan` evaluates license-compatible sqlx-ecosystem options against Principle
  III and may scope Oracle support down to a deferred, documented-interface-only state if none is
  mature/license-compatible at planning time.
- **Proxy type scope**: SOCKS4, SOCKS5 (with optional username/password auth), and HTTP CONNECT are all
  in scope for FR-016, matching QuickFIX/J's `Proxy*` type options.
- **PROXY protocol trust boundary**: PROXY protocol headers (FR-015) are honored only from a configured
  trusted-upstream address set; connections from other sources either have the header rejected/ignored
  or the connection refused, per Clarifications.
- **Backpressure channel separation**: The bounded `in_chan_capacity` applies to the application-message
  channel only; administrative/session-level messages use a separate, prioritized channel per
  Clarifications, so FR-019 never risks heartbeat/test-request starvation.
- **Benchmark is non-gating**: The `benches/session.rs` benchmark (FR-014) is an observation/regression
  tool; it does not enforce a numeric latency SLO as a CI gate, per Clarifications.
- **PROXY protocol / SOCKS / inline PEM / cipher suites**: These are additive TLS/transport
  configuration surfaces; existing file-path-based TLS configuration and unproxied deployments continue
  to work unchanged.
- **CLI packaging**: The `truefix-dict` CLI wraps the existing `build.rs`-internal parsing/codegen logic
  rather than reimplementing it, keeping a single source of dictionary-parsing/codegen truth.
- **Backpressure default**: `in_chan_capacity` defaults to `None` (today's unbounded behaviour); bounded
  backpressure is strictly opt-in.
- **Extended hooks build on 002**: The logon predicate, pre-reset hook, and `SessionStatus`-carrying
  refusal extend the typed-callback-outcome mechanism feature 002 introduced (FR-016/FR-022 of 002),
  rather than introducing a parallel callback path.
- **Verification discipline**: Every correctness item is gated by an AT or session-level test authored as
  part of this feature's "done", consistent with Principle VII (inventory-based completeness).
- **Delivery**: One pass, internally staged with per-stage checkpoints (mirroring 001 and 002); the AT
  suite is the release gate.

## Out of Scope (Deferred)

Unchanged from feature 002's boundary — only QuickFIX/Go-only extras and audit items marked "不适合"
remain excluded:

- `DynamicQualifier` dynamic session qualifiers, `ConnectionValidator`/`NewListenerCallback` acceptor
  hooks, `ResetSeqTime`/`EnableResetSeqTime` mid-connection scheduled resets, and `HeartBtIntOverride`
  (QuickFIX/Go-only).
- MongoDB store/log backends (QuickFIX/Go-only NoSQL option).
- JMX MBean remote management, OSGi bundling, SLF4J facade replacement, `@Handler`-annotation-based
  cracking, `FieldNotFound`-style named exceptions, SleepycatStore — all marked "不适合" in the audit as
  having no idiomatic Rust equivalent or being superseded by existing TrueFix mechanisms
  (`metrics`/`Monitor`, tokio async, `tracing`, codegen `crack_<version>`, typed `RejectReason`).

## Dependencies

- Builds on features `001-fix-engine-parity` and `002-qfj-parity-completion` and the constitution
  (`.specify/memory/constitution.md`, v2.0.0).
- AT scenario expansion (FR-001/FR-002) depends on and validates every other requirement in this
  feature — most new scenarios directly exercise FR-006–FR-013.
- Component model (FR-009) is a prerequisite for higher-fidelity FIX Latest conversion (FR-012) and
  benefits typed codegen generated for both.
- Durable resend (FR-003/FR-004) depends on the store integration feature 002 already completed at the
  transport layer (seed_sequences/seed_sent_messages); this feature completes the session-owned half.
- Field-order validation (FR-006) and the extra validation toggles (FR-007) both build on the
  group/field validation pipeline feature 002 introduced.
- The extended application hooks (FR-013) depend on the typed callback-outcome mechanism from feature
  002 (its FR-016/FR-022).
- The Appendix A coverage record and the AT conformance gate from feature 001 remain the verification
  surfaces for SC-004/SC-015/SC-016.
