# Phase 1 Data Model: Audit Remediation (docs/todo/003.md)

**Feature**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

This translates the spec's Key Entities (plus the concrete shapes research.md's investigation
surfaced) into grounded types. Every entity here is additive or internal-behavior-only to the
existing `truefix-core`/`truefix-session`/`truefix-transport`/`truefix-config`/`truefix-store`/
`truefix-dict`/`truefix` types — **no existing public type's shape is removed or narrowed**, and
(per plan.md's Technical Context) only one public-API surface grows: `MssqlStore::parse_url`'s
accepted grammar.

## `SessionId` routing key (extends `truefix-session::session_id`, `truefix-transport::route_and_run`)

```text
// No new type — SessionId::new_full already exists (feature 002-era design, confirmed present).
// The fix is entirely in the CALLER: route_and_run extracts 4 more wire tags and calls new_full
// instead of new.

SessionId::new_full(
    begin_string,
    their_target,           // -> our sender_comp_id position (reversed, as today)
    Some(sub_id_142_from_logon),      // NEW extraction: tag 142 = SenderLocationID -> our sender_location_id... 
    // (exact tag->field mapping below)
    their_sender,
    ...
)
```

- **No field/type added.** `route_and_run` (`crates/truefix-transport/src/lib.rs:1433`) changes its
  `SessionId::new(begin, their_target, their_sender)` call to `SessionId::new_full` with 5 additional
  arguments extracted from the inbound Logon: tag 50 (`SenderSubID`) and tag 142
  (`SenderLocationID`) from the counterparty's Logon become **our** `target_sub_id`/
  `target_location_id` (since "our SessionID reverses the counterparty's comp IDs" — same reversal
  rule the existing 3-field extraction already applies); tag 57 (`TargetSubID`) and tag 143
  (`TargetLocationID`) become our `sender_sub_id`/`sender_location_id`. `session_qualifier` has no
  wire tag and stays `None` on the extracted key (R2.1's group-level config-time rejection handles
  the qualifier-disambiguation case instead of a wire lookup).

## `SessionQualifier` group-uniqueness check (new internal validation in `crates/truefix/src/lib.rs`)

```text
// No new public type. New internal validation inside the existing acceptor-group resolution
// (Engine::start's `is_grouped`/`acceptor_groups` logic, crates/truefix/src/lib.rs ~283-300).

fn check_qualifier_disambiguation(members: &[ResolvedSession]) -> Result<(), ConfigError> {
    // For every pair of members in the same acceptor group whose SessionId::new_full key
    // (ignoring session_qualifier, which has no wire representation) would collide,
    // require them to differ by a distinct discriminator already available to this group
    // resolution pass (e.g. distinct AcceptorTemplate assignment, or — since group members
    // share one SocketAcceptPort by construction — reject outright, since no distinct listener
    // exists within a group to serve as the discriminator).
}
```

- New (private, internal) validation function; no new `ConfigError` variant strictly required if an
  existing "ambiguous session configuration" variant fits (reuse `ConfigError::AmbiguousAcceptorTemplate`'s
  shape/pattern, or add a sibling variant — implementation-time choice, additive either way).

## `FileStore`/`CachedFileStore` — `save_and_advance_sender` override (extends `truefix-store::file`)

```text
// Trait method signature UNCHANGED (already exists on MessageStore since feature 005):
async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError>;

// NEW: FileStore and CachedFileStore each gain their own impl (today: falls through to the
// trait's default two-call implementation). New impl body performs body.append(seq, message)
// then seq.set_sender(seq + 1) as one ordered sequence, matching the SQL/MSSQL/Redb backends'
// existing atomicity discipline (established feature 005).
```

- No new type, no signature change — only two backends gain a method **override** where none
  existed. `BodyLog::reset()` (internal, not public) gains a conditional `sync_data()` call.
  `FileStore::reset()`/`CachedFileStore::reset()` (both implement the existing `MessageStore::reset`)
  reorder their two internal calls (`seq.reset()` before `body.reset()`, reversed from today) — same
  public signature, internal-ordering-only change.

## `MssqlStore::parse_url` — dual URL grammar (extends `truefix-store::mssql`, the one disclosed public-surface growth)

```text
// Signature unchanged: fn parse_url(url: &str) -> Result<Config, StoreError>

// Behavior: tries the existing user:password@host[:port]/database form first (unchanged).
// NEW: if that form doesn't match (no '@'), and the remainder contains ';', parses the real
// QuickFIX/J semicolon-delimited grammar instead:
//   host[:port][;databaseName=X;user=Y;password=Z;...]
// into the same tiberius::Config fields the existing branch already populates.
```

- No new public type. `is_jdbc_sql_scheme` (`crates/truefix-config/src/builder.rs`) loses its
  `jdbc:h2:` arm (deletion, not addition — per the H2-scope clarification).
- `splice_credentials` gains inline percent-encoding of `user`/`password` before splicing — no
  signature change, internal-only.

## `MssqlStore` identifier validation (extends `truefix-store::mssql`)

```text
// Ported, not new: crates/truefix-store/src/sql.rs's existing `valid_identifier(&str) -> bool`
// function is reused (moved to a shared location or duplicated with the same logic — implementation
// choice) and called in MssqlStore::connect_with_config before any {sessions}/{messages} interpolation.
```

- No new type. Failure path already exists in shape (`StoreError::Backend("... is not a valid SQL
  table identifier")`) — `MssqlStore` gains the same validation call its siblings already have.

## `Session` per-connection state — reset scope correction (`truefix-session::state::Session`)

```text
// No new fields on the public Session type's external shape (all fields below already exist
// internally). Behavior changes only:

fn on_connected(&mut self) -> Vec<Action> {
    self.ticks_awaiting = 0;              // existing
    self.ticks_since_recv = 0;            // NEW reset here (was previously never reset on reconnect)
    self.test_request_outstanding = false; // NEW reset here
    self.resend_target = None;            // NEW reset here
    self.resend_chunk_end = None;         // NEW reset here
    self.connection_reset_done = false;   // NEW internal field (see below) — replaces overloaded
                                           // use of `logon_sent` as a reset-tracking signal
    ...
}
```

- **One new internal (non-public) field**: `connection_reset_done: bool` on `Session`'s internal
  state, reset to `false` in `on_connected`, set to `true` the first time a `ResetOnLogon`-triggered
  full reset is performed for the current connection. Replaces the `let full = !self.logon_sent`
  heuristic (`B1`'s root cause) with an explicit signal that isn't coupled to send-order between
  local and remote Logons. This is the one new field this feature introduces anywhere in
  `truefix-session` — internal only, no public API change (mirrors the plan's Technical Context
  disclosure: only `MssqlStore::parse_url`'s grammar is a disclosed *public* surface change; this
  field is private state).

## `FieldType::value_ok` — two new match arms (`truefix-dict::model`)

```text
Self::UtcTimeOnly => field.as_utc_time_only().is_ok(),
Self::UtcDate => field.as_utc_date_only().is_ok(),
```

- No new type — two arms added to an existing exhaustive-by-catch-all `match`, using already-present
  `Field` parser methods.

## `data_field_for_length` — four new match arms (`truefix-core::tags`)

```text
618 => 619,   // EncodedLeg... pair (exact field names confirmed against .fixdict at impl time)
620 => 621,
445 => 446,
1039 => 1040,
```

- No new type — four arms added to the existing 13-pair table.

## `tags::is_header`/`is_trailer` — four new tag classifications (`truefix-core::tags`)

```text
// NoHops = 504, HopCompID = 628, HopSendingTime = 629, HopRefID = 630
// added to whichever of is_header/is_trailer's existing tag sets correctly classifies
// the standard FIX NoHops repeating group (header-adjacent per FIX spec).
```

- No new type — extension of an existing tag-classification function's recognized set.

## Production encode/decode path — dictionary-driven wiring (`truefix-core`, `truefix-transport`, `truefix-dict`)

```text
// codegen.rs's generated encode() body changes from:
pub fn encode(&self) -> Vec<u8> { self.0.encode() }
// to (for messages with a declared field_order):
pub fn encode(&self) -> Vec<u8> { self.0.encode_with_order(&FIELD_ORDER) }
// (FIELD_ORDER: a codegen-emitted const array, only for messages that declare one)

// truefix-transport's decode call sites switch from decode(bytes) to
// decode_with_groups(bytes, &*dictionary) where a DataDictionary (already implementing GroupSpec)
// is available.
```

- No new public type — `encode_with_order`/`decode_with_groups`/`GroupSpec` all already exist
  (feature 005); this wiring makes already-implemented, already-correct primitives reachable from the
  real production path for the first time.

## `Engine::start` — cleanup-before-return (no type change)

```text
// Engine::start's Result<Self, EngineError> signature is UNCHANGED.
// Internal-only: on each `return Err(e)` path (two call sites), abort/stop everything already
// pushed into the local acceptors/initiators/failover_initiators Vecs before returning, via a
// shared private helper also used by Engine::shutdown().
```

- No new type, no new public method. `continue_on_error` for grouped acceptors changes from
  `members.first().is_some_and(...)` to `members.iter().all(...)` (strictest-wins) — same `bool`
  computation, different formula.

## `DecodeError` — one new variant (`truefix-core`)

```text
pub enum DecodeError {
    // ...existing variants unchanged...
    BodyLengthTooLarge { declared: usize, max: usize },  // NEW (US6/FR-024)
}
```

- One new additive enum variant on an already-`#[non_exhaustive]`-or-matched-with-wildcard error type
  (confirm exhaustiveness discipline at implementation time — if `DecodeError` is matched
  exhaustively anywhere outside this crate, this is technically a breaking addition and needs the
  same additive-enum handling 005 used for its own error-enum growth).

## `ConfigError`/`TlsError` — new variants for hardening (US6)

```text
// ConfigError gains (exact name TBD at implementation time):
UnrecognizedCipherSuite { names: Vec<String> },   // GAP-53/FR-026

// Possibly a new AmbiguousSessionQualifier variant (or reuse AmbiguousAcceptorTemplate's shape) for
// the SessionQualifier group-uniqueness check above.
```

- Additive enum growth only, consistent with 005's established precedent for error-enum growth in
  this workspace.
