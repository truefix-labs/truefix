# Tasks: Engine Config-Wiring Completion + Additional Store Backends

**Input**: Design documents from `specs/004-engine-wiring-extra-backends/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (all present)

**Tests**: Included — Constitution Principle V mandates test-first discipline for this project; every
prior feature (001/002/003) authored tests alongside every behavioral change, and this feature follows
the same convention.

**Organization**: Tasks are grouped by user story (US1–US6, matching spec.md's priority order:
P1 = US1/US2, P2 = US3/US4/US5, P3 = US6) to enable independent implementation and testing of each.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to
- File paths are exact and grounded in the actual current code (per research.md/data-model.md), not
  placeholders

---

## Phase 1: Setup

- [X] T001 [P] Add `redb` (US5) and `mongodb` (US6) optional dependencies + independent, off-by-default
  `redb`/`mongodb` Cargo features to `crates/truefix-store/Cargo.toml` and `crates/truefix-log/Cargo.toml`,
  mirroring the `mssql` feature precedent from feature 003 (workspace-level version pin in the root
  `Cargo.toml`: `redb = "4"`, `mongodb = "3"`) — **complete**: resolved to `redb` 4.1.0, `mongodb`
  3.7.0. All four feature combinations (`truefix-store`/`truefix-log` × `redb`/`mongodb`) build clean.
- [X] T002 [P] Run the formal license/provenance audit (Principle III) for `redb`/`mongodb` via
  `cargo deny check` once T001 lands; record the result in `docs/parity-matrix.md`'s dependency-audit
  table, confirming research.md §7's informal findings (`redb` MIT OR Apache-2.0, `mongodb` Apache-2.0)
  — **complete, and found + fixed two real issues while running the audit**: (1) `mongodb` transitively
  pulls `tiny-keccak` (`CC0-1.0`) and `webpki-roots` (`CDLA-Permissive-2.0`), neither previously in
  `deny.toml`'s allow-list — both are strictly permissive (no copyleft), added with justification. (2)
  Re-running `cargo deny check` surfaced a **newly-published, pre-existing advisory unrelated to this
  feature**: RUSTSEC-2026-0194 (`quick-xml` 0.36.2, feature 003's Orchestra-parsing dependency —
  quadratic-time duplicate-attribute-name checking, a CPU DoS risk on untrusted XML). Fixed by
  upgrading to `quick-xml` 0.41.0 (the advisory's fix version) in the root `Cargo.toml`, migrating
  `crates/truefix-dict/src/orchestra.rs`'s one call site from the now-deprecated
  `Attribute::unescape_value()` to `Attribute::normalized_value(XmlVersion::Implicit1_0)`; verified
  byte-for-byte unchanged output via the existing fixture-comparison test and the full `dict-tooling`
  suite. `cargo deny check` now exits 0 (`advisories ok, bans ok, licenses ok, sources ok`). See
  `docs/parity-matrix.md`'s "Feature 004 — Dependency & Provenance Audit" section.
- [X] T003 [P] Add a MongoDB service container to CI (`.github/workflows/ci.yml`), mirroring the
  existing `mssql` job exactly (`mongo:<version>` image, `DATABASE_URL_MONGO` env var for
  `cargo test -p truefix-store -p truefix-log --features mongodb`) — **complete**: new `mongo` job
  (mongo:7 service container, `mongosh`/`mongo`-fallback healthcheck) and a new `redb` job (no service
  container needed — `redb` is embedded, unconditional like SQLite). YAML validated.
- [X] T004 [P] Scaffold a stance-update tracking table addition in `docs/parity-matrix.md` for the
  config keys this feature moves recognised→implemented (FR-009): `ContinueInitializationOnError`,
  `UseDataDictionary`/`DataDictionary`/`AppDataDictionary`/`TransportDataDictionary` (already marked
  `Impl` but previously unwired — this feature makes the marking accurate, per research.md §2),
  `JdbcURL`/`JdbcLogIncomingTable`/`JdbcLogOutgoingTable`/`JdbcLogEventTable`/`JdbcLogHeartBeats` —
  **complete**: new "Feature 004 — Stance Tracking Scaffold (T004)" table added, all rows `pending`
  until their respective stage lands.

**Checkpoint**: Dependencies available, license-clean, CI-ready; stance-tracking scaffold in place. No
foundational blockers exist beyond this — US1–US6 touch disjoint files/crates and have no ordering
dependency on each other (unlike some prior features' Phase 2, this feature needs no shared
prerequisite code, so there is no separate Foundational phase).

---

## Phase 2: User Story 1 — Initiator failover wired into `Engine::start` (P1)

**Goal**: A `.cfg` initiator with numbered backup endpoints (`SocketConnectHost1`/`Port1`, etc.)
reconnects to a backup after the primary endpoint fails, over both plain TCP and TLS, with zero
additional Rust code.

**Independent Test**: Start an `Engine`-driven initiator via `.cfg` with a primary and backup endpoint;
kill the primary; verify reconnection to the backup.

### Tests for User Story 1

- [X] T005 [P] [US1] Test: `connect_initiator_reconnecting_multi_tls` reconnects to a backup TLS
  endpoint after the primary fails, in `crates/truefix-transport/tests/failover_tls.rs` (new file,
  extending `reconnect.rs`'s existing plain-TCP pattern with a TLS handshake per attempt) — **complete,
  and found + fixed a real test-authoring bug**: the first draft called `handle.stop();
  handle.join().await;` at the end (matching `reconnect.rs`'s existing plain-TCP test's cleanup
  pattern), which hung indefinitely — confirmed via temporary debug tracing that the TLS handshake
  correctly failed on the primary and succeeded on the backup, and `on_logon` fired on both sides, but
  `run_connection` never returns for a healthy, ongoing session (no logout is ever triggered), and
  `ReconnectHandle` has no graceful-logout capability (unlike `SessionHandle`), so `stop()` only takes
  effect *between* connection attempts, never mid-session. `reconnect.rs`'s existing test avoids this
  because its fake listener drops every connection quickly, keeping `run_connection` short-lived; a
  real, persistent backup acceptor (needed here to prove a genuine TLS handshake) does not. Fixed by
  dropping the `stop()`/`join()` call entirely, matching `tls.rs`'s established pattern (let the handle
  drop and rely on the test runtime's teardown). Stable across 4 repeated runs (~1.05s each).

### Implementation for User Story 1

- [X] T006 [US1] Add `connect_initiator_reconnecting_multi_tls` to
  `crates/truefix-transport/src/lib.rs` (per data-model.md's signature — same rotation/backoff loop as
  `connect_initiator_reconnecting_multi`, TLS handshake per attempt mirroring `connect_initiator_tls`)
- [X] T007 [US1] Add `Engine.failover_initiators: Vec<ReconnectHandle>` field,
  `Engine::failover_initiators() -> &[ReconnectHandle]` accessor, and extend `Engine::shutdown()` to
  `.stop()` every entry, in `crates/truefix/src/lib.rs` — additive only, `Engine.initiators`/
  `Engine::initiators()` untouched — **complete**. Also added `tracing` as a direct dependency of the
  `truefix` crate (previously absent), needed here and reused by T021 (US4).
- [X] T008 [US1] Wire `Engine::start`'s initiator branch in `crates/truefix/src/lib.rs`: when
  `rs.failover_addresses` is non-empty and no proxy is configured, route to
  `failover_initiators` via `connect_initiator_reconnecting_multi[_tls]`; when a proxy *is* configured
  alongside failover addresses, log a `tracing::warn!` (proxy+failover is out of scope per spec
  Assumptions) and use the existing proxy path, ignoring the failover addresses for that session;
  otherwise unchanged (depends on T006, T007) — **complete**: the full backup-endpoint list is
  `[rs.address] ++ rs.failover_addresses` (primary first, then backups in configured order); clippy
  clean, all pre-existing `truefix` tests (`config_start.rs`/`config_tls.rs`/`cracker.rs`/
  `examples_smoke.rs`) still green.
- [X] T009 [P] [US1] Test: a `.cfg`-only initiator with numbered backup endpoints reconnects through
  `Engine::start`, in `crates/truefix/tests/failover_engine.rs` (new file, mirroring
  `config_start.rs`'s pattern) (depends on T008) — **complete**: primary endpoint is an unbound
  ("dead") port (connection refused instantly, no fake listener needed), backup is the engine's own
  acceptor session from the same `.cfg`; also asserts `initiators()` is empty and
  `failover_initiators()` has exactly one entry, confirming the additive-not-shared routing. Stable
  across 3 repeated runs (~1.03s each); clippy/fmt clean.

**Checkpoint**: User Story 1 fully functional and independently testable — `Engine.initiators`'s
existing shape is unchanged throughout. **Reached.**

---

## Phase 3: User Story 2 — Dictionary validation wired into `Engine::start` (P1)

**Goal**: A `.cfg`-only session with `UseDataDictionary=Y` rejects a dictionary-invalid inbound message
identically to a session whose `Services.validator` was wired programmatically, with zero additional
Rust code.

**Independent Test**: Start an `Engine`-driven acceptor via `.cfg` alone; send it a dictionary-invalid
message from a real counterparty; verify the same reject a programmatically-wired validator produces.

### Tests for User Story 2

- [X] T010 [P] [US2] Test: `.cfg` → `ResolvedSession.validator` mapping — bundled version string
  (`DataDictionary=FIX.4.4`), file path, absent/`N` (→ `None`), and each of the five
  `ValidationOptions` toggles — in `crates/truefix-config/tests/validator_mapping.rs` (new file,
  mirroring `store_and_log_mapping.rs`'s pattern) — **complete**: 10 tests, all passing on first run.

### Implementation for User Story 2

- [X] T011 [US2] Add `resolve_validator(map, session) -> Result<Option<(DataDictionary,
  ValidationOptions)>, ConfigError>` and the `ResolvedSession.validator` field in
  `crates/truefix-config/src/builder.rs` (per data-model.md: version-string match against
  `truefix_dict::ALL_DICTS` falls back to `load_from_file`; reuses the five already-implemented
  `ValidationOptions` toggle resolvers) — **complete**: added `truefix-dict` as a direct dependency of
  `truefix-config` (previously absent; safe, no cycle — `truefix-session` already depends on it, and
  `truefix-config` already depends on `truefix-session`). `AppDataDictionary`/
  `TransportDataDictionary` are documented as alternate key names for one dictionary value, not a
  true FIXT dual-dictionary setup — `Services.validator` itself only ever carried a single
  `DataDictionary`, a pre-existing limitation this `.cfg` wiring doesn't change.
- [X] T012 [US2] Wire `rs.validator` into `Services.validator` for both the acceptor and initiator
  branches of `Engine::start`, in `crates/truefix/src/lib.rs` (depends on T011) — **complete**: single
  shared `services` value already covers both branches, so this was one field addition
  (`validator: rs.validator.clone()`).
- [X] T013 [P] [US2] Test: a `.cfg`-only acceptor with `UseDataDictionary=Y` rejects a
  dictionary-invalid message with the correct `SessionRejectReason`, extending
  `crates/truefix/tests/config_start.rs` (depends on T012) — **complete**: reuses the exact malformed
  input (`Side=9`) and expected outcome (`SessionRejectReason=5`, `RefSeqNum=2`) as
  `initiator_validation.rs`/`validation_hook.rs`, proving the `.cfg`-wiring half of that same
  contract. All 3 tests in `config_start.rs` pass; clippy/fmt clean.

**Checkpoint**: User Stories 1 AND 2 both independently functional. **Reached.**

---

## Phase 4: User Story 3 — SQL backend selection wired into `Engine::start` via `JdbcURL` (P2)

**Goal**: A `.cfg`-only session with `JdbcURL` set persists sequence numbers/messages to the named
PostgreSQL/MySQL/SQLite/MSSQL backend and survives a restart, with zero additional Rust code; an
unsupported scheme or uncompiled feature produces a typed error, never a panic.

**Independent Test**: Start an `Engine`-driven session via `.cfg` alone with `JdbcURL=sqlite:...`;
verify sequence numbers/message bodies persist and survive a restart.

### Tests for User Story 3

- [X] T014 [P] [US3] Test: `.cfg` → `JdbcURL`-dispatched `StoreConfig`/`ResolvedSession.sql_log`
  mapping — `postgres://`/`mysql://`/`sqlite:`/`mssql://`/`sqlserver://` schemes, an unrecognized
  scheme, and a recognized scheme whose feature isn't compiled (→ `ConfigError::UnsupportedBackend`,
  never a panic) — extending `crates/truefix-config/tests/store_and_log_mapping.rs` — **complete**: 12
  tests unconditionally, 15 with `--features sql`/`--features mssql`/`--features sql,mssql`; all
  4 feature combinations green.

