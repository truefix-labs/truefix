# Feature Specification: Third-Pass Audit Remediation

**Feature Branch**: `008-third-audit-remediation`

**Created**: 2026-07-04

**Status**: Draft

**Input**: User description: "按照 docs/todo/005.md的内容 确认有问题后修复。" (Per the contents of
`docs/todo/005.md`, after confirming the issues, fix them.)

## Clarifications

### Session 2026-07-04

- Q: Should User Story 3 (the ~32-item P3 tooling/test-harness/hygiene tier) be included in this
  feature's scope, or deferred to a later pass? → A: Include all three user stories (US1+US2+US3)
  in this one pass, following the same precedent feature 007 set for its own P3 tier.
- Q: `NEW-11`'s TLS default trust-store fallback (`FR-049`) — may this add a new external
  dependency (e.g. `rustls-native-certs`), or must it be solved without one? → A: Allow adding one
  narrowly-scoped new dependency for this purpose.
- Q: `NEW-54`'s Mongo sequence-store fix — should pre-existing corrupted rows (stray `"field"` key,
  `sender`/`target` stuck at 1) be migrated/reconciled on store open, or is a forward-only fix
  (new writes correct; old stray data left untouched and ignored) sufficient? → A: Forward-fix
  only — no migration or historical-sequence-number reconstruction is required.
- Q: `NEW-93`'s pre-Logon read-loop timeout (`FR-044`) — reuse the existing `logon_timeout`
  setting, or add a new dedicated config key? → A: Reuse the existing `logon_timeout` setting; no
  new config key.

## User Scenarios & Testing *(mandatory)*

`docs/todo/005.md` is a third-pass audit (2026-07-04) covering defects **not** already tracked in
`docs/todo/003.md` (`BUG-05`-`BUG-22`/`GAP-48`-`GAP-56`/`B1`-`B30`, closed by feature 006) or
`docs/todo/004.md` (`BUG-23`-`BUG-111`, in progress on feature 007). It was produced as four
successive independent full-codebase reviews of all 9 crates, each cross-referenced against
QuickFIX/J (`thrdpty/quickfixj/`) and QuickFIX/Go (`thrdpty/quickfix/`) source, with each later pass
re-verifying (and in several cases refuting, downgrading, or folding together) findings from the
passes before it:

