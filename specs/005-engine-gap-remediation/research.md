# Research: Engine Gap Remediation

**Feature**: [spec.md](./spec.md) | **Date**: 2026-07-02

All decisions below were grounded by reading the actual current code on `main` (post feature 004),
mirroring 003/004's research discipline. One finding **materially corrects** the source document
(`docs/engine-comparison-gaps.md`) itself — see §5 — the kind of correction this project's research
phase exists to catch before task-writing, not after.

## 1. US1/FR-001 — `#` comment-stripping fix

**Finding**: `crates/truefix-config/src/lib.rs:109` calls `strip_comment(raw)` before the `key=value`
split at line 121; `strip_comment` (`:177-182`) does `line.find('#')` — first occurrence anywhere.

- **Decision**: Change `strip_comment` to only strip when `#` is the first non-whitespace character of
  the *raw* line (check `raw.trim_start().starts_with('#')`, return the raw line unmodified otherwise —
  the `key=value` split downstream already trims key/value independently, so no whitespace-handling
  regression). This matches both QFJ (`SessionSettings.java`'s tokenizer only treats `#` as a comment
  start when a new label token is expected) and QFGo (`commentRegEx = ^#.*`, whole-line only).
- **Alternatives considered**: Escaping (`\#`) for literal `#` in values — rejected: neither reference
  engine supports escaping either, and introducing new escape syntax this project's own reference
  engines don't have would be a TrueFix-only extension with no precedent, contradicting Principle III's
  "abstract to protocol/config semantics, not invent new ones."

## 2. US1/FR-002 — `Signature`(89)/`SignatureLength`(93) mapping

**Finding**: `data_field_for_length()` (`crates/truefix-core/src/tags.rs:66-81`) is a fixed 12-pair
`match`, missing `93 => 89`. Consulted unconditionally at decode time
(`crates/truefix-core/src/codec/decode.rs:230`), independent of any loaded dictionary.

- **Decision**: Add `93 => 89, // SignatureLength -> Signature` as a 13th arm. No other code path
  touches this table (confirmed via grep — `data_field_for_length` has exactly one call site).
- **Alternatives considered**: None material — this is a single missing table entry with an
  unambiguous, spec-documented correct value (QFJ's own special-case comment: `Message.java:949`,
  "Special case for Signature which violates above assumption").

## 3. US2/FR-003+FR-004 — `JdbcURL` scheme recognition + credential splicing

