# Contract: Additional Store Backends — `redb` + MongoDB (US5, US6; FR-006–FR-008)

## Surface

```text
// truefix-store, behind the `redb` feature
pub struct RedbStoreConfig { pub path: PathBuf, pub session_id: String }
impl RedbStoreConfig { pub fn new(path: impl Into<PathBuf>) -> Self; }
pub struct RedbStore { /* opaque */ }
impl RedbStore {
    pub async fn connect(path: &Path) -> Result<Self, StoreError>;
    pub async fn connect_with_config(config: RedbStoreConfig) -> Result<Self, StoreError>;
}
impl MessageStore for RedbStore { /* full trait */ }

// truefix-log, behind the `redb` feature
pub struct RedbLogConfig { pub path: PathBuf, pub include_heartbeats: bool, /* table-name fields */ }
pub struct RedbLog { /* opaque */ }
impl RedbLog {
    pub async fn connect(path: &Path) -> Result<Self, LogError>;
    pub async fn connect_with_config(config: RedbLogConfig) -> Result<Self, LogError>;
}
impl Log for RedbLog { /* full trait */ }

// truefix-store, behind the `mongodb` feature
pub struct MongoStoreConfig { pub uri: String, pub database: String, /* collection-name fields */, pub session_id: String }
pub struct MongoStore { /* opaque */ }
impl MongoStore {
    pub async fn connect(uri: &str) -> Result<Self, StoreError>;
    pub async fn connect_with_config(config: MongoStoreConfig) -> Result<Self, StoreError>;
}
impl MessageStore for MongoStore { /* full trait */ }

// truefix-log, behind the `mongodb` feature
pub struct MongoLogConfig { pub uri: String, pub database: String, /* collection-name fields */, pub include_heartbeats: bool }
pub struct MongoLog { /* opaque */ }
impl MongoLog {
    pub async fn connect(uri: &str) -> Result<Self, LogError>;
    pub async fn connect_with_config(config: MongoLogConfig) -> Result<Self, LogError>;
}
impl Log for MongoLog { /* full trait */ }

// truefix-store facade
pub enum StoreConfig {
    // ...existing variants...
    #[cfg(feature = "redb")]
    Redb { path: PathBuf },      // NEW
    #[cfg(feature = "mongodb")]
    Mongo { uri: String },       // NEW
}
```

`connect`/`connect_with_config`, and the `*Config::new` shorthand-with-defaults pattern, mirror
`MssqlStore`/`MssqlStoreConfig` exactly (the precedent established in feature 003 for an
independent-feature, non-`sql` backend).

## Behaviour

1. `RedbStore`/`RedbLog` (US5): every `MessageStore`/`Log` operation runs inside
   `tokio::task::spawn_blocking` around the synchronous `redb` API (research.md §5). `reset()` clears
   sequence numbers and message bodies for the session within one `redb` write transaction — atomic by
   construction (FR-007).
2. `MongoStore`/`MongoLog` (US6): every operation uses the native async `mongodb` API directly, never
   wrapped in `tokio::time::timeout`/raced in a `select!` (research.md §6's driver-cancellation
   caveat).
3. Both backends satisfy the exact same `MessageStore`/`Log` trait contracts as `SqlStore`/`MssqlStore`
   and their logs — no new trait methods, no behavioral divergence in what `next_sender_seq`/
   `next_target_seq`/`save`/`get`/`reset` mean.
4. Both are off by default: `cargo build --workspace`/`cargo test --workspace` compile and require
   neither `redb` nor `mongodb` nor their crates unless the respective feature is explicitly enabled.

## No breaking changes

`StoreConfig`/`LogConfig` gain new, `#[cfg(feature = ...)]`-gated enum variants — every existing match
arm in this workspace already either matches exhaustively per-feature (like the existing `sql`/`mssql`
arms) or the enums are matched with a catch-all in code outside their owning crates; verified during
implementation that no existing exhaustive match breaks without a `#[cfg]`-matching arm added
alongside.

## Acceptance (maps to spec US5/US6 scenarios)

- Sequence numbers and message bodies persist across a process restart against the same `redb` file
  (SC-005, embedded store half). ✔
- `reset()` atomically clears both sequence numbers and message bodies — no crash-mid-reset partial
  state (SC-005, embedded store half). ✔
- Two session identities sharing one `redb` file / one MongoDB database don't see each other's data
  (SC-005, both halves — the same session-isolation contract `SqlStore`/`MssqlStore` already satisfy). ✔
- `MongoStore`/`MongoLog` conformance tests skip cleanly (not fail) when no reachable MongoDB instance
  is configured (SC-005, MongoDB half). ✔

## Test hooks

- `crates/truefix-store/tests/redb_backend.rs` / `crates/truefix-log/tests/redb_log.rs` — full-contract
  + session-isolation tests, unconditional (no external service needed, like SQLite today), mirroring
  `sql_backends.rs`'s exact pattern.
- `crates/truefix-store/tests/mongo_backend.rs` / `crates/truefix-log/tests/mongo_log.rs` — same
  pattern, gated on a `DATABASE_URL_MONGO`-style env var, mirroring `mssql_backend.rs`'s
  gated-skip pattern exactly.
- A new CI `mongo` job (service container), mirroring the existing `mssql` job.
