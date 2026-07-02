# Phase 0 Research: Close Remaining QuickFIX/J Parity Gaps

**Feature**: [spec.md](./spec.md) | **Date**: 2026-07-01

This research grounds each Technical Context decision in the actual codebase (verified via direct file
inspection, not assumption) and resolves the technology choices the spec deliberately left open
(PROXY/SOCKS/HTTP-CONNECT crate, MSSQL/Oracle drivers, FIX Orchestra parsing).

## 1. Dictionary grammar extension for `component` (FR-009)

- **Current state**: `crates/truefix-dict/src/parser.rs` supports `version`, `field`, `header`,
  `trailer`, `message`, and `group` directives only (parser.rs:52-157). `group` syntax is
  `group <count_tag> <name> <delimiter> <members_list>` (e.g. `group 453 NoPartyIDs 448 448,447,452`).
  `GroupDef` (model.rs:259-266) holds `count_tag`/`delimiter`/`members`. No XML parsing dependency exists
  in `truefix-dict`.
- **Decision**: Add a `component <Name> <members_list>` directive, parsed the same way as `group` minus
  the count-tag/delimiter (a component has no repeating envelope of its own — a component that *contains*
  a repeating structure references a `group` member by name). Add `ComponentDef { name, members: Vec<ComponentMember> }`
  to `model.rs` where `ComponentMember` is `Field(tag)` or `Group(name)` or `Component(name)` (nested).
  `message`/`group` member lists gain a `component:<Name>` token alongside plain tag numbers, expanded
  at dictionary-load time (not decode time) into the flat member list the existing decoder already
  understands — this keeps `decode.rs`/`validate.rs` changes minimal (expansion happens once, at
  `DataDictionary` construction).
- **Rationale**: Expanding components at load-time (vs. decode-time) avoids touching the hot decode
  path at all — `Message`/group decode logic in `truefix-core` sees only the fully-expanded member list
  it already handles, preserving Principle I's no-panic/no-new-hot-path-branching posture.
- **Alternatives considered**: Expand components at decode-time (rejected — adds a lookup on every
  decode call for no behavioral benefit) — decode-time expansion is only justified if components were
  ever dynamically swapped per-message, which FIX does not do.

## 2. `DataDictionary::load_from_file` / `extend` (FR-010)

- **Current state**: `DataDictionary::parse(&str)` is the only public entry point (per audit).
- **Decision**: `load_from_file(path)` is a thin wrapper (`std::fs::read_to_string` + `parse`) returning
  a typed `DictLoadError` (path + underlying parse error). `extend(&mut self, other: &DataDictionary)`
  merges `fields`/`messages`/`groups`/`components` maps; a tag/name already present with a **different**
  definition is a typed `DictMergeConflict` error (tag, both definitions); an identical redefinition is a
  no-op merge (idempotent).

## 3. Field type completeness — Data/UtcDateOnly/UtcTimeOnly (FR-011)

- **Current state**: `truefix-core::Field` has 6 constructor/accessor pairs (String/Int/Decimal/Char/
  Bool/UTCTimestamp); `FieldType` enum already declares 17 variants (per audit) with no runtime
  conversions for most.
- **Decision**: Add `Field::bytes(&[u8]) -> Field` / `as_bytes(&self) -> Option<&[u8]>` (raw byte storage,
  no UTF-8 assumption — required because Data fields, e.g. tag 96 `RawData`, may contain arbitrary bytes
  including embedded SOH); `Field::utc_date_only(Date) -> Field` / `as_utc_date_only(&self) ->
  Option<Date>` formatting `YYYYMMDD` via the existing `time` crate (already a workspace dependency —
  no new dependency); `Field::utc_time_only(Time)` / `as_utc_time_only()` formatting `HH:MM:SS[.sss...]`
  the same way the existing `UTCTimestamp` formatter does, minus the date part.
- **No new dependency required** — `time` crate formatting primitives cover both.

## 4. FIX Latest dictionary source (FR-012)

- **Current state**: 9 normalized `.fixdict` files exist (FIX40/41/42/43/44/50/50SP1/50SP2/FIXT11,
  573–2194 bytes each, `crates/truefix-dict/dict-src/normalized/`), each apparently hand-normalized from
  the public FIX specification (Principle III provenance). No FIX Orchestra (XML) parsing exists yet.
