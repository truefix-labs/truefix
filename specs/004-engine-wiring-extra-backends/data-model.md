# Phase 1 Data Model: Engine Config-Wiring Completion + Additional Store Backends

**Feature**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

This translates the spec's Key Entities (plus the concrete shapes research.md's investigation
surfaced) into grounded types. Every entity here is additive to the existing `truefix-transport`/
`truefix-config`/`truefix-store`/`truefix-log` types found during Phase 0 — no existing public type's
shape is removed or narrowed.

## `Engine.failover_initiators` (extends `truefix::Engine`)

```text
Engine {
    acceptors: Vec<AcceptorHandle>,               // unchanged
    initiators: Vec<SessionHandle>,                // unchanged: single-endpoint initiators only
    failover_initiators: Vec<ReconnectHandle>,     // NEW: initiators with 1+ failover_addresses
}

impl Engine {
    pub fn initiators(&self) -> &[SessionHandle]              // unchanged signature
    pub fn failover_initiators(&self) -> &[ReconnectHandle]   // NEW accessor
    pub fn shutdown(&self)                                     // extended: also .stop()s every
                                                                // failover_initiators entry
}
```

- **Selection rule**: at `Engine::start`, a resolved initiator session goes into `initiators` when
  `rs.failover_addresses.is_empty()`, otherwise into `failover_initiators` — never both, matching the
  existing acceptor/initiator split-by-kind pattern.
- **No breaking change**: `initiators()`'s return type is untouched; `failover_initiators()` is a new,
  independent accessor.

## `connect_initiator_reconnecting_multi_tls` (extends `truefix-transport`)

```text
connect_initiator_reconnecting_multi_tls(
    addrs: Vec<SocketAddr>,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    tls: Arc<rustls::ClientConfig>,
    server_name: rustls::pki_types::ServerName<'static>,
) -> ReconnectHandle
```

- Same rotation/backoff loop as the existing `connect_initiator_reconnecting_multi`, but each
  connection attempt performs the TLS handshake per-attempt (mirroring `connect_initiator_tls`'s
  single-attempt pattern) before calling `run_connection`.
- Proxy+failover is explicitly not modeled (no `connect_initiator_reconnecting_multi_via_proxy*`) —
  out of scope per spec Assumptions.

## `ResolvedSession.validator` (extends `truefix-config::builder::ResolvedSession`)

```text
ResolvedSession {
    // ...existing fields unchanged...
    validator: Option<(truefix_dict::DataDictionary, truefix_dict::ValidationOptions)>,  // NEW
}
```

