# Feature Specification: Audit 006 Remediation

**Feature Branch**: `010-audit-006-remediation`

**Created**: 2026-07-07

**Status**: Draft

**Input**: User description: "按照docs/todo/006.md的内容，确认有问题后修复"

## Clarifications

### Session 2026-07-07

- Q: Should `NEW-105` (`CancelOrdersOnDisconnect`) and `NEW-106` (`PossResend`) be implemented in
  this feature or recorded as deferred parity gaps? → A: Implement both in this feature.
- Q: For `NEW-148`, should dynamic acceptor sessions keep persistent-store support via
  per-identity store/services, or should unsafe persistent dynamic templates be rejected? → A:
  Implement a per-dynamic-identity store/services factory; dynamic persistent sessions remain
  supported.
- Q: For `NEW-137`, should duplicate sequence saves be exposed by changing `save()`'s return
  contract or by adding a separate detection capability? → A: Add a separate duplicate-detection
  capability such as `contains(seq)` or equivalent; keep the core `save()` contract unchanged.
- Q: For `NEW-108`, should file log/store growth be handled by documentation only or by built-in
  configurable limits/rotation? → A: Implement built-in configurable size limits or rotation for
  file logs/stores in this feature.
- Q: For `NEW-146`, should section-order rejection be enforced during decode, validation, or both?
  → A: Enforce it through validation using the section-order flag/policy.
- Q: Should `NEW-134` TLS scheduled initiator and `NEW-135` configurable listener backlog both be
  implemented in this feature? → A: Implement both `NEW-134` and `NEW-135`.

## User Scenarios & Testing *(mandatory)*

`docs/todo/006.md` is a follow-up audit of TrueFix findings not already tracked in
`docs/todo/005.md`. It covers the original `NEW-97`-`NEW-108` pass, an all-crate parallel review
(`NEW-111`-`NEW-147`, with retired IDs omitted), and a focused critical-path review
(`NEW-148`-`NEW-155`). This feature converts the confirmed actionable items in that audit into
fixable, independently testable requirements.

The audit explicitly notes that `NEW-109`, `NEW-114`, `NEW-110`, `NEW-131`, `NEW-141`, and
`NEW-144` are retired or false positives and are out of scope. It also corrects the earlier
`docs/todo/005.md` `NEW-54` record as already fixed/not applicable; this feature must not reopen
that item.

### User Story 1 - Critical protocol and lifecycle integrity (Priority: P1)

An operator running TrueFix in production needs high-impact session, transport, and store failures
from `docs/todo/006.md` fixed so counterparties see correct FIX lifecycle behavior, session state is
not corrupted across reconnects or dynamic identities, and shutdown/backpressure/resource limits do
what the product promises.

**Why this priority**: These findings can cause cross-session sequence contamination, messages lost
from durable recovery, hung or unstoppable sessions, unbounded memory growth, or protocol-visible
logout/sequence behavior that diverges from FIX and QuickFIX/J expectations.

**Independent Test**: Each accepted `NEW-*` item in this story has a focused unit, integration, or
acceptance-style test that fails before the fix and passes after it, covering both initiator and
acceptor behavior wherever the session role matters.

**Acceptance Scenarios**:

1. **Given** an initiator requests logout before logon completes, **When** the session processes the
   request, **Then** the peer receives a graceful logout or the session takes an explicit disconnect
   path rather than silently dropping the request (`NEW-97`).
2. **Given** a Logon is rejected by validation at or above the expected inbound sequence, **When**
   the next connection is established, **Then** sequence tracking reflects the consumed Logon and no
   spurious recovery gap is created (`NEW-98`).
3. **Given** malformed or dictionary-invalid `SequenceReset` input, **When** the session receives it,
   **Then** it is validated and rejected consistently with other inbound messages (`NEW-99`).
4. **Given** a teardown caused by latency, identity, or similar error, **When** a logout diagnostic is
   sent, **Then** that one-shot message is not replayed from durable resend history on a later
   connection (`NEW-100`).