### Implementation for User Story 3

- [X] T015 [US3] Add `ConfigError::UnsupportedBackend { session: String, scheme: String }` to
  `crates/truefix-config/src/lib.rs` — **complete**.
- [X] T016 [US3] Add the `JdbcURL` scheme-dispatch branch to `resolve_store` in
  `crates/truefix-config/src/builder.rs` (`StoreConfig::Sql`/`Mssql`, feature-gated;
  `FileStorePath`/memory-store resolution unchanged when `JdbcURL` absent) (depends on T015) —
  **complete, plus new cross-crate feature-plumbing**: `StoreConfig::Sql`/`Mssql` are themselves
  `#[cfg(feature = "sql"/"mssql")]`-gated in `truefix-store`, so `truefix-config` needed its own
  pass-through `sql`/`mssql` features (`sql = ["truefix-store/sql"]`, new — `truefix-config` never
  needed this before, since it previously only ever produced feature-agnostic `StoreConfig` variants
  like `File`/`Memory`). Implemented via a pair of `#[cfg(feature = "sql")]`/
  `#[cfg(not(feature = "sql"))]` function variants (`sql_store_config`/`mssql_store_config`) so an
  unsupported/uncompiled scheme always produces `ConfigError::UnsupportedBackend`, never a compile
  error or a panic, regardless of which features are active.