**Finding**: `is_sql_scheme`/`is_mssql_scheme` (`crates/truefix-config/src/builder.rs:872-881`) only
match TrueFix's own sqlx-native schemes. Real QFJ `.cfg` files use `jdbc:<subprotocol>://...`
(confirmed against QFJ's own fixtures: `ATServer.java:121`, `JdbcTestSupport.java:38`), with
credentials in separate `JdbcUser`/`JdbcPassword` keys (`JdbcUtil.java:69-72`).

- **Decision**: `is_sql_scheme`/`is_mssql_scheme` gain a second check for `jdbc:postgresql://`/
  `jdbc:postgres://`/`jdbc:mysql://`/`jdbc:sqlite:`/`jdbc:h2:` (sqlite-family) vs. `jdbc:sqlserver://`
  (mssql-family), checked as a **distinct, ordered group** before the existing sqlx-native checks (no
  overlap possible — the two groups' prefixes are disjoint by construction, since one requires the
  literal `jdbc:` prefix and the other doesn't). When a `jdbc:`-prefixed URL is recognized,
  `jdbc_store_config`/its `mssql`/`sql`-gated helpers strip the `jdbc:` prefix before constructing the
  sqlx/tiberius connection string, then — **new step** — if the resulting URL has no `user:pass@`
  segment and `JdbcUser`/`JdbcPassword` are both set in the session's raw `.cfg` map, splice them into
  the URL's authority component before it reaches `StoreConfig::Sql`/`Mssql`. `JdbcUser`/`JdbcPassword`
  move from `Recognized` to `Implemented` in `keys.rs` once this lands.
- **Alternatives considered**: Threading `JdbcUser`/`JdbcPassword` through as a *separate* pair of
  `StoreConfig::Sql` fields instead of splicing into the URL string — rejected: `SqlStoreConfig`/
  `MssqlStoreConfig` (the underlying connect-with-config structs) only ever take one URL, not a
  URL+credentials pair; sqlx/tiberius's own connection-string parsers already expect credentials
  embedded (`ConnectOptions::from_str`), so splicing before that parse is the smaller, more consistent
  change relative to how `SqlStore`/`MssqlStore::connect(url)` already work.

## 4. US2/FR-005+FR-006+FR-006a — `AcceptorBuilder` wiring and the port-grouping mechanism

**Finding**: `Engine::start`'s acceptor branch (`crates/truefix/src/lib.rs:294-306`) calls
`Acceptor::bind_with(rs.address, rs.session, ...)` — one independent single-session bind per
`[SESSION]` block, always. `AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` are parsed by
nothing in `builder.rs` (zero grep hits) despite being `Impl` in `keys.rs:133-135`. The type that
*does* implement all three — `AcceptorBuilder<A>` (`crates/truefix-transport/src/lib.rs:1171-1229`,
`.with_session()`/`.with_dynamic_template()`/`.allow_remotes()`) — is only ever constructed from tests
and `examples/multi_acceptor.rs`.

- **Decision** (grouping mechanism resolved via `/speckit-clarify`): partition `resolved`'s acceptor
  sessions by `rs.address` (the full bind `SocketAddr`, which already IS "same host + same
  `SocketAcceptPort`" — no new parsing needed, this is exactly the field the loop already reads).
  For each group:
  - **Size 1, no `AcceptorTemplate`/`DynamicSession`/`AllowedRemoteAddresses` set anywhere in that
    session's raw `.cfg`**: keep today's `Acceptor::bind_with` path unchanged — zero behavior change
    for the overwhelmingly common single-session-per-port case, zero migration cost.
  - **Size > 1, or any of the three keys present**: build one `AcceptorBuilder::bind(addr, app.clone())`,
    `.with_session(session_config)` for every session in the group, `.with_dynamic_template(...)` if
    exactly one group member sets `DynamicSession=Y`/has an `AcceptorTemplate` value (new
    `ConfigError::AmbiguousAcceptorTemplate { addr }` if more than one member does — a real
    misconfiguration, not silently resolved by picking one), `.allow_remotes(...)` if
    `AllowedRemoteAddresses` is set (union across the group if more than one member sets it, since the
    field's own semantics are "addresses allowed to connect to this acceptor," a listener-level
    property, not a per-session one — matches this feature's Edge Cases entry on singular
    connection-level settings needing conflict detection, extended here to allow-lists specifically
    being union-combined rather than conflicting, since a list is naturally composable unlike a scalar
    like `SocketAcceptAddress`/TLS config).
  - TLS: if any group member sets `SocketUseSSL=Y`, the whole group's listener terminates TLS
    (`.with_tls(...)`) — a group is one physical listener, so TLS is necessarily listener-scoped, not
    per-session; conflicting per-session TLS material within one group is the same typed-startup-error
    treatment as the existing Edge Cases entry for other singular connection-level settings.
  This closes `GAP-17` (per-session, not builder-global, allow-lists) as a side effect: once each
  acceptor group has its own `AcceptorBuilder`, allow-lists are naturally scoped per bind address/port
  group rather than one flat list shared by an entire multi-session builder as `GAP-17` describes for
  the *pre-existing* `AcceptorBuilder` API — the multi-session builder itself already supports
  per-instance allow-lists (`.allow_remotes()` is an instance method); the gap was purely that
  `Engine::start` never constructed more than one implicit "group" (each session its own).
- **Alternatives considered**: (a) A new explicit `.cfg` key (e.g. `AcceptorGroup=<name>`) to declare
  grouping instead of inferring it from `SocketAcceptPort` — rejected per `/speckit-clarify`'s answer:
  zero-migration-cost auto-grouping was preferred over adding new required config surface. (b) Always
  route every acceptor session through `AcceptorBuilder`, even size-1 groups with no special keys —
  rejected: needlessly changes the code path (and, in principle, timing/behavior) for the common case
  that works fine today, for no observable benefit; the size-1-no-special-keys fast path is free to
  keep and reduces this change's blast radius.

## 5. US3/FR-007 — resend veto and gap-fill substitution: **correction to the source document**

**Finding (corrects `docs/engine-comparison-gaps.md`'s GAP-07 framing)**: the source document and the
verification pass that produced it state TrueFix's `build_resend` "never gives the `Application` trait
a chance to suppress" a stale resend. Reading the actual call chain shows this is **not quite right**:
`Application::to_app` (`crates/truefix-session/src/application.rs:49-51`) **is already invoked** for
every `Action::Send` carrying a non-admin message — including resend-originated ones — inside
`perform_actions` (`crates/truefix-transport/src/lib.rs:1032-1052`). `build_resend`
(`crates/truefix-session/src/state.rs:623-652`) pushes resent application messages via `self.send_raw`,
which produces exactly this `Action::Send` variant; there is no separate "live send" vs. "resend"
action type, so both already funnel through the same `to_app` check.

The **real** gap is narrower and different in kind: when `to_app` returns `Err(DoNotSend)` for a
resend-originated message, `perform_actions` calls `session.discard_sent(seq)`
(`crates/truefix-session/src/state.rs:207-209`, a bare `self.store.remove(&seq)`) and moves on — it
does **not** emit a compensating `SequenceReset-GapFill` for the skipped sequence number. For a *live*
send this is correct (the sequence number was never promised to the counterparty, so silently not
sending it is invisible/harmless). For a *resend*, the sequence number **was already promised** in an
earlier connection — silently skipping it without a GapFill leaves the counterparty's own
next-expected-sequence tracking permanently stuck waiting for a message that will never arrive, since
nothing tells it the number was intentionally void. This exactly matches this feature's own FR-007
wording ("substituting a gap-fill for that message when suppressed"), which anticipated the correct
requirement even though the audit that produced GAP-07 mischaracterized the starting mechanism.

- **Decision**: `Action::Send` gains a `origin: SendOrigin` field (new `enum SendOrigin { Live, Resend
  { seq: u64 } }`, `Live` for every existing call site except `build_resend`'s, which becomes
  `Resend { seq }`) — additive to the enum's only variant carrying a `Message` today, not a breaking
  change to any *external* API (`Action` is `truefix-session`-internal, re-exported only as an opaque
  type transport consumes, never constructed outside this workspace). In `perform_actions`, when
  `to_app` returns `Err` for `Action::Send { origin: SendOrigin::Resend { seq }, .. }`, instead of only
  calling `discard_sent(seq)`, additionally call a new `Session::gap_fill_after_veto(seq) -> Action`
  (thin wrapper around the existing private `gap_fill` helper, `state.rs:653-656`, already used by
  `build_resend` itself for genuinely-missing stored messages) and immediately process the resulting
  `Action::Send` (the GapFill) through the normal `to_admin`+write+log+persist path — GapFills are
  admin-typed, so this recursion terminates in one more step, never loops. `Action::Send { origin:
  SendOrigin::Live, .. }` keeps today's exact behavior (discard only, no GapFill) — zero behavior
  change for the non-resend path, which is the overwhelming majority of sends.
- **Alternatives considered**: (a) Add a wholly new `Application` callback (e.g.
  `resend_approved(&self, seq: u64, session: &SessionId) -> bool`) instead of reusing `to_app` —
  rejected once the finding above was made: `to_app` already receives the exact resent `Message`
  (mutable, inspectable, matching QFJ's own `resendApproved` calling `application.toApp` on the literal
  resend candidate) — adding a second, narrower callback for the same decision point would be a
  parallel mechanism achieving the same thing `to_app` already achieves, contradicting this project's
  own "no parallel implementation" discipline (the same reasoning `truefix-dict`'s CLI/`build.rs` code
  sharing already established). (b) Compute the GapFill synchronously inside `build_resend` itself,
  before any veto is known — impossible: the veto decision is async (`Application::to_app`) and
  `Session` is deliberately sans-IO/synchronous (an established architectural boundary, see
  `on_before_reset`'s precedent for why async decisions never move into `Session` itself); the
  transport layer must be the one to react to the veto and call back into `Session` for the compensating
  action, exactly as this decision's chosen design does.

## 6. US3/FR-008+FR-009 — PossDup anti-replay: reusing `reject_logon`

**Finding**: `Session::reject_logon(&self, reject: &Reject) -> Vec<Action>`
(`crates/truefix-session/src/state.rs:215-223`) already implements exactly the "send Logout (with
optional `SessionStatus`) + transition to `Disconnected` + `Action::Disconnect`" sequence
`/speckit-clarify`'s resolved answer requires for both this FR and FR-010. It's currently invoked only
from the `Application::from_admin` rejection path (an app-level decision surfaced through the
transport layer), but nothing about its implementation is app-callback-specific — it just needs a
`Reject` value, which a purely session-internal protocol violation can construct directly without going
through `Application` at all (matching QFJ's own placement: `validatePossDup` is `Session`-internal
logic, not routed through `Application`).

- **Decision**: `on_received`'s `Ordering::Less` + `poss_dup` branch (`state.rs:511-513`, currently
  `Vec::new()`) gains an `OrigSendingTime`-vs-`SendingTime` comparison; on violation, calls
  `self.reject_logon(&Reject { reason: <SessionRejectReason ~ "value is incorrect">, ref_tag: Some(122)
  /* OrigSendingTime */, text: Some("OrigSendingTime is later than SendingTime on a PossDup message"),
  session_status: None })` directly (no `Application` round-trip). `RequiresOrigSendingTime` (FR-009)
  gates whether a *missing* `OrigSendingTime` on this same code path is also treated as a violation
  (new `SessionConfig.requires_orig_sending_time_on_low_seq: bool` field or reuse of the field
  `ValidationOptions.requires_orig_sending_time` already has at the dictionary-validate layer — final
  choice deferred to `/speckit-tasks`, since it's a naming/reuse decision, not a design one; either way
  it's a new, explicit switch at the session-config layer, since this code path runs before/independent
  of dictionary `validate()`, per this decision's own §5-adjacent finding that `validate()` never
  reaches this branch).
- **Alternatives considered**: A new dedicated `Session::reject_poss_dup(...)` method instead of
  reusing `reject_logon` — rejected: the resulting wire behavior (Logout + disconnect) and internal
  state transition are identical; introducing a second method for the same effect is unwarranted
  duplication given `reject_logon`'s implementation has zero `from_admin`-specific logic to strip out.

## 7. US3/FR-010 — duplicate Logon: same mechanism as §6

**Finding**: `on_logon`'s `match self.config.role { ... _ => {} }` (`state.rs:711-730`) silently
no-ops when a Logon arrives while already `LoggedOn`.

- **Decision**: Add an explicit arm (or an early check before the existing `match`) for "already
  `LoggedOn`" that calls `self.reject_logon(&Reject { reason: <SessionRejectReason ~ "already logged
  on" or a generic session-level reason — no dedicated FIX reason code exists for this case in the base
  spec>, ref_tag: None, text: Some("session is already logged on") })`, reusing the exact same
  mechanism as §6's PossDup fix. Both FRs share one implementation primitive; `/speckit-tasks` can
  reasonably sequence them as one task or two adjacent ones.
- **Alternatives considered**: None material — this is the smaller, already-precedented half of the
  pattern §6 establishes.

## 8. US4/FR-011 — inbound chunked-resend auto-continuation

**Finding**: `on_resend_request` (`state.rs:600-621`) answers exactly the range requested;
`on_sequence_reset` (`state.rs:657-679`, not shown in this excerpt but confirmed via the source
document's citation) advances `next_in_seq` with no follow-up. TrueFix's *outbound* auto-chunking
(`resend_request_chunk_size`, already implemented) is a different code path — it governs how a store's
own resend loop breaks a large *outbound* replay into chunks, not how an application reacts to an
*inbound* gap larger than one chunk.

- **Decision**: When TrueFix is the requester (inbound gap detected via `Ordering::Greater` in
  `on_received`, `state.rs:500-508`) and `resend_request_chunk_size > 0`, track the *originally
  detected* gap's full upper bound (today `request_resend(begin)` — confirmed open-ended, requests
  through the current known end) separately from the chunk actually requested. When a `SequenceReset`
  or the last message of a chunk brings `next_in_seq` up to the chunk boundary but *below* the tracked
  full gap, automatically emit the next chunk's `ResendRequest` (reusing the existing chunk-request
  construction logic, parameterized by the new chunk start) instead of waiting for an external
  `ResendRequest` (which, notably, would never arrive from a well-behaved reference-engine counterparty
  in the first place, since neither QFJ nor QFGo need to be told the same thing twice — the receiving
  side auto-continues, not the sender).
- **Alternatives considered**: Re-requesting the whole remaining range unchunked once one chunk lands
  — rejected: defeats the purpose of `resend_request_chunk_size` existing at all (bounding one
  resend-servicing burst's size on *both* sides of the exchange, not just the responder's side).

## 9. US5/FR-012+FR-013 — `SessionId` sub-ID/location-ID/qualifier construction path

**Finding**: `SessionId` (`crates/truefix-session/src/session_id.rs:8-23`) has all five fields;
`SessionId::new()` (`:27-42`) is the *only* constructor and hardcodes all five to `None`. Zero
`SessionId { .. }` struct-literal constructions exist anywhere outside this file. `SessionConfig`
(`crates/truefix-session/src/config.rs:19-25`) has no matching fields at all — `SessionConfig` is what
`builder.rs::resolve_one` actually populates from `.cfg`, and what `Session::new` derives its identity
from via `config.session_id()`.

- **Decision**: Add `sender_sub_id: Option<String>`, `sender_location_id: Option<String>`,
  `target_sub_id: Option<String>`, `target_location_id: Option<String>`,
  `session_qualifier: Option<String>` to `SessionConfig` (five new `Option<String>` fields, all
  additive/`None`-defaulted — matches this feature's own "existing `.cfg` files keep working"
  Assumption). `SessionConfig::session_id()` (wherever it currently constructs a `SessionId` from just
  `begin_string`/`sender_comp_id`/`target_comp_id`) passes these five through to a new `SessionId`
  constructor, `SessionId::new_full(begin_string, sender_comp_id, sender_sub_id, sender_location_id,
  target_comp_id, target_sub_id, target_location_id, session_qualifier)` (or equivalently, extend
  `SessionId::new` with a builder-style `.with_qualifier(...)` etc. — final shape deferred to
  `/speckit-tasks`; either is additive to the public type). `builder.rs::resolve_one` parses the five
  new `.cfg` keys (`SenderSubID`/`SenderLocationID`/`TargetSubID`/`TargetLocationID`/
  `SessionQualifier`) into these `SessionConfig` fields, exactly mirroring how `begin_string`/
  `sender_comp_id`/`target_comp_id` are already parsed today. Two `[SESSION]` blocks producing distinct
  `SessionId`s (by `Hash`/`Eq`, which `SessionId` already derives across all 8 fields including
  `session_qualifier`) are naturally "distinct sessions" with no extra uniqueness logic needed — the
  derive already exists and already includes the qualifier field; the gap was purely that nothing ever
  populated it.
- **Alternatives considered**: None material — this is a straight "thread five already-declared,
  never-populated fields through the one missing hop" fix; the hard design work (deciding `SessionId`
  should carry these fields at all, and that `Hash`/`Eq` should span all of them) was already done in
  an earlier feature.

## 10. US6/FR-014 — reconnect backoff array

**Finding**: `connect_initiator_reconnecting_multi`/`_tls` (`crates/truefix-transport/src/lib.rs:1432,
1462,1484,1518`) use one fixed `Duration` for every retry, sourced from
`config.reconnect_interval: u32` (a single seconds value) on `SessionConfig`.

- **Decision**: Add `SessionConfig.reconnect_interval_steps: Vec<u32>` (new field, empty by default —
  when empty, the existing single-`reconnect_interval` behavior is preserved verbatim, zero regression
  for every `.cfg` not using this feature). When non-empty, the reconnect loop indexes into it by
  attempt number, clamped to the last element once exhausted (matching QFJ's `int[]`-with-sticky-last-
  value semantics exactly). `.cfg` parsing: `ReconnectInterval` accepts either a single integer
  (today's behavior, unchanged) or a space/comma-separated list of integers (QFJ's own `.cfg` grammar
  for this key already supports a list in the same field — confirmed as the natural, zero-new-key
  choice, avoiding a second `ReconnectIntervalSteps`-style key that real QFJ `.cfg` files wouldn't
  contain anyway).
- **Alternatives considered**: Numbered keys (`ReconnectInterval1`, `ReconnectInterval2`, ... mirroring
  this project's own existing `SocketConnectHost1`/`Port1` numbered-key convention for failover
  endpoints) — rejected: `ReconnectInterval`'s value in real QFJ `.cfg` files is a single
  space-delimited string, not numbered keys (unlike failover hosts, which genuinely are QFJ's own
  numbered-key convention) — matching QFJ's actual grammar for *this specific key* takes priority over
  internal consistency with an unrelated key's convention, per this feature's overall
  drop-in-`.cfg`-compatibility theme (same reasoning as `JdbcURL`'s BUG-04 fix).

## 11. US6/FR-015 — `SocketLocalHost`/`SocketLocalPort`

**Finding**: Every initiator connect path (`lib.rs:320,338,1442,1495`) calls
`TcpStream::connect(addr)` directly, no local-bind step. Keys already `Recognized` in `keys.rs:145-146`.

- **Decision**: `SessionConfig` gains `local_bind_addr: Option<SocketAddr>` (parsed from
  `SocketLocalHost`+`SocketLocalPort`, both required together — a `SocketLocalPort` with no
  `SocketLocalHost` is ambiguous, matching how `SocketAcceptPort` already requires
  `SocketAcceptAddress` conventions elsewhere in this codebase). When set, every initiator connect
  path uses `socket2::Socket`'s `bind()` before `connect()` (the same `socket2`-based construction
  pattern the existing socket-options code already uses for `SocketReuseAddress`/`SocketLinger`/etc.,
  confirmed no new dependency needed) instead of the bare `TcpStream::connect`.
- **Alternatives considered**: None material — this is a standard local-bind-then-connect pattern with
  one clear implementation using an already-vendored crate (`socket2`).

## 12. US6/FR-016 — `SocketConnectTimeout`

**Finding**: `keys.rs:150` marks it `Recognized`; zero `tokio::time::timeout` wrapping around any
`TcpStream::connect` call, including feature 004's newest failover path.

- **Decision**: `SessionConfig` gains `connect_timeout: Option<Duration>` (parsed from
  `SocketConnectTimeout`, seconds — matching this project's existing `Duration`-from-seconds parsing
  convention for e.g. `LogonTimeout`). Every initiator connect call site wraps its `TcpStream::connect`
  (or, per §11, the `socket2` bind-then-connect sequence) in `tokio::time::timeout(dur, ...)` when set;
  `None` (default) preserves today's unbounded-wait behavior exactly.
- **Alternatives considered**: None material.

## 13. US7/FR-017 — session creation-time persistence

**Finding**: No `.session` file, no `creation_time` column anywhere in `truefix-store`'s backends
(confirmed: `file.rs`, `sql.rs`, `redb.rs`, `mongo.rs` all lack it).

- **Decision**: `MessageStore` trait gains one new method with a default implementation returning
  `None`: `fn creation_time(&self) -> Option<time::OffsetDateTime> { None }` (a default-method addition
  is non-breaking for existing external implementors — matches this project's own precedent,
  `was_corrupted()`, which is already a defaulted trait method on this exact trait). Each backend
  overrides it: `FileStore`/`CachedFileStore` write a sibling `.session` file (or a `creation_time`
  line inside an existing metadata file, implementation detail for `/speckit-tasks`) at first
  `connect`; `SqlStore`/`MssqlStore` gain a `creation_time` column on the sessions table (populated at
  row-insert time, in the same transaction per §14 below); `RedbStore` gains a fourth table (or a
  reserved key in an existing table) for the timestamp; `MongoStore` adds a field to `SessionDoc`.
  `reset()` updates the stored creation time to "now" for every backend that has one (matching QFJ/
  QFGo's own "creation time updates on reset" semantics this FR quotes).
- **Alternatives considered**: A required (non-defaulted) trait method — rejected: would be a breaking
  change to `MessageStore` for any external implementor, unlike a defaulted addition; this project's
  own `was_corrupted()` precedent already established the defaulted-addition pattern for exactly this
  kind of optional-capability trait growth.

## 14. US7/FR-018 — atomic save + sequence-increment for SQL-family stores

**Finding**: `SqlStore`/`MssqlStore` (`crates/truefix-store/src/sql.rs`) issue `save()` and
`set_next_sender_seq()`/`set_next_target_seq()` as independent `sqlx`/`tiberius` statements — no shared
transaction. `RedbStore::reset()` is already correctly atomic (one write transaction, feature 004);
its `save`/`set_next_*` are not (same shape as the SQL stores today).

- **Decision**: The specific call site this FR targets — the engine's own "persist an outbound message,
  then advance the sender sequence" sequence (wherever in the session/transport integration this pairing
  currently happens as two separate `MessageStore` calls) — gets a new `MessageStore` trait method,
  `async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError>`
  (default implementation calls `save` then `set_next_sender_seq(seq + 1)` sequentially, preserving
  today's behavior for any backend that doesn't override it), overridden by `SqlStore`/`MssqlStore`
  (single `sqlx`/`tiberius` transaction) and `RedbStore` (single `redb` write transaction, mirroring
  its existing `reset()`). `MemoryStore`/`NoopStore`/`FileStore`/`CachedFileStore` don't need an
  override — their existing implementations are already effectively atomic (single-threaded in-process
  data structures, or file-system operations with no multi-statement window to begin with) or don't
  need the guarantee (`NoopStore`).
- **Alternatives considered**: Adding transactionality generically inside the existing `save`/
  `set_next_sender_seq` pair via a caller-side lock — rejected: doesn't actually close the crash-window
  gap for SQL backends (two separate network round-trips to the database are still two separate
  commits no matter what the *caller* does); the fix has to live inside the backend that can actually
  offer a single-transaction guarantee.

## 15. US7/FR-019 — structured log schema: timestamp + session-identity column

**Finding**: Every structured log backend's schema is `(id, text)` only —
`crates/truefix-log/src/sql.rs:159-183` (SQL/MSSQL, `ensure_table`), and the same shape confirmed
propagated to `redb.rs`/`mongo.rs` (feature 004's new backends inherited the pattern).

- **Decision**: Extend each backend's schema additively: SQL/MSSQL `ensure_table` gains `logged_at`
  (a `TIMESTAMP`/equivalent column, server-default-now on insert) and `session_id` (`TEXT`, populated
  from the same `session_id: &str` parameter `SqlLog::connect_with_config`/`insert_text` already
  receive today — currently unused for anything beyond table selection); `RedbLog`'s table value type
  changes from `&str` (bare text) to a small serialized struct/tuple `(timestamp, session_id, text)`;
  `MongoLog`'s `doc! { "text": text }` gains `"logged_at"`/`"session_id"` fields. All four backends
  already *have* a `session_id`-equivalent piece of context available at the call site (it's either a
  constructor parameter or, for `SqlLog`/`MssqlLog`, implicit in which table-name set was chosen) — this
  is a schema-widening change, not a new-context-plumbing one.
- **Alternatives considered**: A separate correlation table/side-channel instead of widening each log
  row — rejected: defeats the stated purpose (audit/replay from *one* row, without an external join);
  widening the row is the direct fix for "impossible to audit from a shared log table."

## 16. US8/FR-020+FR-021 — stance-registry corrections + JDBC pool/table-name wiring

**Finding**: Four keys (`SocketAcceptProtocol`/`SocketConnectProtocol`/`JdbcDataSourceName`/
`JdbcConnectionTestQuery`) are `Recognized` but structurally cannot be wired (§ found during the
full-registry audit that produced `docs/engine-comparison-gaps.md`'s "Documentation-accuracy notes").
Separately, `SqlStoreConfig` (`crates/truefix-store/src/sql.rs:56-67`) already has `sessions_table`/
`messages_table`/`session_id`/`pool: SqlPoolOptions` fields that `StoreConfig::Sql { url }`'s bare-URL
shape (used by `.cfg`-driven `jdbc_store_config`, §3/§4 above) discards.

- **Decision**: `keys.rs` stance downgrades (four keys → `Unsupported` with the specific reasons already
  documented in `docs/engine-comparison-gaps.md`'s notes — VM_PIPE-only alternative, JNDI-only lookup,
  `sqlx`'s automatic `test_before_acquire` having no `.cfg`-expressible custom-query hook). Separately,
  `StoreConfig::Sql`/`Mssql` gain the same additive-fields treatment §4 already applies elsewhere in
  this plan: either extend the existing variants with optional `sessions_table`/`messages_table`/
  `session_id`/`pool` fields (defaulted to `SqlStoreConfig::new`'s existing defaults when absent — zero
  behavior change for `.cfg` files not setting these keys), or thread a full `SqlStoreConfig`/
  `MssqlStoreConfig` through in place of the bare `url: String` the variants carry today (final
  structural choice deferred to `/speckit-tasks`, since both are additive/non-breaking and the
  difference is purely which struct shape is easier to construct from `builder.rs`'s already-parsed
  key/value map). Once threaded, `jdbc_store_config`/`resolve_store` parse
  `JdbcMaxActiveConnection`/`JdbcMaxConnectionLifeTime`/`JdbcMinIdleConnection`/`JdbcConnectionTimeout`/
  `JdbcConnectionIdleTimeout`/`JdbcConnectionKeepaliveTime`/`JdbcStoreMessagesTableName`/
  `JdbcStoreSessionsTableName`/`JdbcSessionIdDefaultPropertyValue` into it, and all nine move from
  `Recognized` to `Implemented`.
- **Alternatives considered**: None material for the four downgrades (each has exactly one correct
  reason already established by the source-document audit). For the pool/table-name wiring: keeping
  `StoreConfig::Sql { url }` bare and adding a *parallel* `StoreConfig::SqlWithOptions { .. }` variant
  — rejected: splits one logical concept into two enum variants for no reason once the plain variant
  can just grow optional fields; every existing `match` on `StoreConfig` in this workspace is
  non-exhaustive (has a wildcard or is inside `build_store`'s own exhaustive-by-design match, which
  would need updating either way), confirmed via grep, so field-widening is strictly simpler.

## 17. US9/FR-022 through FR-031 — dictionary/codec model completeness

This is the largest, most structurally varied cluster in this feature — nine sub-decisions, one per
FR, since each touches a different part of `truefix-dict`'s model/parser/validator/codegen with little
shared implementation:

### 17a. FR-022 — 11 additional `FieldType` variants

- **Decision**: Extend the `FieldType` enum (`crates/truefix-dict/src/model.rs:9-44`) with the 11
  variants QFJ has and TrueFix doesn't (`PriceOffset, LocalMktDate, DayOfMonth, UtcDate, Time,
  Currency, Exchange, MultipleValueString, MultipleStringValue, MultipleCharValue, Country`), each
  with a `value_ok(&str) -> bool` arm in the existing type-checking match (`model.rs:72-84`) matching
  its documented FIX format: `LocalMktDate`/`UtcDate` reuse the existing `UtcDateOnly` format check;
  `Time` reuses `UtcTimestamp`'s; `DayOfMonth` is a bounded integer (1-31); `Currency`/`Exchange`/
  `Country` are fixed-length alpha codes (ISO 4217/MIC/ISO 3166 — format-checked for length/charset,
  not validated against an actual currency/exchange/country code list, matching QFJ's own converter
  behavior which is also format-only); `PriceOffset` reuses the existing decimal (`Price`-family)
  check; `MultipleCharValue` is a space-separated list of single characters; `MultipleValueString`/
  `MultipleStringValue` are QFJ's two historical names for the same space-separated-enum semantics
  (both map to one TrueFix behavior — §17b covers the enum-membership half of this).
- **Alternatives considered**: A dedicated crate/lookup table for real currency/exchange/country code
  validation — rejected as out of scope: QFJ itself doesn't validate against real code lists either
  (`(raw)` per the source document's comparison table), so matching QFJ's actual behavior means
  format-only, not a new correctness bar this feature doesn't need to set.

### 17b. FR-023 — open-enum sentinel

- **Decision**: `FieldDef` (`model.rs:89-98`) gains `pub open_enum: bool` (defaulted `false` — every
  existing bundled `.fixdict`/hand-written dictionary keeps its current closed-enum behavior
  unchanged). The normalized `.fixdict` grammar gains an `open` modifier on a field's value list
  (parser change in `crates/truefix-dict/src/parser.rs`, additive grammar — a dictionary file without
  the new modifier parses identically to today). `validate.rs`'s enum-membership check
  (`model.rs:102-104`/`validate.rs:125-133`) short-circuits to "always pass" when `open_enum` is set.
  `MultipleValueString`/`MultipleStringValue` fields (§17a) that are also `open_enum` combine both
  checks: space-split, then each token is enum-checked unless open.
- **Alternatives considered**: A sentinel *value* (QFJ's literal `"__ANY__"` string in the values list)
  instead of a struct field — rejected: a boolean flag is simpler to check and can't collide with a
  real enumerated value that happens to be the literal string `"__ANY__"` (a real, if remote,
  correctness edge case a sentinel-value design would have to additionally guard against).

### 17c. FR-024+FR-025 — per-group child dictionaries + repeating-group mutation API

- **Decision**: `GroupDef` (`model.rs:364-371`) gains `pub child: Option<Box<DataDictionary>>` — a
  nested dictionary scoped to just that group's fields/messages-as-entries, built during dictionary
  construction by projecting the group's `members` (already-flattened tag list) into a minimal
  `DataDictionary` (reusing the existing `DataDictionary` builder path, not a new type). `validate.rs`'s
  group-walking logic (`:238-260`) recurses into `child` when present, applying the same `validate`
  entry point to each group instance's `FieldMap` — reusing existing validation logic recursively
  rather than writing new group-specific validation rules. Separately (independent implementation,
  same FR cluster since QFJ bundles them together conceptually): `crates/truefix-core/src/field_map.rs`
  gains `replace_group(count_tag, index, entry: FieldMap)`, `remove_group(count_tag, index)`,
  `get_group(count_tag, index) -> Option<&FieldMap>` alongside the existing `add_group` — straightforward
  `Vec`-index operations on `Member::Group`'s existing `entries: Vec<FieldMap>`, no new storage.
- **Alternatives considered**: Flattening child-dictionary validation into the parent `validate()`
  function directly (special-casing groups inline) instead of recursive child-dictionary validation —
  rejected: the existing `validate()` entry point already handles every check this needs (required
  fields, enum membership, type format, unknown-field policy) — recursing into it for each group
  instance reuses all of that for free; special-casing would duplicate every one of those checks a
  second time for group content specifically.

### 17d. FR-026 — header/trailer repeating groups at the core codec layer

- **Decision**: `crates/truefix-core/src/codec/decode.rs`'s header/trailer parsing (`:69-77`, currently
  flat `FieldMap`s with no group awareness) gains the same `GroupSpec`-driven group-building path
  `decode_with_groups`'s body-parsing already uses (`:65-90`, `build_group`, `:93-126`) — parameterized
  by a header/trailer `GroupSpec` set instead of the message-body one, threaded through from whichever
  dictionary defines header/trailer groups (today none do; this is dormant until a dictionary source
  declares one, e.g. `NoHops`/tag 504, which the normalized-`.fixdict` grammar already supports
  declaring in a `header`/`trailer` section per existing `parser.rs` directives — confirmed no parser
  change needed, only the decode-side consumer).
- **Alternatives considered**: None material — this reuses the identical group-parsing machinery the
  body already has; the gap was purely that header/trailer decoding never called it.

### 17e. FR-027 — custom per-message field emission order

- **Decision**: `MessageDef` (`model.rs:108-134`) gains `pub field_order: Option<Vec<u32>>` (parsed
  from the normalized `.fixdict`'s existing member-tag-list ordering when a new `ordered` directive
  modifier is present on a `message` block — additive grammar, same pattern as §17b). `Message::encode`
  (`crates/truefix-core/src/codec/encode.rs`) accepts an optional field-order slice; when present,
  fields present in the order list are emitted in that order (with any dictionary-unlisted/UDF fields
  appended after, matching QFJ's own `FieldOrderComparator` "unspecified fields last" semantics) instead
  of `Vec<Member>` insertion order. The dual-track codegen (`codegen.rs`) gains the same optional
  parameter for typed-struct emission, keeping both tracks byte-identical (Principle IV) when a
  `field_order` is present.
- **Alternatives considered**: A per-field priority number instead of a full order list — rejected:
  QFJ's own `int[] fieldOrder` is a full ordered list, not per-field priorities; matching that shape
  directly avoids inventing a different ordering model with different edge-case semantics (e.g. ties).

### 17f. FR-028+FR-029 — dictionary version metadata + BeginString match validation

- **Decision**: `DataDictionary` (`model.rs:152-162`) gains `pub(crate) version_meta: Option<VersionMeta>`
  (new `struct VersionMeta { major: u8, minor: u8, service_pack: Option<u8>, extension_pack: Option<u8>
  }`), populated by the normalized-`.fixdict` parser from a new optional `version-meta` directive
  (additive — dictionaries without it parse exactly as today, and `version_meta` stays `None`, per the
  Edge Case already recorded in spec.md). `validate.rs` gains a version-match check
  (`beginstring_matches_dictionary_version`) that's a no-op when `version_meta` is `None`
  (spec.md Edge Case), and otherwise compares the message's `BeginString`/`ApplVerID` against the
  loaded dictionary's metadata, flagging a mismatch as a session-level reject (matching QFJ's
  `DataDictionary.java:632-639` treatment).
- **Alternatives considered**: Deriving version metadata implicitly from the existing `version: String`
  field (e.g. parsing `"FIX.4.4"` back into major=4/minor=4) instead of an explicit new field —
  rejected: loses service-pack/extension-pack information the string form doesn't carry at all
  (`"FIX.5.0SP2"` parses to major=5/minor=0/SP=2 unambiguously, but reconstructing this via string
  parsing on every access is both slower and a redundant parallel representation of the same fact an
  explicit struct field states once, at load time).

### 17g. FR-030 — value→label lookup

- **Decision**: `FieldDef.values: Vec<String>` becomes `Vec<(String, Option<String>)>` (value +
  optional human-readable description) — or, to avoid a breaking change to `FieldDef`'s existing public
  `Vec<String>` field shape, add a parallel `pub value_labels: BTreeMap<String, String>` field instead
  (value → label, only for values that have one) alongside the unchanged `values: Vec<String>` (final
  choice — replace vs. parallel field — deferred to `/speckit-tasks`; the parallel-field option is
  additive/non-breaking, the replace option is more normalized but touches every existing `values`
  read site in this workspace, a wider blast radius for the same outcome). The normalized `.fixdict`
  grammar gains an optional `# label` inline comment convention or a `value=VAL:Label` extended syntax
  for declaring labels (additive grammar either way).
- **Alternatives considered**: None material beyond the field-shape choice already flagged above for
  `/speckit-tasks`.

### 17h. FR-031 — bundled dictionary coverage expansion, targeted at QuickFIX/J parity

**Finding**: `/speckit-clarify`'s resolved answer targets QuickFIX/J's own bundled dictionaries in
`thrdpty/quickfixj` as the concrete per-version reference, not an abstract "Appendix A" list or an
undefined best-effort.

- **Decision**: For each of TrueFix's 9 bundled FIX versions (FIX40 through FIX50SP2; `FIX.Latest` is
  sourced from FIX Orchestra already, a different mechanism — see `docs/todo-gap-analysis.md`'s TODO-10
  — and is out of this FR's scope since it has no single QFJ XML file to diff against the same way),
  diff TrueFix's normalized `.fixdict` field/message set against the corresponding QFJ XML dictionary
  file (`thrdpty/quickfixj/.../FIX4?.xml` or equivalent), read **for field/message enumeration only**
  (names, tags, types, message associations — data facts, not source code, matching Principle III's
  "abstract to protocol/config semantics" boundary already established for AT-scenario porting), and
  add every missing field/message to the normalized `.fixdict` source. This is inherently the largest
  single piece of *content* work in this feature (as opposed to *mechanism* work, which the other FRs
  in this cluster are) — `/speckit-tasks` should size it per-version rather than as one task, given the
  independent, parallelizable nature of each version's diff-and-fill work.
- **Alternatives considered**: Targeting QuickFIX/Go's bundled dictionaries instead of QuickFIX/J's —
  rejected per `/speckit-clarify`'s literal answer ("QuickFIX 的模式" in context clearly referred to
  the reference engine this whole feature already treats as primary for `.cfg`-compatibility, i.e. QFJ,
  consistent with GAP-33's own framing quoting QFJ specifically); QFGo's dictionaries are a secondary
  source that could inform edge cases but not the primary coverage target.

## 18. Dependency audit

**Finding**: No FR in this feature's final scope requires a new external crate. `GAP-10`
(`chrono-tz` for IANA time-zone names) is explicitly out of scope per spec.md's Assumptions. §11's
local-bind implementation reuses the already-vendored `socket2` crate (already a direct dependency of
`truefix-transport`, used for the existing socket-options feature set). Every other FR is
logic/schema/model change against already-present dependencies (`sqlx`, `tiberius`, `redb`, `mongodb`,
`tokio`, `time`).

- **Decision**: No Phase 0 dependency/license-audit gate is needed for this feature (contrast to 004,
  which added `redb`/`mongodb` and required one) — confirmed by scanning every FR above for a new-crate
  requirement and finding none.