5. **Given** dynamic acceptor sessions resolve to distinct session identities, **When** they use
   persistent storage, **Then** a per-dynamic-identity store/services factory gives each resolved
   identity isolated sequence numbers, message history, and session services (`NEW-148`).
6. **Given** `Engine::shutdown()` is called or the engine is dropped, **When** accepted sessions are
   already running, **Then** shutdown stops those session tasks as well as the listener loops
   (`NEW-149`).
7. **Given** inbound application delivery is configured with a finite capacity, **When** the
   application side is slow, **Then** all pre-application staging remains bounded and backpressure
   reaches the connection (`NEW-150`).
8. **Given** a TLS handshake stalls on either acceptor or initiator paths, **When** configured
   connection/pre-logon timeouts elapse, **Then** the handshake is cancelled and any per-session
   active state is released (`NEW-151`).
9. **Given** a file store opens with a corrupt trailing record, **When** later messages are saved,
   **Then** those valid messages remain discoverable after restart rather than disappearing behind
   the old corrupt tail (`NEW-152`).
10. **Given** file-store sequence persistence reports an I/O failure, **When** in-memory sequence
    state is queried in the same process, **Then** it still matches the last successfully persisted
    value (`NEW-154`).

---

### User Story 2 - Configuration, operational parity, and store robustness (Priority: P2)

Operators migrating from QuickFIX/J need defaults, configuration validation, logging, store reset
semantics, engine handles, and acceptor loops to behave predictably and fail visibly rather than
silently accepting typos, losing diagnostics, or exposing unmanageable sessions.

**Why this priority**: These issues are broad operational risks but generally have narrower blast
radius than the P1 failures. They affect deployability, diagnosability, and parity expectations
rather than every message on every session.

**Independent Test**: Each configuration/default item has a parser or resolved-settings test; each
store/log/transport item has a targeted failure-mode test; each API-management item has a public API
or integration test demonstrating the new observable behavior.

**Acceptance Scenarios**:

1. **Given** default heartbeat and test-request timing is used, **When** a silent peer stops sending
   traffic, **Then** probing and timeout behavior matches QuickFIX/J-compatible expectations
   (`NEW-102`, `NEW-103`).
2. **Given** field order and PossDup defaults are omitted from config, **When** messages are
   validated, **Then** defaults are strict enough to match QuickFIX/J parity and replay protection
   expectations (`NEW-104`, `NEW-111`).
3. **Given** boolean, TLS protocol, or cipher-suite configuration contains an unrecognized value,
   **When** settings are parsed, **Then** the operator receives a typed configuration error instead
   of a silent fallback (`NEW-112`, `NEW-113`; cipher-suite follow-up from prior audit remains out
   of this feature unless referenced by a new `006.md` task).
4. **Given** an accept loop encounters a transient accept error, **When** the listener itself remains
   usable, **Then** the error is observable and accepting continues (`NEW-115`).
5. **Given** Mongo, SQL, or MSSQL stores reset message history and sequence numbers, **When** the
   operation succeeds or fails, **Then** the two parts commit atomically or leave the previous state
   intact (`NEW-116`).
6. **Given** file-store send persistence writes a message and advances sender sequence, **When** one
   step fails, **Then** the store never exposes an advanced sequence without the corresponding
   message being recoverable (`NEW-117`).
7. **Given** a no-op store is used for a live session lifetime, **When** sender or target sequence is
   changed, **Then** subsequent reads reflect that in-memory sequence (`NEW-118`).
8. **Given** a plain `SequenceReset` jumps inbound sequence forward, **When** queued out-of-order
   messages below the new sequence exist, **Then** they are removed and cannot leak indefinitely
   (`NEW-119`).
9. **Given** a queued message fails validation with disconnect-on-error enabled, **When** it is
   drained, **Then** the peer receives a logout before disconnect just as on the in-order path
   (`NEW-120`).
10. **Given** multi-session handles and acceptor sessions are managed through the engine, **When** an
    operator needs a specific session, **Then** session identity, addressability, state query, and
    skipped-session reporting are available without relying on config ordering or log scraping
    (`NEW-121`, `NEW-122`, `NEW-123`, `NEW-143`).
