# Feature Specification: Third/Fourth-Pass Audit Remediation

**Feature Branch**: `009-audit-005-remediation`

**Created**: 2026-07-05

**Status**: Draft

**Input**: User description: "按照 docs/todo/005.md的内容 确认有问题后修复。" (Per the contents of
`docs/todo/005.md`, after confirming the issues, fix them.)

## Clarifications

### Session 2026-07-05

- Q: FR-014 (`NEW-10`) covers 7 validation config keys registered as `Impl` but unwired. 5 have a
  direct `ValidationOptions` field; `ValidateSequenceNumbers` and `RejectInvalidMessage` have none.
  Should this feature add real enforcement for those 2, or just downgrade their registry status? →
  A: Downgrade only — mark both as `Recognized` (not `Impl`) in the key registry; no new enforcement
  logic is added for either key in this feature.
- Q: FR-082/FR-083 (`NEW-35`-`NEW-38`, `NEW-41`, `NEW-42`) are pure performance/allocation-hygiene
  items the source audit itself calls correctness-neutral. Should this feature actually implement
  fixes for these, or only track them as a backlog note with no code change? → A: Implement fixes in
  this feature — these become MUST requirements with their own tasks and tests, not tracked-only
  items.
- Q: FR-062 (`NEW-83`) needs the AT harness to represent Logon-time identity-rejection scenarios,
  which today's dynamic-template acceptor can't. Should the harness gain an additional fixed-identity
  mode alongside the existing dynamic one, or should dynamic-template mode be replaced entirely? →
  A: Add a new fixed-identity mode alongside the existing dynamic-template mode — no existing
  scenario's acceptor setup changes.
- Q: FR-042 (`NEW-73`) — every real `.cfg`-driven deployment already gets `reconnect_interval=30`
  correctly via `builder.rs`; only the bare `SessionConfig::new()` struct default is `5`. Should this
  feature change that literal default to `30`, or just document the divergence? → A: Change the
  default to `30` — a one-line field-default change, no behavior change for any `.cfg`-driven
  session.

## User Scenarios & Testing *(mandatory)*

`docs/todo/005.md` is a 2026-07-04 audit of all 9 crates (`truefix-core`, `truefix-dict`,
`truefix-session`, `truefix-store`, `truefix-log`, `truefix-transport`, `truefix-config`, `truefix`,
`truefix-at`), built from **four successive, independent full-codebase review passes**, each
cross-referenced against QuickFIX/J (`quickfixj-core`) and QuickFIX/Go source, and each later pass
re-verifying (and in several cases refuting, downgrading, or folding together) findings from the
passes before it. Every item is cross-referenced against `docs/todo/003.md` and `004.md` and
confirmed **not** already tracked there (`003.md`/`004.md` are out of scope for this feature —
closed by feature 006 / in progress on feature 007 respectively).

**Numbering**: `NEW-01` onward, independent of the `BUG`/`GAP` series used by `003.md`/`004.md`.