- [X] T017 [US3] Add the new `SqlLogSpec` type, the `ResolvedSession.sql_log: Option<SqlLogSpec>`
  field, and the matching `JdbcURL` branch in `resolve_log`, in
  `crates/truefix-config/src/builder.rs` — additive sibling to the existing, untouched
  `log: Option<LogSpec>` (File-log) field per data-model.md's corrected design; when both
  `FileLogPath` and `JdbcURL` are present, `sql_log` wins and a `tracing::warn!` notes the precedence
  (depends on T015) — **complete**: added `tracing` as a new direct dependency of `truefix-config`
  (previously absent) for the precedence warning; `resolve_log`'s signature changed from
  `fn(&Map) -> Option<LogSpec>` to `fn(&Map, &str) -> (Option<LogSpec>, Option<SqlLogSpec>)` (an
  internal, non-`pub` function — no external breaking change).
- [X] T018 [US3] Wire `rs.sql_log` into `Engine::start`'s log-building path in
  `crates/truefix/src/lib.rs`: when `Some`, construct `SqlLog`/`MssqlLog` via
  `connect_with_config` (dispatched by the same URL-scheme check as the store side) instead of the
  existing `FileLog`-only `build_log` helper (depends on T016, T017) — **complete, plus feature
  plumbing extended one layer further than T016's**: `truefix`'s own `sql`/`mssql` features now
  forward to all three of `truefix-config`, `truefix-store`, and `truefix-log`'s same-named
  features (`Engine::start` needs the whole chain: `truefix-config` to construct
  `StoreConfig::Sql`/`Mssql`, `truefix-store`'s `build_store` dispatch arm, and `truefix-log`'s
  `SqlLog`/`MssqlLog` construction). Found and fixed a real type-checking bug while writing this:
  `SessionPrefixLog<L: Log>` is generic over the *concrete* wrapped type, so it can't wrap an
  already-erased `Arc<dyn Log>` (no blanket `impl Log for Arc<dyn Log>` exists) — the first draft
  tried to wrap twice (once inside `connect_sql_log`, again in the caller), which doesn't
  type-check; fixed by doing the `SessionPrefixLog` wrap exactly once, at the point each cfg-gated
  connector still holds the concrete `SqlLog`/`MssqlLog` type. Also had to add `SqlLogSpec` to
  `truefix-config`'s `pub use builder::{...}` re-export list (present in `builder.rs` since T017,
  but not yet re-exported from the crate root — a real oversight `truefix`'s own build caught
  immediately). All 4 feature combinations (default, `sql`, `mssql`, `sql,mssql`) build and clippy
  clean; existing `truefix` test suite (`config_start.rs`/`config_tls.rs`/`cracker.rs`/
  `examples_smoke.rs`/`failover_engine.rs`) unaffected.