- **Decision**: FIX Orchestra repositories are published as XML by FPL under an open specification
  license compatible with deriving normalized data (Principle III requires verifying this at
  implementation time — **flagged as a Phase 0 license-audit task**, not resolved here). Parse Orchestra
  XML with `quick-xml` (MIT/Apache-2.0 dual-licensed, no transitive copyleft) — a **new** dependency,
  added behind a `dict-tooling`/build-time-only feature so it does not bloat the runtime engine's
  dependency graph. The conversion emits the same normalized `.fixdict` grammar every other version
  uses (extended with `component` per §1), so `load_fixlatest()` and `crack_fixlatest` reuse 100% of the
  existing loader/codegen pipeline — no FIX-Latest-specific runtime code path.
- **Alternatives considered**: Hand-transcribing FIX Latest fields/messages the way the other 9 versions
  were (rejected — FIX Latest's message/component catalogue is large enough that hand transcription risks
  transcription error and defeats the purpose of Orchestra's machine-readable source; Orchestra parsing
  is a one-time build-time tool, not a runtime dependency).

## 5. Session-owned durable resend (FR-003/FR-004)

- **Current state** (`crates/truefix-session/src/state.rs`): `store: BTreeMap<u64, Message>` (line 90,
  sent messages for resend) and `queue: BTreeMap<u64, Message>` (line 92, out-of-order inbound) are
  in-memory only. `seed_sequences()` (line 153) and `seed_sent_messages()` (line 163) let a caller
  (today, the transport layer) push store-recovered state in at startup, but nothing re-reads the store
  during `build_resend()` (line 569) or `reset()` (line 234) — both operate on the in-memory `BTreeMap`
  only. `MessageStore` trait (`crates/truefix-store/src/lib.rs:50-65`) already has the 7 async methods
  needed: `next_sender_seq`/`next_target_seq`/`set_next_sender_seq`/`set_next_target_seq`/`save`/`get`/
  `reset`.
