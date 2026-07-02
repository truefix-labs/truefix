# Research: Engine Config-Wiring Completion + Additional Store Backends

**Feature**: [spec.md](./spec.md) | **Date**: 2026-07-02

All decisions below were grounded by reading the actual current code (`crates/truefix/src/lib.rs`,
`crates/truefix-config/src/builder.rs`, `crates/truefix-transport/src/lib.rs`,
`crates/truefix-dict/src/lib.rs`), not assumed from the spec or the gap-analysis doc alone — mirroring
003's research discipline, which corrected an audit inaccuracy (sqlx has no MSSQL driver) before any
code was written.

## 1. US1/FR-001 — Initiator failover wiring: the handle-type mismatch

**Finding (corrects the gap analysis's implicit assumption that this is "just wiring")**:
`crates/truefix/src/lib.rs`'s `Engine::start` never reads `rs.failover_addresses` at all — confirmed by
grep; it's dead data on `ResolvedSession` today. But swapping the initiator branch to
`connect_initiator_reconnecting_multi` isn't a drop-in replacement: that function returns a
`ReconnectHandle` (`stop()`, `join()`), not the `SessionHandle` (`logout()`, `send()`, `join()`) every
other `connect_initiator*` variant returns, and `Engine.initiators: Vec<SessionHandle>` /
`Engine::initiators()` is public API. Additionally, **no TLS-aware or proxy-aware reconnecting-multi
variant exists** — only `connect_initiator_reconnecting`/`connect_initiator_reconnecting_multi`
(plain TCP only), while `connect_initiator_tls`/`connect_initiator_via_proxy[_tls]` have no
reconnecting counterpart.

- **Decision**: Add a new `connect_initiator_reconnecting_multi_tls` to `truefix-transport` (same
  rotation/backoff loop as the existing multi-endpoint function, wrapping each connection attempt in
  the TLS handshake exactly as `connect_initiator_tls` does per-attempt instead of once). Scope
  proxy+failover **out of this feature** (see Assumptions) — it's a rarer combination and would need
  proxy target-host resolution semantics reconciled with per-endpoint rotation, a materially bigger
  design problem than TLS+failover.
  Resolve the handle-type mismatch **additively**: add `Engine.failover_initiators: Vec<ReconnectHandle>`
  (a new field) and `Engine::failover_initiators() -> &[ReconnectHandle]` (a new accessor), leaving
  `Engine.initiators`/`Engine::initiators()` untouched — a session only appears in one Vec or the
  other, by whether `rs.failover_addresses` is non-empty. No breaking change to either public accessor's
  existing type.
  `Engine::shutdown()` gains a loop calling `.stop()` on every `failover_initiators` entry (currently it
  only aborts acceptors; initiators aren't currently stoppable from `shutdown()` at all — `ReconnectHandle`
  is the first initiator handle type that even has a synchronous stop, so this is a small added
  capability, not a removed one).
- **Alternatives considered**: (a) Unify ALL initiators onto the reconnecting-multi connector
  (single-endpoint list when no failover configured) — rejected: `connect_initiator_reconnecting`
  auto-reconnects on every drop, which is a behavior change for every non-failover initiator too
  (today's one-shot connectors don't auto-reconnect), out of this feature's stated scope (GAP-02 is
  about *unused failover_addresses*, not about changing default reconnect semantics broadly). (b) Make
  `Engine.initiators` hold an enum of both handle types — rejected: changes `initiators()`'s return
  type, a real (if soft) breaking change, for no benefit over the fully-additive two-Vec design.

## 2. US2/FR-002 — Dictionary/validator `.cfg` wiring: value format

**Finding**: `UseDataDictionary`/`DataDictionary`/`AppDataDictionary`/`TransportDataDictionary` are
already marked `Impl` in `crates/truefix-config/src/keys.rs`, but `builder.rs` has no
dictionary/validator resolution function at all — confirmed by grep (no `resolve_validator`, no
`truefix_dict` import in `builder.rs`). This is the pre-existing registry inaccuracy TODO-09 already
flagged ("validation 组的任何键都没有从 `.cfg` 接入 `Engine::start`"), not a new discovery, but this
plan is what actually closes it.

