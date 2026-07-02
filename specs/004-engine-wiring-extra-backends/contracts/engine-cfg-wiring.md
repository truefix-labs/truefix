# Contract: Engine `.cfg`-Wiring Completion (US1–US4; FR-001–FR-005)

## Surface

```text
// truefix-transport
pub fn connect_initiator_reconnecting_multi_tls<A>(   // NEW
    addrs: Vec<SocketAddr>,
    config: SessionConfig,
    app: Arc<A>,
    services: Services,
    tls: Arc<rustls::ClientConfig>,
    server_name: rustls::pki_types::ServerName<'static>,
) -> ReconnectHandle;

// truefix-config
pub struct ResolvedSession {
    // ...existing fields, including the unchanged `log: Option<LogSpec>` (File-log only)...
    pub validator: Option<(truefix_dict::DataDictionary, truefix_dict::ValidationOptions)>,  // NEW
    pub sql_log: Option<SqlLogSpec>,  // NEW — see data-model.md's "SqlLogSpec" section for why this
                                       // is a new sibling field rather than turning the existing
                                       // `LogSpec` struct into an enum (that would be a real breaking
                                       // change to a pub-fields type; this project's bias is additive)
}
pub struct SqlLogSpec { pub url: String, pub include_heartbeats: bool, /* table-name fields */ }
pub enum ConfigError {
    // ...existing variants...
    UnsupportedBackend { session: String, scheme: String },  // NEW
}
pub type LenientResolve = (Vec<ResolvedSession>, Vec<(String, ConfigError)>);  // NEW
impl SessionSettings {
    pub fn resolve_lenient(&self) -> Result<LenientResolve, ConfigError>;  // NEW
}

// truefix (facade)
impl Engine {
    pub fn initiators(&self) -> &[SessionHandle];              // unchanged
    pub fn failover_initiators(&self) -> &[ReconnectHandle];   // NEW
    pub fn shutdown(&self);                                     // extended (also stops failover_initiators)
}
```

## Behaviour

1. **US1 (initiator failover)**: `Engine::start` routes a resolved initiator session to
   `failover_initiators` (via `connect_initiator_reconnecting_multi[_tls]`) when
   `rs.failover_addresses` is non-empty, and to `initiators` (today's one-shot connectors, unchanged)
   otherwise. Proxy-configured initiators are never routed to `failover_initiators`, even if failover
   addresses are also present (proxy+failover is out of scope; the proxy path takes precedence and
   failover addresses are ignored for that session — documented, not silent, via a `tracing::warn!` at
   resolve time).
2. **US2 (dictionary/validator wiring)**: `Engine::start` copies `rs.validator` into each session's
   `Services.validator`, for both acceptor and initiator branches. No change to `Services`'s own shape
   (the field already existed).
3. **US3 (SQL backend `.cfg` dispatch)**: `resolve_store` reads `JdbcURL` and dispatches by scheme to
   `StoreConfig::Sql`/`Mssql`, before falling back to the existing `FileStorePath`/memory-store
   resolution when absent. `resolve_log` gains a parallel `JdbcURL` branch populating the new
   `ResolvedSession.sql_log: Option<SqlLogSpec>` field — independent of, and mutually exclusive with,
   the existing `log: Option<LogSpec>` (File-log) field, which is otherwise untouched.
4. **US4 (`ContinueInitializationOnError`)**: two failure classes, both tolerated per-session when
   `ContinueInitializationOnError=Y`. *Resolution*-time failures (e.g. a missing `DataDictionary`)
   are tolerated by `Engine::start` calling the new `SessionSettings::resolve_lenient()` instead of
   `resolve()` — the flag is read from that session's raw `.cfg` map, since resolution itself
   failed. *Runtime*-startup failures (e.g. a port already in use), which only surface once
   resolution has already succeeded, are tolerated by `Engine::start`'s per-session loop body
   (restructured into an inline `async {}` block per session), reading
   `rs.session.continue_initialization_on_error` normally. Both report a skipped session via
   `tracing::error!` and continue to the next session rather than returning `Err` and abandoning
   already-processed sessions.

## No breaking changes

- `Engine.initiators`/`Engine::initiators()` keep their existing `Vec<SessionHandle>`/`&[SessionHandle]`
  shape; `failover_initiators` is a wholly new field/accessor.
- `ResolvedSession.validator` and `ConfigError::UnsupportedBackend` are new, additive members of
  existing public types (adding a field to a `pub`-fields struct / a variant to a non-exhaustive-in-
  practice enum matched via existing `match`+wildcard patterns in `truefix`'s own `EngineError`
  conversion, verified during implementation not to require an exhaustive-match update anywhere in this
  workspace outside `truefix-config` itself).

## Acceptance (maps to spec US1–US4 scenarios)

- A `.cfg` initiator with numbered backup endpoints reconnects to a backup after the primary fails,
  zero additional Rust code (SC-001). ✔
- A `.cfg` session with `UseDataDictionary=Y` rejects a dictionary-invalid message identically to a
  programmatically-wired validator, zero additional Rust code (SC-002). ✔
- A `.cfg` session with `JdbcURL` set persists to the named SQL backend and survives a restart, zero
  additional Rust code (SC-003). ✔
- A multi-session `.cfg` acceptor with `ContinueInitializationOnError=Y` and one misconfigured session
  starts every other session; the same file with the flag unset fails entirely (SC-004). ✔

## Test hooks

- `truefix-transport`: a new test proving `connect_initiator_reconnecting_multi_tls` reconnects to a
  backup TLS endpoint after the primary fails (extends the existing plain-TCP reconnect test pattern).
- `truefix-config`: `.cfg` → `ResolvedSession.validator` mapping tests (mirroring
  `store_and_log_mapping.rs`'s existing pattern); `.cfg` → `JdbcURL`-dispatched `StoreConfig`/
  `sql_log` mapping tests; `ContinueInitializationOnError` unaffected-by-default + skip-on-error
  tests.
- `truefix`/`truefix-transport`: an `Engine::start`-level integration test proving a multi-session
  `.cfg` acceptor with one misconfigured session starts the others when the flag is set, and fails
  entirely when it isn't (extends the existing `config_start.rs` pattern from feature 002).
