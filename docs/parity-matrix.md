# Parity Traceability Matrix

> **Historical traceability record.** Sections were appended across multiple remediation features;
> early rows can be superseded by later code. Use
> [`truefix_config::APPENDIX_A_KEYS`](../crates/truefix-config/src/keys.rs) and current tests as the
> source of truth. See the [documentation index](README.md) for current workspace scope.

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

`sqlx` was bumped `0.8` → `0.9` (also fixing the `getrandom` duplicate-version warning's severity
mismatch and confirming no new copyleft licenses entered the graph). In 0.9, `sqlx-mysql` made the
`rsa` dependency an opt-in `mysql-rsa` feature rather than a mandatory one; enabling it still pulls
a version affected by RUSTSEC-2023-0071 (Marvin Attack: RSA private-key timing sidechannel, no
patched version exists). This is deliberately **kept enabled and the advisory ignored** in
`deny.toml` rather than dropped: MySQL 8's default `caching_sha2_password` auth plugin requires
this exact RSA public-key exchange on a fresh, non-TLS connection — exercised for real by CI's own
`sql` job (a live `mysql:8` service container) — and the advisory's actual risk (leaking a
*private* key via decryption timing) doesn't apply to our usage, which only performs public-key
*encryption* client-side and never holds or uses an RSA private key. Dropping the feature to chase
a clean `cargo deny` run would trade a non-applicable advisory for a real, working-functionality
regression.

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

## Feature 003 — Dependency & Provenance Audit (T002)

Before any code depending on a new external crate is written (Principle III), the following were
checked against crates.io metadata and the FIX Trading Community's published terms:

| Dependency | Version checked | License | Verdict |
|------------|------------------|---------|---------|
| `quick-xml` | 0.41.0 | MIT | Compatible — permissive, no copyleft |
| `tokio-socks` | 0.5.3 | MIT | Compatible |
| `tiberius` | 0.12.3 | MIT/Apache-2.0 | Compatible |
| `ppp` | 2.3.0 | Apache-2.0 | Compatible |
| `oracle` (rust-oracle) | 0.6.3 | UPL-1.0/Apache-2.0 | **Final verdict (US14): deferred, not implemented.** The crate itself is permissively licensed, but at build/runtime it requires linking Oracle Instant Client — closed-source, distributed only under Oracle's own OTN License Agreement (click-through, non-redistributable, disallows use for building a competing product without a separate commercial agreement). That's a materially different obligation from the ODBC/JDBC-driver comparison this row originally floated: ODBC/JDBC are open, vendor-neutral *protocols* with many independently-licensed driver implementations, whereas Oracle Instant Client is the *only* practical way to speak Oracle's wire protocol and is itself proprietary. Requiring every operator who enables an off-by-default `oracle` feature to separately accept Oracle's OTN terms is an obligation this workspace's Principle III clean Apache-2.0 OR MIT release stance shouldn't impose implicitly. Per the spec's Clarifications (spec.md's TODO-14 Q&A), this downgrades Oracle to documented-interface-only: `StoreConfig`/`LogConfig` deliberately do **not** grow an `Oracle` variant; an operator whose infrastructure standardizes on Oracle continues to implement `MessageStore`/`Log` directly against `Client<Compat<TcpStream>>`-equivalent driver of their choice, exactly like any other custom backend not bundled by TrueFix. |
| FIX Orchestra (FPL/FIXTradingCommunity repositories) | current | Apache License 2.0 (the machine-readable repository/schema content; the prose Technical Specification document itself is separately CC-BY-ND, not implicated here since only the machine-readable Orchestra XML data is parsed) | Compatible — confirmed via the FIX Trading Community's own published terms |

None of these dependencies have been added to any `Cargo.toml` yet — they are wired in only when the
stage that actually consumes them lands (US9 → `quick-xml`; US12 → `ppp`/`tokio-socks`; US14 →
`tiberius`/possibly `oracle`), so the workspace doesn't carry unused dependencies across stages that
haven't started (per Principle I's minimalism and this project's "no code before it's needed"
convention).

## Feature 003 — Stance Tracking Scaffold (T004)

Config keys/toggles this feature will move from **Recognized** → **Implemented** (FR-021), tracked
here as each lands; see `crates/truefix-config/src/keys.rs` (`APPENDIX_A_KEYS`) for the authoritative
per-key registry this table mirrors.

| Key | Stage | Status |
|-----|-------|--------|
| `ValidateFieldsOutOfOrder` | G3 (US3) | done |
| `ValidateChecksum` / `ValidateIncomingMessage` / `AllowPosDup` / `RequiresOrigSendingTime` | G3 (US8) | done |
| `SendRedundantResendRequests` / `ClosedResendInterval` / `ResetOnError` / `DisconnectOnError` / `DisableHeartBeatCheck` / `RejectMessageOnUnhandledException` / `LogonTag` / `MaxScheduledWriteRequests` / `ContinueInitializationOnError` / `LogMessageWhenSessionNotFound` / `RefreshOnLogon` / `ForceResendWhenCorruptedStore` | G4 (US4) | done |
| `UseTCPProxy` / `TrustedProxyAddresses` / `ProxyType` / `ProxyHost` / `ProxyPort` / `ProxyUser` / `ProxyPassword` / `SocketPrivateKeyBytes` / `SocketCertificateBytes` / `SocketCABytes` / `CipherSuites` / `SocketSynchronousWrites` / `SocketSynchronousWriteTimeout` | G11 (US12) | done |
| `InChanCapacity` (`in_chan_capacity`) | G13 (US14) | done |

## Feature 003 — US2: session-owned durable resend consistency (FR-003/004)

**Durable resend across a crash-restart (FR-003) was already correct**: `run_connection`
(`crates/truefix-transport/src/lib.rs`) seeds a fresh `Session` with `seed_sequences` +
`seed_sent_messages` from the store on every new connection — including a post-crash reconnect —
and `crates/truefix-session/tests/restart_resend.rs` (feature 002) already proves that mechanism
replays pre-restart PossDup bodies correctly. Adding a literal `Session::with_store(config,
store: Arc<dyn MessageStore>)` (as the original TODO-02/FR-003 wording suggested) would have required
`Session` to hold an async store handle, which conflicts with its deliberately sans-IO design
(`Session` performs no I/O so it stays deterministic and unit-testable per its own module doc); it was
also unnecessary, since the existing seed-based mechanism already achieves the same durability
guarantee through the established "transport bridges async I/O, Session emits declarative Actions"
pattern.