- **Decision**: `DataDictionary` (and `AppDataDictionary`/`TransportDataDictionary` for FIXT sessions)
  accepts either (a) one of `truefix_dict::ALL_DICTS`'s version strings (`"FIX.4.4"`, `"FIX.5.0SP2"`,
  `"FIX.Latest"`, etc. — which conveniently already match `BeginString` values used throughout the
  codebase) to select a bundled dictionary via the matching `load_fixXX()`, or (b) a filesystem path,
  loaded via `truefix_dict::load_from_file`, tried when the value doesn't match a known bundled version
  string. `UseDataDictionary=N` (or absent — matches today's default, since the key currently has no
  resolver at all) means `Services.validator` stays `None`, identical to today. The five
  `ValidationOptions` fields already implemented (TODO-09) are read from their existing, already-`Impl`
  keys and merged into the same `(DataDictionary, ValidationOptions)` tuple `ResolvedSession` gains.
- **Alternatives considered**: Requiring `DataDictionary` to always be a file path (matching QuickFIX/J,
  which has no "bundled dictionary" concept at all — every dictionary is a file) — rejected: TrueFix's
  own `ALL_DICTS` bundled-dictionary mechanism is a first-class, already-public capability (used
  throughout the AT harness and examples); forcing every `.cfg` deployment to also ship a `.fixdict`
  file on disk just to use a bundled version would be needless friction this feature can avoid for free.

## 3. US3/FR-003 — SQL backend `.cfg` dispatch: scheme-based, not driver-class-based

**Finding**: `resolve_store`/`resolve_log` in `builder.rs` only understand `FileStorePath`/
`FileLogPath`; `JdbcURL` and friends are parsed into `Map` but never read by any resolver — confirmed
by grep (`Jdbc` appears zero times in `builder.rs`). `SqlStore::connect(url)`/`MssqlStore::connect(url)`
already scheme-sniff their own URL (`postgres://`/`mysql://`/`sqlite:` vs `mssql://`/`sqlserver://`)
internally — there's no Java-style driver-class registry in this codebase to mirror `JdbcDriver`
against.

- **Decision**: `resolve_store` gains a `JdbcURL` branch, dispatched by prefix: `postgres://`/
  `postgresql://`/`mysql://`/`sqlite:` → `StoreConfig::Sql { url }` (requires the `sql` feature
  compiled in); `mssql://`/`sqlserver://` → `StoreConfig::Mssql { url }` (requires `mssql`). An
  unrecognized scheme, or a recognized scheme whose feature isn't compiled in, is a typed
  `ConfigError::InvalidValue`/a new `ConfigError` variant naming the missing feature — never a panic or
  a silent fallback to the memory store (FR-004). `resolve_log` gains a matching `JdbcURL` branch —
  **correction, found while drafting tasks.md**: this does *not* land as a `LogConfig::Sql`/`Mssql`
  variant (`truefix_log::LogConfig` has no such variant, and `truefix-config`'s existing `resolve_log`/
  `LogSpec` is a File-log-only `pub`-fields struct, not an enum — turning it into one would be a real
  breaking change this project's additive-first bias doesn't need to take on here). Instead it
  populates a new, independent `ResolvedSession.sql_log: Option<SqlLogSpec>` field, reusing the
  already-implemented `JdbcLogIncomingTable`/`JdbcLogOutgoingTable`/`JdbcLogEventTable`/
  `JdbcLogHeartBeats` keys (also currently `Rec`, unread) for table names/heartbeat filtering. See
  data-model.md's "`SqlLogSpec`" section for the full corrected design.
