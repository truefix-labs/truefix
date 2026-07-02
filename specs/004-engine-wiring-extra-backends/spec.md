# Feature Specification: Engine Config-Wiring Completion + Additional Store Backends

**Feature Branch**: `004-engine-wiring-extra-backends`

**Created**: 2026-07-02

**Status**: Draft

**Input**: User description: "根据以上的列表实现这一轮的功能" — scoped by user decision to the six gap
items (GAP-01 through GAP-06) recorded in `docs/todo-gap-analysis.md`'s "2026-07-02 全量代码对比 —
剩余功能差距" section, after removing the Oracle item (already a final, documented deferral, not an
open gap) and the standalone perf/stress-test-suite item (tooling investment, deprioritized), and
adding a concrete `RedbStore`/`RedbLog` design as the modern replacement for QuickFIX/J's
`SleepycatStore`.

## Context

Features `001`–`003` brought TrueFix to full behavioral parity with QuickFIX/J's protocol engine,
gated by a 353/353-scenario AT suite. A fresh code-vs-code comparison on 2026-07-02 (cross-referencing
`thrdpty/quickfixj` and `thrdpty/quickfix` against the current `truefix-*` crates) found that the
*capabilities* audited in earlier features are essentially complete, but found six remaining gaps that
are **not** protocol-correctness defects — they are either (a) capabilities implemented at the Rust-API
level but not reachable purely from a `.cfg` file, or (b) optional storage backends TrueFix doesn't yet
offer:

- **`.cfg`-reachability gaps** (GAP-01, GAP-02, GAP-05): `Engine::start`'s dictionary/validator wiring,
  initiator failover, and SQL-backend selection all work correctly when called from Rust, but a
  `.cfg`-only deployment — the headline capability feature 002's US1 introduced — cannot reach them
  without writing additional Rust code. This directly undercuts the "config-driven start" value
  proposition for exactly the settings groups (`validation`, `initiator` numbered endpoints, `sql`)
  that already have recognized-but-inert Appendix A keys.
- **Multi-session startup robustness gap** (GAP-06): `ContinueInitializationOnError` remains
  `Recognized`-but-inert; a single misconfigured session in a multi-session acceptor currently aborts
  the whole engine's startup rather than optionally skipping just that session.
- **Additional storage backends** (GAP-03, GAP-04): QuickFIX/Go's MongoDB backend and a modern,
  license-clean replacement for QuickFIX/J's `SleepycatStore` (Berkeley DB JE — obsolete technology,
  not worth porting directly) are not yet offered. Feature 003's own spec explicitly deferred MongoDB
  as out of scope; this feature reverses that deferral by user decision, disclosed here rather than
  silently reinstated.

None of the six items touch the session state machine, wire codec, or AT-scenario-tested protocol
behavior — the AT suite (353/353) and all existing conformance tests are expected to remain unaffected
throughout.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Start a failover-capable initiator purely from `.cfg` (Priority: P1)

An operator configures an initiator session with primary and backup connection endpoints
(`SocketConnectHost`/`Port` plus numbered `SocketConnectHost1`/`Port1`, etc.) in a `.cfg` file and
starts the engine via `Engine::start`. Today, `truefix-transport` already has a fully-implemented,
tested multi-endpoint reconnecting connector (`connect_initiator_reconnecting_multi`), but
`Engine::start`'s initiator path calls a one-shot, non-reconnecting connector instead, so the
already-parsed `failover_addresses` are silently unused.

**Why this priority**: The underlying capability is already built and tested — this is a pure wiring
gap with no new logic to write, the lowest-cost, highest-value item in this feature.

**Independent Test**: Start an `Engine`-driven initiator via `.cfg` with one primary and one backup
endpoint; kill the primary; verify the initiator reconnects to the backup endpoint without any
additional Rust code beyond `Application` callbacks.

**Acceptance Scenarios**:

1. **Given** a `.cfg` initiator session with only `SocketConnectHost`/`SocketConnectPort` set (no
   numbered endpoints), **When** `Engine::start` runs, **Then** behavior is unchanged from today (a
   plain, non-multi-endpoint connection) — no regression for the common case.