- [X] T019 [P] [US3] Test: a `.cfg`-only session with `JdbcURL=sqlite:...` persists and survives a
  restart through `Engine::start`, extending `crates/truefix/tests/config_start.rs` (`--features sql`)
  (depends on T018) — **complete, and found a real pre-existing (not newly introduced) gap**: the
  first draft shared one `JdbcURL`/sqlite file between the acceptor and initiator sessions (mirroring
  `config_start.rs`'s other tests' shared-`[DEFAULT]`-key style) and got `persisted == 1` instead of
  advancing — root cause: `StoreConfig::Sql`/`Mssql` carry only a `url: String`, no `session_id`, so
  `SqlStore::connect(url)` always defaults `session_id` to `"default"` — two sessions sharing one
  `JdbcURL` collide on the same store row, a limitation that predates this feature entirely (already
  true for any two direct callers of `StoreConfig::Sql { url }`) and matches
  `JdbcSessionIdDefaultPropertyValue` still being `Recognized`, not `Implemented`. Fixed the test by
  giving each session its own SQLite file (`JdbcURL` set per-`[SESSION]`, not `[DEFAULT]`) — the
  correct way to configure two sessions against a JdbcURL-selected backend today. Documented as a
  disclosed, separate, pre-existing scope boundary in the test's own doc comment (not silently
  worked around). All 4 `config_start.rs` tests pass with `--features sql`; clippy/fmt clean;
  default-feature build/tests unaffected.

**Checkpoint**: User Stories 1–3 all independently functional. **Reached.**

---

## Phase 5: User Story 4 — `ContinueInitializationOnError` (P2)

**Goal**: A multi-session `.cfg` acceptor with one misconfigured session and
`ContinueInitializationOnError=Y` starts every other, validly-configured session; the same file with
the flag unset fails startup entirely, exactly as today.

**Independent Test**: Start a multi-session `.cfg` acceptor, one session deliberately misconfigured;
verify the others start with the flag set, and verify today's fail-fast behavior is unchanged without
it.

### Tests for User Story 4

- [X] T020 [P] [US4] Test: multi-session `.cfg` acceptor, one misconfigured session, both with
  `ContinueInitializationOnError=Y` (other sessions start, failure logged) and without (whole startup
  fails, unchanged), in `crates/truefix/tests/continue_on_error.rs` (new file) — **complete**: 2 tests.

### Implementation for User Story 4

