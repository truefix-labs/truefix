# Parity Traceability Matrix

Maps the spec parity baselines to where they are realized and verified (T096; parity.md
CHK033/CHK036). This is the human-readable companion to the machine-checked registries
(`truefix_config::APPENDIX_A_KEYS`, the AT scenario list).

## Appendix A — configuration keys → owning crate

Every key is enumerated with a stance in `crates/truefix-config/src/keys.rs`
(`APPENDIX_A_KEYS`), and `tests/key_coverage.rs` asserts none is silently unrecognized (SC-004).

| Appendix A group | Owning crate(s) | Notes |
|------------------|-----------------|-------|
| identity, session behavior, validation toggles | `truefix-session`, `truefix-dict` | session config + dictionary validation |
| scheduling (StartTime/EndTime/Weekdays/TimeZone/NonStop) | `truefix-session` (`schedule`) | `Schedule::is_in_session` |
| acceptor / dynamic, initiator, socket, SSL/TLS | `truefix-transport` | routing, dynamic sessions, allow-list, rustls, `SocketOptions` |
| proxy | — | documented unsupported (not implemented) |
| file store, SQL store | `truefix-store` | SQL behind the `sql` feature |
| file/screen/facade log | `truefix-log` | tracing facade; SLF4J-specific keys documented unsupported |
| Sleepycat/JE | — | documented unsupported (deferred v1) |

Stances: **Implemented** (honored), **Recognized** (parsed; behavior partial/pending), or
**Unsupported(reason)**. See the registry for per-key detail.

## Appendix B — AT scenarios → fixtures

The AT runner (`truefix-at`) drives a real acceptor as a black box. Authored scenarios live in
`crates/truefix-at/src/scenarios.rs`; the conformance test (`tests/conformance.rs`) runs the matrix
across the target versions and is the CI gate.

| Scenario (behaviour) | Versions | Status |
|----------------------|----------|--------|
| 1a ValidLogonWithCorrectMsgSeqNum | 4.2, 4.4 | ✅ |
| 1a ValidLogonMsgSeqNumTooHigh (→ ResendRequest) | 4.2, 4.4 | ✅ |
| 2b MsgSeqNumTooHigh (→ ResendRequest) | 4.2, 4.4 | ✅ |
| 2c MsgSeqNumTooLow (→ Logout + disconnect) | 4.2, 4.4 | ✅ |
| 4b ReceivedTestRequest (→ Heartbeat echo) | 4.2, 4.4 | ✅ |
| 13b UnsolicitedLogoutMessage (→ Logout + disconnect) | 4.2, 4.4 | ✅ |
| Remaining 67 server scenarios + special-category suites | all | ⏳ to port onto the runner |

## Codec reference vectors

`crates/truefix-core/tests/fixtures/reference/` holds QuickFIX/J-sourced wire vectors,
cross-validated in `tests/reference_vectors.rs` (byte-exact BodyLength/CheckSum).

## Feature 002 — dependency license audit (T003, Constitution III)

New/newly-exercised dependencies for the parity-completion work, verified Apache-2.0 OR
MIT-compatible (no copyleft introduced); enforced in CI by the `deny` job (`deny.toml`):

| Dependency | Use (002) | License |
|------------|-----------|---------|
| `rustls-pki-types` (`pem::PemObject`) | load TLS key/cert/CA from `.cfg` paths (US7) | MIT OR Apache-2.0 |
| `sqlx-postgres` | SQL store/log Postgres backend (US12) | MIT OR Apache-2.0 |
| `sqlx-mysql` | SQL store/log MySQL backend (US12) | MIT OR Apache-2.0 |
| `metrics` | observability facade export (US9) | MIT |

All within the existing Apache-2.0 OR MIT release posture; `cargo deny check` gates regressions.