- **Pass 1** (`NEW-01`-`NEW-53`): first independent review, all 9 crates.
- **Pass 2** (`NEW-54`-`NEW-83`): a second, fully independent re-review of all 9 crates.
- **Pass 3** (`NEW-84`-`NEW-89`): a final pre-release verification pass that re-derived every
  Pass-1/2 finding from source and **refuted** `NEW-01`, `NEW-31`, and `NEW-72` (parity with
  QuickFIX/J's own documented behavior, not TrueFix-specific defects), folded `NEW-57` into
  `NEW-56` (same root cause), and downgraded `NEW-04` and `NEW-71` from P1 to P3 (real but far
  narrower in practice than first stated).
- **Pass 4** (`NEW-90`-`NEW-96`): a remaining-coverage sweep of crate areas Pass 3 had only
  spot-checked (`truefix-store`'s SQL/MSSQL/redb backends, `truefix-transport`'s proxy/TLS code,
  `truefix-config`'s builder in full, all of `truefix-log`, `truefix`'s `lib.rs`, and
  `truefix-at`'s scenario/conformance code).

The document's own final "Updated Summary (all four passes)" and "Final pre-release fix priority
(all four passes)" sections are the authoritative triage this spec inherits. This spec excludes:
refuted findings (`NEW-01`, `NEW-31`, `NEW-57`, `NEW-72`); the 6-row "Missing features" table
(net-new capabilities QuickFIX/J-Go has that TrueFix doesn't, not confirmed defects); the
`Parity-Note`-tagged `NEW-40` (explicitly matches QuickFIX/J's own accepted behavior); and the
`BUG-92` default-value cross-reference the document itself flags as belonging to `004.md`/feature
007, not `005.md`.

**Numbering**: `NEW-01` onward, continuing from `003.md`/`004.md`'s `BUG`/`GAP` series with an
independent prefix as the audit document itself defines.

**Independent Test** (feature-level): For every `NEW-NN` this spec's user stories cover, a targeted
test exists demonstrating the specific previously-broken behavior now works correctly — reusing the
established table-driven-unit-test / two-process-integration-test / AT-scenario pattern from every
prior feature in this repository.

### User Story 1 - Protocol-correctness, data-loss, and security defects (Priority: P1)

An operator running TrueFix in production hits a class of defects the audit rates most severe:
complete functional failure of a persistence backend, protocol handshake bugs that leave a peer's
resend demand unanswered or silently skip acceptor-side sequence resets, an unauthenticated
pre-Logon denial-of-service vector on the multi-session acceptor path, and several FIXT 1.1 /
config / decode gaps that silently produce wrong behavior rather than erroring.

**Why this priority**: Each item in this tier either loses persisted sequence-number state, breaks
interoperability or protocol correctness in a way a live counterparty will observe, or is a
remotely-triggerable resource-exhaustion vector reachable by an unauthenticated peer. This is the
audit's own P0/confirmed-P1 tier — the same bar features 006 and 007 have held every prior
audit-derived top tier to.

**Independent Test**: Each item has its own targeted test (unit, two-process integration, or AT
scenario) demonstrating the previously-broken behavior now works correctly.

**Acceptance Scenarios**:

1. **Given** a `MongoStore`-backed session calls `set_next_sender_seq`/`set_next_target_seq`,
   **When** the update is written, **Then** it lands on the actual `sender`/`target` document
   fields (not the literal key `"field"`), and `next_sender_seq()`/`next_target_seq()` reflect the
   new value immediately and after a restart (`NEW-54`).
2. **Given** a peer sends a `ResendRequest` with `BeginSeqNo=0`, **When** it is processed, **Then**
   it is treated as "from sequence 1" (per FIX protocol semantics) and answered, rather than
   silently dropped (`NEW-02`).
3. **Given** an acceptor is configured with `ResetOnLogon=Y`, **When** a Logon is received,
   **Then** the acceptor resets both of its own sequence numbers to 1 regardless of whether the
   inbound Logon itself carried `ResetSeqNumFlag=Y` (`NEW-03`).
4. **Given** a multi-session acceptor's routing-by-Logon-content path accepts a new TCP connection,
   **When** the remote peer sends nothing, sends bytes containing no SOH, or trickles data slowly,
   **Then** the connection is bounded by a timeout and a capped read buffer — it cannot be held
   open indefinitely or grow its buffer without limit before a `Session`/`logon_timeout` exists
   (`NEW-93`).
5. **Given** an inbound gap-fill `SequenceReset` carries a `MsgSeqNum` ahead of what is expected,
   **When** it is received, **Then** it is queued and answered with a `ResendRequest` like any
   other too-high message (mirroring QuickFIX/J's `verify(..., isGapFill, isGapFill)`), instead of
   being applied immediately and silently discarding legitimately queued messages below its target
   (`NEW-84`).
6. **Given** a FIX 5.x (`FIXT.1.1`) message, **When** it is decoded, **Then** `ApplVerID(1128)`,
   `ApplReportID(1129)`, `LastApplVerID(1130)`, `ApplExtID(1156)`, and the `NoApplIDs(1351)` group
   and its members route into the message header (not the body), matching FIXT 1.1's transport
   header layout (`NEW-55`).
7. **Given** a session configured with FIXT 1.1 dual dictionaries (`AppDataDictionary` +
   `TransportDataDictionary`) but no plain single-dictionary `validator`, **When** an inbound
   message is classified/decoded on the transport layer, **Then** header/trailer repeating groups
   (e.g. `NoHops`) are still structured via group-aware decode, not silently flattened (`NEW-58`).
8. **Given** a session teardown occurs for any reason (graceful Logout vs. TCP drop), **When** the
   store-reset decision is made, **Then** `reset_on_logout` is honored only for a graceful Logout
   and `reset_on_disconnect` only for a non-graceful disconnect — the two flags no longer share one
   `||` condition that lets either leak into the other's scenario (`NEW-56`).
9. **Given** a codegen-generated message struct is constructed via its generated `new()` and
   encoded without an explicit `BeginString`/`MsgType` header field having been set, **When**
   `encode` runs, **Then** it does not silently emit a malformed `8=<SOH>` or `35=<SOH>` wire
   message (`NEW-59`).
10. **Given** an acceptor/initiator exits its configured trading-hours schedule window while
    logged on, **When** the session tears itself down for that reason, **Then** a `Logout` is sent
    before the disconnect, rather than an abrupt TCP drop with no protocol-level notice (`NEW-62`).
11. **Given** an inbound Logon fails data-dictionary validation and `disconnect_on_error=false`,
    **When** the rejection is processed, **Then** it is routed through the same `Logout`+disconnect
    path every other Logon-rejection reason uses, instead of sending a bare session-level `Reject`
    and leaving the session stranded in `AwaitingLogon` (`NEW-63`).
12. **Given** a FIX 5.0/5.0SP1/5.0SP2 message (all sharing `BeginString=FIXT.1.1`), **When** a
    version-specific `crack_fix50`/`crack_fix50sp1`/`crack_fix50sp2` dispatcher processes it,
    **Then** it consults `ApplVerID(1128)` to select the correct version-specific struct type
    instead of dispatching any FIXT 1.1 message into FIX 5.0 types (`NEW-06`).
13. **Given** a field of type `MultipleCharValue` with a valid multi-token wire value (e.g.
    `"A B"`), **When** its enumerated values are checked via `allows()`, **Then** it is validated
    per-token (consistent with `value_ok()`'s existing per-token handling), not as one whole-string
    comparison (`NEW-07`).
14. **Given** a `MongoStore`-backed session, **When** `save_and_advance_sender` is called, **Then**
    the save and the sender-sequence advance happen atomically within one MongoDB transaction, the
    same crash-safety guarantee `SqlStore`/`MssqlStore`/`RedbStore`/`FileStore` already provide
    (`NEW-08`).
15. **Given** an MSSQL log destination URL uses the semicolon `sqlserver://host;databaseName=...`
    form or has percent-encoded credentials, **When** `MssqlLog::parse_url` parses it, **Then** it
    succeeds the same way the store's equivalent `parse_url` already does (`NEW-09`).
16. **Given** a `.cfg` file sets `ValidateFieldsHaveValues`, `ValidateUnorderedGroupFields`,
    `ValidateUserDefinedFields`, `AllowUnknownMsgFields`, or `FirstFieldInGroupIsDelimiter` to a
    non-default value, **When** the session is built, **Then** the corresponding validation
    behavior actually changes; `ValidateSequenceNumbers` and `RejectInvalidMessage` are either
    wired to a real effect or explicitly downgraded from `Impl` to `Recognized` in the config-key
    registry so the registry no longer overstates what's implemented (`NEW-10`).
17. **Given** `SocketUseSSL=Y` is set with no `SocketTrustStore`/`SocketTrustStoreBytes`
    configured, **When** a TLS client handshake is attempted, **Then** it falls back to a default
    (e.g. platform-native) trust store instead of an empty one that rejects every server
    certificate (`NEW-11`).
18. **Given** a PROXY protocol v2 header whose encoded length exceeds the transport's fixed peek
    buffer, **When** it is peeked, **Then** the buffer grows (or the header is read incrementally)
    until the full header is available, instead of silently dropping unread header bytes into the
    FIX stream (`NEW-12`).
19. **Given** a trusted proxy delivers its PROXY header slowly across multiple TCP segments (still
    within the configured timeout), **When** the header is being peeked, **Then** the peek loop
    keeps trying until the outer timeout actually fires, instead of giving up after a fixed
    iteration count well short of it (`NEW-13`).
20. **Given** a scheduled initiator's connect attempt hangs (no `connect_timeout` configured),
    **When** the schedule window closes or a stop is requested during that hang, **Then** the
    boundary/stop condition is still observed promptly instead of being blocked until the connect
    resolves (`NEW-14`).
21. **Given** `validate_user_defined_fields=false`, **When** an inbound message carries an empty or
    repeated user-defined field, **Then** it is skipped entirely (no rejection for emptiness or
    repetition), matching QuickFIX/J's full UDF skip rather than only skipping some UDF checks
    (`NEW-17`).

---

### User Story 2 - Confirmed defects with narrower blast radius (Priority: P2)

A second tier of confirmed defects that need an unusual configuration, a specific counterparty
behavior, or a specific input to trigger — real correctness, security, or robustness problems, but
ones a deployment can operate with in the short term while they're being fixed.

**Why this priority**: Every item here is a confirmed, real defect (not a design difference or a
refuted claim) — but each has a narrower trigger condition than User Story 1's items, consistent
with the audit's own P2 ("Medium — narrower blast radius / edge case / test quality") tier.

**Independent Test**: Each item has its own targeted test exercising the specific narrow trigger
condition and demonstrating the corrected behavior.

**Acceptance Scenarios**:

1. **Given** a `Logout`, `Reject`, or `SequenceReset` with a too-high sequence number arrives while
   the session is in `AwaitingLogon` state, **When** it is processed, **Then** it is rejected
   without emitting a `ResendRequest` (no ResendRequests before logon), matching the same guard
   already applied to non-admin messages (`NEW-18`).
2. **Given** a message contains a header repeating group (e.g. `NoHops(627)`), **When** it is
   validated against a data dictionary, **Then** header groups are checked the same way body
   groups already are (`NEW-19`).
3. **Given** a field of type `MonthYear` or `LocalMktDate`, **When** its wire value is checked,
   **Then** it is validated against the correct date/month format instead of accepted unchecked
   (`NEW-20`).
4. **Given** a wire message contains a field with tag `0`, **When** it is decoded, **Then** it is
   rejected as an invalid tag rather than accepted (`NEW-21`).
5. **Given** a repeating group's declared count (`NoXxx`) does not match its actual number of
   entries on the wire, **When** the message is decoded and re-encoded, **Then** the mismatch is
   either preserved faithfully or reported, not silently corrected to match the actual entry count
   (`NEW-22`).
6. **Given** any of the SQL/MSSQL/Redb/Mongo async log backends, **When** the underlying database
   is slow or unavailable, **Then** the pending-write queue is bounded (backpressure or a capped
   channel), rather than growing without limit (`NEW-24`).
7. **Given** `CipherSuites` lists a mix of valid names and one typo'd/unrecognized name, **When**
   TLS config is resolved, **Then** the unrecognized name is reported as a configuration error
   instead of being silently dropped while the valid names succeed (`NEW-25`).
8. **Given** a reconnecting initiator's target host resolves to multiple IPs or its DNS record
   changes after startup, **When** a reconnect attempt is made, **Then** the target address is
   re-resolved at connect time rather than fixed once at config-load time (`NEW-26`).
9. **Given** `Engine::start` resolves a socket address, **When** it runs on the async runtime,
   **Then** DNS resolution uses a non-blocking async resolver instead of blocking DNS inside the
   async body (`NEW-27`).
10. **Given** a session's `reset()`/`reset_on_error()` is invoked mid-session (not immediately
    followed by a reconnect), **When** `reset_sequences` runs, **Then** it also clears
    `resend_target`/`resend_chunk_end`/`test_request_outstanding`, matching the equivalent cleanup
    `on_connected` already performs (`NEW-32`).
11. **Given** `SocketReuseAddress=N` is configured, **When** `AcceptorBuilder::bind` binds its
    listener, **Then** `SO_REUSEADDR` is actually left unset, instead of being hardcoded to true
    regardless of the setting (`NEW-33`).
12. **Given** an inbound `SequenceReset` (plain, non-gap-fill) carries `NewSeqNo=0`, **When** it is
    processed, **Then** it is treated as a value error (rejected), not silently ignored as if the
    field were absent (`NEW-34`).
13. **Given** an inbound message's `ApplVerID(1128)` is present but an empty string, **When**
    `FixtDictionaries::application_for` resolves the application dictionary, **Then** it falls back
    to `DefaultApplVerID` the same way it would for a genuinely absent `ApplVerID` (`NEW-64`).
14. **Given** a custom or malicious dictionary defines a cyclic repeating-group member reference,
    **When** a message using that dictionary is decoded with `decode_with_groups`, **Then**
    decoding fails cleanly (a bounded/reported error) instead of recursing without a depth or cycle
    guard driven by attacker-controlled message content (`NEW-65`).
15. **Given** a `DayOfMonth` field, **When** its value is checked, **Then** values outside 1-31 are
    rejected (`NEW-66`).
16. **Given** a field of type `Length`, `SeqNum`, or `NumInGroup`, **When** its value is checked via
    `value_ok`, **Then** negative values are rejected, consistent with the decode path's existing
    rejection of negative values for these types (`NEW-67`).
17. **Given** a repeating group's `NoXxx` count field itself has a non-numeric or otherwise
    type-invalid value, **When** the group is validated (on both the flat-decode and
    group-aware-decode paths), **Then** it is reported as an incorrect-data-format error
    consistently on both paths (`NEW-68`).
18. **Given** a repeating-group member field inside a group has an empty value where one is
    required, **When** the group is validated, **Then** it is rejected the same way an empty
    top-level field already is (respecting the existing reverse-route-tag exemption) (`NEW-69`).
19. **Given** a gap-fill `SequenceReset` is missing its `NewSeqNo` field entirely, **When** it is
    processed, **Then** it is rejected as a required-field violation instead of silently no-op'ing
    (falling through to `drain_queue` with no reject and no sequence update) (`NEW-70`).
20. **Given** the transport's frame-resync logic encounters a buffer where `8=` is not immediately
    preceded by an SOH byte (e.g. a garbage prefix), **When** it searches for the next valid frame
    start, **Then** it does not discard an actual, well-formed subsequent FIX frame along with the
    garbage (`NEW-74`).
21. **Given** two concurrent/duplicate connect attempts for the same `session_id` against a
    `MongoStore`, **When** each ensures its session row exists, **Then** a unique constraint (index
    or upsert) prevents duplicate session documents from being created (`NEW-75`).
22. **Given** `BodyLog::reset` and `BodyLog::append` could ever run concurrently, **When** `reset`
    truncates the underlying file, **Then** it acquires the same index lock `append` holds before
    truncating, closing the fragile lock-ordering gap (`NEW-76`).
23. **Given** a `FileStore`'s on-disk creation-time file exists but contains unparseable content,
    **When** it is read, **Then** a typed error is returned (consistent with how an unparseable
    sequence-number file is already handled), instead of silently defaulting to the Unix epoch
    (`NEW-77`).
24. **Given** a repeating-group `field_order` list containing a duplicate tag, **When**
    `render_members_ordered` encodes the group, **Then** each member is emitted exactly once, not
    once per duplicate occurrence in the order list (`NEW-80`).
25. **Given** a decoded message contains duplicate occurrences of the same field tag at the same
    level, **When** `FieldMap::set` updates that tag's value, **Then** the stale duplicate no
    longer survives to be re-emitted on encode (`NEW-81`).
26. **Given** an inbound `Logout` carries a `MsgSeqNum` ahead of what is expected, **When** it is
    received, **Then** it is processed immediately (Logout reply, `ResetOnLogout` honored,
    disconnect) rather than queued behind a `ResendRequest` — mirroring the existing special-case
    handling already given to too-high `ResendRequest` messages (`NEW-85`).
27. **Given** a too-high `ResendRequest` was already answered immediately upon arrival, **When**
    the session's own sequence gap later closes and `drain_queue` reaches that same queued
    message, **Then** it is not reprocessed/re-answered a second time — only the sequence counter
    advances (`NEW-86`).
28. **Given** an MSSQL-backed store or log connection, **When** its connection is established,
    **Then** an operator can opt into real TLS server-certificate validation (a config field or URL
    property), instead of certificate validation being unconditionally disabled with no way to turn
    it back on (`NEW-90`).
29. **Given** a PROXY protocol header that is fully and correctly parsed but carries an
    `Unknown`/`Unspecified`/Unix-socket address (all spec-legal values), **When** it is processed,
    **Then** its byte length is still consumed from the stream even though its address is not
    remapped, instead of leaving those bytes to be misread as FIX data (`NEW-94`).
30. **Given** a `.cfg` file sets `LogMessageWhenSessionNotFound`, **When** the acceptor resolves its
    configuration, **Then** the setting actually reaches the `Services` struct that consumes it,
    instead of always being hardcoded to `false` (`NEW-96`).

---

### User Story 3 - Hardening, tooling correctness, and test-coverage gaps (Priority: P3)

The remaining lower-priority items: real-but-low-impact production defects, dictionary-codegen
tooling correctness issues (affecting regenerated `.fixdict` files and generated Rust code, not the
shipped hand-verified dictionaries or any currently-running session), acceptance-test harness
coverage gaps, and allocation/complexity hygiene notes.

**Why this priority**: These items round out full closure of `005.md` without blocking the P1/P2
tiers above. Several are narrower-than-first-stated versions of P1 candidates that a later pass in
the audit document itself downgraded after re-verification.

**Independent Test**: Each item has its own targeted test (unit test, codegen fixture, or AT
scenario addition) demonstrating the corrected behavior.

**Acceptance Scenarios**:

1. **Given** a caller invokes `Message::decode` directly (bypassing `frame_length`'s framing-layer
   cap), **When** the declared body length exceeds `MAX_BODY_LEN`, **Then** decode itself also
   rejects it, closing the public-API consistency gap between the two entry points (`NEW-04`,
   downgraded P1→P3: real gap, but not an exploitable unbounded-memory path since the input is
   already fully resident before `decode` is called).
2. **Given** the `fix-repository` dictionary converter processes a repeating component, **When** it
   builds the group definition, **Then** the count field is excluded from `members` and the
   delimiter is the first real body field — matching the `orchestra.rs` converter and the shipped
   `.fixdict` format (`NEW-05`).
3. **Given** a resend of a stored message, **When** `CachedFileStore::get` is called for a
   sequence number not yet cached, **Then** the result is inserted into the cache (read-through),
   so subsequent resends of the same sequence don't always hit disk (`NEW-23`).
4. **Given** the AT harness's `start_acceptor`, **When** any of the 7 FIX versions besides 4.2/4.4
   run a field-validation scenario, **Then** a dictionary validator is loaded for them too
   (`NEW-28`).
5. **Given** a `CheckLatency`-family AT scenario expects a `Logout`, **When** it asserts on the
   response, **Then** it also verifies the Logout's reason text/field rather than accepting a
   Logout sent for any reason (`NEW-29`).
6. **Given** an AT scenario completes all its steps, **When** the runner finishes, **Then** it
   asserts the read buffer is empty (no unexpected extra message was sent) (`NEW-30`).
7. **Given** `SessionId`'s `Display` implementation, **When** two sessions differ only by
   `SubID`/`LocationID`, **Then** their rendered log strings are distinguishable (`NEW-39`).
8. **Given** `dict/parser.rs` loads a dictionary with a duplicate tag/name/message-type
   definition, **When** parsing completes, **Then** the duplicate is reported as an error instead
   of silently overwriting the earlier definition (`NEW-43`).
9. **Given** a dictionary STRING enum value contains a literal `#` character, **When**
   `strip_comment` processes the line, **Then** the value is not truncated (`NEW-44`).
10. **Given** `validate.rs`'s `parse_fix_begin_string` parses a FIX 5.0/5.0SP1/5.0SP2
    `BeginString`, **When** it computes the version tuple, **Then** the three sub-versions are
    distinguished rather than all collapsing to `(5, 0)` (`NEW-45`).
11. **Given** a config file contains a self-referential or circular `${var}` reference, **When**
    variables are resolved, **Then** it is reported as an error instead of producing a literal
    `${...}` in the output (`NEW-46`).
12. **Given** an `UnresolvedVariable` config error occurs after per-session merging, **When** it is
    reported, **Then** it carries the correct source line number instead of always `line: 0`
    (`NEW-47`).
13. **Given** a long-lived acceptor's `Monitor` map, **When** a connection disconnects, **Then**
    its entry is removed instead of the map growing monotonically for the process's lifetime
    (`NEW-48`).
14. **Given** an HTTP CONNECT proxy response, **When** its status line is parsed, **Then** the
    status token is validated as numeric rather than only checked via `starts_with('2')`
    (`NEW-49`).
15. **Given** an AT `check_match` assertion, **When** the actual message carries extra/erroneous
    fields beyond the expected set, **Then** the mismatch is reported rather than silently passing
    (`NEW-50`).
16. **Given** AT scenarios exercising server-sent messages, **When** the runner validates them,
    **Then** outbound `MsgSeqNum(34)` ordering is verified where it currently is not (`NEW-51`).
17. **Given** the `server_acceptance_suite_passes` AT conformance test, **When** one scenario among
    many fails, **Then** the failure is reported per-scenario rather than as one
    all-or-nothing pass/fail (`NEW-52`).
18. **Given** `DataDictionary::extend()` merges a second dictionary in, **When** the merge
    completes, **Then** a `field_by_name` collision (same name, different tag) is detected and
    `member_tags` is recomputed to reflect the merge (`NEW-53`).
19. **Given** the `orchestra.rs` Orchestra-XML converter encounters any of the type tokens
    `fix_repository::map_type` already handles (`PRICEOFFSET`, `TIME`, `UTCDATE`, `DAYOFMONTH`,
    `CURRENCY`, `EXCHANGE`, `MULTIPLEVALUESTRING`, `MULTIPLESTRINGVALUE`, `MULTIPLECHARVALUE`,
    `COUNTRY`), **When** it maps the type, **Then** conversion succeeds instead of aborting with
    `UnknownType`; `localmktdate` also maps to the correct distinct type instead of collapsing into
    `UTCDATEONLY` (`NEW-60`).
20. **Given** `truefix-dict/src/codegen.rs` changes without any `.fixdict` file changing, **When**
    the crate is rebuilt, **Then** Cargo re-runs `build.rs` and regenerates code rather than using
    a stale `OUT_DIR/generated.rs` (`NEW-61`).
21. **Given** an acceptor has no dictionary configured at all, **When** a Logon omits `HeartBtInt`
    or sends it as `0`, **Then** the acceptor rejects the Logon instead of silently keeping its
    configured default heartbeat interval (`NEW-71`, downgraded P1→P3: with a dictionary
    configured, this is already rejected upstream).
22. **Given** `SessionConfig::new()` is constructed directly (bypassing `truefix-config`'s `.cfg`
    resolution, which already defaults correctly to 30), **When** its bare struct default for
    `reconnect_interval` is read, **Then** it matches QuickFIX/J-Go's 30-second default instead of
    5, for API-level default consistency (`NEW-73`, narrowed: real `.cfg`-driven deployments are
    already unaffected).
23. **Given** `codegen.rs`'s `parse_dict` encounters an unknown directive or a malformed
    field/group/message line, **When** it parses, **Then** it reports an error consistent with the
    runtime `parser::parse`'s behavior for the same input, instead of silently skipping it
    (`NEW-78`).
24. **Given** two dictionary enum value labels sanitize to the same generated Rust identifier,
    **When** codegen emits the enum, **Then** the collision is detected/resolved instead of
    producing duplicate, uncompilable variants (`NEW-79`).
25. **Given** the `generate_code` CLI processes a non-UTF-8 dictionary file, **When** it computes
    the dual-track provenance hash, **Then** the runtime-`parse` and `codegen::generate` hashing
    paths use the same byte representation (both lossy or both raw), so the CLI's own dual-track
    check cannot silently diverge from the in-crate test's (`NEW-82`).
26. **Given** the AT harness needs to exercise Logon-time identity validation (wrong
    SenderCompID/TargetCompID/BeginString on Logon, QuickFIX/J's `1c` scenario category), **When**
    such a scenario runs, **Then** a fixed-identity (non-dynamic-template) acceptor mode is
    available to represent it (`NEW-83`).
27. **Given** a non-Logon message fails the latency, CompID-identity, or PossDup-falsification
    check, **When** the session responds, **Then** it sends a session-level `Reject` with the
    correct reason before the `Logout`, matching QuickFIX/J's two-message sequence (`NEW-87`).
28. **Given** an inbound Logon carries `DefaultApplVerID(1137)`, **When** it is accepted, **Then**
    the value is validated against the registered/known `ApplVerID` set before being stored and
    used to key application-dictionary lookups (`NEW-88`).
29. **Given** a `.cfg`-driven session omits `LogoutTimeout`, **When** its default is applied,
    **Then** it matches QuickFIX/J's 2-second default instead of 10 (`NEW-89`, half only —
    `ReconnectInterval`'s half of this pairing is `NEW-73` above and already narrowed).
30. **Given** `Engine::shutdown()` (or `Drop`) runs while an async DB-backed log backend
    (SQL/MSSQL/Mongo/Redb) still has queued entries, **When** shutdown completes, **Then** the
    queued entries are flushed/written (or a caller can await that draining) before the process
    exits, instead of being silently lost (`NEW-91`).
31. **Given** `Engine::start` builds its acceptor-group startup order from a `HashMap`, **When**
    groups are iterated, **Then** the order is stable/deterministic across process runs (e.g.
    sorted by address) so partial-startup-failure diagnostics are reproducible (`NEW-92`).
32. **Given** a configured TLS trust-store file contains one or more malformed certificates,
    **When** it is loaded, **Then** the number of certificates that failed to load is surfaced
    (log warning or typed error when the resulting store ends up empty), instead of silently
    producing an empty, non-functional root store with no diagnostic (`NEW-95`).

### Edge Cases

- What happens when a `MongoStore` upgraded from the broken `set_seq` (`NEW-54`) has pre-existing
  session rows carrying a stray `"field"` key from the old buggy writes? Per the resolved
  clarification above, this is a forward-only fix: no migration is attempted, the stray key is
  simply never read or relied upon again, and its presence must not cause new reads/writes to
  crash or misbehave.
- How does the system handle a peer that is both a legitimate slow proxy (`NEW-13`) and a
  slowloris-style attacker (`NEW-93`) on the same multi-session acceptor port at the same time —
  the fix for one must not defeat the other's protection.
- What happens when a dictionary exercises both the `NEW-65` cyclic-group guard and a legitimately
  deep (but finite, non-cyclic) nested group structure — the cycle/depth guard must not reject
  valid deeply-nested dictionaries.
- How does `NEW-56`'s reason-aware teardown interact with a session that disconnects for a reason
  carrying no clear "graceful Logout vs. ungraceful drop" signal (e.g. a local `reset()` call) —
  the reason enum must have a defined mapping for every existing teardown call site.

## Requirements *(mandatory)*

### Functional Requirements

**Persistence and store correctness**

- **FR-001**: `MongoStore::set_seq` MUST write to the actual `sender`/`target` document field
  (not the literal string `"field"`), so sequence-number persistence functions correctly. (`NEW-54`)
- **FR-002**: `MongoStore` MUST provide a transactional `save_and_advance_sender` override with the
  same atomicity guarantee as the SQL/MSSQL/Redb/File stores. (`NEW-08`)
- **FR-003**: `MongoStore`'s `sessions` collection MUST prevent duplicate rows for the same
  `session_id` under concurrent connects (via upsert or a unique index). (`NEW-75`)
- **FR-004**: `BodyLog::reset` MUST acquire the index lock before truncating the underlying file,
  matching `append`'s lock ordering. (`NEW-76`)
- **FR-005**: `FileStore`'s creation-time file reader MUST return a typed error for present-but-
  unparseable content, instead of silently defaulting to the Unix epoch. (`NEW-77`)
- **FR-006**: `CachedFileStore::get` MUST populate its cache on a read-through miss. (`NEW-23`)
- **FR-007**: Async SQL/MSSQL/Mongo/Redb log backends' internal write queues MUST be bounded
  (backpressure or capacity cap) instead of unbounded channels. (`NEW-24`)
- **FR-008**: `Engine::shutdown()` (and `Drop for Engine`) MUST ensure queued entries in async
  DB-backed log backends are flushed (or exposed as an awaitable drain) before completing.
  (`NEW-91`)
- **FR-009**: `MssqlLog::parse_url` MUST support the semicolon connection-string form and
  percent-decoded credentials, matching the store's equivalent parser. (`NEW-09`)
- **FR-010**: MSSQL store and log connections MUST offer an operator-configurable option to enable
  real TLS server-certificate validation, instead of unconditionally disabling it with no opt-out.
  (`NEW-90`)

**Session protocol correctness**

- **FR-011**: `on_resend_request` MUST treat `BeginSeqNo=0` as "from sequence 1" rather than
  silently dropping the request. (`NEW-02`)
- **FR-012**: An acceptor configured with `ResetOnLogon=Y` MUST reset both of its own sequence
  numbers to 1 upon receiving any Logon, independent of the inbound Logon's own
  `ResetSeqNumFlag`. (`NEW-03`)
- **FR-013**: A gap-fill `SequenceReset` with a `MsgSeqNum` ahead of expectation MUST be queued and
  answered with a `ResendRequest` via the same too-high dispatch path other message types use,
  instead of applying its `NewSeqNo` jump unconditionally. (`NEW-84`)
- **FR-014**: Session teardown MUST route through a single reason-aware decision point that
  applies `reset_on_logout` only for a graceful Logout-driven teardown and `reset_on_disconnect`
  only for a non-graceful disconnect, replacing the current `||` conflation. (`NEW-56`)
- **FR-015**: A schedule-window exit that tears down a logged-on session MUST send a `Logout`
  before disconnecting. (`NEW-62`)
- **FR-016**: A Logon that fails data-dictionary validation MUST be rejected via the same
  `Logout`+disconnect path other Logon-rejection reasons use, regardless of
  `disconnect_on_error`. (`NEW-63`)
- **FR-017**: A pre-logon (`AwaitingLogon`-state) admin message (`Logout`, `Reject`, or
  `SequenceReset`) with a too-high sequence number MUST be rejected without emitting a
  `ResendRequest`. (`NEW-18`)
- **FR-018**: An inbound `SequenceReset` (non-gap-fill) with `NewSeqNo=0` MUST be rejected as a
  value error rather than silently ignored. (`NEW-34`)
- **FR-019**: A gap-fill `SequenceReset` missing `NewSeqNo` entirely MUST be rejected as a
  required-field violation rather than silently falling through with no reset and no reject.
  (`NEW-70`)
- **FR-020**: `reset_sequences()` MUST clear `resend_target`, `resend_chunk_end`, and
  `test_request_outstanding`, matching `on_connected`'s existing cleanup. (`NEW-32`)
- **FR-021**: An inbound `Logout` with a too-high `MsgSeqNum` MUST be processed immediately
  (Logout reply, `ResetOnLogout` honored, disconnect) rather than queued behind a
  `ResendRequest`. (`NEW-85`)
- **FR-022**: A too-high `ResendRequest` that was already answered immediately upon arrival MUST
  NOT be reprocessed when `drain_queue` later reaches it — only the sequence counter advances.
  (`NEW-86`)
- **FR-023**: A non-Logon message failing the latency, CompID-identity, or PossDup-falsification
  check MUST receive a session-level `Reject` (with the correct reason) before the `Logout`.
  (`NEW-87`)
- **FR-024**: An inbound Logon's `DefaultApplVerID(1137)` MUST be validated against the registered/
  known `ApplVerID` set before being stored and used for application-dictionary lookups. (`NEW-88`)
- **FR-025**: On an acceptor with no dictionary configured, a Logon omitting `HeartBtInt` (or
  sending `0`) MUST be rejected rather than silently retaining the acceptor's configured default.
  (`NEW-71`)
- **FR-026**: `SessionConfig::new()`'s bare-struct default for `reconnect_interval` MUST be 30
  seconds (matching QuickFIX/J-Go), for consistency with `.cfg`-driven resolution which already
  defaults to 30. (`NEW-73`)
- **FR-027**: A `.cfg`-driven session's default `LogoutTimeout` MUST be 2 seconds, matching
  QuickFIX/J's default. (`NEW-89`)

**FIXT 1.1 / dictionary / validation correctness**

- **FR-028**: `tags::is_header` MUST include the FIXT 1.1 transport header tags `1128`, `1129`,
  `1130`, `1156`, and the `1351`/`1352`-`1355` group, so these fields route into the message header
  during decode. (`NEW-55`)
- **FR-029**: Transport-layer message classification MUST use group-aware decode (structuring
  header/trailer repeating groups) when FIXT dual-dictionaries are configured, even when no plain
  single-dictionary validator is set. (`NEW-58`)
- **FR-030**: `crack_fix50`/`crack_fix50sp1`/`crack_fix50sp2` dispatchers MUST consult
  `ApplVerID(1128)` to select the correct FIX 5.x sub-version's struct types. (`NEW-06`)
- **FR-031**: `FieldDef::allows()` MUST validate `MultipleCharValue` per-token, consistent with
  `value_ok()`'s existing per-token handling. (`NEW-07`)
- **FR-032**: The 5 config-key/`ValidationOptions`-field pairs (`ValidateFieldsHaveValues`,
  `ValidateUnorderedGroupFields`, `ValidateUserDefinedFields`, `AllowUnknownMsgFields`,
  `FirstFieldInGroupIsDelimiter`) currently marked `Impl` but unwired MUST be wired in
  `resolve_validator`; `ValidateSequenceNumbers` and `RejectInvalidMessage` MUST either gain a real
  effect or be downgraded to `Recognized` in the key registry. (`NEW-10`)
- **FR-033**: UDF short-circuit skipping (when `validate_user_defined_fields=false`) MUST run
  before the empty-value and repeated-tag checks, so UDFs are fully exempted rather than partially
  checked. (`NEW-17`)
- **FR-034**: Header repeating groups (e.g. `NoHops`) MUST be validated the same way body groups
  already are. (`NEW-19`)
- **FR-035**: `MonthYear` and `LocalMktDate` field values MUST be format-validated. (`NEW-20`)
- **FR-036**: A wire field with tag `0` MUST be rejected during decode as an invalid tag. (`NEW-21`)
- **FR-037**: A repeating group whose declared `NoXxx` count does not match its actual entry count
  MUST NOT silently re-encode with a corrected count with no error/warning signal. (`NEW-22`)
- **FR-038**: `DayOfMonth` field values MUST be range-checked to 1-31. (`NEW-66`)
- **FR-039**: `Length`, `SeqNum`, and `NumInGroup` field values MUST reject negative values in
  `value_ok`, consistent with the decode path's existing behavior. (`NEW-67`)
- **FR-040**: A `NoXxx` group count field with a non-numeric/type-invalid value MUST be reported
  as an incorrect-data-format error on both the flat-decode and group-aware-decode validation
  paths. (`NEW-68`)
- **FR-041**: An empty value on a required repeating-group member field MUST be rejected the same
  way an empty top-level field already is (respecting the existing reverse-route-tag exemption).
  (`NEW-69`)
- **FR-042**: `FixtDictionaries::application_for` MUST treat an empty-string `ApplVerID` the same
  as an absent one, falling back to `DefaultApplVerID`. (`NEW-64`)
- **FR-043**: Dictionary loading MUST guard against cyclic repeating-group member definitions so
  that `decode_with_groups` cannot be driven into unbounded/deep recursion by message content
  alone. (`NEW-65`)

**Transport / networking correctness and security**

- **FR-044**: The multi-session `AcceptorBuilder` connection's pre-Logon read loop MUST be bounded
  by the existing `logon_timeout` setting (no new config key) and a capped buffer size, closing the
  unauthenticated slowloris-shaped resource-exhaustion path that exists before a
  `Session`/`logon_timeout` is otherwise established. (`NEW-93`)
- **FR-045**: PROXY protocol header peeking MUST handle headers larger than the current fixed peek
  buffer (grow the buffer or read incrementally) instead of silently dropping unread bytes into the
  FIX stream. (`NEW-12`)
- **FR-046**: PROXY protocol header peeking MUST keep retrying until the outer timeout fires,
  instead of giving up after a fixed iteration count well short of it. (`NEW-13`)
- **FR-047**: A fully-parsed, spec-legal PROXY header carrying an `Unknown`/`Unspecified`/Unix-
  socket address MUST still have its byte length consumed from the stream, instead of leaving those
  bytes to be misread as FIX data. (`NEW-94`)
- **FR-048**: A scheduled initiator's connect attempt MUST NOT block schedule-boundary or stop-flag
  detection; the connect MUST race against those signals (e.g. via `tokio::select!`). (`NEW-14`)
- **FR-049**: TLS client configuration with no explicit trust store configured MUST fall back to a
  default platform-native trust store instead of an empty one; adding one narrowly-scoped new
  dependency for this purpose is permitted. (`NEW-11`)
- **FR-050**: TLS trust-store loading MUST surface how many certificates failed to load (log
  warning, or a typed error when the resulting store is empty despite non-empty input), instead of
  silently producing an unusable empty store with no diagnostic. (`NEW-95`)
- **FR-051**: `CipherSuites` configuration MUST report an error for any listed name that matches no
  known suite, even when other listed names are valid. (`NEW-25`)
- **FR-052**: Reconnecting-initiator target address resolution MUST occur at connect time (not
  fixed once at config-load time), so DNS changes and multi-IP records are honored on reconnect.
  (`NEW-26`)
- **FR-053**: `Engine::start`'s socket-address resolution MUST use a non-blocking async DNS
  resolver instead of blocking DNS inside async code. (`NEW-27`)
- **FR-054**: `AcceptorBuilder::bind` MUST honor the configured `SocketReuseAddress` setting instead
  of hardcoding `SO_REUSEADDR=true`. (`NEW-33`)
- **FR-055**: The transport's frame-resync logic MUST NOT discard a well-formed subsequent FIX
  frame while searching past a non-SOH-terminated garbage prefix. (`NEW-74`)
- **FR-056**: HTTP CONNECT proxy response parsing MUST validate that the status token is numeric,
  not merely that it starts with `'2'`. (`NEW-49`)
- **FR-057**: A long-lived acceptor's connection `Monitor` map MUST remove an entry when its
  connection disconnects, instead of growing monotonically for the process lifetime. (`NEW-48`)
- **FR-058**: `.cfg` key `LogMessageWhenSessionNotFound` MUST be parsed and threaded through to the
  `Services` struct that already consumes it. (`NEW-96`)

**Encode/decode and data-structure correctness**

- **FR-059**: Codegen-generated `new()`/`encode()` MUST NOT silently emit a malformed
  `8=<SOH>`/`35=<SOH>` wire message when required header fields (`BeginString`, `MsgType`) have not
  been set. (`NEW-59`)
- **FR-060**: `render_members_ordered` MUST NOT emit a group member more than once when its tag
  appears multiple times in `field_order`. (`NEW-80`)
- **FR-061**: `FieldMap::set` MUST NOT leave a stale duplicate field to be re-emitted on encode when
  duplicate tags exist at the same level. (`NEW-81`)
- **FR-062**: A caller invoking `Message::decode` directly MUST have the same `MAX_BODY_LEN`
  enforcement `frame_length` already applies, for public-API consistency. (`NEW-04`)
- **FR-063**: `SessionId`'s `Display` implementation MUST render `SubID`/`LocationID` so two
  sessions differing only in those fields are distinguishable in logs. (`NEW-39`)

**Dictionary tooling correctness** (affects dictionary regeneration and codegen only, not shipped
`.fixdict` files or any currently-running session)

- **FR-064**: The `fix-repository` converter MUST exclude the count field from repeating-group
  `members` and set the delimiter to the first real body field, matching `orchestra.rs` and the
  shipped `.fixdict` format. (`NEW-05`)
- **FR-065**: `orchestra.rs`'s `map_type` MUST handle the type tokens `fix_repository::map_type`
  already handles (`PRICEOFFSET`, `TIME`, `UTCDATE`, `DAYOFMONTH`, `CURRENCY`, `EXCHANGE`,
  `MULTIPLEVALUESTRING`, `MULTIPLESTRINGVALUE`, `MULTIPLECHARVALUE`, `COUNTRY`) and MUST map
  `localmktdate` to its own distinct type instead of collapsing it into `UTCDATEONLY`. (`NEW-60`)
- **FR-066**: `build.rs` MUST declare `cargo:rerun-if-changed=src/codegen.rs` so edits to the
  `#[path]`-included codegen module trigger a rebuild. (`NEW-61`)
- **FR-067**: `dict/parser.rs` MUST report an error on a duplicate tag/name/message-type
  definition instead of silently overwriting the earlier one. (`NEW-43`)
- **FR-068**: `dict/parser.rs`'s `strip_comment` MUST NOT truncate a STRING enum value that
  contains a literal `#` character. (`NEW-44`)
- **FR-069**: `validate.rs`'s `parse_fix_begin_string` MUST distinguish FIX 5.0/5.0SP1/5.0SP2
  instead of collapsing all three to the same version tuple. (`NEW-45`)
- **FR-070**: `config`'s variable resolution MUST report an error for a self-referential/circular
  `${var}` reference instead of emitting a literal `${...}` in output. (`NEW-46`)
- **FR-071**: A config `UnresolvedVariable` error MUST report the correct source line number after
  per-session merging, instead of always `line: 0`. (`NEW-47`)
- **FR-072**: `DataDictionary::extend()` MUST detect a `field_by_name` collision (same name,
  different tag) and recompute `member_tags` after a merge. (`NEW-53`)
- **FR-073**: Codegen's `parse_dict` MUST report an error for an unknown directive or malformed
  field/group/message line, consistent with the runtime `parser::parse`'s behavior for the same
  input, instead of silently skipping it. (`NEW-78`)
- **FR-074**: Codegen's enum-variant sanitization MUST detect/resolve label collisions that would
  otherwise produce duplicate, uncompilable Rust identifiers. (`NEW-79`)
- **FR-075**: The `generate_code` CLI's dual-track provenance hash MUST use the same byte
  representation (both lossy or both raw) for the runtime-`parse` and `codegen::generate` hashing
  paths. (`NEW-82`)

**Acceptance-test harness coverage** (affects `truefix-at`'s test coverage/quality, not production
session/transport code)

- **FR-076**: The AT harness's `start_acceptor` MUST load a dictionary validator for FIX versions
  beyond 4.2/4.4 so field-validation scenarios can run against all 9 supported versions. (`NEW-28`)
- **FR-077**: `CheckLatency`-family AT scenarios MUST assert on the resulting `Logout`'s reason
  text/field, not merely that a Logout of any kind was sent. (`NEW-29`)
- **FR-078**: The AT `run_scenario` runner MUST assert the read buffer is empty at scenario
  completion. (`NEW-30`)
- **FR-079**: The AT harness's `check_match` assertion MUST reject extra/erroneous fields on an
  actual message, not merely confirm expected fields are present. (`NEW-50`)
- **FR-080**: AT scenarios exercising server-sent messages MUST verify outbound `MsgSeqNum(34)`
  ordering. (`NEW-51`)
- **FR-081**: The `server_acceptance_suite_passes` AT conformance test MUST report per-scenario
  pass/fail rather than a single all-or-nothing result. (`NEW-52`)
- **FR-082**: The AT harness MUST provide a fixed-identity (non-dynamic-template) acceptor mode so
  Logon-time identity-validation scenarios (QuickFIX/J's `1c` category: wrong
  SenderCompID/TargetCompID/BeginString on Logon) can be represented. (`NEW-83`)

**Operational hygiene / diagnostics**

- **FR-083**: `Engine::start`'s acceptor-group startup iteration order MUST be stable/deterministic
  across process runs (e.g. sorted by socket address), so partial-startup-failure diagnostics are
  reproducible. (`NEW-92`)

### Key Entities

- **Sequence-number store row** (`MongoStore`): the `sender`/`target` fields tracked per
  `session_id`; FR-001/FR-002/FR-003 concern its write-correctness and uniqueness.
- **Session teardown reason**: a new internal distinction (graceful Logout vs. non-graceful
  disconnect vs. local reset) threaded through the single reset-decision point FR-014 introduces.
- **FIXT 1.1 transport header tag set**: the tag list `tags::is_header` must recognize per
  FR-028, and the `ApplVerID`-driven dispatch FR-030 depends on.
- **Config-key registry entry**: the `Impl`/`Recognized`/`Unsupported` classification per key in
  `truefix-config/src/keys.rs`, several of which FR-032/FR-058 correct to match actual wiring.
- **Dual-track provenance hash**: the hash computed independently by the runtime dictionary parser
  and by `codegen::generate`, which FR-075 requires to use consistent byte semantics.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every `NEW-NN` item in this spec's three user stories (83 items: 21 in US1, 30 in
  US2, 32 in US3) has a corresponding automated test that fails against the pre-fix behavior and
  passes after the fix, added to the existing unit/integration/AT test suites.
- **SC-002**: The full existing test suite (unit tests across all 9 crates, `truefix-at`'s
  acceptance scenarios, and any newly added tests from this feature) passes with zero failures and
  zero new `#[ignore]`d tests.
- **SC-003**: A `MongoStore`-backed session's sequence numbers survive a process restart with the
  same fidelity already demonstrated for the SQL/MSSQL/Redb/File backends (verifiable via a
  targeted integration test exercising `set_next_sender_seq`/`set_next_target_seq` followed by a
  fresh `MongoStore::connect` and `next_sender_seq`/`next_target_seq` read).
- **SC-004**: A simulated multi-session-acceptor client that opens a connection and sends nothing
  is disconnected within a bounded time instead of holding the connection/task open indefinitely.
- **SC-005**: No `.fixdict` file shipped in this repository changes as a result of this feature —
  all dictionary-tooling fixes (FR-064–FR-075) are verified via the codegen/converter test suites,
  not by any change to a shipped dictionary's content.
- **SC-006**: Zero clippy warnings and zero new `unsafe` blocks introduced by this feature's changes
  (matching the constitution's existing code-quality bar).

## Assumptions

- Per the audit document's own third-pass corrections, `NEW-01`, `NEW-31`, `NEW-57`, and `NEW-72`
  are refuted (parity with QuickFIX/J's own documented behavior, not TrueFix defects) and are
  excluded from this spec's scope entirely — no fix is required or expected for them.
- `NEW-40` (FileLog never fsyncs) is explicitly tagged `Parity-Note` in the audit document — it
  matches QuickFIX/J's own accepted "loose durability" behavior and is excluded from this spec's
  scope as a non-defect.
- The 6-row "Missing features" table (`docs/todo/005.md`, e.g. `EncryptMethod` inbound validation,
  `Username`/`Password` Logon authentication hooks, inbound `LastMsgSeqNumProcessed` validation,
  `MaxMessagesInResendRequest`, `MessageStore::refresh()`, a combined FIXT validate/dispatch helper)
  describes net-new QuickFIX/J/Go capabilities TrueFix does not implement at all — these are
  feature gaps, not confirmed defects in existing behavior, and are out of scope for this
  remediation-focused spec.
- The audit document's own note that `BUG-92`'s default-value half (`SessionConfig::new`'s
  `reset_on_logon` defaulting to `true` vs. QuickFIX/J's `false`) may still be open is a `004.md`
  item owned by the in-progress feature 007, not a `005.md` finding — verifying and, if needed,
  fixing it is that feature's responsibility, not this one's.
- This feature is built on top of features 001-007; any in-flight changes from feature 007 land
  first (or are rebased onto), since several `005.md` findings (e.g. `NEW-56`'s teardown-reason
  routing) touch the same `state.rs` machinery feature 007 is actively modifying.
- One narrowly-scoped new external dependency (e.g. `rustls-native-certs`) is permitted specifically
  to close `NEW-11`/`FR-049` (TLS default trust-store fallback) — resolved via Clarifications
  (Session 2026-07-04). No other item in this spec requires a new dependency.
- Where an item's fix direction in `docs/todo/005.md` offers multiple viable approaches (e.g.
  `NEW-56`'s "reason enum" mechanism, or `NEW-24`'s "backpressure vs. capped channel" choice for
  bounding log-writer queues), the specific mechanism is a planning-phase decision within the
  constraint that the described behavior (the acceptance scenario) holds.