**The actual remaining gap was reset consistency (FR-004)**: two internally-triggered full resets —
`on_logon`'s `ResetSeqNumFlag=Y` handling, and `enter_disconnected`'s `ResetOnLogout`/
`ResetOnDisconnect` handling — cleared `Session`'s in-memory sent-message map but had no way to tell
the durable store to clear itself too (only the explicit `Control::Reset` control-channel path was
paired with a manual `store.reset().await` at its call site). Fixed by adding `Action::ResetStore` —
returned by `Session::handle()` alongside `Send`/`Disconnect` — which `truefix-transport`'s
`perform_actions` now handles by calling `store.reset().await`. Proven by
`crates/truefix-session/tests/reset_store_consistency.rs` (session-level: the action is emitted
exactly when a full reset actually occurs, not on ordinary logons/logouts) and
`crates/truefix-transport/tests/restart_continuity.rs`'s `reset_on_logout_clears_the_durable_store`
(end-to-end: reopens the store from disk after a `ResetOnLogout`-triggered logout and asserts both
the sequence number and previously-persisted message bodies were actually cleared).

## Feature 003 — US3: field-order validation + US8: extra validation toggles (FR-006/007)

**`ValidateFieldsOutOfOrder` (FR-006)**: `truefix_core::Message` gained a `fields_out_of_order()`
flag, set by `decode()` while classifying tags into header/body/trailer — the three `FieldMap`s
preserve *within-section* wire order but classify by static tag identity, so cross-section
interleaving (a body field arriving before the header section is done, etc.) is only observable
during decode itself; the flag is how that observation survives past decode for
`truefix-dict::validate()` to act on when `ValidationOptions::validate_fields_out_of_order` is
enabled (default `false`, matching today's lenient behaviour). Also catches the third-wire-field
not being MsgType(35). Proven by `crates/truefix-core/tests/field_order.rs` (decode-level),
`crates/truefix-dict/tests/field_order.rs` (validate()-level toggle gating), and three new AT
scenarios (`14g_HeaderBodyTrailerFieldsOutOfOrder`, `15_HeaderAndBodyFieldsOrderedDifferently`,
`2t_FirstThreeFieldsOutOfOrder`) requiring raw out-of-order bytes sent via `Step::SendRaw`, since
`Message::encode()` always re-emits canonical order.

**Four extra `ValidationOptions` toggles (FR-007)**: `validate_incoming_message` (master switch,
default `true`), `allow_pos_dup`/`requires_orig_sending_time` (PossDup acceptance policy, default
`true`/`false` respectively), and `validate_checksum` (documented as always-mandatory — TrueFix's
decoder validates the wire checksum unconditionally as a decode-time error via the existing
`RejectGarbledMessage` path; this flag is accepted for QuickFIX/J config-key parity but does not
weaken enforcement, a deliberate Principle I/II decision, not an oversight). Proven by
`crates/truefix-dict/tests/validation_toggles.rs`; `validateChecksum`'s AT special-category suite
(`validate_checksum_suite()`) reuses the existing `garbled_message_dropped`/`garbled_message_rejected`
scenarios as its content and has its own dedicated conformance test.

**Registry finding (Principle VII)**: while wiring these, found that `ValidateChecksum`,
`ValidateIncomingMessage`, `AllowPosDup`, and `RequiresOrigSendingTime` were already marked
**Implemented** in the Appendix A registry *before* this session — but `ValidationOptions` didn't
have these fields and `validate()` didn't check them at all until now. More broadly, **no key in the
entire "validation" group is actually wired from `.cfg` to `Engine::start`'s `Services.validator`**
(`crates/truefix-config/src/builder.rs` has no `resolve_validator`/dictionary-mapping function at
all — dictionary validation is only ever set programmatically via the `Services` API, e.g. by the AT
harness). This session's stance update for `ValidateFieldsOutOfOrder` (Recognized → Implemented)
follows the same established precedent as its four siblings for consistency, but the broader gap —
`.cfg`-driven dictionary/validation wiring into `Engine::start` doesn't exist for **any** key in this
group — is a real, larger finding tracked here for a future session, not fixed by this one.

## Feature 003 — US4: remaining session config switches (FR-008)

All 12 previously-recognised-but-inert switches now have a determination, and — unlike the
"validation" group — the 8 real `SessionConfig`-level ones are genuinely wired from `.cfg` through
`crates/truefix-config/src/builder.rs::resolve_one` (the "session" group already had this wiring
pattern established by earlier keys like `ResetOnLogon`/`PersistMessages`).

**Real behavior implemented (`Implemented`)**: `SendRedundantResendRequests` (removes the
resend-suppression guard when a gap arrives while a resend is already pending),
`ResetOnError`/`DisconnectOnError` (a new `Session::reset_on_error()` helper + wiring into the
identity/latency/dictionary-validation error paths), `DisableHeartBeatCheck` (splits `on_tick`'s
`LoggedOn` match arm on the flag), `LogonTag` (an optional custom tag appended to outbound Logons,
parsed from a `LogonTag=<tag>=<value>` `.cfg` key via a new `resolve_logon_tag` helper),
`RefreshOnLogon` (a new `Session::refresh_sequences()` — unconditional, unlike `seed_sequences` —
wired into `truefix-transport::dispatch()`'s existing post-logon transition point),
`ForceResendWhenCorruptedStore` (a new `MessageStore::was_corrupted()` default-`false` trait method,
overridden by `FileStore`/`CachedFileStore`, checked at connect time to force a full
`session.reset()` + `store.reset()` rather than trusting/resending recovered-but-untrusted data), and
`LogMessageWhenSessionNotFound` (turned out to be acceptor/routing-level — fires in
`truefix-transport::route_and_run` before any `Session` exists — so it's a new
`Services.log_message_when_session_not_found` flag, not a `SessionConfig` field).

**Intentional no-ops (`Unsupported(reason)`, not `Implemented`)**: `RejectMessageOnUnhandledException`
(Rust's typed-error architecture, Principle I, has no unhandled-exception class), `ClosedResendInterval`
(the single-threaded sans-IO session has no concurrent resend-servicing race to resolve),
`MaxScheduledWriteRequests` (actions are returned synchronously for the transport to write
immediately — no internal outbound write queue exists to bound). These are marked `Unsupported` rather
than `Implemented` — more accurate than the "validation" group's precedent of marking a documented
no-op `Implemented`, since these three have genuinely zero observable effect rather than an
always-satisfied invariant like `validate_checksum`.

**Stays `Recognized`, not `Implemented`**: `ContinueInitializationOnError` — governs whether a
multi-session `Engine::start` keeps starting other sessions when one fails during bring-up; that's
multi-session startup orchestration in the `truefix` crate, not `Session` runtime behavior, so it has
no effect at the layer this feature touched. The field round-trips through `SessionConfig` (so the
setting doesn't get lost), but implementing the real behavior is a future-session task for
`truefix::Engine::start`.

**A real bug found and fixed along the way**: the `on_tick` `LoggedOn`-state heartbeat-timeout
`enter_disconnected()` call site had different indentation than the other two `enter_disconnected()`
call sites fixed during US2's `Action::ResetStore` work, so an exact-string `replace_all` edit
silently missed it — that one path wasn't emitting `Action::ResetStore` on
`ResetOnDisconnect`/`ResetOnLogout` when the session timed out due to peer silence. Fixed while adding
`DisableHeartBeatCheck` (which touches the same match arm).

## Feature 003 — US5: dictionary component model (FR-009)

Added a `component <Name> <members>` directive to the normalized `.fixdict` grammar. `message`/`group`
member lists (`req:`/`opt:`/the group members token) now accept `component:<Name>` tokens interspersed
with plain tag numbers. Components are resolved in a dedicated pass after the whole document is parsed
(so forward references and component-referencing-component work), with cycle detection via a
`resolving: BTreeSet<String>` guard (`ParseError::ComponentCycle`) and a clear error for a reference to
an undefined component (`ParseError::UnknownComponent`). By the time a `DataDictionary` exists,
components are fully expanded into flat tag lists — `decode.rs`/`validate.rs` are completely unchanged,
matching the design intent in `research.md` §1.

**Real limitation found and disclosed, not fixed**: `crates/truefix-dict/build.rs` has its own,
separate codegen-time parser (`parse_dict`, the other half of Principle IV's dual-track design) that
does **not** understand `component`/`component:<Name>` — its member-list parsing is
`list.split(',').filter_map(|s| s.parse::<u32>().ok())`, which silently *drops* a `component:Name`
token (it fails to parse as `u32` and `filter_map` discards it) rather than erroring. If a bundled
dictionary ever adopts `component`, codegen would silently emit an incomplete typed struct (missing the
component's fields) while the runtime `DataDictionary` stayed fully correct — a genuine dual-track
divergence risk. This is currently **dormant**: no bundled dictionary (`dict-src/normalized/*.fixdict`)
uses `component` yet, so today's dual-track hash and round-trip tests are unaffected. Extending
`build.rs`'s codegen parser to understand components (and its own cycle detection) is a larger task
than this US's runtime-model scope — tracked here explicitly so it isn't silently forgotten before any
future session adds `component` to a bundled dictionary or uses it for FIX Latest (US9).

## Feature 003 — US6: runtime dictionary loading (FR-010)

`DataDictionary::load_from_file(path)` (new `DictLoadError::Io`/`Parse`, both naming the offending
path) and `DataDictionary::extend(&mut self, other)` (merge an extension dictionary into a base one).
`extend()` uses a two-phase design — a full conflict-check dry run across all four keyed maps
(fields/messages/groups/components) before any mutation — so a `DictMergeConflict` leaves `self`
completely untouched rather than partially merged. Header/trailer tag sets are unioned unconditionally
(no conflict concept applies to set membership); `hash` is deliberately left unchanged, since it
identifies the base dual-track (bundled) source that `extend()` sits outside of by design. Required
adding `PartialEq`/`Eq` to `FieldDef`/`MessageDef`/`GroupDef`/`ComponentDef` (previously `Debug, Clone`
only) to distinguish an idempotent identical redefinition from a genuine conflict.

**A real test-isolation bug found and fixed**: the initial `load_from_file` test helper generated
unique temp-file names from a nanosecond timestamp; two test threads in the same binary occasionally
observed the same nanosecond, causing one test's file write to race with another's read — an
intermittent failure that only appeared under `cargo test --workspace` (parallel execution across the
whole suite), not when the test file was run in isolation. Fixed with an atomic counter, matching the
`unique_dir()` pattern `truefix-transport`'s tests already use for the same reason.

## Feature 003 — US7: field type completeness (FR-011)

Added `Field::bytes`/`as_bytes` (Data fields — thin, semantically-named wrappers over the existing
`new`/`value_bytes`, since the wire representation is identical but the call sites benefit from
signaling "this is a Data field"), `Field::utc_date_only`/`as_utc_date_only` (`YYYYMMDD`), and
`Field::utc_time_only`/`as_utc_time_only` (`HH:MM:SS.sss` at millisecond precision, matching the
existing `utc_timestamp`'s precision convention; tolerant of up to picosecond input, truncated to
nanosecond, matching `as_utc_timestamp`'s existing behavior). New `FieldError::NotDateOnly`/
`NotTimeOnly` variants. `Field::double`/`as_double` (Float/Price/Qty non-Decimal compatibility) is
excluded per the spec's Assumptions — the audit itself marks it optional since `rust_decimal` already
covers Price/Qty.

**Scope boundary, not a gap**: `truefix-dict::FieldType::value_ok` still accepts Data/UtcDateOnly/
UtcTimeOnly values without format checking (the `_ => true` catch-all) — wiring these new accessors
into dictionary-level format validation is a natural follow-on but wasn't part of FR-011, which is
specifically scoped to `truefix-core::Field`.

## Feature 003 — US9: FIX Latest support (FR-012)

The tenth dictionary. `crates/truefix-dict/src/orchestra.rs` (new, `--features dict-tooling`,
`quick-xml`-based, off by default so it never enters the runtime engine's dependency graph) converts
FIX Orchestra XML into the same normalized `.fixdict` grammar every bundled dictionary already uses —
`load_fixlatest()` and `build.rs`'s codegen therefore need zero FIX-Latest-specific runtime code. The
converter supports a representative subset of the Orchestra repository schema (`field`/`component`/
`group`/`message`, `fieldRef`/`componentRef`/`groupRef`, `presence`), with `StandardHeader`/
`StandardTrailer` components specialized into the dictionary's `header`/`trailer` directive rather than
a `component:` reference — matching every other bundled dictionary's convention. No Orchestra XML file
content is vendored (Principle III): `dict-src/orchestra/FIXLATEST.orchestra.xml` is a hand-authored
fixture reproducing the FPL-published schema *shape*, exactly like the other 9 dictionaries were
hand-normalized from the public FIX specification. `FIXLATEST.fixdict` mirrors FIX40–44's convention
(a self-contained session + application layer in one file) and adds a `Parties` component wrapping a
`NoPartyIDs` group plus `TradeDate`(UtcDateOnly)/`RawData`(Data) fields on `NewOrderSingle`, so the
US5 component model and US7 field types both get exercised by real, shipped content, not just tests.

**A real gap found and fixed** (flagged as a known risk back in US5's own write-up, above): `build.rs`'s
own dictionary parser — independent of the runtime `parser.rs` — had never been taught the `component`
directive or `component:<Name>` reference tokens; its `req:`/`opt:`/group-member-list parsing did
`token.parse::<u32>().ok()`, which silently *drops* any `component:<Name>` token instead of erroring.
This was latent and untested until `FIXLATEST.fixdict` became the first bundled dictionary to actually
use `component:` in a message definition — codegen would have silently omitted `NewOrderSingle`'s
`NoPartyIDs` group accessor with no build warning. Fixed by porting `parser.rs`'s two-phase
resolve-then-expand design (with the same cycle detection, `panic!`ing on a build-time cycle rather than
returning a `Result`, matching this file's existing "bad input panics the build" convention) into
`build.rs`. Verified via the generated code: `NewOrderSingle::no_party_ids()`/`set_no_party_ids()` are
present and correctly typed.

`SUITE_VERSIONS` (`crates/truefix-at/src/scenarios.rs`) gained `"FIX.Latest"` as its 9th entry. No new
scenario functions were needed for the "logon-to-heartbeat" independent test criterion: every
version-agnostic scenario (logon/sequencing/resend/admin-message handling — the bulk of the suite) is
parameterized over `SUITE_VERSIONS` already and runs unmodified against the new version, since
`start_acceptor`'s session-layer protocol logic has never depended on a dictionary being loaded for
FIX.5.0/FIX.5.0SP1/FIX.5.0SP2 either. AT conformance grew from 318/318 to **353/353** scenario runs
passing.

**Scope boundary, not a gap**: `build.rs`'s field `type_mapping` has no entry for `DATA`/`UTCDATEONLY`/
`UTCTIMEONLY` normalized type tokens, so `NewOrderSingle::raw_data()`/`trade_date()` fall through to the
default string-like accessor (`&str`/`as_str`) rather than US7's new `Field::bytes`/`utc_date_only`
typed accessors. The underlying wire bytes are identical either way (the typed wrapper is a thin view
over the same `Message`), so this is a codegen ergonomics gap, not a correctness one — wiring US7's
accessors into `build.rs`'s type-mapping table is a natural follow-on outside FR-012's scope (which is
about the dictionary *source*, not codegen's per-type accessor selection).

## Feature 003 — US10: extended application hooks (FR-013)

Two QuickFIX/J-only extras (`ApplicationExtended`'s `canLogon` predicate + `onBeforeSessionReset`, and
`RejectLogon`'s `SessionStatus`), both approved by the spec's Clarifications as suitable additions.

- **Extended logon predicate**: not a new method — `Application::from_admin(&Message, &SessionId) ->
  Result<(), Reject>` already refuses a Logon (documented as such since 002/FR-016); US10 just
  documents this explicitly as the "logon predicate" extension point, since it already runs before the
  engine completes logon processing.
- **`SessionStatus`-carrying refusal**: `truefix_core::Reject` gains `session_status: Option<u16>`.
  `Session::reject_logon` stamps it as SessionStatus (tag 573) on the outbound Logout when set.
- **`on_before_reset`**: a new `Application` trait method (no-op default, same shape as `on_logon`/
  `on_logout`).

**Design correction (found during implementation, same pattern as US2)**: the plan's data-model sketch
described `on_before_reset` as "invoked at the top of `reset()`, before state is cleared" — but
`Session::reset()` lives in `truefix-session`'s sans-IO state machine, which never holds an
`Application` handle (an architectural boundary established during US2, not something this US could or
should relitigate). The transport layer — which already drives `Application` — calls
`app.on_before_reset(id).await` immediately before each of the three places a reset actually takes
effect: the explicit `Monitor::reset()` control path, the `ForceResendWhenCorruptedStore` internal
trigger, and inside the `Action::ResetStore` handler (which already covers every other
internally-triggered reset — logon-time `ResetSeqNumFlag`, `ResetOnLogout`/`ResetOnDisconnect` — per
US2's own `Action::ResetStore` design). Verified end-to-end in `crates/truefix-transport/tests/
application_hooks.rs`: both the explicit-reset and internal-reset paths fire the hook exactly once.

**A minor, disclosed exception to "no breaking API changes"**: adding `session_status` to `Reject` (a
public struct with all-`pub` fields, no `#[non_exhaustive]`) is a source-level break for any external
struct-literal construction of `Reject { .. }` without the new field — Rust struct literals must name
every field. Both in-repo call sites (`crates/truefix-session/tests/typed_callbacks.rs`,
`crates/truefix-transport/tests/auth.rs`) were updated. This is the smallest possible instance of the
tension and matches the plan's own approved `data-model.md` design (an additive `Reject` field) rather
than introducing a parallel, non-breaking mechanism — disclosed here rather than silently claimed away.

## Feature 003 — US1 closeout: AT full closure (FR-001/002/005)

Closes the "early slice, later closeout" split established at this feature's start (`research.md` §11):
Phase 4 (early) got AT coverage to 8 versions/318 runs; this closeout brings in the 9th version
(FIX.Latest, US9) and the two remaining special-category suites (`timestamps`, `resynch` — the third,
`validateChecksum`, already landed during US3/T022).

- `SUITE_VERSIONS` now has all 9 targeted versions; conformance is **353/353 scenario runs passing**.
- New `scenarios::timestamps_suite()` (reuses `check_latency_timestamps`) and `scenarios::
  resynch_suite()` (reuses the existing resend/reset scenario family: gap-fill, bounded-end,
  not-duplicated, begin-zero-ignored, nothing-to-resend, out-of-order queueing, SequenceReset
  Reset/GapFill both directions, chunked resend) — each independently runnable and independently
  gated by its own `#[tokio::test]` in `crates/truefix-at/tests/conformance.rs`.
- New `crates/truefix-at/tests/coverage.rs`: a permanent regression-floor gate (9 versions present,
  ≥353 scenario runs, all 3 special suites non-empty with distinct scenario names) so a future change
  can't silently shrink the corpus without a test failure.
- Confirmed (T055) that US3's field-order scenarios (`2t_FirstThreeFieldsOutOfOrder`,
  `14g_HeaderBodyTrailerFieldsOutOfOrder`, `15_HeaderAndBodyFieldsOrderedDifferently`) and the
  `validateChecksum` suite were already folded into `server_suite()`/their own dedicated test —
  `docs/todo/001.md`'s TODO-01 checkboxes were stale (still marked "deferred until US3
  lands") and have been corrected to reflect actual completion.

**Honest scope framing, not a claimed "73/73"**: this project does not vendor or otherwise verify
against QuickFIX/J's actual scenario list (Principle III forbids deriving content from copied source),
so "100% of the 73 published server scenarios" is not something this codebase can mechanically check
itself against. What *is* verifiable and enforced: every scenario this project has itself authored
passes across every version it targets (353/353, CI-gated), the corpus cannot silently shrink
(`coverage.rs`'s regression floor), and every individually-deferred scenario name is recorded with an
explicit reason in `docs/todo/001.md`'s TODO-01 (harness capability gaps, dictionary-content
gaps, or genuinely ambiguous reference semantics this project won't guess at) rather than silently
dropped.

## Feature 003 — US12: network hardening (FR-015/016/017)

Five capabilities, each behind its own config surface:

- **PROXY protocol (v1/v2)** — new `crates/truefix-transport/src/proxy.rs` (`ppp` crate, Apache-2.0).
  A physical connection's PROXY header is parsed only when its peer IP is in
  `Services.trusted_proxy_addresses` (`UseTCPProxy`+`TrustedProxyAddresses`) — an untrusted source's
  header is never trusted (Clarifications). Wired into both the single-session `Acceptor` (strips the
  header so its bytes are never mistaken for FIX wire data, even though there's no allow-list to gate
  on there) and the multi-session `AcceptorBuilder` (the resolved IP feeds the existing
  `allowed_remotes` check).
- **Forward proxy (SOCKS4/SOCKS5+auth/HTTP CONNECT)** — SOCKS via `tokio-socks` (MIT); HTTP CONNECT
  hand-rolled (a single request line + header block, not worth a full HTTP client dependency). New,
  purely additive `connect_initiator_via_proxy`/`connect_initiator_via_proxy_tls` functions — no
  existing `connect_initiator*` signature changed.
- **Inline PEM bytes** — `TlsSpec` gains `key_store_bytes`/`trust_store_bytes: Option<Vec<u8>>`,
  parsed via `rustls-pki-types::pem`'s byte-slice `PemObject` methods (no new dependency, confirmed in
  `research.md`). **Design deviation, disclosed**: the contract's draft sketched three separate keys
  (`SocketPrivateKeyBytes`/`SocketCertificateBytes`/`SocketCABytes`, matching QuickFIX/J's own
  three-file convention), but this codebase's `TlsSpec` already combines cert+key into one
  `key_store_path` (a design decision predating this feature) — the inline-bytes counterpart follows
  that same combined convention (`SocketKeyStoreBytes`/`SocketTrustStoreBytes`) rather than
  reintroducing a three-way split this codebase never had. `key_store_path` changes from `PathBuf` to
  `Option<PathBuf>` (exactly one of path/bytes must be set) — another small, disclosed exception to
  "no breaking API changes", same category as US10's `Reject.session_status`.
- **Cipher suites** — filters `rustls::crypto::aws_lc_rs::default_provider()`'s suite list by
  case-insensitive match against each suite's `Debug`-formatted name (e.g.
  `"TLS13_AES_128_GCM_SHA256"`), fed to `ServerConfig::builder_with_provider`/
  `ClientConfig::builder_with_provider`.
- **Synchronous writes + timeout** — `Services.sync_write_timeout: Option<Duration>` wraps the
  outbound `write_all` in `perform_actions` with `tokio::time::timeout`; a timeout logs a distinct
  event (`"... synchronous write timed out after ..."`, not a generic I/O failure) via `Log::on_event`
  and tears the connection down. This is the closest this codebase's existing `Result<bool, ()>`
  connection-teardown convention gets to a "typed" error without a broader architecture change — the
  distinguishing signal is the log event, not a new propagated error type (disclosed scope choice, not
  hidden).

All five wired end-to-end through the facade (`truefix::Engine::start`), not just left as inert parsed
config — `ProxySpec`/`trusted_proxy_addresses`/`sync_write_timeout` all reach `Services`/
`connect_initiator_via_proxy*` at engine-start time.

Tests: `crates/truefix-transport/tests/proxy_protocol.rs` (trusted/untrusted PROXY header, 2 tests),
`proxy_client.rs` (SOCKS4/SOCKS5±auth/HTTP CONNECT, 4 tests — each hand-rolls a minimal local proxy
server that genuinely forwards to a real `Acceptor` FIX session, proving the tunnel carries real FIX
traffic end-to-end, not just completing a handshake), `tls_hardening.rs` (inline PEM bytes, matching
vs. disjoint cipher suites, 3 tests), `sync_writes.rs` (a stalled-peer write-timeout test using a small
socket buffer, 1 test), plus `crates/truefix-config/tests/network_hardening_mapping.rs` (16 `.cfg` →
`ResolvedSession` mapping tests, including error paths).

## Feature 003 — US13: `truefix-dict` CLI (FR-018)

A standalone binary (`crates/truefix-dict/src/bin/truefix-dict.rs`, `[[bin]] required-features =
["dict-tooling"]`) with three subcommands (`generate-dict`, `generate-code`, `validate`), matching
QuickFIX/J's `dictgenerator` / QuickFIX/Go's `generate-fix` in spirit.

**A real refactor, not just a thin wrapper**: the contract requires the CLI to wrap the *same*
parse/codegen logic `build.rs` already uses — no parallel implementation (Principle IV). But that logic
lived entirely inside `build.rs` itself, which a `src/bin/*.rs` binary in the same package can't depend
on (a build script can't depend on its own not-yet-built library crate — the classic chicken-and-egg
problem). Solved by extracting the logic into a new `crates/truefix-dict/src/codegen.rs` module, shared
via `#[path = "src/codegen.rs"] mod codegen;` in `build.rs` (same *source file*, two independent
compilations — `build.rs`'s own crate, and the library's `pub mod codegen`) rather than one binary
depending on the other.

**A real correctness fix that came out of the refactor**: `build.rs`'s codegen internals previously
`panic!`ed directly on malformed input (bad tag/component tokens, unknown components, component
cycles) — acceptable when this code was only ever reachable from `cargo build` failing a whole
compilation, but no longer acceptable once the same functions became reachable from a user-facing CLI
tool fed arbitrary files (Principle I: no panics on a path a user can trigger with ordinary bad input).
All internal panics were converted to a new `CodegenError` (`thiserror`), propagated via `Result`;
`build.rs`'s own top-level `main()` still panics on error (the correct, idiomatic behavior for a build
script to fail loudly), but only there, after calling the now-fallible shared functions. The CLI's own
`main()` catches the `Result` and prints a clean `error: ...` message with a non-zero exit code instead.

CLI argument parsing is hand-rolled (`--flag value` pairs via `std::env::args()`) rather than pulling in
`clap` — three subcommands, each with at most 3-4 flags, don't justify a new dependency (and its own
license/provenance audit).

Tests: `crates/truefix-dict/tests/cli.rs` (8 tests) drive the *actual compiled binary* as a subprocess
(`std::process::Command` + `CARGO_BIN_EXE_truefix-dict`), not the library functions directly — proving
the CLI itself (argument parsing, exit codes, error message text) works end-to-end, including two
tests specifically asserting a malformed/missing argument produces a clean error rather than a panic
backtrace. `generate_dict_converts_the_bundled_orchestra_fixture` and
`generate_code_produces_typed_rust_from_a_sample_fixdict` cross-check the CLI's output against the
shipped, `build.rs`-generated artifacts to prove the two code paths (build script vs. CLI) really do
produce byte-identical results from the shared source.

## Feature 003 — US14: inbound backpressure + MSSQL/Oracle SQL backends (FR-019/020)

**Backpressure (FR-019)**: `crates/truefix-transport/src/lib.rs`'s `run_connection` splits the socket
via `tokio::io::split` into a dedicated `read_loop` task and the original processing loop. Two
in-process channels carry decoded inbound messages from the reader to the processor: an always-
unbounded `admin_tx`/`admin_rx` for session-level messages (Heartbeat/TestRequest/ResendRequest/
Logon/Logout/Reject), and an application channel bounded by the new `SessionConfig.in_chan_capacity:
Option<usize>` (`InChanCapacity`, FR-008-adjacent) for everything else. `in_chan_capacity: None` (the
default) collapses the application channel to a receiver whose sole sender is dropped immediately —
`classify_buffered` then routes every message through `admin_tx` in strict wire order, byte-for-byte
identical to pre-US14 single-channel behavior (Acceptance Scenario 2's explicit requirement).
`Some(n)` engages true splitting: decode-time (fast, never blocks) is fully decoupled from
delivery-time (a `tokio::select!` racing the cancel-safe `Sender::reserve()` against continued socket
reads) via a local `VecDeque<Inbound>` staging admin-vs-application classification before either
channel send — this is what lets the reader keep draining new admin traffic off the wire even while a
saturated application channel has a message queued for delivery, satisfying the "admin must not
starve behind a full application channel" requirement (spec Clarifications) without the reader itself
ever blocking on a full channel mid-decode. `crates/truefix-transport/tests/backpressure.rs` proves
both properties (2 tests): a filled application channel doesn't drop messages, and concurrent admin
traffic still gets a prompt reply.

**MSSQL (FR-020)**: `MssqlStore`/`MssqlLog` (`crates/truefix-store/src/mssql.rs`,
`crates/truefix-log/src/mssql.rs`), behind a new `mssql` feature independent of the existing `sql`
feature. Reached via `tiberius` (TDS driver; sqlx has no official MSSQL support) rather than sharing
`sql.rs`'s sqlx-based implementation — a deliberate, disclosed deviation from tasks.md T076's literal
wording ("implement the MSSQL branch on `SqlStore`/`SqlLog`"), since `tiberius`'s `Client` type and
connection lifecycle share no common trait with `sqlx::Pool`; the two backends implement the same
`MessageStore`/`Log` trait contracts and pass the identical conformance-test pattern
(`MssqlStoreConfig`/`MssqlLogConfig` mirror `SqlStoreConfig`/`SqlLogConfig`'s shape, minus pool
settings — `tiberius` has no built-in connection pool, so a single connection is serialized behind a
`tokio::sync::Mutex`, judged adequate for a FIX session's inherently sequential store/log access
pattern rather than pulling in a separate pooling crate for one backend). `tiberius`'s `rustls`
feature (chosen over its `native-tls` option to keep this workspace's single, consistent TLS stack)
transitively reintroduces `rustls-pemfile` (RUSTSEC-2025-0134) — the same advisory this workspace's
own PEM handling already migrated off of; accepted and scoped to the `mssql` feature only via
`deny.toml`'s `ignore` list (see that file's inline justification), since `tiberius` offers no
rustls-based path around it and the alternative (`native-tls`) would fragment the TLS stack instead.
Server-certificate validation is intentionally skipped (`Config::trust_cert()`) — MSSQL's own TDS-level
encryption still protects the connection in transit; this only forgoes CA-chain validation, matching
how containerized/dev SQL Server instances (no real CA-issued cert) are typically reached, the same
tradeoff the project's CI service container in `.github/workflows/ci.yml`'s new `mssql` job exercises.
Conformance tests (T073): `crates/truefix-store/tests/mssql_backend.rs` (2 tests) and
`crates/truefix-log/tests/mssql_log.rs` (1 test), gated on `DATABASE_URL_MSSQL` exactly like the
existing Postgres/MySQL precedent in `sql_backends.rs`/`sql_log.rs` — they compile and pass-by-skip
everywhere `DATABASE_URL_MSSQL` is unset, and run for real against the new CI service container.

**Oracle (FR-020, deferred)**: see this document's "Feature 003 — Dependency & Provenance Audit
(T002)" table above for the final license rationale. No `StoreConfig::Oracle`/`LogConfig::Oracle`
variant exists; an operator needing Oracle implements `MessageStore`/`Log` directly, same as any other
backend TrueFix doesn't bundle. This is the spec's explicitly pre-approved downgrade path (Clarifications:
"MAY downgrade Oracle support to documented-interface-only (deferred) if no license-compatible mature
option exists"), not a silently dropped requirement.

**Config wiring (T078)**: `InChanCapacity` is a new `Impl` key in
`crates/truefix-config/src/keys.rs`, resolved into `SessionConfig.in_chan_capacity` in
`crates/truefix-config/src/builder.rs`'s `resolve_one` (mirrors the other numeric session switches
via the existing `usize_key` helper). The `Jdbc*` keys' stance comment was updated to note MSSQL's
availability behind the new `mssql` feature, but their stance stays `Recognized` (not `Implemented`):
QuickFIX/J dispatches SQL backend choice from a single `JdbcURL` via its JDBC driver registry, but
TrueFix has no equivalent single-entry-point dispatch — PostgreSQL/MySQL/SQLite (`sqlx`) and MSSQL
(`tiberius`) are reached only via their own `StoreConfig`/`LogConfig` Rust-API variant, not yet parsed
from these `.cfg` keys. This is the same boundary the `sql` feature already had before this feature
(pre-existing from feature 002, not a US14 regression) — a unified `.cfg`-driven SQL-backend dispatch
across four structurally different native drivers is out of this feature's scope.

## Feature 004 — Dependency & Provenance Audit (T002)

| Dependency | Version | License | Verdict |
|------------|---------|---------|---------|
| `redb` | 4.1.0 | MIT OR Apache-2.0 | Compatible — permissive, no copyleft. `cargo deny check` clean; no transitive advisory-flagged dependency (pure Rust, minimal dependency tree). |
| `mongodb` | 3.7.0 | Apache-2.0 | Compatible — permissive, no copyleft, matches QuickFIX/Go's own choice of driver for its equivalent feature. **Pulls in two transitive licenses not previously in `deny.toml`'s allow-list**: `tiny-keccak` (via `macro_magic`/`const-random`) under `CC0-1.0` (public-domain-equivalent dedication — strictly more permissive than MIT/Apache-2.0, no conditions at all) and `webpki-roots` (bundled Mozilla root CA certificate data) under `CDLA-Permissive-2.0` (an MIT-style permissive license for data). Neither is copyleft; both added to `deny.toml`'s allow list with justification, since Principle III's actual concern is copyleft contamination, not "not yet explicitly enumerated." |

**Incidental fix, found by re-running `cargo deny check` for this audit (not caused by `redb`/
`mongodb`)**: `quick-xml` 0.36.2 (feature 003's `dict-tooling`-only Orchestra-parsing dependency) had a
newly-published advisory, RUSTSEC-2026-0194 (quadratic-time duplicate-attribute-name checking — a CPU
DoS risk when parsing untrusted XML with many attributes on one tag). Fixed by upgrading to `quick-xml`
0.41.0 (the advisory's stated fix version), which required one call-site migration:
`Attribute::unescape_value()` is deprecated in favor of `Attribute::normalized_value(XmlVersion)` in
0.41 — `crates/truefix-dict/src/orchestra.rs` now passes `XmlVersion::Implicit1_0` (FIX Orchestra
sources don't declare an XML version, so the implicit-1.0 default rules apply). Verified
byte-for-byte unchanged output via the existing
`orchestra_conversion.rs::the_bundled_orchestra_fixture_matches_the_shipped_fixlatest_dict` test and
the full `dict-tooling` test suite, all green after the upgrade.

## Feature 004 — Stance Tracking Scaffold (T004) — final sweep (T032)

Config keys/toggles this feature moves from **Recognized** → **Implemented** (FR-009); see
`crates/truefix-config/src/keys.rs` (`APPENDIX_A_KEYS`) for the authoritative per-key registry this
table mirrors.

| Key | Stage | Status |
|-----|-------|--------|
| `ContinueInitializationOnError` | W4 (US4) | **landed** — new `SessionSettings::resolve_lenient()` (resolution-time tolerance) plus `Engine::start`'s per-session loop (startup-time tolerance); stance `Rec` → `Impl` |
| `UseDataDictionary` / `DataDictionary` / `AppDataDictionary` / `TransportDataDictionary` (already marked `Impl`, previously unwired — this feature makes the marking accurate) | W2 (US2) | **landed** — `builder.rs::resolve_validator` reaches `Services.validator` via `ResolvedSession.validator`; stance unchanged (`Impl`), marking now accurate |
| `JdbcURL` | W3 (US3) | **landed** — scheme-dispatched store/log selection in `resolve_store`/`resolve_log`; stance `Rec` → `Impl` |
| `JdbcLogIncomingTable` / `JdbcLogOutgoingTable` / `JdbcLogEventTable` / `JdbcLogHeartBeats` | W3 (US3) | **landed** — consumed building the new `SqlLogSpec`; stance `Rec` → `Impl` |
| `SocketConnectHost1` / `SocketConnectPort1` (already marked `Impl` since feature 002, parsed but not reconnected-through — this feature makes the marking accurate) | W1 (US1) | **landed** — `Engine::start` now routes to `connect_initiator_reconnecting_multi[_tls]` when `failover_addresses` is non-empty; stance unchanged (`Impl`), marking now accurate |

Keys that stay `Recognized` on purpose (US3 scope boundary, not a regression): `JdbcDriver` / `JdbcUser`
/ `JdbcPassword` / `JdbcDataSourceName` (the URL already carries scheme/user/password inline, nothing
left for these to configure) and `JdbcStoreMessagesTableName` / `JdbcStoreSessionsTableName` /
`Jdbc*Connection*` pool settings (`StoreConfig::Sql`/`Mssql` carry only a bare `url: String`, and
`SqlLogSpec` has no pool-settings field — both pre-existing limitations from before this feature, out of
scope to fix here).

`RedbStore`/`RedbLog` (US5) and `MongoStore`/`MongoLog` (US6) introduce no new `.cfg` keys — per
spec/data-model.md, both are library-level additions selectable only via `StoreConfig::Redb`/`Mongo`
through the direct Rust API, matching `SqlLog`/`MssqlLog`'s precedent that not every store/log backend
is `.cfg`-selectable (only `LogConfig::Screen`/`File`/`Tracing`/`Composite` are). See
`docs/todo/001.md`'s GAP-01–GAP-06 entries for the user-facing gap-closure summary.

## Feature 005 — Stance Tracking Scaffold (T002) — final sweep (T099)

Config keys this feature changes stance for (`docs/todo/002.md`'s BUG-03/BUG-04/
doc-accuracy findings, FR-003/004/006/012/013/014/015/016/020/021), tracked here as each landed; see
`crates/truefix-config/src/keys.rs` (`APPENDIX_A_KEYS`) for the authoritative per-key registry.

| Key | Stage | Status |
|-----|-------|--------|
| `JdbcURL` (`jdbc:`-prefixed recognition added) | G2 (US2) | **landed** — `is_sql_scheme`/`is_mssql_scheme` recognize `jdbc:postgresql://`/`jdbc:mysql://`/`jdbc:sqlserver://`/`jdbc:hsqldb:` in addition to the existing sqlx-native schemes; stance unchanged (`Impl`), coverage widened |
| `JdbcUser` / `JdbcPassword` | G2 (US2) | **landed** — spliced into the connection string when the `jdbc:` URL doesn't already embed credentials; stance `Rec` → `Impl` |
| `AllowedRemoteAddresses` / `DynamicSession` / `AcceptorTemplate` (already `Impl`, previously unreachable — this feature makes the marking accurate) | G2 (US2) | **landed** — `Engine::start` now groups `.cfg` sessions sharing a `SocketAcceptPort` into a real multi-session `AcceptorBuilder`; stance unchanged (`Impl`), marking now accurate |
| `SenderSubID` / `SenderLocationID` / `TargetSubID` / `TargetLocationID` / `SessionQualifier` (already `Impl`, previously never actually populated by any builder path — this feature makes the marking accurate) | G5 (US5) | **landed** — `SessionConfig` gained the 5 fields, parsed from `.cfg` and threaded into `SessionId::new_full()`; stance unchanged (`Impl`), marking now accurate |
| `ReconnectInterval` (single value or stepped list) | G6 (US6) | **landed** — `reconnect_delay()` steps through a `Vec<u32>`; stance `Rec` → `Impl` |
| `SocketLocalHost` / `SocketLocalPort` | G6 (US6) | **landed** — bind-before-connect via new `tcp_connect()`; stance `Rec` → `Impl` |
| `SocketConnectTimeout` | G6 (US6) | **landed** — wraps every initiator connect path via `with_connect_timeout()`; stance `Rec` → `Impl` |
| `SocketAcceptProtocol` / `SocketConnectProtocol` (→ `Unsupported`) | G8 (US8) | **landed** — VM_PIPE (in-JVM-process transport) has no Rust equivalent; stance `Rec` → `Unsup` |
| `JdbcDataSourceName` (→ `Unsupported`) | G8 (US8) | **landed** — same JNDI-lookup mechanism as its already-`Unsupported` siblings `JndiContextFactory`/`JndiProviderURL`; stance `Rec` → `Unsup` |
| `JdbcConnectionTestQuery` (→ `Unsupported`) | G8 (US8) | **landed** — `sqlx`'s pool already validates connection liveliness automatically (`test_before_acquire`), no custom-query hook exposed at the `.cfg` level; stance `Rec` → `Unsup` |
| `JdbcMaxActiveConnection` / `JdbcMinIdleConnection` / `JdbcConnectionTimeout` / `JdbcConnectionIdleTimeout` / `JdbcMaxConnectionLifeTime` / `JdbcConnectionKeepaliveTime` / `JdbcStoreMessagesTableName` / `JdbcStoreSessionsTableName` / `JdbcSessionIdDefaultPropertyValue` (9 keys) | G8 (US8) | **landed** — threaded into `StoreConfig::Sql`/`Mssql`'s new `sessions_table`/`messages_table`/`session_id`/`pool: SqlPoolOptions` fields via `jdbc_store_config()`; stance `Rec` → `Impl`. `JdbcConnectionKeepaliveTime` required adding a new `SqlPoolOptions.keepalive: Option<Duration>` field (research.md's finding of "5 pool-tuning siblings" was off by one — there are 6, this one had no existing field at all); parsed and stored but intentionally not wired into `sqlx`'s pool (no matching `sqlx` hook exists), matching the project's established precedent for accepted-but-inert config keys |

Also landed this feature, with no `.cfg`-key-stance change of their own (protocol/codec behavior, not
config surface): `GAP-07`/`08`/`18a` (US3, session-state-machine safeguards), `GAP-09` (US4, chunked
resend), `GAP-38`/`39`/`41` (US7, store/log hardening), and the entire US9 dictionary/codec cluster
(`GAP-22`–`29`/`32`/`33`) — see `docs/acceptance-record.md`'s "005" section for the full narrative and
`docs/todo/002.md` for the struck-through gap citations.