`rustls-pemfile` (originally used for US7's PEM loading) was replaced by `rustls-pki-types`'s
built-in `pem::PemObject` trait after `cargo deny`'s CI job flagged it as unmaintained
(RUSTSEC-2025-0134 — the crate's repository was archived; its own advisory recommends this exact
migration, since recent `rustls-pemfile` versions are already a thin wrapper over the same
`rustls-pki-types` PEM code). `rustls-pki-types` was already a transitive dependency via `rustls`;
`truefix-transport` now depends on it directly with the `std` feature enabled, and
`load_certs`/`load_private_key` in `tls_config.rs` call `CertificateDer::pem_file_iter`/
`PrivateKeyDer::from_pem_file` directly instead of going through the removed crate.

## Feature 002 — US6: all-message typed codegen + MessageCracker (Principle IV complete)

`build.rs` now generates, per bundled dictionary version (`fix40`..`fix50sp2`, `fixt11`), from the
same normalized source the runtime parses:

- **Typed message structs** (`truefix_dict::fix44::NewOrderSingle`, etc.) — thin wrappers over the
  generic `Message`, so encode/decode is byte-identical with the generic codec path by construction
  (no separate wire representation to keep in sync).
- **Named field accessors** (`.symbol()`/`.set_symbol()`) typed per the field's dictionary type
  (`i64`/`rust_decimal::Decimal`/`char`/`bool`/`time::OffsetDateTime`/`&str`).
- **Field-value enums** (`Side::Buy`, `OrdType::Market`, ...) for fields carrying labeled enum
  values (the normalized `.fixdict` format gained an optional `Value=Label` enum-token suffix,
  authored from the public FIX specification's standard enum tables — protocol facts, not copied
  source, per Constitution Principle III).
- **Typed repeating-group entry structs** (`NoPartyIDsEntry`, with nested `NoPartySubIDsEntry`),
  generated recursively from the dictionary's `group` directives.
- **A `crack_<version>` dispatcher** + `<Version>MessageHandler` trait (e.g. `crack_fix44`,
  `FIX44MessageHandler`) routing an inbound message to its typed handler method by
  `(BeginString, MsgType)` — completing `truefix_core::MessageCracker`'s contract (FR-020/022).

This completes the previously partially-met **Constitution Principle IV (dual-track data
dictionary)**: build-time codegen now produces genuinely strongly-typed messages, not only MsgType
constants, alongside the unchanged runtime `DataDictionary` validator — both still provably from one
source (`dual_track.rs`).

A real bug was caught while wiring the UTCTIMESTAMP field type: `time::OffsetDateTime`'s own
`Display` output is not the FIX wire format. Fixed via a dedicated `Field::utc_timestamp`
constructor (millisecond precision) used by the generated setters instead of `.to_string()`.

## Feature 002 — US10: full socket options + multi-endpoint failover (FR-019)

`truefix_transport::SocketOptions` now covers the full Appendix A socket-tuning surface —
`SocketKeepAlive`/`SocketReuseAddress`/`SocketLinger`/`SocketOobInline`/
`SocketReceiveBufferSize`/`SocketSendBufferSize`/`SocketTrafficClass` alongside the pre-existing
`SocketTcpNoDelay` — applied via `socket2::SockRef` best-effort against a live connection, and
`SO_REUSEADDR` applied at bind time (`bind_listener_with_options`) since `tokio::net::TcpListener`
cannot express it directly. `connect_initiator_reconnecting_multi` rotates round-robin through a
`Vec<SocketAddr>` on every (re)connect attempt, so a dead primary endpoint fails over to a
configured backup instead of retrying the same dead address forever.

`truefix-config` gained a data-only `SocketOptionsSpec` (mirroring `SocketOptions`'s fields, kept
in `truefix-config` rather than `truefix-transport` since `SocketOptions::apply()` is an inherent
impl that must live alongside its `socket2`-consuming type) plus `failover_addresses: Vec<SocketAddr>`
on `ResolvedSession`, parsed from numbered `SocketConnectHost<N>`/`SocketConnectPort<N>` keys
(N=1,2,... contiguous from 1; a gap stops enumeration; a host without its matching port, or vice
versa, is a typed `ConfigError`). `Engine::start` converts `SocketOptionsSpec` into
`transport::SocketOptions` and threads it through `Services` for both acceptor and initiator
sessions, so socket options are fully config-driven end-to-end.

**Scope boundary (documented, not a regression)**: `Engine::start`'s initiator path still uses the
one-shot `connect_initiator_with`/`connect_initiator_tls` (returning `SessionHandle`, which supports
`.logout()`/`.send()`) rather than `connect_initiator_reconnecting_multi` (returning the more
limited `ReconnectHandle`, supporting only `.stop()`/`.join()`); unifying them would require
extending `ReconnectHandle` to proxy send/logout to whichever endpoint is currently active, which is
a real API design task of its own. `failover_addresses` is therefore fully parsed and tested at the
config layer, and the rotation mechanism is fully implemented and tested at the transport layer
(`crates/truefix-transport/tests/failover.rs`), but `Engine::start` does not yet wire the two
together — no different from `Engine::start` not doing single-endpoint auto-reconnect today either.

## Feature 002 — US12: storage & logging completeness (FR-024/025/026)

**SQL store/log (FR-024)**: `truefix_store::SqlStore` and `truefix_log::SqlLog` now support
PostgreSQL, MySQL, and SQLite, selected by the connect URL's scheme (`postgres://`/`postgresql://`,
`mysql://`, else SQLite). Each backend gets its own native SQL text (placeholder style, upsert
syntax, column types) via a `Pool` enum matched once per call — deliberately not the `sqlx::Any`
driver, since `Any` doesn't unify bind-placeholder syntax across backends and native per-backend SQL
is both simpler and more correct. `SqlStoreConfig`/`SqlLogConfig` add configurable table names
(`JdbcStoreSessionsTableName`/`JdbcStoreMessagesTableName`/`JdbcLogIncomingTable`/
`JdbcLogOutgoingTable`/`JdbcLogEventTable`) and pool settings (`JdbcMaxActiveConnection`/
`JdbcMinIdleConnection`/`JdbcConnectionTimeout`/`JdbcConnectionIdleTimeout`/
`JdbcMaxConnectionLifeTime`). A `session_id` discriminator column lets multiple sessions share one
table pair, matching QuickFIX/J's JDBC schema more faithfully than the previous single-session
SQLite-only store. PostgreSQL/MySQL are exercised by `crates/truefix-store/tests/sql_backends.rs`,
gated on `TRUEFIX_TEST_POSTGRES_URL`/`TRUEFIX_TEST_MYSQL_URL` (unset in this sandbox — no live
service available — so those cases skip cleanly rather than failing the suite); SQLite runs
unconditionally. These `Jdbc*` keys remain **Recognized** rather than **Implemented**: the
capability is real and tested at the Rust-API level, but `.cfg` doesn't yet auto-select a SQL store
via `JdbcURL` the way `FileStorePath` auto-selects a file store — that auto-wiring is a natural next
step, not attempted here to keep this change bounded.