- **Decision**: Add `Session::with_store(config, store: Arc<dyn MessageStore>)` alongside the existing
  constructor (store-less sessions keep today's in-memory-only behavior — no regression). When a store
  is attached, `build_resend()` calls `store.get(begin, end)` instead of reading `self.store`, and
  `reset()` calls `store.reset()` in addition to clearing in-memory state. The in-memory `BTreeMap`
  remains as a write-through cache (writes still go to both, matching how `seed_sent_messages` already
  primes it) so a store-less path and a store-backed path share the same read/write call sites, differing
  only in whether the store calls are no-ops.
- **Why this satisfies SC-002 (crash, not just reconnect)**: today `seed_sent_messages` only runs once at
  transport startup; if `Session` itself never re-reads the store, a `Session` object rebuilt from a
  fresh process (crash, not graceful shutdown) that raced past its own seed step, or whose seed step is
  skipped/partial, has no way to recover. Reading from the store directly inside `build_resend()` removes
  that single point of failure.

## 6. Extended application hooks (FR-013)

- **Current state** (`crates/truefix-session/src/application.rs:17-58`): `Application` trait methods are
  all `&self` (not `&mut self`), all `async`: `on_create`, `on_logon`, `on_logout` (no-op defaults),
  `to_admin`, `from_admin(&Message, &SessionId) -> Result<(), Reject>` (logon rejection today happens
  here), `to_app`, `from_app(...) -> Result<(), BusinessReject>`.
- **Decision**: Reuse `from_admin`'s existing `Reject` return path for the "extended logon predicate" —
  it is already the mechanism that can refuse a logon (US10 doesn't need a new callback shape, just a
  documented convention that `from_admin` is where arbitrary refusal logic runs for Logon messages, plus
  ensuring `Reject`'s construction can carry a `SessionStatus` (tag 573) value through to the outbound
  Logout). Add one new trait method `on_before_reset(&self, &SessionId)` (no-op default, mirroring
  `on_logon`/`on_logout`'s shape) invoked at the top of `reset()` before state is cleared. **Phase 0
  follow-up**: confirm whether `Reject`'s struct (wherever it's defined post-002) already has a field for
  an arbitrary FIX tag/value pair it could carry `SessionStatus` in, or whether a `session_status:
  Option<u16>` field must be added — this is an implementation-detail check for `/speckit-tasks`, not a
  design fork (either way the public shape is additive, not breaking).

## 7. Inbound bounded channel + admin/application separation (FR-019)

- **Current state** (`crates/truefix-transport/src/lib.rs` / `framing.rs`): there is **no existing
  inbound message channel** — inbound bytes are read directly off the `TcpStream` inside a single
  `tokio::select!` loop (lib.rs:530, `read = stream.read(&mut chunk)`), framed via `frame_length()`
  (framing.rs:13-43), and drained/dispatched inline by `drain_messages()` (lib.rs:606) in the same loop
  iteration — admin and application messages are processed inline together, with no queue in between.
  (A *separate*, unrelated bounded `mpsc::channel(8)` already exists for **outbound** control commands —
  Logout/Reset/Send — at lib.rs:424; this is not the inbound path.)
- **Decision**: This is a genuine architectural addition, not a config toggle on an existing channel.
  Split the single read-and-process loop into two halves connected by channels: (a) the socket-reading/
  framing half stays as today (reads + `frame_length` + full decode enough to classify admin vs.
  application by MsgType), then routes the decoded message onto (b) one of two `tokio::sync::mpsc`
  channels — an **unbounded admin channel** (heartbeat/TestRequest/ResendRequest/Logon/Logout/Reject/
  SequenceReset) and a **bounded application channel** sized by `in_chan_capacity` (default: unbounded,
  preserving today's behavior when the config key is unset). The session processing loop drains the
  admin channel with priority (`tokio::select!` with `biased;` ordering, admin arm first) before
  considering the application channel, guaranteeing admin traffic is never starved by a full application
  channel. When `in_chan_capacity` is `Some(n)` and full, the socket-reading half's `send().await` on the
  bounded channel naturally blocks — which in turn stops draining the socket, so backpressure ultimately
  propagates to the OS TCP receive buffer and then to the peer's send window, exactly matching
  QuickFIX/Go's `InChanCapacity` behavior.
- **Alternatives considered**: A single channel with priority-tagged messages (rejected — a full single
  channel still blocks admin-message delivery behind queued application messages, the exact starvation
  case FR-019 exists to prevent).

## 8. sqlx MSSQL — correcting the audit's assumption (FR-020)

- **Current state**: `crates/truefix-store/Cargo.toml:14` enables `sqlite`, `postgres`, `mysql-rsa`
  sqlx features. **sqlx has never shipped an MSSQL driver** (confirmed: no `mssql` feature exists in the
  sqlx 0.9 line the workspace pins); `docs/todo-gap-analysis.md`'s TODO-14 phrasing ("`sqlx` `mssql`
  feature") is **inaccurate** and is corrected here.
- **Decision**: Implement MSSQL support against **`tiberius`** (pure-Rust, MIT-licensed, actively
  maintained by Prisma Labs) directly, alongside — not through — `sqlx`, behind a separate `mssql`
  Cargo feature on `truefix-store`/`truefix-log`. `SqlStore`/`SqlLog` already dispatch by connection-URL
  scheme (sql.rs:302-327: `postgres://`, `mysql://`, else SQLite); this decision adds a `mssql://` (or
  `sqlserver://`) branch calling into a `tiberius`-backed implementation of the same internal trait the
  sqlx-backed branches already implement, so the public `SqlStore`/`SqlLog` surface is unchanged.
  FR-020's spec wording ("via sqlx or an equivalent driver") already anticipated this.

## 9. Oracle — driver choice deferred (per Clarifications)

- Per the spec's Clarifications, this specification intentionally does **not** prescribe an Oracle
  driver. Candidate researched: the `oracle` crate (dual MIT/Apache-2.0 Rust bindings) dynamically links
  the **closed-source, separately-licensed** Oracle Instant Client at runtime — the client library is
  operator-supplied and not bundled/redistributed by TrueFix, which is the same model ODBC/JDBC drivers
  use and is generally compatible with Principle III's "no copyleft contamination of TrueFix's own
  source" test (the proprietary component never enters TrueFix's repository or binary distribution).
  **Phase 0 follow-up task for `/speckit-tasks`**: perform the actual license/redistribution review
  before implementation; if it fails, FR-020's Oracle clause downgrades to a documented, deferred
  interface (no code), per the spec's explicit allowance.

## 10. PROXY protocol, SOCKS4/5, HTTP CONNECT (FR-015/FR-016)

- **Current state**: no proxy-related code exists in `truefix-transport` today; `CipherSuites` is
  registered as `Recognized` but unimplemented (`crates/truefix-config/src/keys.rs:142`).
- **Decisions** (new dependencies, each MIT/Apache-2.0 dual or MIT — Phase 0 license-audit task per
  Principle III before first use, consistent with how 002 treated new dependencies):
  - **PROXY protocol (v1/v2) parsing**: `ppp` crate (pure-Rust PROXY-protocol parser). Applied only after
    the trusted-upstream source-IP check (per Clarifications) — the acceptor's existing IP-allow-list
    check runs against the **physical** peer address first; only when that physical address is in a
    separately-configured trusted-upstream set does the engine parse the PROXY header and substitute the
    declared original client IP for subsequent allow-list/logging purposes.
  - **SOCKS4/SOCKS5 (+ optional user/pass auth)**: `tokio-socks` crate (async, tokio-native, MIT/Apache).
  - **HTTP CONNECT**: hand-rolled (the method is a single request line + header block + status-line
    response — implementing it directly avoids pulling in a full HTTP client crate/dependency just for a
    CONNECT handshake).
  - **Cipher suites / inline PEM**: no new dependency — `rustls`'s `ServerConfig`/`ClientConfig` builders
    already accept a custom `CryptoProvider`/cipher-suite list, and `rustls-pki-types::pem` (already the
    workspace's PEM-parsing dependency per the recent RUSTSEC-2025-0134 migration) parses PEM from an
    in-memory byte slice exactly as it does from a file today — inline-bytes support is a config-surface
    change, not a new parsing dependency.

## 11. AT scenario coverage & version matrix (FR-001/FR-002)

- **Current state** (`crates/truefix-at/src/scenarios.rs`, `runner.rs`): scenarios are individual
  functions returning a `Scenario` struct, aggregated in `server_suite()` (~40+ entries today, closer to
  the audit's "~22/73 counted as distinct case instances" once version-multiplication is factored out).
  `SUITE_VERSIONS = ["FIX.4.2", "FIX.4.4"]`; dictionaries wired via `truefix_dict::load_fix42()`/
  `load_fix44()`. Since 8 of the 9 bundled normalized dictionaries are already non-fixLatest application
  versions (§4) — fix40/41/42/43/44/50/50SP1/50SP2 — broadening `SUITE_VERSIONS` to the remaining six
  (fix40/41/43/50/50SP1/50SP2) is primarily a **wiring** change (add `load_fix40()` etc. to
  `runner.rs`, confirm each already-existing loader function's presence in `truefix-dict::lib.rs`) plus
  **authoring the ~51 net-new scenario functions** the audit lists, not new dictionary infrastructure.
  Only the `fixLatest` slice of the version matrix is gated on §4's new work.
- **Decision**: Author new scenarios grouped by their dependency: (a) scenarios needing only
  already-existing engine behavior (the majority — integrity/CompID/sequence/PossDup/reject-layer
  classes) ship early and run against the existing FIX.4.2/4.4 wiring plus newly-wired fix40/41/43/50/
  50SP1/50SP2; (b) scenarios needing `ValidateFieldsOutOfOrder` (14g/15/2t) ship once that toggle (US3) lands;
  (c) `fixLatest`-targeted scenarios ship once §4 lands. This ordering is reflected in the staged
  delivery plan (AT coverage is split across an early stage and a closeout stage in `plan.md`, not one
  monolithic stage) rather than blocking all AT work on every other feature.

## Summary of new dependencies (Phase 0 license-audit list)

| Crate | Purpose | License (as published) | Audit status |
|-------|---------|------------------------|---------------|
| `quick-xml` | FIX Orchestra XML parsing (FIX Latest, build/tooling-time only) | MIT/Apache-2.0 dual | Verify at task-start |
| `ppp` | PROXY protocol v1/v2 parsing | MIT/Apache-2.0 dual (verify) | Verify at task-start |
| `tokio-socks` | SOCKS4/5 proxy client | MIT/Apache-2.0 dual | Verify at task-start |
| `tiberius` | MSSQL driver (pure Rust) | MIT | Verify at task-start |
| `oracle` (tentative, may be deferred) | Oracle driver bindings | MIT/Apache-2.0 dual, but links proprietary Instant Client at runtime | Full legal review at task-start; may downgrade to deferred per spec |

No dependency above is assumed pre-approved; each MUST pass the same license-compatibility check
Principle III requires before its first line of dependent code is written.