- **Alternatives considered**: Modeling `JdbcDriver` as a literal dispatch key (`JdbcDriver=postgresql`
  → Postgres, etc., independent of `JdbcURL`'s own scheme) — rejected: redundant with information
  already in `JdbcURL`'s scheme, and every `SqlStore`/`MssqlStore` constructor already takes the full
  URL, not a separate driver name; introducing a second, potentially-contradictory selector adds
  ambiguity (what if `JdbcDriver=mysql` but `JdbcURL=postgres://...`?) for no behavioral gain.

## 4. US4/FR-005 — `ContinueInitializationOnError`: two failure classes, not one

**Finding**: `Engine::start`'s multi-session loop is confirmed all-or-nothing — every fallible step
(`build_store`, `Acceptor::bind_with`, TLS config construction, every `connect_initiator*` variant)
uses `?`. But there's a **second, earlier** all-or-nothing barrier this section's first draft missed:
`Engine::start`'s very first line is `let resolved = settings.resolve()?;`, and
`SessionSettings::resolve()` itself is `self.sessions().iter().map(|(i, map)| resolve_one(map, i))
.collect()` — a single `ConfigError` from *any* session's `resolve_one` (e.g. a missing
`DataDictionary` under `UseDataDictionary=Y`) aborts the whole `resolve()` call **before**
`Engine::start`'s per-session loop even begins. Restructuring only that loop (as originally
sketched) would therefore never help a config-resolution failure — the loop never receives a
`ResolvedSession` for a session that failed to resolve at all. `SessionConfig.
continue_initialization_on_error: bool` already exists (from feature 003/TODO-04) but its own doc
comment said "has no effect here" (correctly, before this feature); it's a per-connection `Session`
field, but the engine-level concern spans *both* the resolution barrier and the runtime-startup
barrier — two genuinely different failure classes needing two different mechanisms.

- **Decision**: Two-part fix. (1) A new `SessionSettings::resolve_lenient() ->
  Result<LenientResolve, ConfigError>` in `truefix-config`, replacing `Engine::start`'s call to
  `resolve()`: it resolves sessions one at a time, and on a `resolve_one` failure, reads
  `ContinueInitializationOnError` **directly from that session's own raw `.cfg` map** (not from a
  `SessionConfig` that doesn't exist yet, since resolution itself failed) — the key is a simple
  `bool_key` lookup that can never itself fail to parse, so it's always readable even when other
  keys in the same session block are broken. A tolerated failure is collected into a `(session
  label, error)` list rather than aborting; an untolerated one still aborts the whole resolve
  immediately, exactly as `resolve()` does today. (2) `Engine::start`'s per-session loop body (store
  build, log build, TLS/proxy config build, bind/connect) is *also* restructured, wrapped in an
  inline `async {}.await` block producing a `Result<(), EngineError>` per session — this covers
  *runtime* startup failures (e.g. a port already in use), which only surface after resolution has
  already succeeded, so `rs.session.continue_initialization_on_error` is available to read normally
  at that point. Both paths report a skipped session via `tracing::error!` (the project's chosen
  observability facade, Constitution Principle I) naming the session identity and error — a
  per-session `Log::on_event` call can't be relied on as the sole channel, since the failure may be
  in constructing the log itself. `Engine::start` keeps returning `Result<Self, EngineError>`: `Err`
  only when a *non-continuing* session fails (today's behavior, unchanged), `Ok` (with however many
  sessions actually started, possibly fewer than requested) once every failure has either propagated
  or been skipped-and-logged.
- **Alternatives considered**: An engine-wide (not per-session) flag — rejected: the config key is
  already scoped per-`[SESSION]`-block by `SessionSettings`'s existing parsing model (like every
  other session switch), and QuickFIX/J's own semantics for this key are per-session; introducing an
  engine-wide override would be a new concept not asked for by the spec or the audit. Only fixing the
  loop-level (runtime) case and leaving the resolution-level case as a documented gap — rejected: it
  would silently fail to satisfy this story's own stated Independent Test (a *resolution* failure,
  the more common real-world misconfiguration case — a typo'd or missing key — not a runtime bind
  race).

## 5. US5/FR-006/FR-007 — Embedded transactional store: `redb`, sync API needs `spawn_blocking`

**Finding**: `redb`'s public API (`Database::create`/`begin_write`/`begin_read`, `TableDefinition<K, V>`,
`insert`/`get`/`remove`, transaction `commit()`) is **fully synchronous** — no async variant exists.
`MessageStore`/`Log` are `async_trait` traits (matching every existing backend: `sqlx`, `tiberius`, and
soon `mongodb` are all natively async). Calling `redb`'s blocking API directly from an async trait
method would block the tokio executor thread for the duration of each disk/B-tree operation.

