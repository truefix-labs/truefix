# Phase 0 Research: Third-Pass Audit Remediation (docs/todo/005.md)

**Feature**: [spec.md](./spec.md)

`docs/todo/005.md` already carries four successive independent full-codebase passes, each
cross-referencing QuickFIX/J/Go and re-verifying (refuting/downgrading/folding) findings from the
passes before it. This research pass does not re-derive that internal rigor from scratch. Following
this project's established "verify before implement" practice (005/006/007's own plan.md
precedent), it **directly re-reads current source** for every User Story 1 (highest-priority) item
— all 21 of them — plus targeted spot-checks of a few User Story 2/3 items with the most
implementation-relevant ambiguity. Every citation below was read fresh during this pass (file paths
and line numbers reflect what was actually opened, not copied from `005.md`'s prose). All 21 User
Story 1 items confirmed exactly as the audit document's own (already self-corrected) final framing
describes — no further correction was needed beyond what `005.md`'s four internal passes already
made. The remaining ~62 User Story 2/3 items rely on `005.md`'s own multi-pass verification as
sufficient evidentiary basis, proportional to their priority (matching 007's Phase 0 precedent for
its own P2/P3 tier).

Organized R1-R3, one section per spec.md user story, in the acceptance-scenario order of spec.md.

---

## R1 — Protocol-correctness, data-loss, and security defects (US1)

### R1.1 — `NEW-54`: `MongoStore::set_seq` writes to literal key `"field"` (FR-001)

**Read**: `crates/truefix-store/src/mongo.rs:139-148` (`set_seq`), `:201-213` (`reset`, for contrast).

```rust
async fn set_seq(&self, sender: bool, seq: u64) -> Result<(), StoreError> {
    let filter = doc! { "session_id": &self.session_id };
    let field = if sender { "sender" } else { "target" };
    let update = doc! { "$set": { field: seq as i64 } };
    ...
```

**Confirmed**: the `bson::doc!` macro's bare-identifier key syntax (`field: seq`) stringifies the
identifier `field` itself to the literal key `"field"` — it does not evaluate the `field` variable.
`reset()` in the same file (line ~208) uses correct string literals (`"sender"`, `"target"`) for
comparison, proving the macro behaves as expected elsewhere and this call site is the outlier.
Every `set_next_sender_seq`/`set_next_target_seq` call writes to a document key literally named
`"field"`, leaving the real `sender`/`target` fields stuck at their `ensure_session_row`-seeded
initial value of 1 forever.

**Decision**: use the arrow form `doc! { "$set": { field.to_string(): seq as i64 } }` (or build the
update document programmatically via `Document::insert(field, seq as i64)`) so the actual
`sender`/`target` string is used as the key. Per Clarifications (Session 2026-07-04): forward-fix
only — no migration of pre-existing rows carrying the stray `"field"` key is attempted; the stray
key is simply never read going forward and its presence must not crash a fresh read/write.

### R1.2 — `NEW-02`: `BeginSeqNo=0` in `ResendRequest` silently dropped (FR-011)

**Read**: `crates/truefix-session/src/state.rs:1063-1094` (`on_resend_request`'s range computation).

```rust
let begin = msg.body.get(crate::tags::BEGIN_SEQ_NO)
    .and_then(|f| f.as_int().ok())
    .filter(|&s| s > 0)
    .map_or(0, |s| s as u64);
...
if begin == 0 || begin > end { return Vec::new(); }
```

**Confirmed** exactly: `BeginSeqNo=0` is filtered to `None` then mapped back to `0` by `map_or`,
and the subsequent `begin == 0` guard silently drops the entire request — no reject, no resend, no
log signal distinguishing this from a malformed request.

**Decision**: treat `BeginSeqNo=0` as `1` (FIX semantics: "from the beginning") before the
`begin == 0 || begin > end` guard, e.g. change the `.map_or(0, |s| s as u64)` default to `1`, or
special-case `0 → 1` right after parsing. `EndSeqNo=0` (already meaning "to infinity", handled via
the existing `end_req == 0` branch just above) is unaffected.

### R1.3 — `NEW-03`: acceptor-side `ResetOnLogon` never consulted (FR-012)

**Read**: `crates/truefix-session/src/state.rs:623-687` (`on_connected`), `:1210-1456` (`on_logon`),
and `grep -n reset_on_logon` across the file (10 matches, all in `on_connected`'s `Initiator` arm or
`on_logon`'s echo-verification logic — none in the `Acceptor` arm of either function).

- `on_connected`'s `Role::Acceptor => Vec::new()` (line 685) never touches sequence state at all.
- `on_logon` computes `inbound_reset_flag` (line 1387-1391, the *peer's own* `ResetSeqNumFlag`) and
  resets on that (line 1392-1397) — for **both** roles equally, since this runs before the
  `match self.config.role` block. But `self.config.reset_on_logon` (the acceptor's own local
  configuration) is never read anywhere in `on_logon`.

**Confirmed** exactly: an acceptor configured with `ResetOnLogon=Y`, receiving a Logon that does
**not** itself carry `ResetSeqNumFlag=Y`, performs no reset at all — the local config setting is
completely inert for the acceptor role, exactly as `NEW-03` describes (distinct from `BUG-28`'s
protocol-level echo/verify handshake, which is about the flag TrueFix itself sends/checks, not this
local-config-driven proactive reset).

**Decision**: in `on_logon`, add `|| self.config.reset_on_logon` (acceptor-only, since the
initiator's own `ResetOnLogon` is already handled by `on_connected`'s pre-Logon-send reset) to the
condition that decides `store_reset`/`reset_sequences`, gated on `Role::Acceptor`.

### R1.4 — `NEW-93`: multi-session acceptor pre-Logon read has no timeout/cap (FR-044)

**Read**: `crates/truefix-transport/src/lib.rs:1763-1798` (`route_and_run`'s pre-Logon read loop).

```rust
let mut buf: Vec<u8> = Vec::new();
let mut chunk = [0u8; 4096];
let logon = loop {
    match stream.read(&mut chunk).await { Ok(0) | Err(_) => return, Ok(n) => { buf.extend_from_slice(...) } }
    match frame_length(&buf) { Ok(Some(total)) => ..., Ok(None) => continue, Err(_) => return }
};
```

**Confirmed** exactly: no `tokio::time::timeout` wraps this loop, and `buf` has no size cap —
`frame_length` (per `crates/truefix-core/src/framing.rs`) returns `Ok(None)` whenever no SOH byte
is present yet, so a peer sending nothing, or sending bytes with no SOH, makes this loop `continue`
indefinitely. `Session`/`logon_timeout` do not exist yet at this point in the connection lifecycle
(confirmed by reading `run_connection`, which constructs the `Session` only after this loop already
returned a decoded Logon) — this is a genuine, real gap distinct from the single-session `Acceptor`
path, which has a live `logon_timeout` ticker from the instant the connection is accepted.

**Decision**: per Clarifications (Session 2026-07-04), wrap this loop's read in
`tokio::time::timeout(Duration::from_secs(config.logon_timeout as u64), ...)` — reusing the
existing `logon_timeout` session-config value already used elsewhere for the analogous
single-session case, no new config key — and cap `buf`'s size defensively (reject/disconnect once
it exceeds a bound, e.g. `MAX_BODY_LEN`, regardless of whether a SOH has appeared).

### R1.5 — `NEW-84`: gap-fill `SequenceReset` bypasses too-high/too-low verification (FR-013)

**Read**: `crates/truefix-session/src/state.rs:744-749` (`on_received`'s early `MsgType=4` dispatch),
`:1139-1208` (`on_sequence_reset`, gap-fill branch at `:1195-1207`).

```rust
if mt.as_deref() == Some("A") { return self.on_logon(msg); }
if mt.as_deref() == Some("4") { return self.on_sequence_reset(&msg); }
// ... only afterward does the Ordering::Equal/Greater/Less dispatch (line 786+) run
```

and the gap-fill branch:

```rust
if let Some(ns) = new_seq {
    if ns >= self.next_in_seq {
        self.next_in_seq = ns;
        self.queue.retain(|&seq, _| seq >= ns);
    }
}
self.drain_queue()
```

**Confirmed** exactly: `on_received` routes `MsgType=4` directly to `on_sequence_reset` *before* the
normal `Ordering::Greater`/too-high dispatch ever runs, and the gap-fill branch applies `NewSeqNo`
unconditionally whenever it's `>= next_in_seq` — there is no path where a gap-fill `SequenceReset`
whose own `MsgSeqNum` is itself too high gets queued/re-verified. Combined with the (correct, from
`BUG-96`) `queue.retain(...)` line, a spurious/reordered gap-fill message can discard legitimately
queued messages permanently.

**Decision**: before applying the gap-fill jump, check the message's *own* `MsgSeqNum` against
`next_in_seq` the same way the generic dispatch does for every other message type — if the
gap-fill's own sequence is ahead of expectation, queue it (reusing the existing `Ordering::Greater`
queuing/`ResendRequest` path) instead of applying `NewSeqNo` immediately. This requires either
routing `SequenceReset` through the same `seq.cmp(&self.next_in_seq)` dispatch other messages use
(with gap-fill's `NewSeqNo`-jump semantics applied only in the `Equal` case), or adding an
equivalent too-high check inside `on_sequence_reset` itself before its existing logic runs.

### R1.6 — `NEW-55`: `tags::is_header` missing FIXT 1.1 transport header tags (FR-028)

**Read**: `crates/truefix-core/src/tags.rs:27-67` (`is_header`).

**Confirmed**: the `matches!` tag list includes `627`/`628`/`629`/`630` (`NoHops`, added by feature
006's `GAP-26`/`FR-032`) but has no `1128` (`ApplVerID`), `1129` (`ApplReportID`), `1130`
(`LastApplVerID`), `1156` (`ApplExtID`), or `1351`-`1355` (`NoApplIDs` group). A FIX 5.x message's
`ApplVerID(1128)` therefore decodes into `message.body` instead of `message.header`.

**Decision**: add `1128 | 1129 | 1130 | 1156 | 1351 | 1352 | 1353 | 1354 | 1355` to the `matches!`
list, mirroring the same verified-against-shipped-dictionary approach feature 006 used for `NoHops`
(confirm against `FIXT11.fixdict`'s own `header` declaration before finalizing the exact tag set, to
avoid repeating the wrong-tag citation feature 006's own research caught and corrected for `NoHops`).

### R1.7 — `NEW-58`: `classify_buffered` ignores `fixt_dictionaries` (FR-029)

**Read**: `crates/truefix-transport/src/lib.rs:1100-1127` (`classify_buffered`), `:198-215`
(`Services` struct, confirming a `fixt_dictionaries` field exists), `:745-746` (`run_connection`
setting `session.set_fixt_dictionaries` from `services.fixt_dictionaries`).

```rust
let decoded = match &services.validator {
    Some((dict, _)) => decode_with_groups(&raw, &HeaderTrailerGroupsOnly(dict)),
    None => decode(&raw),
};
```

**Confirmed** exactly: `classify_buffered` only branches on `services.validator` (the plain
single-dictionary case); `services.fixt_dictionaries` (the dual-dictionary FIXT 1.1 case, genuinely
present and consumed elsewhere in this same file) is never consulted here, so a FIXT 1.1 session
with only `fixt_dictionaries` set (and `validator: None`) falls to plain `decode` — header/trailer
groups are never structured for it.

**Decision**: add a `fixt_dictionaries` arm alongside `validator`'s — when set (and `validator` is
`None`), call `decode_with_groups(&raw, &HeaderTrailerGroupsOnly(&dicts.transport()))` using the
FIXT dual-dictionary's transport half, matching what `GAP-26`/`FR-032` already does for the plain
case.

### R1.8 — `NEW-56`: `enter_disconnected` conflates `reset_on_logout`/`reset_on_disconnect` (FR-014)

**Read**: `crates/truefix-session/src/state.rs:1544-1553` (`enter_disconnected`), and its callers:
`:470` (`Event::Disconnected` in `handle()`), `on_tick`'s two schedule/heartbeat-timeout branches
(`:1565`, `:1580`, `:1595`), plus a scan of every other `self.state = SessionState::Disconnected`
assignment in the file to confirm which paths route through this function vs. assign directly.

```rust
fn enter_disconnected(&mut self) -> Option<Action> {
    self.state = SessionState::Disconnected;
    if self.config.reset_on_disconnect || self.config.reset_on_logout {
        self.reset_sequences(true);
        ...
```

**Confirmed** exactly: the `||` means `reset_on_logout=true` alone triggers a reset on a
schedule-exit/heartbeat-timeout/TCP-drop-driven call to `enter_disconnected` (not a graceful
Logout), and `reset_on_disconnect=true` alone would trigger a reset were `enter_disconnected` ever
called for a graceful-Logout teardown. In the current tree, `enter_disconnected` is called from
`Event::Disconnected` (transport-driven, i.e. non-graceful) and from `on_tick`'s schedule/heartbeat
paths (also non-graceful) — a graceful `on_logout_msg`-driven teardown path was not found calling
`enter_disconnected` directly in this reading (several error-driven paths instead assign
`self.state = SessionState::Disconnected` directly and separately call `reset_on_error()`, which is
a distinct, correctly-scoped function this finding does not concern). The `||` conflation itself
stands exactly as described regardless of exactly which callers currently exercise the graceful
side — a `Logout`-driven call to this same function (needed to fix `NEW-56` cleanly, since it's the
one place a reason parameter is naturally threaded through) will start truly conflating the two
flags the moment it's added, if not fixed concurrently.

**Decision**: thread a `reason: DisconnectReason` (e.g. `{ Logout, TcpDrop, ScheduleExit,
HeartbeatTimeout, LocalReset }` — exact variant set is an implementation choice) parameter through
`enter_disconnected`, checking `reset_on_logout` only for `Logout` and `reset_on_disconnect` for
every non-graceful variant. Route the graceful-Logout teardown path (`on_logout_msg` /
`NEW-63`'s Logout-reply path) through this same function with `reason: Logout` as part of this fix,
so the two flags are correctly scoped from a single call site rather than duplicated logic.

### R1.9 — `NEW-59`: codegen `new()` doesn't stamp `BeginString`; `encode` silently emits malformed output (FR-059, cross-referenced from US1's FR list at FR-059 but exercised here since it blocks correct construction of any codegen-generated message used in this feature's own new tests)

**Read**: `crates/truefix-dict/src/codegen.rs:722-730` (generated `new()`), `crates/truefix-core/src/codec/encode.rs:23-33` (`encode_with_order`'s `BeginString`/`MsgType` extraction).

**Confirmed** exactly: generated `new()` only calls `m.header.set(Field::string(35, msg_type))` —
no `BeginString(8)` set anywhere in codegen's message-constructor template. `encode_with_order`
reads `BEGIN_STRING` via `.map(...).unwrap_or_default()`, yielding an empty `Vec<u8>` on a message
with no `BeginString` set, silently producing `8=<SOH>` on the wire.

**Decision**: per this spec's FR-059 (User Story 1 acceptance scenario 9), the actual mechanism is
a planning-level choice within two viable options — (a) have codegen's `new()` also stamp
`BeginString` from the dictionary's own declared version (available at codegen time via the same
`version_begin_string` used for `crack_*`'s guard), or (b) make `encode`/`encode_with_order` return
a `Result` and reject a message missing required header fields. Given `FIXT.1.1`'s BeginString is
shared across FIX 5.0/5.0SP1/5.0SP2 (not 1:1 with a dictionary), option (a) is the simpler, more
localized fix and is recommended for `/speckit-tasks` to adopt: it requires no public API signature
change to `encode`, and `version_begin_string` already exists in the same file as the exact source
of truth codegen needs.

### R1.10 — `NEW-62`: schedule-window exit disconnects without a `Logout` (FR-015)

**Read**: `crates/truefix-session/src/state.rs:1570-1584` (`on_tick`'s schedule-exit branch).

```rust
SessionState::LoggedOn if self.config.schedule.as_ref().is_some_and(|s| !s.is_in_session(...)) => {
    let reset = self.enter_disconnected();
    let mut actions: Vec<Action> = reset.into_iter().collect();
    actions.push(Action::Disconnect);
    return actions;
}
```

**Confirmed** exactly: only `Action::Disconnect` is pushed — no `Logout` message is built or sent
before the disconnect, an abrupt TCP-level teardown from the counterparty's perspective.

**Decision**: build and push a `Logout` action (via `admin::logout`, matching the pattern already
used in the `latency_ok`/`identity_problem`/heartbeat-timeout branches nearby) before
`Action::Disconnect` in this arm.

### R1.11 — `NEW-63`: dictionary-invalid Logon with `disconnect_on_error=false` sends bare `Reject`, strands session (FR-016)

**Read**: `crates/truefix-session/src/state.rs:1344-1366` (`on_logon`'s dictionary-validation
branch).

```rust
if let Some(reject) = self.validate_app(&msg) {
    let mut actions = vec![reject];
    ...
    if self.config.disconnect_on_error {
        ... // Logout + Disconnect
    }
    return actions;  // else: bare Reject only, state unchanged (still AwaitingLogon)
}
```

**Confirmed** exactly: when `disconnect_on_error=false`, this branch returns only the session-level
`Reject`, leaves `self.state` untouched (still `AwaitingLogon`), and sends no `Logout` — unlike
every other Logon-rejection branch in this same function (duplicate Logon, out-of-state Logon,
too-low-seq, missing-MsgSeqNum, negative-HeartBtInt, stale-NextExpectedMsgSeqNum, out-of-schedule),
all of which call `self.reject_logon(...)` (Logout + Disconnect) unconditionally regardless of
`disconnect_on_error`.

**Decision**: replace this branch's body with a call to `self.reject_logon(&Reject { ... })` (same
pattern as the other Logon-rejection branches immediately above and below it in this function),
removing the `disconnect_on_error`-conditional special case entirely so dictionary-invalid Logons
are rejected consistently with every other Logon-rejection reason.

### R1.12 — `NEW-06`: FIX 5.0/SP1/SP2 `crack_*` dispatchers ignore `ApplVerID` (FR-030)

**Read**: `crates/truefix-dict/src/codegen.rs:843-855` (`version_begin_string`), `:795-838`
(the `crack_{module}` generator template).

```rust
"FIX50" | "FIX50SP1" | "FIX50SP2" => "FIXT.1.1",
...
let _ = writeln!(code, "    if message.begin_string() != Some({:?}) {{ return false; }}", version_begin_string(name));
```

**Confirmed** exactly: the generated `crack_fix50`/`crack_fix50sp1`/`crack_fix50sp2` functions each
guard solely on `BeginString == "FIXT.1.1"` (identical for all three sub-versions) — no reference to
`ApplVerID(1128)` anywhere in the generator template. Any FIX 5.x message matches whichever of the
three `crack_*` functions happens to be called, dispatching into that version's struct types
regardless of the message's actual sub-version.

**Decision**: add an `appl_ver_id: &str` parameter to `crack_fix50`/`crack_fix50sp1`/`crack_fix50sp2`
(or read `message.header.get(APPL_VER_ID)` internally, now reachable once `NEW-55`/`FR-028` routes
tag 1128 into the header), and guard on the version-specific `ApplVerID` enum value in addition to
`BeginString == "FIXT.1.1"`.

### R1.13 — `NEW-07`: `MultipleCharValue` not validated per-token in `allows()` (FR-031)

**Read**: `crates/truefix-dict/src/model.rs:189-200` (`FieldDef::allows`).

```rust
match self.field_type {
    FieldType::MultipleValueString | FieldType::MultipleStringValue => value
        .split(' ').all(|tok| self.values.iter().any(|v| v == tok)),
    _ => self.values.iter().any(|v| v == value),  // MultipleCharValue falls here
}
```

**Confirmed** exactly: `FieldType::MultipleCharValue` is absent from the per-token match arm,
falling to the whole-string `_` arm. A valid wire value like `"A B"` (two space-separated
single-char tokens) fails `allows()` even when both `"A"` and `"B"` are individually enumerated,
inconsistent with `value_ok()`'s existing per-token handling for the same type (confirmed at
line ~131, `Self::MultipleCharValue => field...` — a separate, already-correct per-token check).

**Decision**: add `FieldType::MultipleCharValue` to the `MultipleValueString | MultipleStringValue`
match arm in `allows()`.

### R1.14 — `NEW-08`: `MongoStore` lacks a transactional `save_and_advance_sender` (FR-002)

**Read**: `grep -n "fn save_and_advance_sender"` across `truefix-store/src/{file,sql,mongo}.rs` —
matches in `file.rs` (twice, `FileStore`/`CachedFileStore`) and `sql.rs` (`SqlStore`), zero matches
in `mongo.rs`.

**Confirmed**: `MongoStore` has no override, so it falls back to the `MessageStore` trait's default
two-call implementation (`save()` then `set_next_sender_seq()` as two independent operations) —
the same crash-safety gap the trait's own doc comment warns about, already closed for
File/Cached-File/Sql/Mssql-backed stores.

**Decision**: add a `save_and_advance_sender` override to `MongoStore` using a MongoDB
multi-document transaction (`ClientSession::start_transaction`/`commit_transaction`), wrapping the
`messages.update_one` (save) and `sessions.update_one` (sender-seq advance) calls — mirroring
`SqlStore`'s existing single-transaction pattern.

### R1.15 — `NEW-09`: `MssqlLog::parse_url` missing semicolon-form and percent-decode (FR-009)

**Read**: `crates/truefix-log/src/mssql.rs:27-60` (`parse_url`), `crates/truefix-store/src/mssql.rs:60-115,150-210` (`parse_url`/`parse_semicolon_form`, for comparison).

**Confirmed** exactly: the log's `parse_url` only handles the `mssql://`/`sqlserver://` path form
(`rsplit_once('@')`, already correctly fixed per `BUG-70`/`FR-030` for the *split point*, but never
extended further) and passes `user`/`password` to `AuthMethod::sql_server` raw, with no
`percent_decode` call anywhere in the file. The store's `parse_url` has both a `parse_semicolon_form`
function (`sqlserver://host;databaseName=...` JDBC-style URLs) and `percent_decode` calls on both
credentials (lines 99-100) — genuinely absent from the log crate's copy.

**Decision**: port `percent_decode` (a private helper, ~15 lines) and `parse_semicolon_form` from
`truefix-store/src/mssql.rs` into `truefix-log/src/mssql.rs`, applying the same dispatch (path-form
vs. semicolon-form based on whether the URL after the scheme prefix contains `@`+`/` vs. `;`) the
store version already uses.

### R1.16 — `NEW-10`: 7 validation config keys unwired in `builder.rs` (FR-032)

**Read**: `crates/truefix-config/src/keys.rs:60-77` (confirms all 7 keys registered `Impl`),
`grep -n "validate_fields_have_values\|validate_unordered_group_fields\|validate_user_defined_fields\|allow_unknown_msg_fields\|first_field_in_group_is_delimiter"` across `builder.rs` — **zero matches**.

**Confirmed** exactly: all 7 keys (`ValidateFieldsHaveValues`, `ValidateUnorderedGroupFields`,
`ValidateUserDefinedFields`, `ValidateSequenceNumbers`, `AllowUnknownMsgFields`,
`RejectInvalidMessage`, `FirstFieldInGroupIsDelimiter`) are registered `Impl` in the key registry
but genuinely never read anywhere in `builder.rs`'s `resolve_validator` (confirmed at line 609) or
elsewhere in the file.

**Decision**: wire the 5 keys with a direct `ValidationOptions` field (`validate_fields_have_values`,
`validate_unordered_group_fields`, `validate_user_defined_fields`, `allow_unknown_msg_fields`,
`first_field_in_group_is_delimiter`) into `resolve_validator` via the existing `bool_key(...)`
helper pattern already used for the 5 keys that *are* wired. For `ValidateSequenceNumbers` and
`RejectInvalidMessage` (no direct `ValidationOptions` field exists), downgrade their `keys.rs`
classification from `Impl` to `Recognized` unless `/speckit-tasks` determines a real field/effect is
warranted — the registry MUST NOT continue claiming `Impl` for a key with zero wiring.

### R1.17 — `NEW-11`: TLS client with no trust store → empty `RootCertStore` → all handshakes fail (FR-049)

**Read**: `crates/truefix-transport/src/tls_config.rs:117-135` (`load_root_store`/
`load_root_store_bytes`, confirming `RootCertStore::empty()` use), `:219-221`
(`build_client_config`).

```rust
pub fn build_client_config(spec: &TlsSpec) -> Result<Arc<ClientConfig>, TlsConfigError> {
    let roots = trust_store(spec)?.unwrap_or_else(RootCertStore::empty);
```

**Confirmed** exactly: with no `SocketTrustStore`/`SocketTrustStoreBytes` configured, `trust_store`
returns `Ok(None)`, and `build_client_config` falls back to a genuinely empty `RootCertStore` —
every server certificate is rejected, TLS is unusable without an explicit trust store.

**Decision**: per Clarifications (Session 2026-07-04), add the `rustls-native-certs` dependency
(narrowly-scoped, single-purpose, MIT/Apache-2.0 dual-licensed — compatible with Principle III's
license discipline) and use it as the fallback when no explicit trust store is configured, replacing
`RootCertStore::empty` in `build_client_config`'s `unwrap_or_else`.

### R1.18/R1.19 — `NEW-12`/`NEW-13`: PROXY header peek buffer cap and slow-arrival timeout (FR-045/FR-046)

**Read**: `crates/truefix-transport/src/proxy.rs:197-247` (`PROXY_HEADER_TIMEOUT`,
`PROXY_HEADER_PEEK_BUF`, `peek_proxy_header`).

```rust
const PROXY_HEADER_TIMEOUT: Duration = Duration::from_secs(5);
const PROXY_HEADER_PEEK_BUF: usize = 4096;
async fn peek_proxy_header(...) -> SocketAddr {
    let mut buf = vec![0u8; PROXY_HEADER_PEEK_BUF];
    for _ in 0..10 {
        ...
        match parse_proxy_header(peeked) {
            Some((addr, len)) => { ...; return addr; }
            None => {
                if n >= buf.len() { return peer; }  // full peek, still no header -> give up
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }
    peer
}
```

**Confirmed both** exactly: (`NEW-12`) the peek buffer is a fixed 4096 bytes; a v2 header whose TLV
set exceeds that (spec max 64 KiB) never parses, `n >= buf.len()` fires, and the function returns
the raw peer address **without consuming any bytes** — the unread PROXY-protocol bytes remain to be
misread as FIX data. (`NEW-13`) the loop is bounded to a fixed 10 iterations × 5ms sleeps (~50ms
total), independent of and far shorter than the outer 5-second `PROXY_HEADER_TIMEOUT` — a header
arriving in slow segments over, say, 500ms causes this function to give up at ~50ms while 4.95s of
budget remains unused.

**Decision**: for `NEW-12`, grow `buf` dynamically (e.g. double on each `n >= buf.len()`, up to a
sane cap such as 64 KiB matching the PROXY v2 spec maximum) instead of giving up. For `NEW-13`,
replace the fixed `for _ in 0..10` loop with an unbounded `loop` that only exits when
`parse_proxy_header` succeeds or a byte-read error occurs — the existing outer
`tokio::time::timeout(PROXY_HEADER_TIMEOUT, ...)` wrapper (already present at the call site,
`strip_trusted_proxy_header`) becomes the sole time bound, exactly as it already is for the
`stream.peek`/`read_exact` calls within.

### R1.20 — `NEW-14`: `run_scheduled_initiator`'s connect blocks stop/schedule detection (FR-048)

**Read**: `crates/truefix-transport/src/lib.rs:2255-2349` (`run_scheduled_initiator`'s full loop
body).

```rust
while !loop_stop.load(Ordering::SeqCst) {
    ...
    if was_in_session && current.is_none() && next_retry_at.is_none_or(...) {
        match connect_initiator_with(addr, config.clone(), app.clone(), services.clone()).await {
            ...
        }
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
}
```

**Confirmed** exactly: `connect_initiator_with(...).await` is awaited inline within the same loop
iteration that checks `loop_stop` (only re-checked at the top of the *next* iteration) and the
schedule boundary (`decide_schedule_action`, also only re-evaluated next iteration). With
`config.connect_timeout: None` (the default), a hanging/black-holed connect attempt blocks this
entire loop indefinitely — confirmed by this file's own `connect_timeout_tests` module (lines
2357+), which explicitly documents "with no connect_timeout configured, the connect future should
never resolve" as expected behavior for the underlying `with_connect_timeout` primitive that
`connect_initiator_with` presumably wraps.

**Decision**: wrap the `connect_initiator_with(...).await` call in `tokio::select!` alongside a
future that resolves when `loop_stop` flips (e.g. polled via a short interval, or converted to a
`tokio::sync::Notify`/watch channel) — matching the spirit of `on_tick`'s 200ms polling cadence
already used for schedule-boundary detection elsewhere in this same function, so a hanging connect
no longer defeats stop-flag/schedule responsiveness.

### R1.21 — `NEW-17`: UDF validation skip ordered after empty-value/repeated-tag checks (FR-033)

**Read**: `crates/truefix-dict/src/validate.rs:99-154` (`validate`'s per-field loop).

**Confirmed** exactly: the repeated-tags check (`opts.check_repeated_tags`, lines 111-120) and the
empty-value check (`opts.validate_fields_have_values`, lines 129-138) both run *before* the
`is_udf && !opts.validate_user_defined_fields => continue` short-circuit (lines 142-145, inside the
`match self.field(tag) { None => ... }` arm reached only for undefined/UDF tags). An empty or
repeated UDF is rejected by the earlier checks even when `validate_user_defined_fields=false`,
contradicting the intent of fully skipping UDF validation.

**Decision**: move the UDF-tag detection (`tag >= UDF_START`) and its short-circuit `continue` to
the top of the per-field loop body (before the repeated-tags and empty-value checks), gated on
`!opts.validate_user_defined_fields`, so UDFs are fully exempted rather than partially checked —
mirroring QuickFIX/J's complete UDF skip when this option is off.

---

## R2/R3 — User Story 2/3 items (P2/P3)

Per the same proportional-effort precedent 006/007 established for their own lower-priority tiers,
the ~62 items in User Stories 2 and 3 rely on `docs/todo/005.md`'s own four-pass internal
self-verification (Pass 1-4, plus the third-pass "Corrections to prior findings" section that
already refuted/downgraded specific items before this feature's spec was written) as sufficient
evidentiary basis. Three items received a targeted spot-check during this pass because their fix
direction has more than one reasonable implementation shape (beyond what `/speckit-clarify` already
resolved for `NEW-24`/`NEW-54`/`NEW-93`/`NEW-11`):

- **`NEW-90`** (MSSQL `trust_cert()` hardcoded, no opt-out) — confirmed via
  `grep -n "trust_cert" crates/truefix-store/src/mssql.rs crates/truefix-log/src/mssql.rs`: 3 call
  sites (`store`'s path-form `parse_url`, `store`'s `parse_semicolon_form`, `log`'s `parse_url`),
  none guarded by any conditional. The audit's own suggested fix (a `TrustServerCertificate=`
  URL property or config field, defaulting to today's behavior for backward compatibility) is
  adopted as-is — no alternative shape is materially better here since it mirrors the existing
  JDBC-URL-property convention this codebase already uses for other MSSQL settings.
- **`NEW-96`** (`LogMessageWhenSessionNotFound` unwired) — confirmed via
  `grep -n "log_message_when_session_not_found"` across `truefix-config/` (zero matches) and
  `truefix-transport/src/lib.rs` (one match, the `Services` struct field itself, genuinely
  consumed at its use site). This is a pure wiring gap (add the field to `ResolvedSession`, parse
  it in `builder.rs`, thread it into `Services` construction in `truefix/src/lib.rs`) — no
  alternative shape considered necessary.
- **`NEW-73`/`NEW-89`** (`SessionConfig::new()` bare-struct defaults for `reconnect_interval`/
  `logout_timeout`) — confirmed via `crates/truefix-session/src/config.rs` and
  `crates/truefix-config/src/builder.rs::resolve_reconnect_interval` (already correctly defaults to
  30) vs. the `u32_key(map, "LogoutTimeout", &session, 10)?` call (still defaulting to the
  QFJ-diverging `10`). Both are single-constant changes; no design decision required beyond what
  `005.md`'s own fourth-pass correction already settled (`reconnect_interval`'s `.cfg`-driven path
  is unaffected; only the bare-struct default and `LogoutTimeout`'s `.cfg`-driven default need
  changing).

No other User Story 2/3 item surfaced an implementation-shape ambiguity requiring resolution before
`/speckit-tasks` — each has a single, clearly-described fix direction in `005.md` itself.

## Summary of decisions requiring disclosure

Per Constitution Principle I (public API stability) and this project's established practice of
calling out any additive public-API surface growth in the plan itself (see 007's plan.md
precedent):

1. **New dependency**: `rustls-native-certs` (or equivalent), added per Clarifications (Session
   2026-07-04) to close `NEW-11`/`FR-049`. License-compatible (Apache-2.0 OR MIT — same dual-license
   TrueFix itself uses), narrowly scoped to trust-anchor loading only.
2. **Config-key registry correction**: `ValidateSequenceNumbers` and `RejectInvalidMessage` may be
   downgraded from `Impl` to `Recognized` in `crates/truefix-config/src/keys.rs` (`NEW-10`/`FR-032`)
   if `/speckit-tasks` determines no real field/effect is warranted for either — a disclosed,
   non-breaking correction to the registry's own claims (no behavior change for any `.cfg` that
   doesn't set these keys, since they have no effect today either way).
3. **New internal enum**: a `DisconnectReason`-shaped type (exact name/variants TBD at
   `/speckit-tasks`) threaded through `enter_disconnected` to resolve `NEW-56` — internal to
   `truefix-session`, not part of the public API surface (no `pub` export required for this fix).

No other item in this feature requires a new public type, trait, or breaking signature change.
