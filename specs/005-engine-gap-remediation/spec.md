# Feature Specification: Engine Gap Remediation

**Feature Branch**: `005-engine-gap-remediation`

**Created**: 2026-07-02

**Status**: Draft

**Input**: User description: "根据 docs/engine-comparison-gaps.md 制定变更计划" (create a change plan based on `docs/engine-comparison-gaps.md`)

## Clarifications

### Session 2026-07-02

- Q: Does a PossDup anti-replay violation (FR-008/GAP-08 — `seq < expected`, `PossDupFlag=Y`,
  `OrigSendingTime` later than `SendingTime`) trigger a full session logout+disconnect, or a lighter
  rejection that keeps the session connected? → A: Full logout + disconnect, matching QuickFIX/J's
  `validatePossDup` exactly and the same severity already locked in for the duplicate-Logon case
  (User Story 3, Acceptance Scenario 4).
- Q: When multiple `.cfg` `[SESSION]` blocks share the same `SocketAcceptPort`, how should
  `Engine::start` recognize they belong to one shared multi-session acceptor (FR-006)? → A: Automatic
  grouping by matching `SocketAcceptPort` — no new `.cfg` key introduced; sessions with the same port
  are grouped into one `AcceptorBuilder`, sessions with distinct ports keep their own independent bind.
  Zero migration cost for existing `.cfg` files.
- Q: How far should bundled dictionary field/message coverage expand this round (GAP-33/FR-031)? → A:
  Match QuickFIX/J's own bundled dictionary pattern — per targeted FIX version, source field/message
  coverage from the corresponding reference dictionary file in `thrdpty/quickfixj` (concrete, per-source
  parity target) rather than an abstract "100% of Appendix A" requirement or an open-ended
  best-effort with no defined target.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Fix real correctness bugs in config parsing and message decoding (Priority: P1)

Operators who write `.cfg` files or receive FIX messages containing rare-but-legal field
combinations get correct behavior instead of silent corruption. Two independent bugs: (a) a config
value containing `#` gets silently truncated instead of preserved verbatim (breaks passwords/URLs
containing `#`), and (b) a `Signature` (tag 89) field with an embedded SOH byte gets mis-tokenized
during decode because the decoder doesn't know its length field is tag 93, not the usual `dataTag - 1`
convention.

**Why this priority**: These are the two clearest "wrong behavior with no toggle to work around it"
bugs in the source document — one silently corrupts config values (including credentials), the other
is a wire-protocol correctness edge case. Both are P0 findings (`BUG-01`, `BUG-02`).

**Independent Test**: Can be fully tested by (a) parsing a `.cfg` file with a `#` character embedded in
a password value and asserting the full value round-trips, and (b) round-tripping a message containing
a `Signature`(89)/`SignatureLength`(93) field pair with an embedded SOH byte in the signature value and
asserting it decodes byte-identically.

**Acceptance Scenarios**:

1. **Given** a `.cfg` file with `Password=ab#cd`, **When** it's parsed, **Then** the resulting value is
   exactly `ab#cd`.
2. **Given** a `.cfg` file with a genuine whole-line comment (`# this is a comment`), **When** it's
   parsed, **Then** the line contributes no key/value.
3. **Given** a FIX message containing tag 93 (SignatureLength) and tag 89 (Signature) where the
   signature value contains an embedded SOH byte, **When** the message is decoded, **Then** the
   Signature field's value is extracted using the SignatureLength-specified byte count, not tokenized
   on SOH.

---

### User Story 2 - Fix `.cfg` keys that silently misrepresent what the engine actually does (Priority: P1)

Operators trust the Appendix A key-stance registry to tell them whether a `.cfg` key actually does
anything. Two groups of keys currently lie in opposite directions: three acceptor keys
(`AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate`) claim to be fully implemented but are
completely unreachable from `.cfg`-driven engine startup; and `JdbcURL`, while genuinely wired, only
recognizes TrueFix's own URL scheme, not the actual JDBC URL format real QuickFIX/J `.cfg` files use —
so an unmodified QuickFIX/J config pointed at TrueFix fails outright.