2. **Given** a `.cfg` initiator session with `SocketConnectHost1`/`SocketConnectPort1` (and further
   numbered endpoints) present, **When** `Engine::start` runs and the primary endpoint becomes
   unreachable, **Then** the engine reconnects through the configured backup endpoint(s), matching the
   behavior `truefix-transport`'s existing `connect_initiator_reconnecting_multi` test already proves
   at the Rust-API level.

---

### User Story 2 - Validate inbound messages against a dictionary declared purely in `.cfg` (Priority: P1)

An operator sets `UseDataDictionary`, `DataDictionary`/`AppDataDictionary`/`TransportDataDictionary`,
and any of the five already-implemented `ValidationOptions` toggles (`ValidateFieldsOutOfOrder`,
`ValidateChecksum`, `ValidateIncomingMessage`, `AllowPosDup`, `RequiresOrigSendingTime`) in a `.cfg`
file. Today none of this reaches `Engine::start`: `builder.rs` has no dictionary/validator resolution
function at all, so a `.cfg`-only deployment gets zero inbound dictionary validation — an operator must
additionally write Rust code constructing `Services.validator` by hand, exactly as the AT test harness
already does internally.

**Why this priority**: The highest-value gap — a `.cfg`-only deployment of an engine advertised as
config-driven currently cannot validate inbound messages at all without extra code, which is a visible,
easily-hit surprise for a new integrator.

**Independent Test**: Start an `Engine`-driven acceptor via `.cfg` alone (`UseDataDictionary=Y`,
`DataDictionary=<path-or-bundled-version>`), send it a dictionary-invalid message from a real
counterparty, and verify it produces the same reject the AT harness's programmatically-wired validator
produces for the identical input — with no `Services.validator` code written by the operator.

**Acceptance Scenarios**:

1. **Given** a `.cfg` session with `UseDataDictionary=Y` and a `DataDictionary` value naming one of the
   bundled normalized dictionaries (or a file path), **When** `Engine::start` runs, **Then** the
   resolved session's `Services.validator` is populated automatically, with no additional Rust code.
2. **Given** the same `.cfg` session also sets one or more of the five implemented `ValidationOptions`
   toggles, **When** the engine validates an inbound message, **Then** the toggle's behavior matches
   the already-tested `ValidationOptions` semantics exactly (no new validation logic — this story is
   `.cfg`-plumbing only).