- [X] T021 [US4] Restructure `Engine::start`'s multi-session loop in `crates/truefix/src/lib.rs`:
  wrap each iteration's fallible work (store/log build, TLS/proxy config, bind/connect) in a per-
  iteration `Result`; on failure, check the failing session's own
  `rs.session.continue_initialization_on_error` — `true` logs via `tracing::error!` (naming the
  session identity and error) and `continue`s to the next session; `false`/default propagates via `?`,
  exactly as today (depends on T020 existing and failing first) — **complete, but the actual design
  is materially different from this task's original framing — a deeper architectural finding**:
  restructuring only `Engine::start`'s per-session loop, as originally planned, would never have
  worked for the scenario T020 actually tests (a `ConfigError` at *resolution* time). Grounding
  against the real code found that `SessionSettings::resolve()` is itself all-or-nothing via a
  `.collect()` over every session's `resolve_one` call, and it runs to completion *before*
  `Engine::start`'s per-session loop even begins — so a resolution failure never reaches the loop
  this task planned to restructure at all. Fixed with a two-part design instead: (1) new
  `SessionSettings::resolve_lenient() -> Result<LenientResolve, ConfigError>` in
  `crates/truefix-config/src/builder.rs`, reading `ContinueInitializationOnError` directly from each
  session's raw, pre-resolution `.cfg` map (a simple, never-failing boolean key, readable even when
  *other* keys in that same session are broken) — this is what actually tolerates T020's
  missing-`DataDictionary` scenario. (2) `Engine::start`'s loop *is* still restructured, exactly as
  planned, wrapping each session's store/log/bind/connect work in an inline `async {}.await` block
  evaluated to a `Result<(), EngineError>` — this covers the second, genuinely-in-scope failure
  class this task named (*runtime* startup failures like a port already in use), which do surface
  after resolution succeeds, inside that per-session work. A `continue;` targeting the outer loop
  (the existing failover-routing early-exit) had to become `return Ok(())` instead, since
  `continue`/`break` can't cross an `async {}` block boundary the way they can a nested `if`. Also
  wired the previously-completely-unread `ContinueInitializationOnError` key into
  `SessionConfig.continue_initialization_on_error` in `resolve_one` (a second, separate pre-existing
  `.cfg`-wiring gap this task closed alongside its main one — the field existed since feature 003 but
  no `.cfg` key ever populated it). Updated `ContinueInitializationOnError`'s stance from `Recognized`
  to `Implemented` in `crates/truefix-config/src/keys.rs` and refreshed `SessionConfig`'s own doc
  comment (previously said "has no effect", now accurate). `cargo test --workspace` green; AT suite
  unaffected.

**Checkpoint**: User Stories 1–4 all independently functional. **Reached.**

---

## Phase 6: User Story 5 — `RedbStore`/`RedbLog` (P2)

**Goal**: Sequence numbers and message bodies persist across a process restart against the same
embedded `redb` file; `reset()` atomically clears both; two session identities sharing one file stay
isolated.

**Independent Test**: Configure a session with `RedbStore`; save state; kill and reopen; verify
persistence, atomic reset, and session isolation.

### Implementation for User Story 5

- [X] T022 [US5] Implement `RedbStoreConfig`/`RedbStore` in `crates/truefix-store/src/redb.rs` (new
  file) per data-model.md: `Arc<redb::Database>`, two tables (sequence numbers keyed by `session_id`;
  messages keyed by `(session_id, seq)`), every `MessageStore` method wrapped in
  `tokio::task::spawn_blocking` (research.md §5), `reset()` atomic within one write transaction
  (depends on T001) — **complete**: 3 tables actually (`sender_seq`/`target_seq` split, not one
  composite-value table — simpler `TableDefinition<&str, u64>` × 2 instead of needing a tuple
  *value* type, only tuple *keys* for the messages table). Compiled and passed clippy's
  `unwrap_used`/`expect_used`/`panic`/`indexing_slicing` denies clean on the first real attempt.
- [X] T023 [US5] Wire `StoreConfig::Redb { path }` (new, `#[cfg(feature = "redb")]`) and the matching
  `build_store` dispatch arm in `crates/truefix-store/src/lib.rs` (depends on T022) — **complete**.
- [X] T024 [P] [US5] Test: full `MessageStore` contract + session-isolation, unconditional (no
  external service needed — `redb` is embedded, like SQLite today), in
  `crates/truefix-store/tests/redb_backend.rs` (new file, mirroring `sql_backends.rs`'s pattern)
  (depends on T023) — **complete, and found a real, load-bearing `redb` behavior difference from
  every other backend**: the first draft's session-isolation test (two independent
  `RedbStore::connect_with_config` calls against the *same file*, mirroring `MssqlStore`'s pattern
  exactly) failed with `"Database already open. Cannot acquire lock."` — unlike a networked SQL
  server, `redb::Database::create`/`open` takes an **exclusive file lock per call**; two independent
  connections to one file cannot coexist in the same process, a genuine `redb` design property, not
  a bug. Fixed by adding a new `RedbStore::with_session_id(&self, session_id) -> Self` method
  (clones the cheap `Arc<Database>`, swaps only `session_id`) as the correct way to share one file
  across sessions, and rewrote the test to use it. Also added a fourth test,
  `redb_persists_across_restart` (a fresh `connect` against the same file after the first handle
  drops), covering FR-006's restart-survival requirement more directly than the full-contract test
  alone. All 3(→4) tests stable across 3 repeated runs; clippy/fmt clean.