11. **Given** file logging is enabled, **When** event lines are written or writes fail, **Then**
    events are timestamped consistently and write failures are visible (`NEW-124`, `NEW-125`).
12. **Given** logging is configured through the generic log configuration surface, **When** file
    options are supplied, **Then** those options reach the selected backend (`NEW-126`).
13. **Given** required header/trailer fields or BeginString dictionary versions are validated,
    **When** mismatches or omissions are present, **Then** validation is explicit and documented
    rather than silently skipped (`NEW-127`, `NEW-145`).
14. **Given** duplicate session blocks or confusing initiator port errors appear in config, **When**
    parsing fails, **Then** errors identify the duplicate identity or exact missing port key
    (`NEW-128`, `NEW-129`).
15. **Given** config values contain `#`, **When** parsing, **Then** only documented comment positions
    are stripped and values are not unexpectedly truncated (`NEW-130`).
16. **Given** explicitly configured socket options cannot be applied, **When** binding or connecting,
    **Then** the failure is observable to operators (`NEW-132`).
17. **Given** scheduled initiator, listener backlog, or TLS scheduled initiator behavior is needed,
    **When** the operator configures those lifecycle variants, **Then** TLS scheduled initiators and
    configurable listener backlog behavior are available and testable (`NEW-134`, `NEW-135`).
18. **Given** a file store resets, **When** the process crashes around reset, **Then** old body
    records cannot be replayed into a fresh sequence state (`NEW-136`).
19. **Given** schedule-driven logout starts, **When** the peer can respond, **Then** TrueFix waits
    for the logout response or timeout instead of immediately closing the transport (`NEW-139`).
20. **Given** gap-fill messages are sent while last-processed sequence reporting is enabled, **When**
    they leave the session, **Then** they include the same last-processed metadata as other outbound
    session messages (`NEW-140`).
21. **Given** administrative traffic is continuous, **When** ticks, controls, or application messages
    are also pending, **Then** administrative priority cannot starve the rest of the session
    lifecycle indefinitely (`NEW-153`).

---

### User Story 3 - Lower-risk hardening, API consistency, and documented backlog (Priority: P3)

Maintainers need the remaining low-severity findings either corrected or deliberately documented so
the codebase remains strict, diagnosable, and performant without expanding scope beyond
`docs/todo/006.md`.

**Why this priority**: These findings are real but lower impact: narrow malformed-input handling,
diagnostic quality, allocation pressure, optional feature gaps, or API-signature considerations that
may require careful compatibility planning.

**Independent Test**: Each implemented hardening item has a targeted test. Items intentionally
documented rather than implemented have a traceable decision and are excluded from code-fix success
metrics.

**Acceptance Scenarios**:

1. **Given** integer or decimal wire values include surrounding whitespace, **When** typed validation
   runs, **Then** they are rejected rather than silently normalized (`NEW-101`).
2. **Given** a framed message has an invalid checksum trailer terminator, **When** frame detection
   runs, **Then** the frame is rejected as malformed (`NEW-107`).
3. **Given** file log or message store files approach configured growth limits, **When** new records
   are written, **Then** built-in size-limit or rotation behavior prevents unbounded file growth
   without losing required resend/recovery semantics (`NEW-108`).
4. **Given** application-level possible resend semantics are used, **When** inbound messages carry
   `PossResend(97)`, **Then** applications can reliably observe and act on that condition
   (`NEW-106`).
5. **Given** cancel-on-disconnect behavior is required for open orders, **When** the session
   disconnects, **Then** the product supports a safe operator-visible mechanism for issuing the
   required cancel requests (`NEW-105`).
6. **Given** a high-throughput read path classifies complete frames, **When** processing many
   messages, **Then** allocation pressure is reduced without changing accepted or emitted wire data
   (`NEW-133`).