- **Decision**: `RedbStore` holds `Arc<redb::Database>`; every `MessageStore` method wraps its `redb`
  work in `tokio::task::spawn_blocking(move || { ... })` (cloning the `Arc` into the closure), then
  `.await`s the `JoinHandle` and maps a `spawn_blocking` join error to `StoreError::Backend`. Table
  shape mirrors `SqlStore`'s two-table design: one `TableDefinition<&str, (u64, u64)>`-equivalent for
  `session_id → (sender_seq, target_seq)`, one `TableDefinition<(&str, u64), &[u8]>`-equivalent for
  `(session_id, seq) → message bytes` (redb supports tuple keys via its `Key` trait for composite keys,
  confirmed by its own multi-dimensional-key examples). `reset()` opens a single write transaction,
  deletes every message-table entry for the session, resets the sequence entry, and commits once —
  satisfying FR-007's atomicity requirement natively (redb transactions are all-or-nothing by
  construction; no partial-commit state is observable to a concurrent reader or a subsequent crash).
- **Alternatives considered**: Running `redb` operations directly on the async task without
  `spawn_blocking` — rejected: violates the "no blocking the executor" discipline implicit in
  Constitution Principle I's production-readiness bar, and inconsistent with every other I/O-bound
  backend in this codebase being genuinely non-blocking-async.

## 6. US6/FR-008 — MongoDB store: official `mongodb` crate, native async

**Finding**: The official `mongodb` crate's async API is enabled by default (requires `tokio`, no extra
feature flag needed) — `Client::with_uri_str`, `Database::collection::<T>()`,
`find_one`/`insert_one`/`update_one`/`delete_one`, all `async`. One documented driver-level caveat:
cancelling an in-flight future (e.g. via `tokio::select!` or `tokio::time::timeout` racing it against
something else) can leave the driver's internal connection state inconsistent — the driver's own docs
warn callers to poll its futures to completion rather than cancel them early.

- **Decision**: `MongoStore`/`MongoLog` use the async `mongodb` API directly (no `spawn_blocking`
  needed, unlike `redb`). Every `MessageStore`/`Log` call **must not** be wrapped in
  `tokio::time::timeout` or raced inside a `select!` (the one caveat above) — if a future timeout is
  ever desired at a higher layer, it must wrap the *caller*, not the `mongodb` future itself, so a
  timeout never cancels a driver operation mid-flight. Collection shape mirrors the two-table SQL
  design: one collection for sequence-number documents (`{ session_id, sender, target }`), one for
  message documents (`{ session_id, seq, data }`), with a compound index on `(session_id, seq)` for the
  message collection (created idempotently on connect, mirroring `ensure_schema`'s pattern in the SQL
  backends).
- **Alternatives considered**: The `mongodb` crate's `sync` feature (a blocking API, requiring
  `spawn_blocking` like `redb`) — rejected: the async API is the default, native, and better-supported
  surface; no reason to take on the `spawn_blocking` pattern when a genuinely async driver is available,
  unlike `redb` where sync is the only option.

## 7. Dependency license audit (Principle III)

| Dependency | Version at spec time | License | Verdict |
|------------|----------------------|---------|---------|
| `redb` | 4.1.0 (actively maintained) | MIT OR Apache-2.0 | Compatible — permissive, no copyleft. Verified via crates.io metadata during the specify conversation; re-confirmed here. No known transitive advisory-flagged dependency at spec time (pure-Rust, minimal dependency tree) — `/speckit-tasks`' T002-equivalent audit re-checks against `cargo deny check` before first use, same discipline as 003's `tiberius`/`ppp`/`tokio-socks` audit. |
| `mongodb` | current stable (async, default `tokio` runtime) | Apache-2.0 | Compatible — permissive, no copyleft. Matches QuickFIX/Go's own choice of the official MongoDB driver for its equivalent feature. |

Neither dependency was added to any `Cargo.toml` yet — wired in only when the stage that consumes it
lands (mirroring 001/002/003's "no code before it's needed" convention), each behind its own
independent, off-by-default feature (`redb`, `mongodb`) on `truefix-store`/`truefix-log`, following the
`mssql` feature precedent from feature 003 exactly (not folded into the existing `sql` feature, which is
`sqlx`-specific).

## Summary of new dependencies

- `redb` (US5, `truefix-store`/`truefix-log`, behind new `redb` feature)
- `mongodb` (US6, `truefix-store`/`truefix-log`, behind new `mongodb` feature)

No other new dependencies — US1–US4 are wiring-only, using types/functions already present in
`truefix-transport`/`truefix-dict`/`truefix-config` (plus one new function,
`connect_initiator_reconnecting_multi_tls`, added to the existing `truefix-transport` crate).