**Scope discipline**: The source document itself tags every row with a category — only rows tagged
**Bug** (including narrow-scope, security-flavored, and one latent/not-yet-triggerable variant),
**Test-Gap**, **Tooling-Only**, **Perf/Hygiene**, **Diagnostic-Only**, or **Default-Divergence**
represent work items. Rows tagged **Refuted** (a later pass in the same document disproved an
earlier candidate: `NEW-01`, `NEW-31`, `NEW-57`, `NEW-72`), **Parity-Note** (`NEW-40` — TrueFix
already matches QuickFIX/J's own accepted behavior), or **Feature-Gap** (six missing capabilities
QuickFIX/J or QuickFIX/Go has that TrueFix simply hasn't built yet — not a defect in existing code)
are excluded from this spec's requirements; see Out of Scope below.

**Independent Test** (feature-level): For every `NEW-NN` this spec's user stories cover, a targeted
test exists demonstrating the specific previously-broken behavior now works correctly — reusing the
established table-driven-unit-test / two-process-integration-test / AT-scenario pattern from every
prior feature in this repository.

### User Story 1 - Protocol-correctness, data-loss, and security defects (Priority: P1)

An operator running TrueFix in production hits a class of defects the audit rates as the most
operationally dangerous: a message store backend that silently never persists sequence numbers, an
unauthenticated pre-Logon denial-of-service vector open to any TCP client on a multi-session
acceptor port, a protocol-mandated `BeginSeqNo=0` resend request going unanswered, an acceptor
`ResetOnLogon` setting that is completely inert, a gap-fill `SequenceReset` that bypasses the same
too-high/too-low verification every other inbound message gets (risking silent, permanent message
loss), FIXT 1.1 header/group-decode gaps that break FIX 5.x application-dictionary resolution, a
teardown path that conflates two distinct reset-on-disconnect/reset-on-logout configuration flags,
and related high-severity gaps across transport, store, and config-validation code.

**Why this priority**: Each item in this tier either causes complete functional failure of a shipped
backend, is an easily-reachable pre-authentication attack surface, silently drops or corrupts
protocol state, or defeats a security control (TLS validation) an operator believes is active. These
are the same bar every prior audit-derived P0/P1 tier in this repository (features 001-007) has held
its top tier to.

**Independent Test**: Each item has its own targeted test (unit, two-process integration, or AT
scenario) demonstrating the previously-broken behavior now works correctly, run against a live
acceptor/initiator pair or a live store/transport backend.

**Acceptance Scenarios**:

1. **Given** a `MongoStore`-backed session that calls `set_next_sender_seq`/`set_next_target_seq`,
   **When** the session later restarts, **Then** the persisted sequence numbers reflect the last
   value actually set, not the initial value of 1 (`NEW-54`).
2. **Given** a multi-session `AcceptorBuilder` port, **When** a client opens a TCP connection and
   either sends nothing or trickles bytes that never complete a valid Logon frame, **Then** the
   connection is closed after a bounded time and its receive buffer never grows without limit
   (`NEW-93`).
3. **Given** a peer sends a `ResendRequest` with `BeginSeqNo=0`, **When** TrueFix processes it,
   **Then** it is treated as "from sequence 1" and answered, not silently dropped (`NEW-02`).
4. **Given** an acceptor configured with `ResetOnLogon=Y`, **When** it receives a Logon, **Then** it
   resets both sequence numbers to 1 regardless of whether the inbound Logon itself requested a
   reset (`NEW-03`).
5. **Given** a gap-fill `SequenceReset` arrives with its own `MsgSeqNum` higher or lower than
   expected, **When** it is processed, **Then** it is routed through the same too-high (queue +
   `ResendRequest`) / too-low (reject) verification every other message type receives, instead of
   being applied immediately (`NEW-84`).
6. **Given** a FIX 5.x (FIXT 1.1) session, **When** a message carrying `ApplVerID(1128)` or other
   FIXT transport-header tags is decoded, **Then** those tags land in the message header (not the
   body), and header/trailer repeating groups are structurally decoded whenever `fixt_dictionaries`
   is configured (`NEW-55`, `NEW-58`).
7. **Given** a session configured with only one of `ResetOnLogout`/`ResetOnDisconnect`, **When** a
   teardown occurs for the *other* reason, **Then** only the applicable flag's configured behavior is
   honored — the two flags no longer leak into each other's scenario (`NEW-56`).
8. **Given** a `MongoStore` under concurrent writers, **When** `save_and_advance_sender` is called,
   **Then** the message save and sequence advance happen atomically within one transaction, matching
   every other transactional store backend (`NEW-08`).
9. **Given** TLS client transport with no explicit trust store configured, **When** it connects to a
   server presenting a certificate signed by a publicly-trusted CA, **Then** the handshake succeeds
   using the operating system's default trust store, rather than failing every handshake against an
   empty root store (`NEW-11`).
10. **Given** an acceptor session's schedule window closes while a counterparty is logged on,
    **When** the session exits the window, **Then** a `Logout` is sent before the TCP disconnect,
    not an abrupt drop (`NEW-62`).
11. **Given** a Logon fails dictionary validation and `DisconnectOnError=N`, **When** TrueFix
    responds, **Then** it sends a `Logout` and disconnects (matching every other Logon-rejection
    path) instead of sending a bare `Reject` and leaving the session stranded in `AwaitingLogon`
    (`NEW-63`).
12. **Given** a `MssqlLog`-backed session using a semicolon-form JDBC URL or percent-encoded
    credentials, **When** the log backend parses its connection URL, **Then** it succeeds using the
    same parsing rules as the equivalent store backend (`NEW-09`).
13. **Given** any of the 7 validation-related config keys registered as implemented
    (`ValidateFieldsHaveValues`, `ValidateUnorderedGroupFields`, `ValidateUserDefinedFields`,
    `ValidateSequenceNumbers`, `AllowUnknownMsgFields`, `RejectInvalidMessage`,
    `FirstFieldInGroupIsDelimiter`), **When** an operator sets a non-default value in a `.cfg` file,
    **Then** the setting has an observable effect (or, for the two without a direct `ValidationOptions`
    field, is downgraded to `Recognized` rather than silently ignored) (`NEW-10`).
14. **Given** a trusted proxy sends a PROXY protocol v2 header exceeding 4096 bytes, or delivers its
    header slowly across multiple TCP segments, **When** TrueFix reads the connection preamble,
    **Then** the full header is captured and its bytes are consumed before FIX decoding begins,
    within the existing 5-second header timeout (`NEW-12`, `NEW-13`).
15. **Given** a scheduled initiator whose connect attempt hangs (no `connect_timeout` configured),
    **When** the stop flag is set or a schedule boundary is crossed during that hang, **Then** the
    loop observes it promptly instead of blocking until the connect attempt resolves (`NEW-14`).
16. **Given** `ValidateUserDefinedFields=N`, **When** an inbound message carries an empty-valued or
    repeated user-defined field, **Then** it is not rejected for those reasons — UDF validation is
    fully skipped, not partially applied (`NEW-17`).
17. **Given** a FIX 5.0/5.0SP1/5.0SP2 message (all sharing `BeginString=FIXT.1.1`), **When** it is
    dispatched via the generated `crack_fix50`/`crack_fix50sp1`/`crack_fix50sp2` functions, **Then**
    dispatch is resolved by `ApplVerID(1128)`, not `BeginString` alone, so the correct version's
    structs are used (`NEW-06`).
18. **Given** a field of type `MultipleCharValue` with a valid multi-token wire value (e.g. `"A B"`),
    **When** its enumerated-value constraint is checked, **Then** each space-delimited token is
    checked individually, consistent with `value_ok`'s existing per-token handling (`NEW-07`).
19. **Given** an MSSQL-backed store or log connection, **When** an operator wants real
    certificate-chain validation instead of the current unconditional `trust_cert()` bypass,
    **Then** a configuration option exists to enable it (`NEW-90`).
20. **Given** `LogMessageWhenSessionNotFound` set in a `.cfg` file, **When** an acceptor receives a
    message for an unrecognized session, **Then** the configured value is actually applied (it is
    parsed into `ResolvedSession`/threaded into `Services`), not permanently hardcoded to `false`
    (`NEW-96`).

---

### User Story 2 - Narrower-blast-radius protocol, dictionary, and store defects (Priority: P2)

Real defects confirmed against QuickFIX/J/Go, each affecting a specific edge case, message type, or
backend rather than every session — pre-logon admin message handling, header repeating-group
validation, several field-type range/format checks, group-decode round-trip fidelity, async-log
backpressure, DNS resolution timing, config-load blocking, Mongo/file-store robustness gaps, and two
message-dispatch ordering bugs (`Logout`/`ResendRequest` handled incorrectly when queued behind a
sequence gap).

**Why this priority**: These items are real, confirmed defects, but each is scoped to an edge case
(a specific field type, a specific message ordering, a specific backend) rather than every session
on every connection — narrower blast radius than User Story 1, but still worth fixing before or
shortly after the P1 tier closes.

**Independent Test**: Each item has its own targeted unit or integration test isolating the specific
edge case described, independent of User Story 1's fixes landing first.

**Acceptance Scenarios**:

1. **Given** a `Logout`, `Reject`, or `SequenceReset` admin message arrives before logon with a
   too-high sequence number, **When** it is processed, **Then** no `ResendRequest` is sent —
   matching the "no ResendRequests before logon" rule already applied to non-admin messages
   (`NEW-18`).
2. **Given** a message with a header-level `NoHops(627)` repeating group, **When** it is validated,
   **Then** the header group is checked the same way body groups are (`NEW-19`).
3. **Given** `MonthYear` or `LocalMktDate` field values, **When** format-validated, **Then** they are
   checked against their documented wire formats rather than accepted unconditionally (`NEW-20`).
4. **Given** a field with tag number `0`, **When** decoded, **Then** it is rejected as an invalid tag
   (`NEW-21`).
5. **Given** a repeating group whose wire-declared count doesn't match its actual entry count,
   **When** decoded then re-encoded, **Then** the declared count is not silently mutated to match
   what was actually parsed (`NEW-22`).
6. **Given** a resend of an uncached sequence number, **When** `CachedFileStore::get` is called,
   **Then** the result is inserted into the cache (read-through), not only populated on `save`
   (`NEW-23` — reclassified here as a correctness/consistency item alongside its related P2 group).
7. **Given** a slow or failed database, **When** an async log backend
   (`sql`/`mssql`/`redb`/`mongo`) queues entries faster than it can write them, **Then** the queue is
   bounded, applying backpressure instead of growing memory usage without limit (`NEW-24`).
8. **Given** a `CipherSuites` config value listing one valid name and one typo'd/unrecognized name,
   **When** parsed, **Then** the typo is treated as a configuration error, not silently dropped
   (`NEW-25`).
9. **Given** a reconnecting initiator whose target host resolves to multiple IPs or whose DNS entry
   changes over time, **When** it reconnects, **Then** DNS is re-resolved at connect time rather than
   once at config-load time (`NEW-26`).
10. **Given** `Engine::start` running on any Tokio runtime flavor, **When** it resolves a
    configured hostname, **Then** it uses non-blocking async DNS resolution instead of blocking
    `std::net::ToSocketAddrs` inside the async body (`NEW-27`).
11. **Given** an operational `reset()` call mid-session (not a reconnect), **When** it completes,
    **Then** `resend_target`/`resend_chunk_end`/`test_request_outstanding` are cleared the same way
    they are after a fresh connection (`NEW-32`).
12. **Given** `SocketReuseAddress=N` configured on a multi-session `AcceptorBuilder`, **When** the
    listener binds, **Then** the configured value is honored rather than always binding with
    `SO_REUSEADDR=true` (`NEW-33`).
13. **Given** an inbound `SequenceReset` (non-gap-fill) with `NewSeqNo=0` present, **When**
    processed, **Then** it is rejected as an invalid value rather than silently ignored (`NEW-34`).
14. **Given** an `ApplVerID(1128)` value of the empty string, **When** `application_for` resolves
    the applicable dictionary, **Then** it falls back to `DefaultApplVerID` the same as if the tag
    were absent entirely (`NEW-64`).
15. **Given** a dictionary containing a cyclic repeating-group member definition, **When** a
    crafted message drives `decode_with_groups` recursion, **Then** recursion is bounded (matching
    the existing component-cycle guard) rather than able to overflow the stack (`NEW-65`).
16. **Given** a `DayOfMonth` field value outside 1-31, or a negative `Length`/`SeqNum`/`NumInGroup`
    value, **When** validated, **Then** it is rejected (`NEW-66`, `NEW-67`).
17. **Given** a repeating group's `NoXxx` count field carries a non-numeric value, **When**
    validated via the grouped-decode path, **Then** it is type-checked and reported as
    `IncorrectDataFormat`, consistent with the flat-decode path (`NEW-68`).
18. **Given** an empty-valued field inside a repeating group entry, **When** validated, **Then** it
    is rejected under the same `TagSpecifiedWithoutValue` rule applied to top-level fields
    (`NEW-69`).
19. **Given** a gap-fill `SequenceReset` missing its required `NewSeqNo` tag, **When** processed,
    **Then** it is rejected rather than silently accepted with no sequence update (`NEW-70`).
20. **Given** no dictionary is configured and an acceptor receives a Logon omitting `HeartBtInt`,
    **When** processed, **Then** the Logon is rejected rather than silently retaining the acceptor's
    prior configured heartbeat interval (`NEW-71`).
21. **Given** a session is constructed programmatically without going through `truefix-config`,
    **When** `reconnect_interval` is left at its default, **Then** the value is 30 seconds, matching
    QuickFIX/J and eliminating the residual API-level inconsistency (`NEW-73`).
22. **Given** an incoming message following a run of non-FIX garbage, **When** frame resynchronization
    runs, **Then** a valid frame is not discarded solely because the preceding garbage didn't end in
    an SOH byte (`NEW-74`).
23. **Given** two connections attempt to establish the same Mongo `session_id` concurrently, **When**
    both race to create the session row, **Then** a unique constraint (index or upsert) prevents
    duplicate session documents (`NEW-75`).
24. **Given** `BodyLog::reset` and `BodyLog::append` could ever run concurrently, **When** `reset`
    truncates the file, **Then** it acquires the index lock before truncating, matching `append`'s
    locking order (`NEW-76`).
25. **Given** a file-store creation-time file exists but contains unparseable content, **When**
    loaded, **Then** it surfaces a typed error, consistent with how the seqnum files already handle
    this case, instead of silently defaulting to the Unix epoch (`NEW-77`).
26. **Given** `render_members_ordered` is given a `field_order` list containing a duplicate tag,
    **When** encoding, **Then** the member is emitted once, not once per occurrence in the order list
    (`NEW-80`).
27. **Given** a `FieldMap` containing duplicate entries for the same tag (e.g. from a decoded
    out-of-order message), **When** `set` updates that tag, **Then** stale duplicate entries are
    removed rather than left to round-trip onto the wire (`NEW-81`).
28. **Given** a `Logout` with a too-high `MsgSeqNum` arrives after logon, **When** processed,
    **Then** it is handled as a Logout immediately (reply, honor reset flags, disconnect) rather than
    queued behind a `ResendRequest` (`NEW-85`).
29. **Given** a too-high `ResendRequest` already answered immediately per existing behavior, **When**
    `drain_queue` later reaches that same queued message, **Then** only the sequence counter advances
    — the resend response is not built and sent a second time (`NEW-86`).
30. **Given** a PROXY protocol header that is fully valid but declares an `Unknown`/`Unspecified`/
    Unix-socket address, **When** it is parsed, **Then** its byte length is still consumed from the
    stream even though the address itself isn't remapped (`NEW-94`).

---

### User Story 3 - Hardening, dictionary/codegen tooling, and AT-harness coverage (Priority: P3)

Lower-severity robustness and performance items across the codebase (allocation hygiene, algorithmic
complexity, diagnostic quality, config defaults), dictionary-regeneration tooling defects that affect
only the `generate-dict`/`generate-code` CLI and `build.rs` pipeline (not the shipped, hand-verified
`.fixdict` files), and gaps in the `truefix-at` acceptance-test harness that reduce the harness's
ability to catch real regressions in the areas it already exercises.

**Why this priority**: None of these change correct, shipped runtime behavior today (tooling-only
items never reach a running session; hygiene items are allocation/complexity observations, not
defects; harness gaps mean a real production bug in the covered area might go undetected by AT, but
the production code itself isn't shown to be wrong). Still worth closing for release hygiene and to
avoid accumulating tooling debt and test blind spots.

**Independent Test**: Each item has its own targeted unit test, `generate-dict`/`generate-code`
CLI-level test, or new/extended AT scenario, independent of User Story 1/2 landing first.

**Acceptance Scenarios**:

1. **Given** `generate-dict --format fix-repository` converts a repeating component, **When** it
   emits the group definition, **Then** the count field is excluded from members and the delimiter
   is the first remaining field — matching the hand-normalized shipped `.fixdict` format and the
   already-correct `orchestra.rs` converter (`NEW-05`).
2. **Given** `orchestra.rs`'s Orchestra-XML-to-dictionary converter encounters any of the 10 type
   tokens `fix_repository::map_type` already handles, **Then** it converts successfully instead of
   aborting with `UnknownType` (`NEW-60`).
3. **Given** `src/codegen.rs` is edited without touching any `.fixdict` file, **When** the crate is
   rebuilt, **Then** `build.rs` regenerates code from the new `codegen.rs` rather than serving a
   stale `OUT_DIR/generated.rs` (`NEW-61`).
4. **Given** `codegen.rs`'s dictionary parser encounters an unknown directive or a malformed
   field/group/message line, **When** parsing, **Then** it reports an error consistent with the
   runtime `parser::parse`'s handling of the same input, instead of silently skipping it (`NEW-78`).
5. **Given** two dictionary enum values whose sanitized identifiers collide, **When** `codegen`
   emits the enum, **Then** duplicate variants are deduplicated rather than producing uncompilable
   generated code (`NEW-79`).
6. **Given** a non-UTF-8 dictionary file, **When** `generate_code`'s CLI processes it, **Then** the
   runtime-hash and codegen-hash tracks use consistent byte handling so the dual-track hash check
   doesn't silently diverge (`NEW-82`).
7. **Given** `truefix-at`'s `read_message`, **When** a read times out, decode fails, or the peer
   disconnects, **Then** these three outcomes are distinguishable rather than all collapsing to
   `None` — removing the false-positive risk in `ExpectDisconnect` and misleading `Expect` failures
   (`NEW-15`, `NEW-16`).
8. **Given** the AT harness's `start_acceptor` for FIX versions other than 4.2/4.4, **When** field
   validation is exercised, **Then** a validator/dictionary is available so field-validation
   scenarios run for all 9 supported versions, not just two (`NEW-28`).
9. **Given** the AT harness's latency/bad-timestamp scenarios, **When** they assert on the resulting
   `Logout`, **Then** they also assert on `Text(58)`/the session-reject reason, not merely that a
   Logout of any kind occurred (`NEW-29`).
10. **Given** an AT scenario completes all its steps, **When** the runner finishes, **Then** it
    asserts the read buffer is empty (no unexpected trailing message) (`NEW-30`).
11. **Given** the AT harness needs to exercise Logon-time identity rejection (`InvalidSenderCompID`/
    `InvalidTargetCompID`/wrong-`BeginString`), **When** running those scenarios, **Then** a
    fixed-identity (non-dynamic-template) acceptor mode is available to make them representable
    (`NEW-83`).
12. **Given** the AT harness's `check_match`, **When** comparing an actual message against expected
    fields, **Then** unexpected extra/erroneous fields are also flagged, not only missing expected
    ones (`NEW-50`).
13. **Given** AT scenarios exercising server-sent sequences, **When** run, **Then** outbound
    `MsgSeqNum(34)` ordering is verified for the scenarios that currently don't check it (`NEW-51`).
14. **Given** `server_acceptance_suite_passes` runs multiple scenarios, **When** one scenario fails,
    **Then** the failure is reported per-scenario rather than failing the entire suite as one
    all-or-nothing test (`NEW-52`).
15. **Given** a `.fixdict` file loaded at runtime via `DataDictionary::extend()` or `load_from_file`,
    **When** it contains duplicate tag/name/msg_type entries, **Then** a duplicate is reported as an
    error rather than silently overwriting the earlier definition (`NEW-43`).
16. **Given** a STRING-typed enum value containing a literal `#` character, **When**
    `strip_comment()` processes the dictionary line, **Then** the value is not truncated at the `#`
    (`NEW-44`).
17. **Given** a FIX 5.0/5.0SP1/5.0SP2 `BeginString`, **When** `parse_fix_begin_string` parses it for
    validation purposes, **Then** the three sub-versions are distinguished rather than all collapsing
    to `(5, 0)` (`NEW-45`).
18. **Given** a config file with a self-referential or circular `${var}` reference, **When**
    resolved, **Then** an error is reported instead of emitting the literal unresolved `${...}` text
    (`NEW-46`).
19. **Given** a `Monitor` entry created per accepted connection, **When** that connection
    disconnects, **Then** the entry is removed rather than accumulating for the lifetime of a
    long-running acceptor process (`NEW-48`).
20. **Given** an HTTP CONNECT proxy response, **When** its status line is checked, **Then** the
    status token is validated as numeric (not just `starts_with('2')`), and the response is read
    efficiently rather than one byte at a time (`NEW-49`).
21. **Given** `DataDictionary::extend()` merges a second dictionary whose field name collides with
    an existing field under a different tag, **When** merged, **Then** the collision is detected
    (rather than silently shadowing), and `member_tags` is recomputed to reflect the merge
    (`NEW-53`).
22. **Given** a non-Logon message triggers a latency, CompID-mismatch, or PossDup-falsification
    disconnect, **When** TrueFix responds, **Then** it sends a session-level `Reject` with the
    appropriate `SessionRejectReason` before the `Logout`, matching QuickFIX/J's two-message sequence
    (`NEW-87`).
23. **Given** an inbound Logon's `DefaultApplVerID(1137)`, **When** processed under FIXT 1.1,
    **Then** it is validated against the transport dictionary's known `ApplVerID` values before being
    accepted, rejecting the Logon on a mismatch (`NEW-88`).
24. **Given** a `.cfg`-driven session omits `LogoutTimeout`, **When** its default is applied,
    **Then** it matches QuickFIX/J's 2-second default rather than the current 10-second default
    (`NEW-89`).
25. **Given** `Engine::shutdown()` is called (or `Engine` is dropped) while an async DB-backed log
    backend (`sql`/`mssql`/`mongo`/`redb`) still has queued, unwritten entries, **When** shutdown
    completes, **Then** those entries are flushed/awaited rather than silently dropped with the
    background writer task (`NEW-91`).
26. **Given** `Engine::start` builds its multi-session acceptor groups from a `HashMap`, **When** a
    partial-startup failure occurs with `ContinueInitializationOnError=N`, **Then** groups are
    processed in a stable, reproducible order (e.g. sorted by socket address) rather than
    process-random `HashMap` iteration order (`NEW-92`).
27. **Given** a configured TLS trust-store file/bytes contains one or more malformed certificates,
    **When** loaded, **Then** the number of certificates successfully added is tracked and surfaced
    (at minimum a warning, or an error when the resulting store ends up empty) rather than silently
    dropping bad entries with no diagnostic (`NEW-95`).
28. **Given** `decode`/`tokenize_validated` is called directly (bypassing `frame_length`), **When**
    a message declares a body length exceeding `MAX_BODY_LEN`, **Then** it is rejected consistently
    with the transport-layer framing cap, closing the public-API inconsistency (downgraded from
    the originally-reported P1 DoS framing; `frame_length` itself was already capped) (`NEW-04`).
29. **Given** repeated field-boundary scanning during decode/framing, double/triple allocation of
    field values on decode, per-field `tag.to_string()` allocation on encode, and
    `render_members_ordered`'s O(m × o) member-rendering, **When** each is fixed, **Then** decode/
    encode/framing behavior is unchanged (same wire output, same accepted/rejected inputs) while
    allocations and complexity are reduced (`NEW-35`, `NEW-36`, `NEW-37`, `NEW-38`) — per
    Clarifications, these are implemented in this feature, not merely tracked.
30. **Given** `SessionId::Display`, **When** two sessions differ only by `SubID`/`LocationID`,
    **Then** the rendered log identity distinguishes them (`NEW-39`).
31. **Given** `store/file.rs`'s `BodyLog::read` opens a new file handle per message during a large
    `range()` resend, and `store/redb.rs`'s `reset()` collects all keys before removing them
    one-by-one, **When** each is fixed, **Then** a large resend reuses a single file handle and a
    store reset avoids the intermediate full-key collection, with behavior otherwise unchanged
    (`NEW-41`, `NEW-42`) — implemented in this feature per Clarifications.
32. **Given** an `UnresolvedVariable` config error occurs after per-session merging, **When**
    reported, **Then** the original source line number is preserved rather than always showing
    `line: 0` (`NEW-47`).

---

### Edge Cases

- What happens when a fix for one `NEW-NN` item and a fix for another interact on the same code path
  (e.g. `NEW-56`'s reason-aware teardown refactor and `NEW-62`'s schedule-exit `Logout` both touch
  session teardown)? Both must be verified together with a combined test, not just independently.
- How does the system handle an operator who has never configured `SocketTrustStore` and depends on
  today's (broken) empty-root-store behavior for some other reason? The new default-trust-store
  fallback (`NEW-11`) must remain overridable — an explicit trust store, when configured, still
  takes precedence.
- How does the system handle a pre-existing Mongo store whose `sender`/`target` fields are already
  stuck at 1 due to `NEW-54`'s bug? See Assumptions — this fix is forward-only; no automatic
  migration of already-corrupted rows is performed.
- What happens to a scenario or test that currently (incorrectly) passes because of one of the
  `Test-Gap` items in User Story 3 (e.g. `NEW-15`/`NEW-16`'s conflated `None`) once the harness is
  fixed to be stricter — do any *production* scenarios start failing? Any such failure indicates a
  separate, previously-hidden production defect and must be triaged, not suppressed by loosening the
  new assertion.

## Requirements *(mandatory)*

### Functional Requirements

#### User Story 1 — Protocol-correctness, data-loss, and security (P1)

- **FR-001**: ~~`MongoStore::set_seq` MUST persist to the actual `sender`/`target` field name, not a
  literal string key mismatched to the intended field (`NEW-54`).~~ **REFUTED during
  `/speckit-implement` (2026-07-05)**: a standalone reproduction of `bson::doc! { "$set": { field:
  seq as i64 } } }` (bson 2.15.0) confirmed the bare identifier `field` is evaluated as an
  expression (the variable's value), not stringified to the literal `"field"` as `NEW-54` claimed.
  `MongoStore::set_seq`'s current code is already correct — no fix applies. This is the one item in
  `docs/todo/005.md` this feature found to be factually wrong despite the source document's own
  four-pass re-verification claim for it.
- **FR-002**: A multi-session `AcceptorBuilder` connection's pre-Logon read loop MUST be bounded by a
  timeout (reusing the existing `logon_timeout` setting) and MUST NOT allow its receive buffer to
  grow without limit before a complete Logon frame is observed (`NEW-93`).
- **FR-003**: `on_resend_request` MUST treat `BeginSeqNo=0` as sequence 1, not as an invalid/absent
  value that causes the request to be silently dropped (`NEW-02`).
- **FR-004**: An acceptor's `on_logon` handling MUST check `config.reset_on_logon` and reset both
  sequence numbers to 1 when true, regardless of the inbound Logon's own `ResetSeqNumFlag` (`NEW-03`).
- **FR-005**: `on_sequence_reset`'s gap-fill branch MUST route through the same
  too-high/too-low (`Ordering::Greater`/`Ordering::Less`) dispatch every other message type receives,
  rather than applying `NewSeqNo` unconditionally (`NEW-84`).
- **FR-006**: `tags::is_header` MUST include the FIXT 1.1 transport-header tags (`1128`, `1129`,
  `1130`, `1156`, and the `1351`/`1352`-`1355` `NoApplIDs` group) so they route into
  `message.header`, not `message.body` (`NEW-55`).
- **FR-007**: `classify_buffered` MUST engage group-aware decoding
  (`HeaderTrailerGroupsOnly`) when `fixt_dictionaries` is configured, not only when
  `services.validator` is set (`NEW-58`).
- **FR-008**: Session teardown MUST determine which of `reset_on_logout`/`reset_on_disconnect` to
  honor based on the actual reason for the teardown (graceful Logout vs. any other disconnect),
  rather than applying both flags with `||` regardless of cause (`NEW-56`).
- **FR-009**: `MongoStore` MUST implement a transactional `save_and_advance_sender` override,
  matching the atomicity guarantee `SqlStore`/`MssqlStore`/`RedbStore`/`FileStore` already provide
  (`NEW-08`).
- **FR-010**: TLS client transport MUST fall back to the operating system's default/native trust
  store when no explicit `SocketTrustStore`/`SocketTrustStoreBytes` is configured, rather than using
  an empty `RootCertStore` (`NEW-11`).
- **FR-011**: A logged-on acceptor session exiting its configured schedule window MUST send a
  `Logout` before disconnecting (`NEW-62`).
- **FR-012**: A Logon that fails dictionary validation MUST be routed through the same
  Logout-and-disconnect rejection path used by every other Logon-rejection case, regardless of
  `disconnect_on_error` (`NEW-63`).
- **FR-013**: `truefix-log`'s `MssqlLog::parse_url` MUST support the semicolon-form connection
  string and percent-decoded credentials, matching `truefix-store`'s `MssqlStore::parse_url`
  (`NEW-09`).
- **FR-014**: `builder.rs::resolve_validator` MUST wire the 5 validation config keys that have a
  direct `ValidationOptions` field (`ValidateFieldsHaveValues`, `ValidateUnorderedGroupFields`,
  `ValidateUserDefinedFields`, `AllowUnknownMsgFields`, `FirstFieldInGroupIsDelimiter`). The
  remaining 2 (`ValidateSequenceNumbers`, `RejectInvalidMessage`) MUST be downgraded from `Impl` to
  `Recognized` in the key registry — no new `ValidationOptions` field or enforcement logic is added
  for either in this feature (`NEW-10`; see Clarifications).
- **FR-015**: PROXY protocol header parsing MUST accommodate v2 headers up to the spec's 64 KiB
  maximum (not just the current 4096-byte peek buffer) and MUST continue reading until the full
  header is available or the outer timeout fires, rather than giving up after a fixed iteration
  count (`NEW-12`, `NEW-13`).
- **FR-016**: `run_scheduled_initiator`'s connect attempt MUST be wrapped so the stop flag and
  schedule-boundary checks are observed promptly even while a connect attempt is outstanding
  (`NEW-14`).
- **FR-017**: Validation MUST fully skip user-defined fields when `validate_user_defined_fields` is
  false — the UDF short-circuit MUST run before the empty-value and repeated-tag checks, not after
  (`NEW-17`).
- **FR-018**: The generated `crack_fix50`/`crack_fix50sp1`/`crack_fix50sp2` dispatch functions MUST
  distinguish FIX 5.0/SP1/SP2 by `ApplVerID(1128)`, not solely by `BeginString` (`NEW-06`).
- **FR-019**: `FieldDef::allows()` MUST check `MultipleCharValue` values per space-delimited token,
  consistent with `value_ok`'s existing per-token handling (`NEW-07`).
- **FR-020**: MSSQL store and log connections MUST offer a configuration option to enable real
  TLS certificate-chain validation instead of unconditionally calling `trust_cert()` (`NEW-90`).
- **FR-021**: `LogMessageWhenSessionNotFound` MUST be parsed from `.cfg` into `ResolvedSession` and
  threaded into every `Services` construction site, rather than being permanently hardcoded to
  `false` (`NEW-96`).

#### User Story 2 — Narrower-blast-radius defects (P2)

- **FR-022**: Pre-logon `Logout`/`Reject`/`SequenceReset` admin messages with a too-high sequence
  number MUST NOT trigger a `ResendRequest` (`NEW-18`).
- **FR-023**: Header-level `NoHops(627)` repeating groups MUST be validated the same way body-level
  groups are (`NEW-19`).
- **FR-024**: `MonthYear` and `LocalMktDate` field values MUST be format-validated against their
  documented wire formats (`NEW-20`).
- **FR-025**: Tag number `0` MUST be rejected as an invalid tag during decode (`NEW-21`).
- **FR-026**: Repeating-group decode MUST NOT silently mutate a declared `NoXxx` count on re-encode
  to match a differing actual entry count (`NEW-22`).
- **FR-027**: `CachedFileStore::get` MUST insert a cache-miss result into the cache (read-through),
  not only populate the cache on `save` (`NEW-23`).
- **FR-028**: Async log backends (`sql`/`mssql`/`redb`/`mongo`) MUST use a bounded channel with
  backpressure instead of an unbounded channel (`NEW-24`).
- **FR-029**: `CipherSuites` parsing MUST error if any individual listed name fails to match a known
  suite, not only when all names fail to match (`NEW-25`).
- **FR-030**: Initiator connection targets MUST be DNS-resolved at connect time, not only once at
  config-load time (`NEW-26`).
- **FR-031**: `Engine::start`'s hostname resolution MUST use non-blocking async DNS resolution
  (`NEW-27`).
- **FR-032**: `reset_sequences()` MUST clear `resend_target`, `resend_chunk_end`, and
  `test_request_outstanding`, matching what `on_connected` already clears (`NEW-32`).
- **FR-033**: `AcceptorBuilder::bind` MUST honor a configured `SocketReuseAddress=N` instead of
  hardcoding `SO_REUSEADDR=true` (`NEW-33`).
- **FR-034**: A present `NewSeqNo=0` in a non-gap-fill `SequenceReset` MUST be rejected as an
  invalid value (`NEW-34`).
- **FR-035**: `FixtDictionaries::application_for` MUST treat an empty-string `ApplVerID` the same as
  an absent one, falling back to `DefaultApplVerID` (`NEW-64`).
- **FR-036**: Cyclic repeating-group member definitions MUST be detected (mirroring the existing
  component-cycle guard) or recursion depth MUST otherwise be bounded during group decode
  (`NEW-65`).
- **FR-037**: `DayOfMonth` values MUST be range-checked to 1-31, and `Length`/`SeqNum`/`NumInGroup`
  values MUST reject negatives, in `FieldType::value_ok` (`NEW-66`, `NEW-67`).
- **FR-038**: A non-numeric repeating-group count field MUST be type-checked and reported as
  `IncorrectDataFormat` on the grouped-decode validation path, consistent with the flat-decode path
  (`NEW-68`).
- **FR-039**: Empty-valued fields inside repeating-group entries MUST be rejected under the same
  `TagSpecifiedWithoutValue` rule applied at the top level (`NEW-69`).
- **FR-040**: A gap-fill `SequenceReset` missing its required `NewSeqNo` MUST be rejected, not
  silently accepted with no sequence update (`NEW-70`).
- **FR-041**: With no dictionary configured, an acceptor MUST reject a Logon that omits
  `HeartBtInt` rather than silently retaining its previously-configured heartbeat interval
  (`NEW-71`).
- **FR-042**: `SessionConfig::new()`'s bare struct default for `reconnect_interval` MUST be changed
  from `5` to `30`, matching QuickFIX/J's default and removing the residual API-level inconsistency
  for callers that bypass `truefix-config` (`NEW-73`; see Clarifications).
- **FR-043**: Frame resynchronization MUST NOT discard a valid subsequent frame solely because
  preceding garbage bytes didn't end in an SOH (`NEW-74`).
- **FR-044**: The Mongo `sessions` collection MUST prevent duplicate rows for the same `session_id`
  under concurrent connects (via a unique index and/or upsert) (`NEW-75`).
- **FR-045**: `BodyLog::reset` MUST acquire the index lock before truncating the underlying file,
  matching `append`'s lock ordering (`NEW-76`).
- **FR-046**: An unparseable (but present) file-store creation-time file MUST surface a typed error,
  consistent with the seqnum files' existing error handling (`NEW-77`).
- **FR-047**: `render_members_ordered` MUST NOT emit a member more than once when its tag appears
  more than once in `field_order` (`NEW-80`).
- **FR-048**: `FieldMap::set` MUST remove stale duplicate entries for a tag it updates, not leave
  them to round-trip onto the wire (`NEW-81`).
- **FR-049**: A `Logout` with a too-high `MsgSeqNum` arriving after logon MUST be processed
  immediately (not queued behind a `ResendRequest`), mirroring the existing special-case for
  too-high `ResendRequest` (`NEW-85`).
- **FR-050**: `drain_queue`'s `process_in_order` dispatch MUST NOT re-invoke `on_resend_request` for
  a `ResendRequest` that was already answered immediately when first received as too-high — only the
  sequence counter advances (`NEW-86`).
- **FR-051**: A fully-parsed PROXY protocol header declaring an `Unknown`/`Unspecified`/Unix-socket
  address MUST still have its byte length consumed from the stream (`NEW-94`).

#### User Story 3 — Hardening, tooling, and AT-harness coverage (P3)

- **FR-052**: `fix_repository.rs`'s `convert()` MUST exclude the count field from a repeating
  group's members and set the delimiter to the first remaining field, matching `orchestra.rs` and
  the shipped hand-normalized `.fixdict` format (`NEW-05`).
- **FR-053**: `orchestra.rs::map_type` MUST support the same type tokens `fix_repository::map_type`
  already supports (`NEW-60`).
- **FR-054**: `build.rs` MUST declare `cargo:rerun-if-changed=src/codegen.rs` (`NEW-61`).
- **FR-055**: `codegen.rs::parse_dict` MUST report an error for unknown directives or malformed
  field/group/message lines, consistent with the runtime `parser::parse`'s handling (`NEW-78`).
- **FR-056**: `codegen.rs`'s enum-variant sanitization MUST deduplicate colliding sanitized
  identifiers (`NEW-79`).
- **FR-057**: `generate_code`'s CLI MUST use consistent byte handling between the runtime-parse hash
  and the codegen hash for non-UTF-8 dictionary input (`NEW-82`).
- **FR-058**: `truefix-at`'s `read_message` MUST distinguish timeout, decode failure, and clean
  disconnect/EOF as three distinct outcomes (`NEW-15`, `NEW-16`).
- **FR-059**: The AT harness's `start_acceptor` MUST provide a validator/dictionary for all 9
  supported FIX versions, not only FIX.4.2/FIX.4.4 (`NEW-28`).
- **FR-060**: Latency/bad-timestamp AT scenarios MUST assert on the resulting Logout's `Text(58)`/
  session-reject reason, not merely that a Logout occurred (`NEW-29`).
- **FR-061**: `run_scenario` MUST assert the read buffer is empty after all steps complete
  (`NEW-30`).
- **FR-062**: The AT harness MUST provide a fixed-identity (non-dynamic-template) acceptor mode,
  additive alongside the existing dynamic-template mode (which remains unchanged for its current
  scenarios), capable of representing Logon-time identity-rejection scenarios
  (`InvalidSenderCompID`/`InvalidTargetCompID`/wrong-`BeginString`) (`NEW-83`; see Clarifications).
- **FR-063**: `check_match` MUST flag unexpected/extra fields present in an actual message, not only
  missing expected fields (`NEW-50`).
- **FR-064**: AT scenarios exercising server-sent sequences MUST verify outbound `MsgSeqNum(34)`
  ordering where not already checked (`NEW-51`).
- **FR-065**: `server_acceptance_suite_passes` MUST report per-scenario pass/fail rather than one
  all-or-nothing test result (`NEW-52`).
- **FR-066**: The runtime dictionary parser (`parser.rs`) MUST report an error on duplicate tag/
  name/msg_type definitions rather than silently overwriting (`NEW-43`).
- **FR-067**: `strip_comment()` MUST NOT truncate a STRING-typed enum value containing a literal `#`
  character (`NEW-44`).
- **FR-068**: `parse_fix_begin_string` MUST distinguish FIX 5.0/5.0SP1/5.0SP2 rather than collapsing
  all three to `(5, 0)` (`NEW-45`).
- **FR-069**: Config variable resolution MUST report an error for self-referential/circular `${var}`
  references instead of emitting literal unresolved text (`NEW-46`).
- **FR-070**: `Monitor` MUST remove a connection's entry on disconnect rather than retaining it for
  the acceptor process's lifetime (`NEW-48`).
- **FR-071**: HTTP CONNECT proxy response handling MUST validate the status token is numeric and
  MUST NOT read the response one byte at a time (`NEW-49`).
- **FR-072**: `DataDictionary::extend()` MUST detect `field_by_name` collisions between fields of
  different tags and MUST recompute `member_tags` after a merge (`NEW-53`).
- **FR-073**: A non-Logon message that triggers a latency, CompID-mismatch, or
  PossDup-falsification disconnect MUST send a session-level `Reject` with the appropriate
  `SessionRejectReason` before the `Logout` (`NEW-87`).
- **FR-074**: An inbound Logon's `DefaultApplVerID(1137)` MUST be validated against the transport
  dictionary's known `ApplVerID` values under FIXT 1.1, rejecting the Logon on mismatch (`NEW-88`).
- **FR-075**: `builder.rs`'s default for `LogoutTimeout` MUST match QuickFIX/J's 2-second default
  (`NEW-89`).
- **FR-076**: `Engine::shutdown()`/`Drop` MUST flush or await outstanding entries queued in each
  async DB-backed log backend's writer task before completing (`NEW-91`).
- **FR-077**: `Engine::start` MUST process multi-session acceptor groups in a stable, reproducible
  order (e.g. sorted by socket address) rather than `HashMap` iteration order (`NEW-92`).
- **FR-078**: TLS trust-store loading MUST track and surface the count of certificates that failed
  to load (at minimum a warning; an error when the resulting store is empty despite non-empty input)
  (`NEW-95`).
- **FR-079**: `tokenize_validated`/`decode` MUST enforce `MAX_BODY_LEN` independently of
  `frame_length`, closing the public-API inconsistency between the two entry points (`NEW-04`).
- **FR-080**: `SessionId::Display` MUST include `SubID`/`LocationID` when present, so sessions
  differing only by those fields render distinguishably in logs (`NEW-39`).
- **FR-081**: `UnresolvedVariable` config errors MUST report the original source line number rather
  than always reporting `line: 0` (`NEW-47`).
- **FR-082**: `store/file.rs`'s `BodyLog::read` MUST avoid opening a new file handle per message
  during a `range()` resend, and `store/redb.rs`'s `reset()` MUST avoid collecting all keys into an
  intermediate `Vec` before removing them one-by-one — both fixed with behavior otherwise unchanged
  (`NEW-41`, `NEW-42`; see Clarifications).
- **FR-083**: Manual linear byte-scanning in the decode/framing hot path, double/triple allocation
  of field values on decode, per-field `to_string()` allocation on encode, and
  `render_members_ordered`'s O(m × o) complexity MUST each be reduced, with decode/encode/framing
  behavior (accepted/rejected inputs, wire output) otherwise unchanged (`NEW-35`, `NEW-36`,
  `NEW-37`, `NEW-38`; see Clarifications).

## Key Entities *(include if feature involves data)*

- **Mongo session document**: The per-`session_id` row in `MongoStore`'s `sessions` collection
  holding `sender`/`target` next-sequence-number values; `NEW-54` and `NEW-75` both concern its
  correctness/uniqueness.
- **`ResolvedSession`**: The fully-resolved, per-session configuration structure `truefix-config`
  produces from a `.cfg` file; several FRs (`NEW-10`, `NEW-96`) add or correct fields threaded
  through it into `Services`/`SessionConfig`.
- **FIXT 1.1 transport header**: The set of tags (`1128`-`1130`, `1156`, `1351`-`1355`) that must
  route into a decoded message's header rather than its body under a FIXT 1.1 dual-dictionary
  configuration (`NEW-55`, `NEW-58`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every one of the 90 `Bug`/`Test-Gap`/`Tooling-Only`/`Perf-Hygiene`/`Diagnostic-Only`/
  `Default-Divergence` items tracked in `docs/todo/005.md` (`NEW-02` through `NEW-96`, excluding the
  4 refuted items and the 1 parity-note) has either a passing regression test demonstrating the fix,
  or (for the 4 items explicitly marked hygiene-only in User Story 3) a tracked follow-up with no
  regression in existing coverage.
- **SC-002**: A `MongoStore`-backed session that persists and reopens shows the correct advanced
  sequence numbers, with zero silent resets to (1, 1) across 100 consecutive save/reopen cycles in a
  test.
- **SC-003**: A simulated slow-Logon / no-Logon connection to a multi-session acceptor is
  disconnected within the configured `logon_timeout` and never grows its receive buffer past a fixed
  bound, across a load test of at least 500 concurrent such connections.
- **SC-004**: 100% of the existing `truefix-at` conformance suite continues to pass after every fix
  in this feature lands, with zero newly-introduced false negatives.
- **SC-005**: Every FIX 5.x AT scenario exercising a message with `ApplVerID` or other FIXT
  transport-header tags shows those tags present in the decoded header (not body) in test
  assertions.
- **SC-006**: `generate-dict --format fix-repository` and `generate-dict --format orchestra` (once
  fixed) produce byte-identical `group` lines to the hand-normalized shipped `.fixdict` files for
  every repeating component both formats already cover.
- **SC-007**: Zero of the 90 in-scope items remain reproducible against the fixed working tree —
  each item's originally-described failure scenario, re-run against the post-fix code, produces the
  corrected behavior.

## Assumptions

- **All three priority tiers (User Story 1, 2, and 3) are addressed in this one pass**, rather than
  splitting into separate features — matching the precedent set by feature 007 (which likewise
  brought a P1+P2+P3 audit tier into a single pass after clarification) and by feature 008's own
  scope decision (recorded in this repository's `CLAUDE.md` before its spec files were lost) to do
  the same for this exact document.
- **`NEW-11`'s default-trust-store fix uses `rustls-native-certs`** (loading the OS-native trust
  store at startup) rather than a bundled `webpki-roots` CA list, since it more closely mirrors the
  JVM-default-trust-store behavior QuickFIX/J relies on and this repository's constitution favors
  narrowly-scoped additions over new bundled data. This is the one new dependency this feature adds.
- **`NEW-54`'s Mongo sequence-store fix is forward-only.** Rows already corrupted by the `"field"`
  literal-key bug (stuck at their initial value of 1) are not automatically migrated/repaired by this
  feature; an operator with an affected deployment must manually reconcile those sequence numbers
  after upgrading. Automatic detection-and-repair of already-corrupted rows is out of scope.
- **`NEW-93`'s pre-Logon DoS timeout reuses the existing `logon_timeout` setting** rather than
  introducing a new dedicated configuration key, keeping the configuration surface unchanged for
  this fix.
- **Refuted findings are not re-litigated.** `NEW-01` (PossDup latency exemption), `NEW-31` (stale
  `LastMsgSeqNumProcessed` on resend), `NEW-57` (folded into `NEW-56`), and `NEW-72` (hardcoded
  `TestRequest` id / echo not verified) were each independently confirmed via direct QuickFIX/J
  source citation to already be at parity with QuickFIX/J's own behavior — no code change is made
  for these.
- **`NEW-40` (FileLog never fsyncs) is a confirmed parity-note, not a defect** — QuickFIX/J has the
  same loose durability — and requires no action.
- **The six `Feature-Gap` items** (`EncryptMethod` inbound validation, `Username`/`Password` Logon
  authentication hooks, inbound `LastMsgSeqNumProcessed` validation, `MaxMessagesInResendRequest`,
  `MessageStore::refresh()`, and a `fixt.rs` combined transport+application validate/dispatch helper)
  are QuickFIX/J/Go capabilities TrueFix has not built at all — not defects in existing code — and
  are explicitly **out of scope** for this remediation feature; they are candidates for a future
  feature-request pass, not a bug-fix pass.
- **Two items the audit document itself flags as out of scope for this feature are excluded**: (1)
  verifying whether feature 007's in-progress work actually flips `SessionConfig::new`'s
  `reset_on_logon` default to `false` (a `docs/todo/004.md`/`BUG-92` item, owned by feature 007, not
  this document), and (2) the correction regarding `docs/todo/004.md`'s `BUG-102` description being
  out of date (also a 004.md/007 item). Both are relayed here only as cross-references; no
  requirement in this spec covers them.
- **This feature builds on 001-fix-engine-parity through 007-second-audit-remediation** and does not
  re-touch any `BUG`/`GAP` item already tracked/closed in `docs/todo/003.md` or `004.md`.