7. **Given** duplicate sequence saves or invalid boolean bytes occur in diagnostic paths, **When**
   they are observed, **Then** a separate duplicate-detection capability makes prior sequence use
   observable without changing the core save contract, and error text is clear enough for callers to
   act on (`NEW-137`, `NEW-138`).
8. **Given** acceptor logon sequence handling executes multiple steps, **When** it transitions to
   logged-on, **Then** the transition happens after sequence handling is complete (`NEW-142`).
9. **Given** section-order checks are split between parsing and validation, **When** a field appears
   in the wrong section, **Then** validation rejects it according to the configured section-order
   policy while decode still permits diagnostic inspection (`NEW-146`).
10. **Given** nested repeating groups are validated through the flat walk, **When** nested members
    are checked, **Then** the nested group's own definition is used consistently (`NEW-147`).
11. **Given** direct public decode receives a checksum value, **When** it is not exactly three ASCII
    digits, **Then** it is rejected consistently with stream framing expectations (`NEW-155`).

### Edge Cases

- Fixes that touch the same lifecycle path, such as logout, schedule exit, validation rejection, and
  shutdown, must be tested together so one corrected path does not re-break another.
- Configuration default changes must preserve explicit operator overrides and document any
  migration-visible behavior change.
- Store crash-safety fixes must define success and failure behavior separately; returning an error
  must not leave runtime state claiming success.
- Dynamic acceptor identities must never share mutable session state unless explicitly configured as
  non-persistent and documented as such.
- Items categorized as feature gaps in `docs/todo/006.md` must be either implemented with tests or
  explicitly recorded as a deferred parity gap; they must not be silently ignored.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The implementation MUST confirm every non-retired item in `docs/todo/006.md` before
  changing behavior, and MUST record any item found false or already fixed in the feature artifacts.
- **FR-002**: Session logout, Logon rejection, schedule exit, validation rejection, and
  disconnect-triggered diagnostics MUST follow FIX-compatible graceful teardown semantics for both
  initiator and acceptor roles (`NEW-97`, `NEW-98`, `NEW-100`, `NEW-120`, `NEW-139`).
- **FR-003**: `SequenceReset` handling MUST be dictionary-validated, sequence-checked, queue-safe,
  and metadata-complete across gap-fill and non-gap-fill variants (`NEW-99`, `NEW-119`, `NEW-140`).
- **FR-004**: Numeric, boolean, checksum, section-order, BeginString, and nested-group validation
  MUST reject malformed input consistently with FIX and documented QuickFIX/J behavior
  (`NEW-101`, `NEW-104`, `NEW-111`, `NEW-127`, `NEW-145`, `NEW-146`, `NEW-147`, `NEW-155`).
- **FR-004a**: Section-order violations MUST be enforced through validation using the section-order
  flag/policy, preserving decode-time diagnostics while rejecting invalid messages when validation
  is applied (`NEW-146`).
- **FR-005**: Configuration parsing MUST reject invalid boolean/TLS protocol values, detect duplicate
  sessions, preserve intended value text, and report precise operator-facing errors
  (`NEW-112`, `NEW-113`, `NEW-128`, `NEW-129`, `NEW-130`).
- **FR-006**: QuickFIX/J-aligned defaults identified by the audit MUST be changed or explicitly
  documented when compatibility cannot be changed safely (`NEW-102`, `NEW-103`, `NEW-104`,
  `NEW-111`, `NEW-125`).
- **FR-007**: Acceptors, initiators, and engine shutdown MUST enforce bounded lifecycle behavior:
  transient accept errors are observable, child sessions are stopped on shutdown, TLS handshakes time
  out, inbound staging is bounded, and admin priority cannot starve session lifecycle work
  (`NEW-115`, `NEW-149`, `NEW-150`, `NEW-151`, `NEW-153`).
- **FR-008**: Dynamic acceptor sessions MUST use a per-dynamic-identity store/services factory so
  persistent state is isolated per resolved identity while dynamic persistent sessions remain
  supported (`NEW-148`).