- [X] T025 [US5] Implement `RedbLogConfig`/`RedbLog` in `crates/truefix-log/src/redb.rs` (new file)
  per data-model.md: same `spawn_blocking` pattern, three append-only tables (incoming/outgoing/event)
  (depends on T001) — **complete**: `redb` has no identity/autoincrement column, so each table's
  next key is tracked as a plain `u64` counter in the background writer task's own memory,
  initialized from the table's current max key (`.last()`) at connect time — safe because every
  write is already serialized through that one task via the channel, no concurrent-counter race is
  possible.
- [X] T026 [P] [US5] Test: `RedbLog` persists incoming/outgoing/event entries, in
  `crates/truefix-log/tests/redb_log.rs` (new file, mirroring `sql_log.rs`'s pattern) (depends on T025)
  — **complete, and found + fixed two real test bugs**: (1) reopening the file immediately after
  `drop(log)` hit the same exclusive-file-lock issue T024 found — `drop()` returns before the
  background writer task has actually noticed its channel closed and dropped its own `Arc<Database>`
  clone, an async race. Fixed with a retry-loop (`open_with_retry`, up to 3s) instead of a fixed
  sleep guess. (2) The heartbeat-filter test initially used `|` as a human-readable field delimiter
  (matching `sql_log.rs`'s existing test style) instead of a real SOH (`\x01`) byte — `is_heartbeat`
  splits on literal SOH, so the filter never triggered on either message, silently logging both
  instead of asserting a real filtering failure; fixed by using `\x01`. 2 tests, stable across 3
  repeated runs; clippy/fmt clean.

**Checkpoint**: User Stories 1–5 all independently functional; `redb` compiles and tests pass with
`--features redb` and is entirely absent from default builds. **Reached.**

---

## Phase 7: User Story 6 — `MongoStore`/`MongoLog` (P3)

**Goal**: `MongoStore`/`MongoLog` satisfy the identical `MessageStore`/`Log` contract as every other
backend, gated on a reachable MongoDB instance, skipping cleanly (not failing) when unavailable.

**Independent Test**: Configure a session with `MongoStore` against a reachable MongoDB instance; save
state; verify retrieval/reset behave equivalently to the SQL-backed conformance tests.

### Implementation for User Story 6

- [X] T027 [US6] Implement `MongoStoreConfig`/`MongoStore` in `crates/truefix-store/src/mongo.rs` (new
  file) per data-model.md: native async `mongodb` API, `SessionDoc`/`MessageDoc` collections with a
  compound `(session_id, seq)` index created idempotently on connect; never wrap a `mongodb` future in
  `tokio::time::timeout`/`select!` (research.md §6) (depends on T001) — **complete**: added
  `serde`/`futures-util` as new optional deps of `truefix-store` alongside the pre-existing `mongodb`
  optional dep (`futures-util` confirmed already transitively present via `mongodb`'s own deps, so this
  adds nothing new to the dependency tree); `TryStreamExt::try_next()` used to drain the `find()`
  cursor. Compiled and passed clippy's `unwrap_used`/`expect_used`/`panic`/`indexing_slicing` denies
  clean on the first real attempt (API usage derived from direct crate-source inspection, not guessed).
- [X] T028 [US6] Wire `StoreConfig::Mongo { uri }` (new, `#[cfg(feature = "mongodb")]`) and the
  matching `build_store` dispatch arm in `crates/truefix-store/src/lib.rs` (depends on T027) —
  **complete**, mirrors the `Redb { path }` variant/arm exactly.
- [X] T029 [P] [US6] Test: full `MessageStore` contract + session-isolation, gated on
  `DATABASE_URL_MONGO` (skip-not-fail when unset, mirroring `mssql_backend.rs`), in
  `crates/truefix-store/tests/mongo_backend.rs` (new file) (depends on T028) — **complete**. No local
  MongoDB/Docker daemon available in this environment (`docker ps` failed: daemon not running), so both
  tests exercised only their skip path here; real assertions run in CI's `mongo` service-container job
  (already wired in an earlier task). Compiles clean, mirrors `mssql_backend.rs`'s structure exactly
  (unique collection-name suffixes per test to avoid cross-run collisions, since Mongo collections
  persist across runs unlike SQLite's temp files).
- [X] T030 [US6] Implement `MongoLogConfig`/`MongoLog` in `crates/truefix-log/src/mongo.rs` (new
  file): background-task-plus-unbounded-channel architecture matching `SqlLog`/`MssqlLog` (the `Log`
  trait is synchronous, `mongodb` isn't) (depends on T001) — **complete**. The `mongodb`/`tokio`
  optional deps and the `mongodb` feature were already present in `truefix-log/Cargo.toml` from an
  earlier setup task (T001-era scaffolding), so no Cargo.toml edit was needed here — only the new
  `src/mongo.rs` module and its `mod`/`pub use` wiring in `lib.rs`. Uses raw `mongodb::bson::Document`
  collections (`doc!{"text": ...}`) rather than a serde-derived struct, since a log entry's only
  payload is a string — no need for `truefix-log` to pull in `serde` as a new dependency the way
  `truefix-store` did. Not added to the `LogConfig` enum, matching `SqlLog`/`MssqlLog`/`RedbLog`'s
  existing precedent (none of the SQL/embedded/document-store logs are `.cfg`-selectable via
  `LogConfig`; only `Screen`/`File`/`Tracing`/`Composite` are). Compiled and passed clippy clean on the
  first real attempt.
- [X] T031 [P] [US6] Test: `MongoLog` persists incoming/outgoing/event entries, gated on
  `DATABASE_URL_MONGO`, in `crates/truefix-log/tests/mongo_log.rs` (new file) (depends on T030) —
  **complete**, mirroring `redb_log.rs`'s two-test shape (persistence + heartbeat-filtering) rather
  than `mssql_log.rs`'s single-test shape, since heartbeat filtering wasn't covered by any prior Mongo
  test. Verifies via a second, independent `mongodb::Client` connection and `count_documents`, mirroring
  `mssql_log.rs`'s "reconnect and verify" pattern. Compiled clean and skipped cleanly with no local
  MongoDB available (same environment constraint as T029) — but since that meant this test's real
  (non-skip) path had never actually executed before landing on the PR, CI's `mongo` job caught a real
  flake on first run against an actual `mongo:7` container: both tests failed with `left: 0, right: 1`
  in ~0.3s total (the fixed `sleep(300ms)` before checking counts wasn't reliably enough time for the
  background writer's *first* write against a freshly-started container — cold WiredTiger/index-creation
  latency, not a `MongoLog` correctness bug; the identical commit's `pull_request`-triggered CI run
  passed, confirming it was timing-flaky rather than deterministic). Fixed by replacing the fixed sleep
  with a `count_with_retry` poll (up to 5s, 50ms interval) before each assertion — the same lesson
  already applied in `redb_log.rs`'s `open_with_retry` for a different root cause (file-lock release
  race), applied again here for connection/write cold-start latency.

**Checkpoint**: All six user stories independently functional; `mongodb` compiles and tests pass with
`--features mongodb` (real assertions when `DATABASE_URL_MONGO`/CI's `mongo` job is available, clean
skips otherwise) and is entirely absent from default builds.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T032 [P] Update `docs/parity-matrix.md` and `docs/acceptance-record.md` with the final stance
  sweep for every config key this feature implements (FR-009) — **complete**. `parity-matrix.md`'s
  "Feature 004 — Stance Tracking Scaffold (T004)" table updated from `pending` rows to `landed`, with
  two corrections found while sweeping `keys.rs`: `JdbcURL`/`JdbcLog*` flip `Rec` → `Impl` (US3), and
  `SocketConnectHost1`/`SocketConnectPort1` (already `Impl` since feature 002 but never actually
  reconnected-through until US1) get a documentation-only note making the existing marking accurate —
  no stance change needed there since it was already `Impl`. `acceptance-record.md` gets a new "004"
  section (see below).
- [X] T033 [P] Tick off GAP-01 through GAP-06 in `docs/todo-gap-analysis.md`'s 2026-07-02 gap section
  — **complete**. All 6 rows struck through with "已完成" + a one-line summary and cross-reference to
  `specs/004-engine-wiring-extra-backends/tasks.md`'s task range; also updated the two duplicate
  mentions in the QF/Go-only and QF/J-only feature lists (MongoDB, SleepycatStore/`redb`) and added a
  clarifying note to GAP-21 (log `.cfg` selection) that GAP-05 partially narrowed its scope (`SqlLog`
  now `.cfg`-selectable via `JdbcURL`; `ScreenLog`/`TracingLog`/`CompositeLog` still aren't).
- [X] T034 [P] Update `README.md`: `.cfg`-only failover/dictionary-validation/SQL-backend-selection,
  `ContinueInitializationOnError`, and the two new store backends (`redb`, `mongodb`) — **complete**.
  Quick-start's config-only-features paragraph extended; architecture table's `truefix-store`/
  `truefix-log` rows gain `Redb`/`Mongo` entries plus a note that both are library-level only (no
  `.cfg` key); new "004" roadmap block; Testing/Building sections gain the `redb`/`mongo` CI jobs and
  `cargo test --features redb`/`mongodb` invocations.
- [X] T035 Run `quickstart.md` validation end-to-end (all per-user-story command blocks), fixing any
  command that doesn't match a real test (per the lesson quickstart.md itself already documents from
  003's Polish phase) — **complete, and found one real quickstart bug**: US1's block ran
  `cargo test -p truefix --test config_start` as its second command, but the actual US1 end-to-end test
  (`initiator_reconnects_through_a_cfg_backup_endpoint_after_the_primary_is_unreachable`) lives in a
  separate file, `crates/truefix/tests/failover_engine.rs`, added during T009 — `config_start.rs` never
  exercises `failover_addresses` at all, so this command was validating US2/US3's tests, not US1's, a
  silent scope drift (not a false-green like 003's substring-filter bug, but the same root failure mode
  of "the command doesn't test what the section claims"). Fixed by pointing US1's block at
  `--test failover_engine`. All other command blocks (US2–US6, Full gate) verified to match real,
  passing tests exactly as written — no further discrepancies found.
- [X] T036 Full-gate sweep: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings` (default, `--features redb`, `--features mongodb`), `cargo test --workspace`,
  `cargo test -p truefix-store -p truefix-log --features redb`,
  `cargo test -p truefix-store -p truefix-log --features mongodb`,
  `cargo test -p truefix-at --test conformance` (confirm scenario-run count unchanged from this
  feature's baseline — SC-006/FR-010, no new AT scenarios), `cargo deny check` — **complete**. `cargo
  fmt --all --check` initially failed on the never-yet-formatted `mongo.rs` files (US6's newest code,
  written in this same session); `cargo fmt --all` fixed both, re-check clean. Clippy clean across all
  3 combos (default; `--features truefix-store/redb,truefix-log/redb`;
  `--features truefix-store/mongodb,truefix-log/mongodb`). `cargo test --workspace`: 351 tests passing,
  0 failed (default features). `--features redb`/`--features mongodb` store+log test runs: all green,
  including the new `mongo_backend.rs`/`mongo_log.rs` tests (skip-clean, no local Docker/MongoDB in this
  environment — real assertions run in CI's `mongo` service-container job). `cargo test -p truefix-at
  --test conformance`: 4/4 suites green; `cargo test -p truefix-at --test coverage` additionally
  confirms `server_suite_scenario_run_count_does_not_regress` and all-9-versions coverage — the
  353/353-scenario baseline is unmodified, satisfying FR-010's release gate. `cargo deny check`:
  `advisories ok, bans ok, licenses ok, sources ok` (only informational `duplicate` warnings for
  multiple crate-graph versions, not errors — pre-existing pattern, not new from this feature).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **User Stories (Phases 2–7)**: All depend only on Setup. **No dependency on each other** — US1–US6
  touch entirely disjoint files (`truefix-transport`+`truefix`'s initiator path for US1;
  `truefix-config`+`truefix` for US2/US3/US4, but distinct functions/fields within those files;
  `truefix-store`/`truefix-log`'s new `redb.rs`/`mongo.rs` modules for US5/US6) and can proceed in any
  order or fully in parallel.
- **Polish (Phase 8)**: Depends on all six user stories being complete.

### Within Each User Story

- Tests are written first and MUST fail before implementation (Principle V).
- Within US3, T015 (new `ConfigError` variant) blocks T016/T017 (both use it); T016/T017 block T018
  (`Engine::start` wiring); all block T019 (end-to-end test).
- Within US1, T006/T007 (independent, different files) both block T008 (which needs both); T008 blocks
  T009.
- Within US5/US6, the store half (T022–T024 / T027–T029) and the log half (T025–T026 / T030–T031) are
  independent of each other and can proceed in parallel.

### Parallel Opportunities

- All Phase 1 (Setup) tasks marked `[P]`.
- All 6 user story phases can proceed in parallel once Setup completes (fully independent — see
  above).
- Within US1: T005 (test) and T006/T007 (implementation, different files) can start in parallel.
- Within US5/US6: the `RedbStore`/`RedbLog` halves (and `MongoStore`/`MongoLog` halves) are
  independent of each other.
- All Phase 8 (Polish) doc-update tasks marked `[P]`.

---

## Parallel Example: User Story 5

```bash
# Launch the RedbStore and RedbLog implementation tracks together (different files, no shared state):
Task: "Implement RedbStoreConfig/RedbStore in crates/truefix-store/src/redb.rs"
Task: "Implement RedbLogConfig/RedbLog in crates/truefix-log/src/redb.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 + User Story 2 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2 (US1, initiator failover) and Phase 3 (US2, dictionary wiring) — both P1, both
   closing the most visible "config-driven start" gaps.
3. **STOP and VALIDATE**: run `quickstart.md`'s US1/US2 blocks independently.

### Incremental Delivery

1. Setup → US1 → US2 → US3 → US4 → US5 → US6, each independently tested and mergeable — priority
   order matches spec.md exactly, but any order is valid since none depend on each other.
2. Polish once all six are done.

### Parallel Team Strategy

With multiple contributors: after Setup, assign one story per contributor — none share files at the
level a merge conflict would be likely (US1: `truefix-transport` + `truefix`'s initiator branch; US2/
US3/US4: `truefix-config`'s `builder.rs`/`lib.rs` + `truefix`'s log/validator/loop wiring, in distinct
functions; US5/US6: `truefix-store`/`truefix-log`'s new sibling modules).

---

## Notes

- `[P]` tasks = different files (or, within US3/US4's single-file `Engine::start` edits, clearly
  disjoint code regions), no dependencies.
- `[Story]` label maps task to specific user story for traceability.
- No AT suite tasks — this feature adds no protocol behavior (FR-010); the AT suite's continued
  green-and-unmodified state is verified once, in Phase 8's full-gate sweep, not per-story (there is
  nothing per-story to verify against it).
- Commit after each task or logical group, following this repository's established convention of a
  transparent, disclosed record of design corrections (see research.md §3's `SqlLogSpec` correction and
  data-model.md's matching note) rather than silently absorbing them.