- **Resolution** (`resolve_validator(map, session) -> Result<Option<(DataDictionary, ValidationOptions)>, ConfigError>`):
  `UseDataDictionary` absent/`N` → `None` (today's behavior, unchanged). `UseDataDictionary=Y` →
  `Some((dict, opts))` where `dict` comes from `DataDictionary` (falling back to
  `AppDataDictionary`/`TransportDataDictionary` for FIXT sessions, same precedence QuickFIX/J uses): a
  value matching one of `truefix_dict::ALL_DICTS`'s version strings loads the bundled dictionary via its
  `load_fixXX()`; any other value is treated as a file path via `load_from_file`. `opts` is built from
  the five already-implemented `ValidationOptions` keys (`ValidateFieldsOutOfOrder`, `ValidateChecksum`,
  `ValidateIncomingMessage`, `AllowPosDup`, `RequiresOrigSendingTime`), reusing their existing resolver
  logic (no new toggle semantics — this entity only adds the missing wiring).
- **Engine wiring**: `Engine::start` copies `rs.validator` straight into `Services.validator` for both
  acceptor and initiator branches — no new `Services` field needed (it already exists, per US10/003).

## `JdbcURL`-resolved store selection (extends `resolve_store`)

Not a new persisted type — a new resolution branch, inserted **before** the existing
`FileStorePath` check in `resolve_store`. Reads `JdbcURL`; if present, dispatches by URL-scheme prefix
to `StoreConfig::Sql { url }` (`postgres://`/`postgresql://`/`mysql://`/`sqlite:` — requires the `sql`
feature) or `StoreConfig::Mssql { url }` (`mssql://`/`sqlserver://` — requires `mssql`); an
unrecognized scheme or a recognized scheme whose feature isn't compiled produces a new typed
`ConfigError::UnsupportedBackend { session, scheme }` variant (additive to the existing `ConfigError`
enum — an existing, `#[non_exhaustive]`-free but non-exhaustively-matched-anywhere-in-this-workspace
enum, grep-confirmed at plan time; see plan.md's Constitution Check). Absent `JdbcURL` falls through to
today's `FileStorePath`/memory-store resolution, unchanged — `StoreConfig` was already an enum, so this
is a straightforwardly additive variant-dispatch change, no different in kind from 003's own
`StoreConfig::Mssql` addition.

## `SqlLogSpec` (new; extends `ResolvedSession`) — corrects an earlier assumption in this doc

**Correction found while drafting tasks.md**: this doc originally (incorrectly) described the log side
of this feature as a `LogConfig::Sql`/`LogConfig::Mssql` dispatch mirroring the store side. Grounding
against the actual code during task breakdown found that's wrong on two counts: (1) there is no
`truefix_log::LogConfig`-level dispatch reachable from `truefix-config` at all — `resolve_log` returns
a plain `pub struct LogSpec { dir, include_heartbeats, include_timestamp, include_milliseconds }`
(File-log-shaped only), and `truefix::Engine::start`'s private `build_log` helper hardcodes
`FileLog::open_with_options` unconditionally; (2) turning the existing `LogSpec` into an enum to
accommodate a SQL variant would be a real breaking change to a `pub`-fields struct external code could
already construct directly — this project's established bias (per 003's precedent, and this feature's
own plan.md) is additive-over-breaking unless truly justified, and there's no need to break `LogSpec`
here.

**Corrected design**: `ResolvedSession` gains a new, independent field —

```text
ResolvedSession {
    // ...existing fields, including the unchanged `log: Option<LogSpec>` (File-log only)...
    sql_log: Option<SqlLogSpec>,  // NEW
}

SqlLogSpec {
    url: String,        // the JdbcURL value, scheme carries backend selection (Sql vs Mssql)
    include_heartbeats: bool,   // from the already-`Recognized` JdbcLogHeartBeats key
    // table-name fields reuse the already-`Recognized` JdbcLogIncomingTable/JdbcLogOutgoingTable/
    // JdbcLogEventTable keys when present, else each backend's own default table names
}
```

- `resolve_log` gains the `JdbcURL` branch, populating `sql_log` (not touching `log`/`LogSpec` at
  all). `log` and `sql_log` are mutually exclusive by construction: when `JdbcURL` is present, the SQL
  branch is used and `FileLogPath` (if also present) is ignored for that session's log, with a
  `tracing::warn!` noting the precedence — an explicit, disclosed scope boundary (fanning out to both
  a file log and a SQL log via `CompositeLog` was considered and rejected as beyond FR-003's stated
  "SQL backend selection reachable from `.cfg`" scope; `CompositeLog` remains available programmatically
  for anyone who wants that combination today).
- `Engine::start`'s `build_log`-equivalent path gains a second branch: when `rs.sql_log` is `Some`,
  build a `truefix_log::SqlLog`/`MssqlLog` (dispatched by the same scheme check as the store side)
  instead of calling the existing `FileLog`-only `build_log` helper.

## `RedbStoreConfig` / `RedbStore` (new, `truefix-store::redb` module, `redb` feature)

Mirrors `SqlStoreConfig`'s shape (minus pool settings — `redb::Database` has its own internal
concurrency handling, no separate pool needed):