- **FR-009**: Store backends MUST preserve atomicity and crash-safety for reset, save-and-advance,
  corrupt-tail recovery, no-op sequence tracking, and runtime/durable sequence consistency
  (`NEW-116`, `NEW-117`, `NEW-118`, `NEW-136`, `NEW-152`, `NEW-154`).
- **FR-010**: Logging MUST expose write failures, maintain useful event timestamps, thread backend
  options through generic configuration, and provide built-in configurable size-limit or rotation
  behavior for file logs/stores so file growth is bounded without losing required resend/recovery
  semantics
  (`NEW-108`, `NEW-124`, `NEW-125`, `NEW-126`).
- **FR-011**: Engine and transport management APIs MUST make live sessions identifiable,
  addressable, queryable, and report skipped sessions without requiring callers to infer from order
  or logs (`NEW-121`, `NEW-122`, `NEW-123`, `NEW-143`).
- **FR-012**: Socket and scheduled-initiator capabilities MUST surface configured option failures
  and MUST implement TLS scheduled initiator support plus configurable listener backlog behavior
  (`NEW-132`, `NEW-134`, `NEW-135`).
- **FR-013**: Application-level resend and cancel-on-disconnect parity gaps MUST be implemented with
  observable application/session behavior and targeted tests (`NEW-105`, `NEW-106`).
- **FR-014**: Performance and diagnostic hardening MUST preserve existing externally visible message
  behavior while reducing allocation pressure or improving caller diagnostics; duplicate sequence
  detection MUST be exposed through a separate capability without changing the core save contract
  (`NEW-133`, `NEW-137`, `NEW-138`).
- **FR-015**: Every implemented requirement MUST include targeted tests at the appropriate layer, and
  session semantics MUST include acceptor and initiator coverage where applicable.

### Key Entities *(include if feature involves data)*

- **Audit Finding**: A `docs/todo/006.md` item identified by `NEW-*`, category, severity, source
  evidence, expected behavior, and disposition.
- **Session Lifecycle Event**: A logon, logout, schedule, validation, disconnect, or shutdown event
  whose externally visible behavior must remain FIX-compatible.
- **Session Identity**: The resolved FIX identity used to isolate stores, controls, monitoring, and
  engine-facing handles.
- **Persistent Store State**: Sender/target sequence numbers plus message history that must remain
  mutually consistent across failures and restarts.
- **Operator Configuration**: Parsed settings and defaults whose invalid values or compatibility
  divergences must be visible to operators.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of non-retired `docs/todo/006.md` findings have a recorded disposition:
  implemented, refuted/already-fixed with evidence, or explicitly deferred as a parity gap.
- **SC-002**: All P1 findings selected for implementation have failing-before/passing-after tests
  and no untested critical lifecycle or store path remains.
- **SC-003**: Session lifecycle scenarios involving logout, validation rejection, schedule exit,
  TLS handshake timeout, shutdown, and bounded inbound staging complete within configured time or
  capacity limits in integration tests.
- **SC-004**: Store crash/error simulations demonstrate no advanced sequence without recoverable
  message data and no successful append that disappears after restart.
- **SC-005**: Configuration tests show invalid boolean/TLS/session/default cases fail or resolve
  deterministically with operator-visible diagnostics.
- **SC-006**: Acceptor and initiator coverage exists for each role-sensitive protocol change.
- **SC-007**: The final verification run for touched crates passes unit/integration tests and the
  relevant acceptance-test subset without introducing regressions in prior audit features.

## Assumptions

- `docs/todo/006.md` is the authoritative source for this feature's scope; `docs/todo/003.md`,
  `004.md`, and `005.md` are referenced only to avoid duplicate or refuted work.
- Retired IDs and false positives named by `docs/todo/006.md` are excluded unless later local
  verification proves a distinct current defect.
- The feature may split implementation into tasks by crate or risk tier, but all dispositions remain
  traceable to one audit-derived feature.
- QuickFIX/J and QuickFIX/Go are used only as behavioral references under the repository
  constitution's license/provenance rules; no source code or private data is copied.
- Feature gaps that require public API changes may need compatibility notes even when implemented.