**Why this priority**: Both are P0 findings (`BUG-03`, `BUG-04`) because they violate the project's own
stance-registry contract and undermine the "drop-in `.cfg` compatibility with QuickFIX/J" goal earlier
rounds of work built toward.

**Independent Test**: Can be fully tested by (a) constructing a `.cfg`-only multi-session acceptor with
`AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` set and asserting the described behavior
actually happens end to end, and (b) starting a session from a `.cfg` file using an unmodified
QuickFIX/J-style `JdbcURL=jdbc:...` value plus separate `JdbcUser`/`JdbcPassword` keys and asserting a
working SQL store/log results.

**Acceptance Scenarios**:

1. **Given** a `.cfg` with `AcceptorTemplate`/`DynamicSession`/`AllowedRemoteAddresses` set for a
   multi-session acceptor, **When** the engine starts, **Then** dynamic session template resolution
   and IP allow-listing actually apply to inbound connections, end to end.
2. **Given** `JdbcURL=jdbc:postgresql://host/db` with `JdbcUser=alice`/`JdbcPassword=secret` set,
   **When** a session starts, **Then** TrueFix connects to a PostgreSQL-backed store using those
   credentials.
3. **Given** `JdbcURL=jdbc:mysql://host/db`, **When** resolved, **Then** TrueFix selects the
   MySQL-backed SQL store, matching its existing `mysql://` behavior.
4. **Given** `JdbcURL=jdbc:sqlserver://host;databaseName=db`, **When** resolved, **Then** TrueFix
   selects the MSSQL-backed store (feature-gated on `mssql`).
5. **Given** both a `jdbc:`-prefixed and TrueFix's existing sqlx-native URL forms, **When** either is
   used, **Then** both continue to work (additive, not breaking).

---

### User Story 3 - Give the application layer the protocol-correctness safeguards both reference engines have (Priority: P1)

Three session-state-machine gaps around resend and duplicate-Logon handling: (a) application messages
are always resent verbatim during gap-fill, with no way for the application to substitute a gap-fill
for a now-stale order; (b) a PossDup message with too-low a sequence number is accepted without
checking `OrigSendingTime` against `SendingTime`, an anti-replay safeguard both reference engines have;
(c) a duplicate Logon on an already-logged-on session is silently ignored instead of rejected.

**Why this priority**: All three are P0 findings (`GAP-07`, `GAP-08`, `GAP-18a`) because they're genuine
protocol-correctness/safety gaps relative to both reference engines, not feature-parity conveniences.

**Independent Test**: Can be fully tested with two-process integration tests: (a) trigger a gap-fill
resend where the application vetoes a specific message, asserting a gap-fill is sent in its place; (b)
send a PossDup message with `seq < expected` and `OrigSendingTime` later than `SendingTime`, asserting
rejection; (c) send a second Logon on an already-logged-on session, asserting rejection/disconnect.

**Acceptance Scenarios**:

1. **Given** an application that vetoes a specific stale message during resend, **When** a gap-fill
   resend reaches that message, **Then** a SequenceReset-GapFill is sent instead, and the vetoed
   message is not delivered on the wire.
2. **Given** no veto is registered, **When** a gap-fill resend runs, **Then** behavior is unchanged
   (backward-compatible default).