**CachedFileStore cache + FileStoreSync (FR-025)**: `FileStore` and `CachedFileStore` were
refactored onto a shared `BodyLog` (an append-only record log plus an in-memory *offset* index,
not the message bytes). `FileStore` now reads message bodies from disk on every `get()` — no
process-memory message cache — matching plain QuickFIX file-store semantics. `CachedFileStore`
additionally keeps a bounded in-memory byte cache (`FileStoreMaxCachedMsgs`; `0` = unbounded,
preserving the pre-FR-025 behavior as the default), evicting the oldest entry past the bound and
falling back to disk for cache misses; the cache is rewarmed from disk on reopen. Both honor a
`FileStoreSync` fsync toggle. `StoreConfig::File`/`CachedFile` gained an `options: FileStoreOptions`
field; `truefix-config`'s `resolve_store` now maps `FileStoreSync`/`FileStoreMaxCachedMsgs` from
`.cfg`, selecting `CachedFile` the moment `FileStoreMaxCachedMsgs` is set (both keys move
Recognized → **Implemented**).

**Log output switches (FR-026)**: `ScreenLog`/`FileLog`/`TracingLog` gained options structs
honoring their registered switches — heartbeat filtering (`FileLogHeartbeats`/
`ScreenLogShowHeartBeats`/`SLF4JLogHeartbeats`, detected via an exact SOH-delimited `35=0` field
match), incoming/outgoing/event visibility (`ScreenLogShow*`), and timestamp/millisecond inclusion
(`FileIncludeTimeStampForMessages`/`FileIncludeMilliseconds`/`ScreenIncludeMilliseconds`). Session-ID
prefixing (`SLF4JLogPrependSessionID` and its file/screen equivalents) is provided generically by a
new `SessionPrefixLog<L: Log>` decorator that wraps *any* `Log` implementation, rather than
duplicating a prefix option across every sink.

`truefix-config` gained a `FileLogPath`-triggered `LogSpec` (mirroring the `FileStorePath` → store
pattern) parsing the three `File*` switch keys; `Engine::start` converts it into a
`FileLog` wrapped in `SessionPrefixLog` and wires it into `Services.log` — closing a real,
pre-existing gap where `FileLogPath` was marked **Implemented** in the Appendix A registry despite
no actual `.cfg`-to-engine wiring existing anywhere in the codebase (verified by grep before this
change: the key appeared only in the registry itself). `FileLogPath`/`FileLogHeartbeats`/
`FileIncludeMilliseconds`/`FileIncludeTimeStampForMessages` are now genuinely
`.cfg`-to-running-log, proven by `crates/truefix/tests/config_start.rs`'s
`engine_builds_file_log_from_cfg` (asserts session-ID-prefixed event lines and heartbeat-filtered
message lines from a config-only start) and `crates/truefix-config/tests/store_and_log_mapping.rs`.
`ScreenLogShow*`/`ScreenIncludeMilliseconds`/`SLF4JLogPrependSessionID`/`SLF4JLogHeartbeats` remain
**Recognized**: the switch behavior is implemented and tested at the crate level, but there's no
natural `.cfg` key that selects "use the screen/tracing log" the way `FileLogPath` does, so no
auto-wiring was added for them.