```text
RedbStoreConfig {
    path: PathBuf,
    session_id: String,
}

RedbStore {
    db: Arc<redb::Database>,
    session_id: String,
}
```

- Three `redb` tables (research.md §5; corrected during implementation from an original
  two-table/tuple-*value* sketch): `sender_seq`/`target_seq` (each `TableDefinition<&str, u64>`,
  keyed by `session_id`) rather than one combined-tuple-value table — simpler, and `u64` already
  implements both `Key`/`Value` directly, no need to invent a tuple-value encoding; and `messages`
  (`TableDefinition<(&str, u64), &[u8]>`, keyed by `(session_id, seq)` — tuple *keys* are natively
  supported by `redb::Key`).
- **`redb` takes an exclusive file lock per `Database::create`/`open` call** (found during
  implementation, not in the original research) — two independent `RedbStore::connect*` calls
  against the same path cannot coexist in one process, unlike a networked SQL connection/pool. A new
  `RedbStore::with_session_id(&self, session_id) -> Self` method (clones the cheap `Arc<Database>`,
  swaps only `session_id`) is the correct way to share one file across sessions.
- Every `MessageStore` method wraps its `redb` transaction in `tokio::task::spawn_blocking`. `reset()`
  clears both tables for `session_id` within one write transaction (atomic — FR-007).
- `was_corrupted()`: `redb` performs its own crash-recovery/checksum validation on open (part of its
  ACID guarantee); a corruption detected there surfaces as a `Database::create`/`open` error at
  connect time (a typed `StoreError`), not a boolean flag to check post-connect — differs slightly from
  `FileStore`'s `was_corrupted()` shape, since `redb` doesn't expose a "recovered but was corrupted"
  distinct from "opened cleanly" the way `FileStore`'s own hand-rolled format does. Default
  `was_corrupted() -> false` (trait default) applies.

## `RedbLogConfig` / `RedbLog` (new, `truefix-log::redb` module, `redb` feature)

Same shape as `SqlLogConfig`/`MssqlLogConfig` (incoming/outgoing/event table-equivalent names,
`include_heartbeats: bool`), using three more `redb` tables (append-only, keyed by an internal
auto-incrementing counter) and the same `spawn_blocking` pattern as `RedbStore`.

## `MongoStoreConfig` / `MongoStore` (new, `truefix-store::mongo` module, `mongodb` feature)

```text
MongoStoreConfig {
    uri: String,
    database: String,
    sessions_collection: String,
    messages_collection: String,
    session_id: String,
}

MongoStore {
    sessions: mongodb::Collection<SessionDoc>,
    messages: mongodb::Collection<MessageDoc>,
    session_id: String,
}

SessionDoc { session_id: String, sender: i64, target: i64 }
MessageDoc { session_id: String, seq: i64, data: Vec<u8> }
```

- A compound index on `(session_id, seq)` for the messages collection, created idempotently on
  connect (mirroring `ensure_schema`'s pattern in the SQL backends).
- Native `mongodb` async API throughout — no `spawn_blocking`. Per research.md §6, no
  `tokio::time::timeout`/`select!` ever wraps a `mongodb` future directly.

## `MongoLogConfig` / `MongoLog` (new, `truefix-log::mongo` module, `mongodb` feature)

Same shape as the SQL/redb logs (incoming/outgoing/event collection names, `include_heartbeats: bool`),
backed by three MongoDB collections instead of tables, same background-task-plus-unbounded-channel
architecture as `SqlLog`/`MssqlLog` (the `Log` trait is synchronous; entries are pushed onto an
unbounded channel and written by a background task holding the `mongodb::Client`).

## `ConfigError::UnsupportedBackend` (extends `truefix-config::ConfigError`)

```text
ConfigError::UnsupportedBackend { session: String, scheme: String }
```

Returned when `JdbcURL`'s scheme is recognized but its backing Cargo feature isn't compiled in, or the
scheme is entirely unrecognized — never a panic, never a silent fallback (FR-004).