3. **Given** a message with `seq < expected`, `PossDupFlag=Y`, and `OrigSendingTime` later than
   `SendingTime`, **When** received, **Then** the session logs out and disconnects (same severity as
   Acceptance Scenario 4's duplicate-Logon handling).
4. **Given** a session already in a logged-on state, **When** a second Logon arrives, **Then** the
   session rejects it (logout + disconnect) rather than silently ignoring it.

---

### User Story 4 - Automatic inbound resend chunk continuation (Priority: P2)

TrueFix already auto-chunks outbound resends but requires the counterparty to manually request each
subsequent chunk; both reference engines auto-continue.

**Why this priority**: A real behavioral gap (`GAP-09`) but lower-risk than User Story 3 — missing
continuation is inefficient, not a correctness violation the counterparty can't work around by
re-requesting.

**Independent Test**: Configure a small resend chunk size, have a counterparty gap larger than one
chunk, and assert TrueFix automatically issues successive resend requests without manual intervention.

**Acceptance Scenarios**:

1. **Given** a resend spanning more than one chunk, **When** the first chunk's gap-fill/replay
   completes, **Then** TrueFix automatically issues the next chunk's request without waiting for a new
   external one.

---

### User Story 5 - Session identity completeness: sub-ID, location-ID, and qualifier (Priority: P2)

`SenderSubID`/`SenderLocationID`/`TargetSubID`/`TargetLocationID`/`SessionQualifier` are declared on
the session-identity type but have no construction path and no `.cfg` wiring, so two sessions sharing
the same BeginString/CompID pair distinguished only by a qualifier cannot be configured.

**Why this priority**: Foundational (`GAP-47`) to a deferred routing gap (see Assumptions,
`GAP-19`/`GAP-20`) but no single reported incident makes it more urgent than the P1 items above.

**Independent Test**: Configure two `.cfg` sessions with identical BeginString/SenderCompID/TargetCompID
but different `SessionQualifier` values, and assert both start as distinct, independently addressable
sessions.

**Acceptance Scenarios**:

1. **Given** a `.cfg` session block with `SenderSubID`/`SenderLocationID`/`TargetSubID`/
   `TargetLocationID`/`SessionQualifier` set, **When** resolved, **Then** the resulting session
   identity carries all five values.
2. **Given** two session blocks differing only by `SessionQualifier`, **When** both start, **Then**
   they are recognized as two distinct sessions, not a duplicate/conflicting configuration.

---

### User Story 6 - Initiator connection robustness (Priority: P2)

Three related initiator-connection gaps: (a) no incrementing reconnect-backoff — every retry waits
the same fixed interval instead of stepping up; (b) no local outbound bind address; (c)
`SocketConnectTimeout` is parsed but never actually bounds the connect attempt, so a hung TCP handshake
can block indefinitely.

**Why this priority**: Operational robustness improvements (`GAP-14`, `GAP-15`, `GAP-16`) for
production initiator deployments; real but not correctness-critical.

**Independent Test**: (a) configure a backoff sequence and assert successive reconnect delays step up
and then stick at the last value; (b) configure a local bind address and assert the outbound connection
originates from it; (c) point at an unreachable/black-holed address with a short connect timeout and
assert the attempt fails within that bound rather than hanging.

**Acceptance Scenarios**:

1. **Given** a configured stepped reconnect interval, **When** repeated reconnect attempts fail,
   **Then** each successive wait uses the next configured value, sticking at the last one once
   exhausted.
2. **Given** a local bind address configured, **When** an initiator connects, **Then** the outbound
   socket is bound to that local address before connecting.
3. **Given** a short connect timeout configured and a non-responsive peer, **When** an initiator
   attempts to connect, **Then** the attempt fails within the configured bound instead of blocking
   indefinitely.

---

### User Story 7 - Store/log persistence hardening (Priority: P2)

Three related persistence gaps: (a) session creation time is never persisted anywhere; (b) SQL-family
stores perform message-save and sequence-number-increment as two independent, non-atomic writes,
risking an inconsistent state on a crash between them; (c) every structured log backend stores only
`(id, text)`, with no timestamp or session-identity column, making audit/replay from a shared log table
impossible.

**Why this priority**: Durability/observability hardening (`GAP-38`, `GAP-39`, `GAP-41`) valuable for
production operators but with existing (if imperfect) workarounds today.

**Independent Test**: (a) create a store, assert a queryable creation timestamp exists and updates on
reset; (b) assert the message-save and sequence-increment are wrapped in one transaction (no
inconsistent state observable on simulated failure); (c) write log entries and assert each row carries
a timestamp and session-identity column.

**Acceptance Scenarios**:

1. **Given** a newly created store, **When** queried, **Then** a creation timestamp is available and is
   updated when the store is reset.
2. **Given** a SQL-family store, **When** an outbound message is saved, **Then** the message body and
   the sequence-number increment are committed atomically.
3. **Given** any structured log backend, **When** an entry is logged, **Then** the persisted row
   includes a timestamp and the owning session's identity.

---

### User Story 8 - Config-key stance registry accuracy sweep (Priority: P3)

A documentation-only pass correcting several `.cfg` key stances found to be inconsistent with actual
behavior during the audit: `SocketAcceptProtocol`/`SocketConnectProtocol` (an in-process-transport
alternative with no Rust equivalent) and `JdbcDataSourceName` (a JNDI-only lookup mechanism with no
Rust equivalent) are marked as if they might one day be wired up, when they can't be; separately, the
JDBC pool-tuning and store table-name keys should get real `.cfg` wiring now that User Story 2 threads
a fuller store configuration through instead of a bare URL.

**Why this priority**: Pure documentation/consistency correctness plus a low-cost wiring follow-on to
User Story 2; no functional risk either way, lowest priority.

**Independent Test**: Run the existing key-stance coverage test and assert each corrected key's stance
matches its actual behavior; for the newly-wired pool/table-name keys, assert a `.cfg` session with
them set actually applies those values to the resulting store.

**Acceptance Scenarios**:

1. **Given** the key-stance registry, **When** audited, **Then** the four keys identified as
   inapplicable are marked accordingly, with a documented reason each.
2. **Given** a `.cfg` session with the JDBC pool-tuning and table-name keys set alongside a
   `jdbc:`-style `JdbcURL`, **When** resolved, **Then** the resulting store uses those settings.

---

### User Story 9 - Dictionary and codec model completeness (Priority: P2)

TrueFix's dictionary/codec model is missing several capabilities both reference engines have: 11
additional field types (currently all treated as raw strings, with no format validation); an open-enum
sentinel letting a field accept values outside its enumerated list; per-group child dictionaries for
deep nested-group validation (today a group's members are a flat tag list); a repeating-group API
that's add-only (no replace/remove/get-by-index); header/trailer repeating groups at the core codec
layer (they're kept flat today); a custom, dictionary-declared field emission order; structured
dictionary version metadata enabling a BeginString-vs-dictionary-version match check; a value→label
lookup for enumerated field values; and bundled dictionary sources that are documented subsets rather
than full FIX specifications.

**Why this priority**: A large body of genuine feature-completeness work (`GAP-22`, `GAP-23`,
`GAP-24`, `GAP-25`, `GAP-26`, `GAP-27`, `GAP-28`, `GAP-29`, `GAP-32`, `GAP-33`) confirmed in scope for
this round. None of these individually represent a protocol-correctness violation the way User Story
3's items do, so they sit at the same completeness tier as the other P2 stories rather than above
them.

**Independent Test**: Exercise each of the 11 new field types with format-validation test cases;
construct a dictionary using a nested group and confirm deep validation fires; mutate a repeating
group via replace/remove/get-by-index; decode a message with a header/trailer repeating group; emit a
message using a custom field order; validate a message's BeginString against dictionary version
metadata; look up a field value's human-readable label.

**Acceptance Scenarios**:

1. **Given** a field typed as one of the 11 newly-recognized types (e.g. `CURRENCY`,
   `LOCALMKTDATE`), **When** a message containing it is validated, **Then** its format is checked
   against that type's rules, not merely accepted as a raw string.
2. **Given** a field's dictionary entry marked as an open enum, **When** a message contains a value
   outside the enumerated list, **Then** it's accepted (bypassing the closed-enum check).
3. **Given** a nested repeating group with its own child-dictionary rules, **When** a message with
   that group is validated, **Then** the nested fields are validated against the child dictionary's
   rules, not just a flat member-tag list.
4. **Given** a repeating group with existing entries, **When** the application calls replace/remove/
   get-by-index, **Then** the group's entries update accordingly.
5. **Given** a header or trailer field defined as a repeating group (e.g. `NoHops`), **When** a
   message containing it is decoded, **Then** the group is parsed as a structured repeating group, not
   left flat.
6. **Given** a dictionary with a custom per-message field order, **When** a message of that type is
   emitted, **Then** fields appear in the configured order rather than plain insertion order.
7. **Given** a dictionary with structured version metadata, **When** a message's BeginString doesn't
   match the loaded dictionary's version, **Then** validation flags the mismatch.
8. **Given** a field's dictionary entry with a value description, **When** a caller looks up that
   field value, **Then** the human-readable label is returned.
9. **Given** the bundled dictionary sources, **When** compared field-by-field and message-by-message
   against QuickFIX/J's own bundled dictionary for the same FIX version, **Then** coverage matches
   (every field/message QuickFIX/J's dictionary defines for that version is also present).

---

### Edge Cases

- What happens when a `.cfg` value legitimately needs a literal `#` as its very first character (not a
  comment)? Both reference engines treat a line starting with `#` as a comment unconditionally; this is
  an inherited ambiguity from the config format itself, not a defect introduced by this feature — no
  new escaping mechanism is introduced.
- What happens when a resend-veto callback itself errors? Treated as a veto (fail-safe: suppress the
  stale message and gap-fill), matching the existing `DoNotSend` suppression semantics elsewhere in the
  codebase.
- What happens if a `.cfg` value could ambiguously match both TrueFix's existing sqlx-native URL scheme
  and the newly-recognized JDBC scheme? Resolved by explicit, ordered prefix matching (`jdbc:` prefixes
  checked as a distinct group), so there is no overlap between the two recognized forms.
- How does an already-`Recognized` key's behavior change if this feature downgrades it to
  `Unsupported`, or the reverse? Existing `.cfg` files continue to parse without error either way — the
  stance registry is documentation/introspection metadata, not parsing-time validation, so no `.cfg`
  file that worked before this feature breaks after it.
- What happens to a dictionary that doesn't declare version metadata (an older or hand-written
  dictionary source predating this feature)? The BeginString-vs-version match check from User Story 9
  is skipped rather than treated as a failure, so dictionaries without version metadata keep validating
  exactly as they do today — the new check only activates once metadata is present.
- What happens when a message field value already contains characters or values not in a dictionary's
  fixed enum, but that field isn't marked as an open enum? Unchanged existing behavior: value-not-in-
  enum is still rejected: the open-enum sentinel only affects fields the dictionary explicitly marks.
- What happens when two `.cfg` `[SESSION]` blocks share a `SocketAcceptPort` (so they're auto-grouped
  into one acceptor per FR-006a) but declare conflicting connection-level settings that must be
  singular per physical listener (e.g. different `SocketAcceptAddress` or different TLS configuration)?
  This is a configuration error surfaced as a typed error at startup, not silently resolved by picking
  one session's value over another's.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST treat `#` as a comment start only when it is the first non-whitespace
  character of a `.cfg` line; a `#` appearing after a `key=value` pair's `=` MUST be preserved as part
  of the value verbatim. (`BUG-01`)
- **FR-002**: System MUST correctly decode a `Signature` (tag 89) field using its `SignatureLength`
  (tag 93) length prefix rather than the default `dataTag - 1` convention, including when the signature
  value contains embedded SOH bytes. (`BUG-02`)
- **FR-003**: System MUST recognize standard JDBC-style `JdbcURL` values (`jdbc:<subprotocol>://...`)
  in addition to its existing sqlx-native URL scheme, dispatching to the same SQL/MSSQL backend
  selection logic. (`BUG-04`)
- **FR-004**: System MUST combine separately-configured `JdbcUser`/`JdbcPassword` `.cfg` values into
  the connection credentials when a `JdbcURL` value does not already embed credentials. (`BUG-04`)
- **FR-005**: System MUST NOT mark a `.cfg` key as fully implemented in its stance registry unless
  `.cfg`-driven engine startup actually honors it end-to-end. (`BUG-03`)
- **FR-006**: System MUST construct multi-session acceptors from `.cfg` when `AllowedRemoteAddresses`/
  `DynamicSession`/`AcceptorTemplate` are present, so these keys actually take effect end-to-end rather
  than being silently ignored by a single-session bind. (`BUG-03`; resolved via user clarification —
  real wiring, not a stance-registry downgrade)
- **FR-006a**: System MUST group `.cfg` `[SESSION]` blocks sharing the same `SocketAcceptPort` into one
  shared multi-session acceptor automatically, with no new `.cfg` key required; session blocks with
  distinct `SocketAcceptPort` values MUST continue to bind independently as today. (`BUG-03`, grouping
  mechanism resolved via user clarification)
- **FR-007**: System MUST give the application layer the opportunity to suppress an individual stale
  application message during gap-fill resend, substituting a gap-fill for that message when suppressed.
  (`GAP-07`)
- **FR-008**: System MUST log out and disconnect a session upon receiving an inbound message with
  `MsgSeqNum` lower than expected, `PossDupFlag=Y`, and an `OrigSendingTime` later than the message's
  own `SendingTime` — the same severity as FR-010's duplicate-Logon handling. (`GAP-08`)
- **FR-009**: System MUST provide a configuration switch requiring `OrigSendingTime` to be present on
  PossDup messages evaluated by FR-008. (`GAP-08`, related finding)
- **FR-010**: System MUST reject a Logon message received while the session is already in a logged-on
  state, rather than silently ignoring it. (`GAP-18a`)
- **FR-011**: System MUST automatically continue an inbound chunked resend by issuing the next request
  once the current chunk is satisfied, without requiring the counterparty to request it. (`GAP-09`)
- **FR-012**: System MUST accept and persist `SenderSubID`/`SenderLocationID`/`TargetSubID`/
  `TargetLocationID`/`SessionQualifier` from `.cfg` into the resolved session identity. (`GAP-47`)
- **FR-013**: System MUST treat two sessions differing only by `SessionQualifier` (with otherwise
  identical BeginString/SenderCompID/TargetCompID) as distinct, independently configurable sessions.
  (`GAP-47`)
- **FR-014**: System MUST support a stepped/incrementing reconnect-backoff sequence for initiators,
  sticking at the last configured value once the sequence is exhausted. (`GAP-14`)
- **FR-015**: System MUST support binding an initiator's outbound connection to a configured local
  host/port before connecting. (`GAP-15`)
- **FR-016**: System MUST bound an initiator's connection attempt by a configured timeout, failing the
  attempt rather than blocking indefinitely when exceeded. (`GAP-16`)
- **FR-017**: System MUST persist a session's creation time and update it whenever the session is
  reset. (`GAP-38`)
- **FR-018**: System MUST persist an outbound message body and its corresponding sequence-number
  increment atomically for SQL-family stores. (`GAP-39`)
- **FR-019**: System MUST record a timestamp and the owning session's identity alongside every entry
  written to a structured log backend (SQL/MSSQL/embedded/document-store). (`GAP-41`)
- **FR-020**: System MUST mark `SocketAcceptProtocol`/`SocketConnectProtocol`/`JdbcDataSourceName`/
  `JdbcConnectionTestQuery` as unsupported (with a documented reason) rather than as pending, since none
  has an applicable Rust-side behavior to eventually wire.
- **FR-021**: System MUST make the JDBC connection-pool tuning keys and the SQL store table-name keys
  reachable from `.cfg`, once FR-003/FR-004 thread a fuller store configuration through.
- **FR-022**: System MUST support at least the 11 additional field types identified in the source
  document (price-offset, local-market-date, day-of-month, UTC date, time, currency, exchange,
  multiple-value-string, multiple-string-value, multiple-char-value, country), each validated against
  its own format rules rather than treated as a raw string. (`GAP-22`)
- **FR-023**: System MUST support an open-enum sentinel allowing a field's value to bypass its
  enumerated-value check when the dictionary marks that field as open. (`GAP-23`)
- **FR-024**: System MUST support a per-group child dictionary enabling deep validation of nested
  repeating-group fields, not just a flat list of member tags. (`GAP-24`)
- **FR-025**: System MUST support replacing, removing, and retrieving repeating-group entries by
  index, not just appending. (`GAP-25`)
- **FR-026**: System MUST support parsing and validating repeating groups declared in a message's
  header or trailer, not only its body. (`GAP-26`)
- **FR-027**: System MUST support a custom, dictionary-declared field emission order for a given
  message type, in addition to the existing insertion-order default. (`GAP-27`)
- **FR-028**: System MUST record structured dictionary version metadata (major/minor/service-pack/
  extension-pack) rather than an opaque version string. (`GAP-28`)
- **FR-029**: System MUST validate that an inbound or outbound message's BeginString matches the
  loaded dictionary's version metadata from FR-028, when that metadata is present. (`GAP-32`)
- **FR-030**: System MUST provide a value→label lookup for enumerated field values that have a
  human-readable description in the dictionary source. (`GAP-29`)
- **FR-031**: System MUST expand each bundled dictionary source's field/message coverage to match
  QuickFIX/J's own bundled dictionary for that FIX version (per-source parity, read for
  architecture/behavior understanding only, per Constitution Principle III — not copied verbatim),
  moving beyond the currently-documented subset. (`GAP-33`; coverage target resolved via user
  clarification)

### Key Entities *(include if feature involves data)*

- **Session identity**: gains a real `.cfg`-driven construction path for sub-ID/location-ID/qualifier,
  which today are declared but never populated.
- **Config-key stance registry**: several entries are corrected in both directions (some downgraded
  from overstated coverage, some upgraded once real wiring lands, some reclassified as inapplicable).
- **SQL store configuration**: the `.cfg`-facing SQL/MSSQL store selection gains pool-tuning and
  table-naming fields it's missing today, and gains recognition of the standard JDBC URL form.
- **Field type**: the dictionary model's set of recognized FIX field types grows by 11 variants, each
  with its own format-validation rule.
- **Data dictionary**: gains structured version metadata, an open-enum marker per field, per-group
  child dictionaries, and value→label descriptions; bundled dictionary sources grow in field/message
  coverage.
- **Repeating group**: gains a richer mutation API (replace/remove/get-by-index) and support for
  appearing in a message's header/trailer, not just its body.
- **Multi-session acceptor**: `.cfg`-driven engine startup gains the ability to construct a
  multi-session acceptor (dynamic templates, per-session/allow-list behavior) instead of only
  single-session binds.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A `.cfg` file with a `#` character inside a password value round-trips to the exact same
  value, verified across all test cases exercising the config parser.
- **SC-002**: A message containing a Signature field with an embedded delimiter byte decodes correctly
  in all conformance test runs exercising it.
- **SC-003**: An unmodified QuickFIX/J-style `.cfg` file (standard `jdbc:` URL plus separate user/
  password keys) starts a working SQL-backed session with zero manual translation of config values.
- **SC-004**: Zero configuration keys in the stance registry claim fully-implemented behavior that
  isn't actually reachable from a `.cfg`-only session start, verified by an automated coverage check.
- **SC-005**: An application can suppress an individual stale message during resend in every exercised
  gap-fill scenario, with the counterparty receiving a gap-fill instead of the stale message.
- **SC-006**: A replay attempt using a PossDup message with a falsified earlier SendingTime results in
  a full session logout+disconnect in every exercised scenario.
- **SC-007**: A duplicate Logon on an already-logged-on session is rejected, not silently accepted, in
  every exercised scenario.
- **SC-008**: An inbound resend spanning multiple chunks completes without any manual/external
  re-request in every exercised multi-chunk scenario.
- **SC-009**: Two sessions sharing identical BeginString/SenderCompID/TargetCompID but distinct
  SessionQualifier values can both be started from one `.cfg` file without a configuration conflict.
- **SC-010**: An initiator configured with a stepped reconnect backoff reaches its final (stuck) delay
  value within the expected number of attempts, verified by timing measurements in integration tests.
- **SC-011**: An initiator with a short connect timeout configured against an unresponsive peer fails
  within the configured bound in every exercised timeout scenario, never hanging indefinitely.
- **SC-012**: Every structured log entry written after this feature ships carries a timestamp and
  session-identity column, queryable without external correlation.
- **SC-013**: The existing acceptance-test conformance suite remains green throughout, and gains new
  passing scenarios covering the new protocol-correctness behavior from User Story 3 (resend veto,
  PossDup anti-replay, duplicate-Logon rejection) — this feature, unlike prior config-wiring-only
  rounds, does touch session-state-machine/protocol behavior, so growing the suite (not just keeping it
  unmodified) is the expected outcome.
- **SC-014**: All 11 newly-supported field types correctly validate both well-formed and malformed
  example values in conformance tests.
- **SC-015**: A message with an open-enum field accepts a value outside its enumerated list without a
  validation error, matching the equivalent QuickFIX/J behavior.
- **SC-016**: A nested repeating group's fields are validated against child-dictionary rules in every
  exercised test case, not merely checked as a flat tag list.
- **SC-017**: Repeating-group mutation (replace/remove/get-by-index) behaves correctly across a group's
  full lifecycle in every exercised test case.
- **SC-018**: A header/trailer repeating group round-trips correctly through decode, validate, and
  encode in every exercised test case.
- **SC-019**: A message emitted with a custom field order matches the configured order byte-for-byte.
- **SC-020**: A BeginString/dictionary-version mismatch is flagged in every exercised mismatch scenario
  once version metadata is present.
- **SC-021**: A field value with a defined label resolves to that label via lookup in every exercised
  case.
- **SC-022**: `AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` set in a `.cfg`-only
  multi-session acceptor actually govern connection acceptance and template resolution, verified by
  integration tests exercising each key.
- **SC-023**: For each bundled FIX version, a field-by-field and message-by-message coverage
  comparison against QuickFIX/J's own bundled dictionary for that version shows no gap.

## Assumptions

- This feature builds on completed features 001-004; `docs/engine-comparison-gaps.md` (2026-07-02) is
  the authoritative source for every gap/bug addressed here, cross-referenced by `GAP-##`/`BUG-##`
  identifiers throughout this spec.
- **Scope boundary** (resolved via user clarification): this feature's scope covers the full source
  document's P0/P1 tiers, including the codec/dictionary-model completeness cluster (User Story 9:
  `GAP-22`/`23`/`24`/`25`/`26`/`27`/`28`/`29`/`32`/`33`). This is a deliberately large round spanning
  two different kinds of change — config/session/transport/store wiring, and dictionary/codegen model
  work touching the project's dual-track data-dictionary architecture — accepted explicitly rather than
  split into two features.
- **BUG-03 fix depth** (resolved via user clarification): real multi-session `AcceptorBuilder` wiring
  into `.cfg`-driven engine startup (FR-006), not a stance-registry-only downgrade.
- Genuinely low-priority/architectural items from the source document are treated as out of scope for
  this round regardless of the above answers, per the source document's own recommendation: `GAP-45`
  (runtime session add/remove — explicitly "not recommended as near-term work" in the source), `GAP-37`
  (`MessageStore` `incr_next_*`/`refresh`/`close` — "possibly not applicable" given TrueFix's sans-IO,
  single-owner session architecture), `GAP-42` (bounded log-writer channels — low priority), `GAP-11`/
  `GAP-46` (QuickFIX/Go-only optional features absent from QuickFIX/J too), `GAP-31` (picosecond
  timestamp precision — "practically never triggered"), `GAP-34`/`GAP-36` (diagnostic tooling, not
  session behavior), `GAP-35` (QuickFIX/Go-only field types), `GAP-13` (QuickFIX/J-only reason-code
  version filtering, no QuickFIX/Go precedent), `GAP-40` (`Log` trait severity levels — a
  trait-signature change touching every log backend, with no reported production incident motivating
  it this round).
- `GAP-17` (per-session, rather than acceptor-global, allow-lists) is folded into FR-006's
  `AcceptorBuilder` wiring rather than tracked as a separate requirement.
- `GAP-19`/`GAP-20` (dynamic-template wildcard matching, acceptor routing by sub-ID/location-ID) are out
  of scope for this round: both build on User Story 5 (`GAP-47`) and User Story 2 (`BUG-03`) landing
  first — revisit in a future round once those are in place.
- User Story 8 (documentation-only stance corrections) is scoped to the specific keys identified as
  incorrect in the source document's audit, not a full registry re-audit beyond what that document
  already covers.
- Existing `.cfg` files that work today continue to work after this feature ships — every fix described
  is additive (new recognized forms, new behavior for previously-inert keys) rather than a breaking
  change to already-honored `.cfg` syntax.