3. **Given** `UseDataDictionary` is absent or `N` (today's default), **When** `Engine::start` runs,
   **Then** behavior is unchanged — no dictionary validation is wired, exactly as today.

---

### User Story 3 - Select a SQL store/log backend purely from `.cfg` (Priority: P2)

An operator sets `JdbcURL` (and the existing `Jdbc*` table-name/pool keys) in a `.cfg` file, expecting
the engine to persist sequence numbers and sent messages to the named database. Today, `SqlStore`/
`SqlLog`/`MssqlStore`/`MssqlLog` are fully implemented and conformance-tested against PostgreSQL/
MySQL/SQLite/MSSQL, but `builder.rs`'s `resolve_store`/`resolve_log` only understand `FileStorePath`/
`FileLogPath` — a `.cfg` file can select the file-backed store but not any SQL backend without
programmatic `StoreConfig`/`LogConfig` construction.

**Why this priority**: Same "capability exists, `.cfg` can't reach it" shape as User Story 2, but for a
narrower, more operationally-specialized audience (operators who already run one of the supported
databases) — valuable, but a smaller slice of the deployed base than dictionary validation.

**Independent Test**: Start an `Engine`-driven session via `.cfg` alone with `JdbcURL` set to a
reachable PostgreSQL/MySQL/SQLite/MSSQL instance; verify sequence numbers and sent-message bodies
persist and survive a restart, matching the existing Rust-API-level SQL store conformance tests.

**Acceptance Scenarios**:

1. **Given** a `.cfg` session with `JdbcURL=postgres://...` (or `mysql://`, `sqlite:`, `mssql://`/
   `sqlserver://`), **When** `Engine::start` runs, **Then** the resolved session's store/log use the
   matching backend (`SqlStore`/`SqlLog` for the `sql`-feature schemes, `MssqlStore`/`MssqlLog` for
   `mssql://`/`sqlserver://`), dispatched by URL scheme — no `JdbcDriver` Java-driver-class value is
   required or interpreted (see Assumptions).
2. **Given** `JdbcURL` is absent, **When** `Engine::start` runs, **Then** the existing
   `FileStorePath`/`FileLogPath`-or-memory-store resolution behaves exactly as today — no regression.
3. **Given** `JdbcURL` names a backend whose feature (`sql`/`mssql`) isn't compiled in, **When**
   `Engine::start` runs, **Then** it produces a typed `ConfigError` naming the missing feature, not a
   panic or a silent fallback to the memory store.

---

### User Story 4 - Continue starting other sessions when one session's configuration fails (Priority: P2)

An operator runs a multi-session acceptor from a single `.cfg` file. One session block has an invalid
or unreachable configuration value (e.g. a bad dictionary path). Today, `ContinueInitializationOnError`
is recognized but has no effect — `SessionConfig` round-trips the flag but nothing reads it, and
`Engine::start`'s current behavior on any session's resolution failure is to abort the whole engine's
startup, taking down every other, correctly-configured session in the same file.

**Why this priority**: A real operational-robustness gap for the acceptor-first, multi-session use case
this project prioritizes, but it's an edge case (a config mistake), not a hot path — lower priority
than the `.cfg`-reachability items that affect every deployment.

**Independent Test**: Start a multi-session `.cfg` acceptor with `ContinueInitializationOnError=Y`, one
session block deliberately misconfigured; verify the other, valid sessions still start successfully and
the misconfigured one is skipped with a logged reason. Repeat with the flag unset (default) and verify
today's fail-fast behavior is unchanged.

**Acceptance Scenarios**:

1. **Given** a multi-session `.cfg` file with `ContinueInitializationOnError=Y` at the session level
   that's misconfigured, **When** `Engine::start` runs, **Then** the other, validly-configured sessions
   start successfully and the misconfigured session is skipped (not started), with the failure reason
   surfaced (e.g. via `Log::on_event` or the `Engine::start` result).
2. **Given** the same `.cfg` file but `ContinueInitializationOnError` absent or `N` (today's default),
   **When** `Engine::start` runs, **Then** the whole startup fails immediately on the first
   misconfigured session, exactly as today — no regression.
3. **Given** a single-session `.cfg` file, **When** its one session is misconfigured, **Then**
   `ContinueInitializationOnError` has no observable effect either way — startup fails, since there is
   no "other session" to continue with.

---

### User Story 5 - Persist sequence numbers and messages to an embedded transactional store (Priority: P2)

An operator wants durable, crash-safe persistence without running a separate database server — the
role QuickFIX/J's `SleepycatStore` (Berkeley DB JE) fills, but Berkeley DB JE itself is obsolete
technology not worth porting to Rust. TrueFix's existing `FileStore`/`CachedFileStore` already cover
basic embedded persistence, but offer no atomic multi-key transaction guarantee across a sequence
number update and a message-body write; an operator migrating from `SleepycatStore` who specifically
wants that guarantee has no equivalent option today.

**Why this priority**: A genuinely optional, additive backend (existing embedded stores already cover
the base need) aimed at a specific migration audience, not a correctness or `.cfg`-reachability gap —
appropriately lower priority than User Stories 1–4.

**Independent Test**: Configure a session with the new embedded transactional store; save sequence
numbers and message bodies; kill the process; reopen the same store file and verify all state survived
and a reset atomically clears both sequence numbers and message bodies in one transaction.

**Acceptance Scenarios**:

1. **Given** a fresh embedded transactional store, **When** sequence numbers are set and message bodies
   are saved, **Then** both survive a process restart against the same underlying file.
2. **Given** the store has persisted state, **When** `reset()` is called, **Then** sequence numbers and
   all persisted message bodies are cleared together, atomically (a crash mid-reset cannot leave the
   store in a state with cleared sequence numbers but stale message bodies, or vice versa).
3. **Given** two distinct session identities sharing the same store file (as `SqlStore`/`MssqlStore`
   already support via a `session_id` discriminator), **When** each writes independently, **Then**
   neither sees the other's data — the same session-isolation contract the SQL backends already satisfy.
4. **Given** this backend is off by default (its own independent feature, uncompiled unless requested),
   **When** the default `cargo build`/`cargo test --workspace` runs, **Then** nothing about it is
   compiled in or required.

---

### User Story 6 - Persist sequence numbers and messages to MongoDB (Priority: P3)

An operator whose infrastructure already standardizes on MongoDB wants `MessageStore`/`Log` backed by
it, matching the option QuickFIX/Go offers (`go.mongodb.org`-based `MongoStore`/`MongoLog`) that
QuickFIX/J does not.

**Why this priority**: The most speculative-demand, largest-new-dependency-surface item in this
feature (a full NoSQL driver, a different persistence/query model than every other backend) — valuable
for a specific class of operator, but not proven-necessary the way the `.cfg`-reachability items are.

**Independent Test**: Configure a session with a MongoDB-backed store/log against a reachable MongoDB
instance; save sequence numbers and message bodies; verify retrieval and reset behave equivalently to
the existing SQL-backed store/log conformance tests.

**Acceptance Scenarios**:

1. **Given** a reachable MongoDB instance, **When** a session is configured with the MongoDB store,
   **Then** it satisfies the exact same `MessageStore` contract (sequence get/set, save/get range,
   reset) as `SqlStore`/`MssqlStore`/the new embedded store from User Story 5, verified by the same
   conformance-test pattern.
2. **Given** no `DATABASE_URL_MONGO`-style environment variable/reachable instance, **When** the
   conformance test suite runs, **Then** the MongoDB-specific tests skip cleanly (matching the
   PostgreSQL/MySQL/MSSQL precedent) rather than failing the build.
3. **Given** this backend is off by default (its own independent feature), **When** the default
   `cargo build`/`cargo test --workspace` runs, **Then** nothing about it is compiled in or required.

---

### Edge Cases

- What happens when a `.cfg` file sets `JdbcURL` to an unparseable value (not a valid URL for any known
  scheme)? → A typed `ConfigError` naming the session and the value, not a panic (User Story 3).
- What happens when `DataDictionary` in `.cfg` names neither a bundled version string nor an existing
  file path? → A typed `ConfigError`, not a panic or a silently-unvalidated session (User Story 2).
- What happens when a numbered failover endpoint (`SocketConnectHost2`) is present but its paired port
  (`SocketConnectPort2`) is missing, or numbering skips a value (`...Host1`, `...Host3`, no `...Host2`)?
  → Unchanged from existing `resolve_failover_addresses` behavior (already handled/tested at the
  `truefix-config` level) — User Story 1 only adds the missing `Engine::start` wiring, it doesn't change
  endpoint-parsing semantics.
- What happens when `ContinueInitializationOnError=Y` but *every* session in the file is misconfigured?
  → `Engine::start` starts zero sessions and reports that no sessions started, rather than silently
  succeeding with nothing running.
- What happens when both the embedded transactional store (User Story 5) and a SQL backend are
  configured for the same session? → Not applicable — `StoreConfig`/`LogConfig` remain single-variant
  enums; a session has exactly one store, matching the existing PG/MySQL/SQLite/MSSQL precedent.

## Requirements *(mandatory)*

### Functional Requirements — `.cfg`-Reachability Completion (P1)

- **FR-001**: `Engine::start`'s initiator path MUST use the multi-endpoint reconnecting connector when
  the resolved session has one or more failover addresses, and MUST preserve today's plain-connect
  behavior when it has none.
- **FR-002**: `truefix-config::builder.rs` MUST resolve `UseDataDictionary`/`DataDictionary`/
  `AppDataDictionary`/`TransportDataDictionary` and the five already-implemented `ValidationOptions`
  fields into a `Services.validator` value, wired automatically by `Engine::start` with no additional
  Rust code required.

### Functional Requirements — `.cfg`-Reachability Completion (P2)

- **FR-003**: `truefix-config::builder.rs` MUST resolve `JdbcURL` into the matching store backend
  (`SqlStore` for `postgres://`/`mysql://`/`sqlite:` schemes when the `sql` feature is compiled in;
  `MssqlStore` for `mssql://`/`sqlserver://` when the `mssql` feature is compiled in) and the matching
  log backend, dispatched by URL scheme, and MUST preserve today's `FileStorePath`/`FileLogPath`/
  memory-store resolution when `JdbcURL` is absent. (`/speckit-plan`'s data model may realize this via
  a new field alongside — not a replacement of — the existing file-log resolution path, if that proves
  the more additive design once grounded in the actual `builder.rs`/`Engine::start` code.)
- **FR-004**: Selecting a `JdbcURL` scheme whose backing feature isn't compiled in MUST produce a typed
  `ConfigError`, not a panic or a silent fallback.
- **FR-005**: `Engine::start`'s multi-session bring-up MUST support `ContinueInitializationOnError`:
  when a session block is `Y`, a resolution/connection failure for that session MUST NOT prevent other
  sessions in the same `.cfg` file from starting; the default (`N`/absent) MUST preserve today's
  fail-fast-on-first-error behavior.

### Functional Requirements — Additional Store Backends (P2/P3)

- **FR-006**: `truefix-store`/`truefix-log` MUST add an embedded, transactional `MessageStore`/`Log`
  implementation (User Story 5) satisfying the identical trait contracts and session-isolation
  guarantee as the existing SQL backends, behind its own independent, off-by-default feature.
- **FR-007**: The embedded transactional store's `reset()` MUST clear sequence numbers and all
  persisted message bodies within a single atomic transaction.
- **FR-008**: `truefix-store`/`truefix-log` MUST add a MongoDB-backed `MessageStore`/`Log`
  implementation (User Story 6) satisfying the identical trait contracts, behind its own independent,
  off-by-default feature, with conformance tests gated on a reachable MongoDB instance (skip-not-fail
  when unavailable), matching the PostgreSQL/MySQL/MSSQL precedent.

### Functional Requirements — Cross-Cutting

- **FR-009**: Every configuration key/toggle this feature moves from `Recognized`/absent to
  `Implemented` (at minimum `ContinueInitializationOnError`, plus any new backend-selector keys) MUST
  update its registered stance in the Appendix A coverage record (`crates/truefix-config/src/keys.rs`,
  `docs/parity-matrix.md`).
- **FR-010**: None of FR-001–FR-008 may change session-state-machine, codec, or protocol-level
  behavior; the existing AT suite (353/353 scenario runs) and all existing conformance tests MUST
  remain green and unmodified in their assertions throughout this feature.

### Key Entities

- **Embedded transactional store config** (User Story 5): a config type analogous to
  `SqlStoreConfig`/`MssqlStoreConfig` — a file path, table-name-equivalent identifiers, and a
  `session_id` discriminator.
- **MongoDB store config** (User Story 6): a config type analogous to the above — a connection URI,
  collection-name-equivalent identifiers, and a `session_id` discriminator.
- **`JdbcURL`-resolved backend selection** (User Story 3): not a new persisted entity, but a new
  resolution path from an existing config key to an existing `StoreConfig`/`LogConfig` variant.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A `.cfg`-only initiator with numbered backup endpoints reconnects through a backup
  endpoint after the primary fails, with zero additional Rust code beyond `Application` callbacks.
- **SC-002**: A `.cfg`-only acceptor or initiator with `UseDataDictionary=Y` rejects a
  dictionary-invalid inbound message identically to the same message sent to a session whose validator
  was wired programmatically today — zero additional Rust code required.
- **SC-003**: A `.cfg`-only session with `JdbcURL` set persists sequence numbers/messages to the named
  PostgreSQL/MySQL/SQLite/MSSQL backend and survives a restart, with zero additional Rust code.
- **SC-004**: A multi-session `.cfg` acceptor with `ContinueInitializationOnError=Y` and one
  misconfigured session starts every other, validly-configured session; the same file with the flag
  unset fails startup entirely, exactly as before this feature.
- **SC-005**: The new embedded transactional store and the new MongoDB store each pass the identical
  `MessageStore`/`Log` conformance-test pattern (full contract + session isolation) already used for
  `SqlStore`/`MssqlStore`.
- **SC-006**: The full workspace gate (format, lint with warnings-as-errors, all tests) and the AT suite
  (353/353 scenario runs, unmodified) remain green after every stage of this feature, across default
  features and every new feature flag this feature introduces.
- **SC-007**: Every configuration key/toggle implemented by this feature is recorded as `Implemented`
  in the Appendix A coverage record.

## Assumptions

- **Additive, not replacing**: All `.cfg`-reachability work (FR-001–FR-005) is purely additive —
  existing programmatic construction of `Services.validator`/`StoreConfig`/`LogConfig` (as the AT
  harness and any existing integrator code already does) continues to work unchanged; `.cfg` resolution
  is a new, optional path to the same configuration surface, not a replacement for it.
- **`JdbcURL` dispatch is scheme-based, not driver-class-based**: QuickFIX/J's `JdbcDriver` key names a
  Java JDBC driver class (e.g. `org.postgresql.Driver`), which has no Rust equivalent. This feature
  dispatches purely from the URL scheme already present in `JdbcURL` (`postgres://`, `mysql://`,
  `sqlite:`, `mssql://`/`sqlserver://`) — the same scheme-sniffing `SqlStore::connect`/
  `MssqlStore::connect` already do internally. `JdbcDriver` itself remains `Recognized` (parsed, unused)
  — it carries no information this dispatch needs.
- **`ContinueInitializationOnError` scope**: Applies only to `Engine::start`'s own multi-session
  bring-up loop (a session block's config fails to resolve, or its initial connection/bind fails at
  startup). It does not apply to a session failing *after* successful startup (e.g. a later reconnect
  failure), which is unrelated, already-handled behavior.
- **Embedded transactional store driver**: `redb` (MIT OR Apache-2.0, actively maintained, currently
  4.1.0) rather than `sled` (same dual license, but its 1.0 rewrite has been in alpha for years with a
  stale stable branch — a materially higher maintenance-risk pick for a production-grade dependency).
  License verified compatible with `deny.toml`'s allow-list; no `deny.toml` changes expected (unlike
  `tiberius`, `redb` is pure Rust with no transitive advisory-flagged dependencies known at spec time —
  `/speckit-plan`'s dependency audit re-confirms this before any code lands, per Constitution Principle
  III).
- **MongoDB driver**: the official `mongodb` crate (Apache-2.0), matching QuickFIX/Go's own choice of
  the official MongoDB driver for its equivalent feature. License compatible with `deny.toml`'s
  allow-list; `/speckit-plan`'s dependency audit confirms before any code lands.
- **Both new backends are off-by-default, independent features**: mirroring the `mssql` feature
  precedent from feature 003 (not folded into the existing `sql` feature, which is sqlx-specific), each
  new backend gets its own Cargo feature, uncompiled and non-required by default builds.
- **Conformance-test gating**: like the PostgreSQL/MySQL/MSSQL precedent, both new backends' conformance
  tests are gated on a reachable service instance (`DATABASE_URL_MONGO`-style env var for MongoDB; the
  embedded transactional store needs no external service and can run unconditionally, like SQLite does
  today) and skip cleanly rather than failing the build when unavailable.
- **No protocol/session-state-machine changes**: every item in this feature is either a `.cfg`-to-
  existing-Rust-API wiring gap or a new, independent, off-by-default storage backend. None require or
  produce any change to session behavior, codec, or the AT scenario corpus.
- **Delivery**: One pass, internally staged with per-stage checkpoints (mirroring 001–003); the existing
  AT suite (353/353) and full workspace gate serve as the release gate — they must stay green
  throughout, not gain new scenarios (this feature adds no protocol behavior for the AT suite to cover).

## Out of Scope (Deferred)

- **Oracle SQL backend** — a final, already-documented deferral from feature 003 (Oracle Instant
  Client's closed-source OTN license conflicts with Principle III), not reopened by this feature.
- **Standalone performance/stress-test suite** (QuickFIX/J's `perf-test`/`stress-test` modules) —
  tooling investment with no correctness or `.cfg`-reachability value, deprioritized out of this round.
- Everything already listed as out of scope in feature 003's spec (QuickFIX/Go-only extras such as
  `DynamicQualifier`/`ConnectionValidator`/`HeartBtIntOverride`; JMX/OSGi/SLF4J/reflection-based
  cracking/Sleepycat-as-a-direct-port — all superseded by existing TrueFix mechanisms or inapplicable
  to Rust) remains out of scope here, **except** MongoDB (User Story 6), whose feature-003 deferral this
  feature explicitly reverses by user decision.

## Dependencies

- Builds on features `001-fix-engine-parity`, `002-qfj-parity-completion`, and
  `003-qfj-full-parity-closure`, and the constitution (`.specify/memory/constitution.md`, v2.0.0).
- FR-006/FR-008 depend on `/speckit-plan`'s dependency-license audit (Principle III) for `redb` and
  `mongodb` clearing cleanly, per the Assumptions above.
