# Acceptance Record

> **Historical evidence record.** This file accumulates feature-by-feature results and therefore
> contains test counts, dependency versions, and “current” statements from different dates. Keep
> them for traceability; use the [documentation index](README.md) for current scope and rerun a
> named command before making a release claim.

Maps the [001 quickstart](../specs/001-fix-engine-parity/quickstart.md) (V1–V9) and
[002 quickstart](../specs/002-qfj-parity-completion/quickstart.md) (V1–V10) validation scenarios to
the automated tests that verify them. All run under `cargo test --workspace` unless noted.

## 001 — FIX engine parity foundation

| Quickstart | What it proves | Verified by |
|------------|----------------|-------------|
| Build & gate | fmt / clippy `-D warnings` / build clean | CI `check` job; local `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings` |
| V1 Codec round-trip & framing (SC-002) | byte-exact BodyLength/CheckSum; no panic on garbled | `truefix-core` `roundtrip.rs`, `reference_vectors.rs`, `garbled.rs`, `groups.rs`, `field_types.rs`, `versions.rs` |
| V2 Live session logon→heartbeat→logout (US1) | two-process handshake on FIX 4.2 & 4.4 | `truefix-transport` `integration_logon.rs` |
| V3 Sequence recovery & resend (US3) | ResendRequest/SequenceReset/PossDup/789 | `truefix-session` `recovery.rs`, `state_machine.rs` |
| V4 Acceptor multi + dynamic session (US4) | routing, dynamic sessions, allow-list refusal | `truefix-transport` `multi_dynamic.rs` |
| V5 Dictionary validation, dual-track (US5) | toggles, two rejection layers, FIXT split, same-source | `truefix-dict` `toggles.rs`, `rejection_layers.rs`, `fixt.rs`, `dual_track.rs`, `versions.rs` |
| V6 Store persistence across restart (US7, SC-006) | file/cached/SQL survive restart; recovery | `truefix-store` `restart_resend.rs` (+ `--features sql`) |
| V7 Config full key coverage (US8, SC-004) | every Appendix A key has a known stance | `truefix-config` `key_coverage.rs` |
| V8 Acceptance Test suite (gate) | scripted server scenarios across versions (see AT corpus below) | `truefix-at` `conformance.rs` |
| V9 Observability (US11, SC-007) | session state/seq/health + reset/force-logout | `truefix-transport` `monitor.rs` |
| TLS / auth / timeouts (US10) | TLS handshake; auth accept/reject; timeouts/latency | `truefix-transport` `tls.rs`, `auth.rs`; `truefix-session` `timeouts.rs` |
| Examples (US13) | executor↔banzai order→ExecutionReport; multi-session | `truefix` `examples_smoke.rs` |

## 002 — QuickFIX/J parity completion

| Quickstart | What it proves | Verified by |
|------------|----------------|-------------|
| V1 Config-driven start (US1, SC-001) | `.cfg`-only acceptor+initiator start; typed `ConfigError` on bad values | `truefix` `config_start.rs` |
| V2 Restart-survivable resend (US2, SC-002) | exact PossDup replay from durable store across a restart | `truefix-session` `restart_resend.rs`, `truefix-transport` `restart_continuity.rs` |
| V3 Repeating-group decode + validation (US3, SC-003) | count/order/nesting/zero-count group validation | `truefix-core` `groups.rs`, `truefix-at` (14i/14j/21/QFJ934) |
| V4 Inbound integrity/reject-layers/reverse-route (US4/US11, SC-004/005) | checksum/length/CompID/sending-time/order/repeated-tag/garbled + reverse-route | `truefix-at` `conformance.rs`, `truefix-session` |
| V5 Typed callback outcomes (US5, SC-006) | `Reject`/`DoNotSend`/`BusinessReject` produce the correct admin/business reply | `truefix-at` `conformance.rs` |
| V6 All-message typed codegen + cracker (US6, SC-007) | typed structs/enums/groups round-trip byte-identically; `crack_<version>` dispatch; dual-track hash | `truefix-dict` (codegen golden + `dual_track.rs`), `truefix` `cracker.rs` |
| V7 TLS/mTLS + socket options/failover (US7/US10, SC-008/011) | config-only mTLS session; min-version refusal; full socket-option set applied; backup-endpoint rotation | `truefix-transport` `tls.rs`, `tls_config.rs`, `socket_options.rs`, `failover.rs` |
| V8 Schedule reset + weekly windows (US8, SC-009) | StartDay/EndDay cross-day windows; disconnect→reset→reconnect boundary semantics | `truefix-session` `schedule.rs`, `schedule_reset.rs` |
| V9 Metrics export (US9, SC-010) | gauges/counters exported and updated across a logon→traffic→reconnect cycle | `truefix-transport` `metrics.rs` |
| V10 Storage/logging completeness (US12, SC-012/014) | PG/MySQL/SQLite SQL store+log; real cached-store eviction+fsync; log output switches; accurate stances | `truefix-store` `sql_backends.rs`/`cached.rs` (`--features sql`), `truefix-log` `switches.rs`/`sql_log.rs`, `truefix-config` `key_coverage.rs`/`store_and_log_mapping.rs`/`socket_and_failover_mapping.rs` |

## Current gate status

- Workspace tests: **green** (`cargo test --workspace --all-features`: 118/118 test binaries passing
  (578 individual tests), 0 failures — grown across features 005 and 006; see the "005"/"006"
  sections below for what drove the growth).
- SQL feature tests: **green** (`cargo test -p truefix-store -p truefix-log --features sql`; SQLite
  cases run unconditionally, PostgreSQL/MySQL cases run when `DATABASE_URL_PG`/`DATABASE_URL_MYSQL`
  are set — CI's `sql` job provides both via service containers, see `.github/workflows/ci.yml`).
- MSSQL feature tests: **green** (`cargo test -p truefix-store -p truefix-log --features mssql`;
  cases skip when `DATABASE_URL_MSSQL` is unset, run for real against CI's new `mssql` job service
  container, see `.github/workflows/ci.yml`).
- `redb` feature tests: **green** (`cargo test -p truefix-store -p truefix-log --features redb`; all
  cases run unconditionally — `redb` is embedded, like SQLite — see the "004" section's US5 entry).
- `mongodb` feature tests: **green** (`cargo test -p truefix-store -p truefix-log --features mongodb`;
  cases skip when `DATABASE_URL_MONGO` is unset, run for real against CI's new `mongo` job service
  container, see `.github/workflows/ci.yml` and the "004" section's US6 entry).
- `dict-tooling` feature tests: **green** (`cargo test -p truefix-dict --features dict-tooling`; the
  Orchestra XML → normalized-`.fixdict` conversion tool, off by default — CI's `dict-tooling` job).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: **clean** (default,
  `--features sql`/`mssql`/`redb`/`mongodb`/`dict-tooling`).
- AT conformance suite: **green** (`cargo test -p truefix-at --test conformance`; 405/405 scenario
  runs across all 9 targeted versions, plus 3 independently-gated special-category suites and a
  regression-floor test — see corpus below, the "003" section's US1-closeout entry, the "005"
  section for the 353→373 growth, and the "006" section for the 373→403→405 growth).
- `cargo deny check`: **clean** (advisories/bans/licenses/sources all `ok` — covers the "006"
  section's new `time-tz` dependency).
- Benchmarks (observation/regression tools; no numeric gate is enforced — Constitution scope, per
  Clarifications):
  - `cargo bench -p truefix-core` (codec encode/decode throughput).
  - `cargo bench -p truefix-session` (session round-trip latency: message-in → processed →
    response-out, for a representative Heartbeat/TestRequest/NewOrderSingle mix; US11, FR-014).

## AT corpus coverage (T085/T086)

The runner exercises every distinct server behavior class across FIX.4.2/4.4 (**56 scenario
classes / 81 scenario runs**, up from 48/76 at the close of 001 — the growth is the 002 additions:
repeating-group malformations, inbound-integrity reject layers, reverse-route, and repeated-tag):

- **Logon**: valid logon, seq-too-high (→ResendRequest), HeartBtInt adoption, ResetOnLogon flag,
  NextExpectedMsgSeqNum/LastMsgSeqNumProcessed reporting.
- **Sequencing**: MsgSeqNum too-high/too-low, PossDup-too-low (ignored), missing-MsgSeqNum reject,
  out-of-order queue + in-order drain.
- **SequenceReset/ResendRequest**: Reset, GapFill (forward + backward-ignored); ResendRequest
  open-ended/bounded/begin-zero/nothing-to-resend/dedup-guard → GapFill.
- **Admin consumption**: Heartbeat and Reject consumed without reply, unsolicited Logout.
- **Field validation (14a–14f + 2r)** on both versions: InvalidTagNumber, RequiredTagMissing,
  TagNotDefinedForMessageType, TagSpecifiedWithoutValue, IncorrectEnumValue, IncorrectDataFormat,
  and the business-level UnregisteredMsgType reject; plus the valid-order accept path.
- **Repeating groups (14i/14j/21/QFJ934, US3)**: correct count, wrong count, out-of-order fields,
  nested group missing its delimiter, and zero-count — each asserting the FIX-correct
  `SessionRejectReason` (including the 14-vs-15 "tag out of order" vs "repeating group fields out
  of order" distinction, a real bug caught and fixed during this feature).
- **Reverse-route (US11)**: `OnBehalfOfCompID`/`DeliverToCompID` reversed on reply/reject; empty
  routing tags handled without producing malformed output.
- **Repeated tag (US4)**: a duplicated non-group tag triggers `TagAppearsMoreThanOnce` (373=13).
- **Application** (active executor app): NewOrderSingle→ExecutionReport round-trip, outbound
  sequencing, application-resend as PossDup, and mixed admin-GapFill + app-PossDup resend.
- **Timer-driven**: idle Heartbeat, TestRequest on counterparty silence, acceptor-initiated Logout
  (these use real 1s ticks; session-level timeout/heartbeat logic is also unit-covered in
  `truefix-session/tests/timeouts.rs`).
- **Special suites**: NextExpectedMsgSeqNum (789), LastMsgSeqNumProcessed (369), CheckLatency/
  timestamps, validateChecksum / rejectGarbledMessages (default-drop), resendRequestChunkSize.
  `resynch` is covered at the transport level by `reconnect.rs` and `restart_continuity.rs`.

The 789/369 work surfaced and fixed a real conformance bug (acceptor Logon stamped sequence state
before consuming the inbound Logon); see `truefix-session` `state_machine.rs`.

## 003 — QuickFIX/J parity closure (in progress)

- **AT full closure (US1, FR-001/002/005, complete)**: `SUITE_VERSIONS` extended from
  `["FIX.4.2", "FIX.4.4"]` to all **9** targeted versions — `["FIX.4.0", "FIX.4.1", "FIX.4.2",
  "FIX.4.3", "FIX.4.4", "FIX.5.0", "FIX.5.0SP1", "FIX.5.0SP2", "FIX.Latest"]` (the last landing with
  US9). The ~34 version-agnostic scenarios (logon/sequencing/admin/resend/reverse-route) now run
  across all 9, and ~20 new named scenarios were authored (identity/CompID mid-session checks,
  `QFJ648_NegativeHeartBtInt`, sequence/PossDup edge cases, `RejectResentMessage`, admin/app
  traffic-pattern suites, plus 3 field-order scenarios from US3: `2t_FirstThreeFieldsOutOfOrder`,
  `14g_HeaderBodyTrailerFieldsOutOfOrder`, `15_HeaderAndBodyFieldsOrderedDifferently`) — the
  conformance suite grew to **353/353 scenario runs passing** (up from 318, 231, ~83 before this
  feature). All **three** special-category suites required by the spec now exist as their own
  discoverable, independently-runnable, independently-gated functions: `validate_checksum_suite()`
  (US3), and `timestamps_suite()`/`resynch_suite()` (US1 closeout) — 4 dedicated conformance tests
  total (`validate_checksum_suite_passes`, `timestamps_suite_passes`, `resynch_suite_passes`,
  plus the main `server_acceptance_suite_passes`). A new `crates/truefix-at/tests/coverage.rs`
  enforces a **regression floor** (9 versions present, ≥353 scenario runs, all 3 special suites
  non-empty with distinct names) as a permanent CI gate against silent corpus shrinkage.
  `docs/todo/001.md`'s TODO-01 remains the authoritative, item-by-item record of every
  individually-deferred scenario name and why (a harness capability this feature didn't build — a
  non-dynamic fixed-identity acceptor mode, predicate-based `ExpectMsg` for outbound
  timestamp-*format*-precision assertions [distinct from the `timestamps_suite()`'s
  CheckLatency-validity coverage, which *is* done], an admin-channel hook in the `Step` model for
  `SessionReset`-style scenarios; dictionary content absent from the bundled subsets [MinQty/tag
  110]; or genuine open product questions this project won't guess the answer to without copying
  QFJ source, e.g. whether a duplicate Logon while already logged on should draw an explicit
  reject). These are deliberate, disclosed scope boundaries, not silent gaps — Principle III forbids
  deriving them from copied QFJ source/test data, so the exact published "73" count is not something
  this project can verify against a vendored list; the 353-run floor and the item-by-item TODO-01
  ledger together are the honest substitute.
- **Session-owned durable resend consistency (US2, FR-003/004)**: the crash-restart resend path
  (`Session::seed_sequences`/`seed_sent_messages`, fed by `run_connection`'s connect-time store
  rehydration) was found to already be correct and already tested
  (`truefix-session/tests/restart_resend.rs`, feature 002). The actual gap was store/in-memory reset
  **consistency**: two internally-triggered full resets (`on_logon`'s `ResetSeqNumFlag`,
  `enter_disconnected`'s `ResetOnLogout`/`ResetOnDisconnect`) had no way to signal the durable store
  to clear itself. Fixed via a new `Action::ResetStore` signal, handled by
  `truefix-transport`'s `perform_actions`. See `docs/parity-matrix.md`'s "Feature 003 — US2" section
  for the full design note.
- **Field-order validation + extra validation toggles (US3, FR-006/007)**: a new
  `Message::fields_out_of_order()` flag, computed by `decode()` while classifying header/body/trailer
  fields (the only point where cross-section wire interleaving is observable), gates
  `ValidationOptions::validate_fields_out_of_order`. Four extra toggles
  (`validate_incoming_message`/`allow_pos_dup`/`requires_orig_sending_time`/`validate_checksum`)
  round out `ValidationOptions`; `validate_checksum` is a documented always-mandatory behavior, not a
  real disable switch (Principle I/II). Found and recorded a pre-existing registry inaccuracy: the
  entire "validation" config-key group has never been wired from `.cfg` to `Engine::start`. See
  `docs/parity-matrix.md`'s "Feature 003 — US3" section.
- **Remaining session config switches (US4, FR-008)**: all 12 previously-inert switches now have a
  determination — 8 genuinely `.cfg`-wired real behaviors, 3 documented intentional no-ops
  (`RejectMessageOnUnhandledException`/`ClosedResendInterval`/`MaxScheduledWriteRequests`), 1 still
  `Recognized` pending the right layer (`ContinueInitializationOnError`, an `Engine::start` concern).
  `LogMessageWhenSessionNotFound` turned out to be acceptor/routing-level, not a `SessionConfig`
  field. Found and fixed a real bug along the way: one `enter_disconnected()` call site (the
  `LoggedOn` heartbeat-timeout path) was missed by an earlier `replace_all` edit in US2 due to
  different indentation, so it wasn't emitting `Action::ResetStore`. See `docs/parity-matrix.md`'s
  "Feature 003 — US4" section.
- **Dictionary component model (US5, FR-009)**: a `component <Name> <members>` directive, resolved
  (with cycle detection) at dictionary-construction time into flat tag lists spliced into referencing
  messages'/groups' member lists — `decode.rs`/`validate.rs` never need to know components exist.
  Found and disclosed (not fixed) a real dual-track gap: `build.rs`'s separate codegen parser doesn't
  understand `component` and would silently drop the reference rather than error, were it ever used in
  a bundled dictionary. See `docs/parity-matrix.md`'s "Feature 003 — US5" section.
- **Runtime dictionary loading (US6, FR-010)**: `load_from_file`/`extend` for custom/extension
  dictionaries, with a two-phase (dry-run-then-apply) merge so a conflict leaves the base dictionary
  untouched rather than partially merged. Found and fixed a real test-isolation race (nanosecond-based
  temp-file naming colliding across parallel test threads — intermittent, only visible under the full
  workspace suite). See `docs/parity-matrix.md`'s "Feature 003 — US6" section.
- **Field type completeness (US7, FR-011)**: `Field::bytes`/`as_bytes` (Data), `utc_date_only`/
  `as_utc_date_only`, `utc_time_only`/`as_utc_time_only` — all round-trip exactly to their FIX wire
  formats. `Field::double`/`as_double` is excluded per the spec's Assumptions (optional per the audit;
  `rust_decimal` already covers Price/Qty). See `docs/parity-matrix.md`'s "Feature 003 — US7" section.
- **FIX Latest support (US9, FR-012)**: the tenth dictionary, sourced from a new build-tooling-only
  Orchestra XML converter (`crates/truefix-dict/src/orchestra.rs`, `--features dict-tooling`,
  `quick-xml`) feeding the same normalized `.fixdict` grammar/pipeline every other version uses — no
  FIX-Latest-specific runtime code. Found and fixed a real gap flagged (but left unfixed) during US5:
  `build.rs`'s own codegen parser didn't understand `component`/`component:<Name>` tokens and would
  silently drop them; `FIXLATEST.fixdict` is the first bundled dictionary to actually use one. AT
  `SUITE_VERSIONS` grew to 9 entries; conformance grew from 318/318 to 353/353 scenario runs passing
  with no new scenario functions needed (the version-agnostic core scenarios already parameterize over
  `SUITE_VERSIONS`). See `docs/parity-matrix.md`'s "Feature 003 — US9" section.
- **Extended application hooks (US10, FR-013)**: `Reject` gains `session_status: Option<u16>`,
  stamped as SessionStatus (tag 573) on the outbound Logout by `reject_logon`; new `Application::
  on_before_reset` (no-op default) fires before every reset. Found the same sans-IO tension as US2:
  `Session::reset()` can't itself call an async `Application` hook, so the transport layer calls it at
  the three points a reset actually takes effect (explicit `Monitor::reset()`,
  `ForceResendWhenCorruptedStore`, and inside the existing `Action::ResetStore` handler). Disclosed a
  small, deliberate exception to "no breaking API changes": the new `Reject` field breaks external
  struct-literal construction without it (both in-repo call sites fixed). See `docs/parity-matrix.md`'s
  "Feature 003 — US10" section.
- **Network hardening (US12, FR-015/016/017)**: PROXY protocol v1/v2 (trusted-upstream-gated, `ppp`
  crate), a forward-proxy client for initiators (SOCKS4/SOCKS5+auth via `tokio-socks`, HTTP CONNECT
  hand-rolled), inline PEM bytes for TLS, configurable cipher suites, and `SocketSynchronousWrites`
  write-timeout — all wired end-to-end through `truefix::Engine::start`, not left as inert config.
  Two more disclosed exceptions to "no breaking API changes" in the same category as US10's `Reject`
  field: `TlsSpec.key_store_path` changes from `PathBuf` to `Option<PathBuf>` (path/inline-bytes are
  now mutually available), and the inline-bytes config surface deliberately deviates from the
  contract's draft three-key sketch to match this codebase's pre-existing combined-keystore design
  (one `SocketKeyStoreBytes` key, not three). See `docs/parity-matrix.md`'s "Feature 003 — US12"
  section.
- **`truefix-dict` CLI (US13, FR-018)**: `generate-dict`/`generate-code`/`validate` subcommands
  wrapping the exact same parse/codegen logic `build.rs` uses (no parallel implementation —
  Principle IV), via a new shared `crates/truefix-dict/src/codegen.rs` module. Found and fixed a
  real correctness gap surfaced by making this logic user-facing: `build.rs`'s codegen internals
  used to `panic!` directly on malformed input, fine for a build script but not for a CLI a user
  feeds arbitrary files to (Principle I) — converted to a proper `CodegenError`/`Result` API;
  `build.rs`'s own `main()` still panics on error (correct for a build script), only now at its own
  top level. See `docs/parity-matrix.md`'s "Feature 003 — US13" section.
- **Inbound backpressure + MSSQL/Oracle SQL backends (US14, FR-019/020)**: `SessionConfig.
  in_chan_capacity: Option<usize>` (`InChanCapacity`) bounds an application-message channel that's
  fully split from an always-unbounded admin/session channel, so a saturated application channel
  never starves Heartbeat/TestRequest/ResendRequest processing; `None` (default) preserves exact
  pre-US14 single-channel ordering. `MssqlStore`/`MssqlLog` add MSSQL via `tiberius` (a separate
  driver from the existing sqlx-backed `SqlStore`/`SqlLog`, since sqlx has no official MSSQL
  support), behind a new independent `mssql` feature. Oracle is confirmed deferred, not implemented,
  per the spec's own pre-approved downgrade path — `oracle`'s Instant Client dependency is
  closed-source under Oracle's OTN terms, incompatible with this project's clean Apache-2.0 OR MIT
  release stance (Principle III). See `docs/parity-matrix.md`'s "Feature 003 — US14" section and its
  updated "Dependency & Provenance Audit (T002)" table for the full Oracle rationale.
- **Config-key stance sweep (T079, FR-021)**: every key this feature moved from
  Recognized/Unsupported to Implemented — the ~12 remaining session switches (US4), field-order +
  extra validation toggles (US3/US8), the network-hardening key set (US12), and `InChanCapacity`
  (US14) — now reads `Impl` in `crates/truefix-config/src/keys.rs`, verified by
  `key_coverage.rs::every_key_has_a_known_stance`. `docs/parity-matrix.md`'s "Stance Tracking
  Scaffold (T004)" table (all rows now `done`) is the authoritative per-stage record. A few keys
  intentionally remain non-`Impl` with a documented reason rather than a false claim of completeness:
  `ClosedResendInterval`/`RejectMessageOnUnhandledException`/`MaxScheduledWriteRequests` stay
  `Unsupported` (no analogous mechanism exists in this architecture — see each key's inline reason
  in `keys.rs`), and `ContinueInitializationOnError` stays `Recognized` (the field round-trips
  through `SessionConfig` but has no effect — its real home would be multi-session bring-up logic in
  `truefix::Engine::start`, not a per-connection `Session` behavior, and building that out wasn't
  part of any 003 user story).

## 004 — Engine wiring & extra backends

Closes GAP-01–GAP-06 from the 2026-07-02 gap analysis (`docs/todo/001.md`). None of the six
gaps were protocol-correctness defects — this feature touches no session-state-machine/codec/protocol
behavior, and the existing 353/353-scenario AT suite staying green **and unmodified** is itself the
release gate (FR-010), not a target for new scenarios.

- **Initiator failover wired into `Engine::start` (US1, GAP-02, FR-001)**: `Engine` gains
  `failover_initiators: Vec<ReconnectHandle>`; when a session's `.cfg` sets `SocketConnectHost1`/
  `SocketConnectPort1` (etc.) and no SOCKS/HTTP proxy is configured, `Engine::start` now routes to the
  existing (previously unused by the facade) `connect_initiator_reconnecting_multi`, plus a new
  `connect_initiator_reconnecting_multi_tls` for the TLS case. Proxy+failover together isn't supported
  yet — a `tracing::warn!` fires and the session falls back to the existing one-shot proxy path rather
  than silently dropping failover. `truefix-transport` `failover_engine.rs` (`truefix` crate,
  `.cfg`-only, dead-primary-port + engine's-own-acceptor-as-backup) and `failover_tls.rs`.
- **`.cfg`-only dictionary/validator wiring (US2, GAP-01, FR-002)**: new
  `builder.rs::resolve_validator` reads `UseDataDictionary`/`DataDictionary`/`AppDataDictionary`/
  `TransportDataDictionary` and resolves either a bundled dictionary (by version string, via
  `truefix_dict::ALL_DICTS`) or a file path (`load_from_file`), producing
  `ResolvedSession.validator: Option<(DataDictionary, ValidationOptions)>` that now actually reaches
  `Services.validator` — previously only settable programmatically. `truefix-config`
  `validator_mapping.rs` (10 tests) + `truefix` `config_start.rs`'s new
  `cfg_only_acceptor_rejects_a_dictionary_invalid_message` end-to-end test.
- **`.cfg`-only SQL backend selection via `JdbcURL` (US3, GAP-05, FR-003)**: `resolve_store`/
  `resolve_log` dispatch on `JdbcURL`'s scheme prefix (`postgres://`/`mysql://`/`sqlite:` vs.
  `mssql://`/`sqlserver://`), scheme-sniffed rather than `JdbcDriver`-class-registry-based (no such
  registry exists in this codebase). Required a new three-layer optional-feature pass-through pattern
  (`truefix-store`/`truefix-log` → `truefix-config` → `truefix` facade) since `truefix-config` now needs
  to *construct* `StoreConfig::Sql`/`Mssql` variants that are feature-gated in a sibling crate. Log side
  gets a new additive `SqlLogSpec` field (not a `LogConfig` enum variant — a design correction made
  mid-implementation after grounding against the actual `LogConfig` shape). `truefix-config`
  `store_and_log_mapping.rs` (+10 tests across all 4 feature combos) + `truefix` `config_start.rs`'s new
  `cfg_only_session_with_jdbc_url_persists_sequence_numbers_across_a_restart` test.
- **`ContinueInitializationOnError` (US4, GAP-06, FR-004)**: new `SessionSettings::resolve_lenient()`
  resolves sessions one at a time, consulting each failed session's own raw `.cfg` for
  `ContinueInitializationOnError` to decide skip-vs-abort — a genuinely new mechanism, not just a loop
  restructuring, because `resolve()`'s `.collect()` is an all-or-nothing barrier that executes *before*
  `Engine::start`'s per-session loop even begins (a design correction found mid-implementation).
  `Engine::start`'s per-session startup body is similarly wrapped to tolerate startup-time (not just
  resolution-time) failures per-session. Stance flips `Recognized` → `Implemented`. `truefix`
  `continue_on_error.rs` (2 tests).
- **`RedbStore`/`RedbLog` (US5, GAP-04, FR-005/006)**: new optional `redb` feature on `truefix-store`/
  `truefix-log`, an embedded transactional KV replacement for QuickFIX/J's obsolete `SleepycatStore`
  (Berkeley DB JE) — chosen over `sled` (same license, but its 1.0 rewrite has been stuck in alpha for
  years). Found a real, load-bearing `redb` design property: `Database::create`/`open` takes an
  exclusive file lock per call, so two independent connections to one file can't coexist in one process
  (unlike networked SQL) — solved with a new `RedbStore::with_session_id` (clones the cheap
  `Arc<Database>`, swaps only `session_id`). No `.cfg` wiring — library-level only, matching
  `SqlLog`/`MssqlLog`'s existing precedent that not every store/log is `.cfg`-selectable.
  `truefix-store` `redb_backend.rs` (4 tests, unconditional — `redb` is embedded like SQLite) +
  `truefix-log` `redb_log.rs` (2 tests).
- **`MongoStore`/`MongoLog` (US6, GAP-03, FR-007/008)**: new optional `mongodb` feature, matching
  QuickFIX/Go's own `MongoStore`/`MongoLog` option (a deliberate reversal of feature 003's own MongoDB
  deferral, now that the actual API surface has been evaluated). Native async `mongodb` crate API
  throughout (no `spawn_blocking`, unlike `redb`); `SessionDoc`/`MessageDoc` collections with a compound
  `(session_id, seq)` unique index. Also library-level only, no `.cfg` wiring, same rationale as US5.
  `truefix-store` `mongo_backend.rs` + `truefix-log` `mongo_log.rs`, both gated on
  `DATABASE_URL_MONGO` (skip-clean without it, real assertions in CI's new `mongo` service-container
  job — no local Docker/MongoDB was available in the implementation environment, so these ran only
  their skip path there).
- **Config-key stance sweep (T032, FR-009)**: `JdbcURL`/`JdbcLogIncomingTable`/`JdbcLogOutgoingTable`/
  `JdbcLogEventTable`/`JdbcLogHeartBeats` flip `Recognized` → `Implemented`;
  `ContinueInitializationOnError` flips `Recognized` → `Implemented`. Two keys already marked `Impl`
  from earlier features get their marking made *accurate* rather than changed:
  `UseDataDictionary`/`DataDictionary`/`AppDataDictionary`/`TransportDataDictionary` (marked `Impl`
  since 003 but never actually wired to `Services.validator` until US2) and `SocketConnectHost1`/
  `SocketConnectPort1` (marked `Impl` since 002 but never actually reconnected-through until US1). See
  `docs/parity-matrix.md`'s "Feature 004 — Stance Tracking Scaffold (T004)" table for the full sweep,
  including which `Jdbc*` keys deliberately stay `Recognized` (URL already carries user/password
  inline; `StoreConfig::Sql`/`Mssql` have no table-name-override field — pre-existing limitations, not
  regressions).
- **Incidental fixes found during T002's dependency audit (not caused by `redb`/`mongodb` themselves)**:
  `quick-xml` 0.36.2 → 0.41.0 for RUSTSEC-2026-0194 (one call-site migration in
  `crates/truefix-dict/src/orchestra.rs`, verified byte-identical output); `mongodb`'s two new
  transitive licenses (`CC0-1.0`, `CDLA-Permissive-2.0`) added to `deny.toml`'s allow-list with
  justification (both strictly permissive, no copyleft).
- **Gate status (T036)**: `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -D
  warnings` clean across default, `--features redb`, and `--features mongodb`; `cargo test --workspace`
  green (**351 passing**, default features); `cargo test -p truefix-store -p truefix-log --features
  redb`/`--features mongodb` green; `cargo test -p truefix-at --test conformance` + `--test coverage`
  confirm the 353/353-scenario baseline is **unchanged** (FR-010); `cargo deny check`:
  `advisories ok, bans ok, licenses ok, sources ok`.

## 005 — Engine gap remediation

Closes every P0/P1 item from the 2026-07-02 full-code audit (`docs/todo/002.md`), spanning
four real bugs, four session-state-machine protocol-correctness gaps, and a long tail of
session/transport/store-log/dictionary-codec feature-completeness gaps — the largest being full parity
with QuickFIX/J's bundled dictionary field/message coverage across all 9 targeted versions. Unlike 004,
this feature touches session-state-machine/codec/protocol behavior, so the AT suite is expected to
*grow*, not stay unmodified. See `docs/todo/002.md` for the closed `GAP-##`/`BUG-##`
citations (struck through) and `specs/005-engine-gap-remediation/tasks.md` for full per-task disclosure.

- **Correctness bugs: `#` truncation, Signature/93 mapping (US1, BUG-01/02, FR-001/002)**: `strip_comment`
  in `crates/truefix-config/src/lib.rs` now only treats `#` as a comment start at the first
  non-whitespace position on a line (previously truncated mid-value, e.g. `Password=ab#cd` → `ab`);
  `data_field_for_length()` in `crates/truefix-core/src/tags.rs` gained the tag-93→89 exception (every
  other length↔data pair follows `lengthTag = dataTag - 1`; 89/`Signature` is the one documented
  exception). `truefix-config` `comment_truncation.rs`, `truefix-core` `signature_length.rs`.
- **`.cfg` keys that misrepresent behavior: multi-session `AcceptorBuilder` wiring + real JDBC URLs (US2,
  BUG-03/04, FR-003/004/005/006/006a)**: `Engine::start` now groups `.cfg` `[SESSION]` blocks sharing a
  `SocketAcceptPort` into one real multi-session `AcceptorBuilder` (real fix, not a stance-registry
  downgrade — `AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` now actually govern
  connection acceptance and template resolution end-to-end); `is_sql_scheme`/`is_mssql_scheme` in
  `crates/truefix-config/src/builder.rs` recognize standard JDBC-style `jdbc:<subprotocol>://...` URLs
  (in addition to TrueFix's existing sqlx-native scheme forms) and splice in separately-configured
  `JdbcUser`/`JdbcPassword` when the URL doesn't already embed credentials. `truefix-config`
  `jdbc_url_mapping.rs`, `truefix-transport`/`truefix` multi-session acceptor tests.
- **Session protocol-correctness safeguards: resend veto, PossDup anti-replay, duplicate-Logon rejection
  (US3, GAP-07/08/18a, FR-007/008/009/010)**: `Application::to_app` (the veto point already existed) now
  actually suppresses a stale application-message resend and substitutes a `GapFill`, via a new
  `Action::Resend(Message, u64)` variant threaded through `perform_actions`; the `PossDupFlag=Y`+
  `seq < expected` early-drop path in `on_received` now validates `OrigSendingTime <= SendingTime`,
  gated by a new `requires_orig_sending_time`-adjacent config switch, logging out and disconnecting on
  violation; a second Logon while already `LoggedOn` is now rejected rather than silently ignored.
  AT suite grew with `app_resend_veto_produces_gap_fill`, `poss_dup_orig_sending_time_after_sending_time`
  (all 9 versions), `duplicate_logon_rejected` (all 9 versions).
- **Automatic inbound chunked-resend continuation (US4, GAP-09, FR-011)**: `Session` gained
  `resend_target`/`resend_chunk_end` fields; a new `maybe_continue_chunked_resend()` helper (called at
  the end of `drain_queue()`) automatically issues the next `ResendRequest` once the current chunk is
  satisfied, without waiting for the counterparty to ask. `truefix-session`
  `multi_chunk_inbound_resend_auto_continues_without_an_external_resend_request`; AT
  `chunked_resend_auto_continues`.
- **Session identity completeness: `SessionQualifier` and sub-ID/location-ID (US5, GAP-47,
  FR-012/013)**: `SessionId::new_full()` (8-arg constructor) and 5 new `SessionConfig` fields
  (`sender_sub_id`/`sender_location_id`/`target_sub_id`/`target_location_id`/`session_qualifier`), parsed
  from `.cfg`. Two sessions sharing BeginString/SenderCompID/TargetCompID but differing only by
  `SessionQualifier` now produce distinct `SessionId`s and can both log on independently. `truefix-config`
  `session_identity_mapping.rs`, `truefix`
  `two_sessions_differing_only_by_qualifier_both_start_and_log_on`.
- **Initiator connection robustness: reconnect backoff array, local bind, connect timeout (US6,
  GAP-14/15/16, FR-014/015/016)**: `reconnect_delay()` in `crates/truefix-transport/src/lib.rs` steps
  through a configured `Vec<u32>` (`ReconnectInterval`, single value or list), sticking at the last value
  and resetting on successful connect; new `tcp_connect()`/`with_connect_timeout()` helpers wire
  `SocketLocalHost`/`SocketLocalPort` (bind-before-connect) and `SocketConnectTimeout` into every
  initiator connect path (plain, TLS, and both reconnecting-multi variants). `truefix-config`
  `initiator_robustness_mapping.rs`, `truefix-transport` `reconnect.rs`
  (`reconnect_interval_steps_are_honored_by_the_reconnect_loop`,
  `initiator_connects_from_the_configured_local_bind_address`).
- **Store/log persistence hardening: creation-time, atomic save+advance, log timestamp/session-identity
  (US7, GAP-38/39/41, FR-017/018/019)**: `MessageStore` trait gained `creation_time()` and
  `save_and_advance_sender()` (defaulted to sequential calls, overridden with real transactions in
  `SqlStore`/`MssqlStore`/`RedbStore`; `MongoStore` intentionally left at the trait default — no
  multi-doc transaction guarantee without a replica set, disclosed not silent); every structured log
  backend (`SqlLog`/`MssqlLog`/`RedbLog`/`MongoLog`) gained `logged_at`/`session_id` columns/fields.
  `truefix-store` `creation_time_file.rs` + per-backend extensions; `truefix-log` per-backend log tests.
- **Config-key stance registry accuracy sweep (US8, FR-020/021, depends on US2)**:
  `SocketAcceptProtocol`/`SocketConnectProtocol`/`JdbcDataSourceName`/`JdbcConnectionTestQuery` downgraded
  to `Unsupported` with documented reasons (VM_PIPE/JNDI have no Rust equivalent; `sqlx` has no
  custom-query pool-liveliness hook); the 9 JDBC pool-tuning/table-name keys threaded into
  `StoreConfig::Sql`/`Mssql`'s new `sessions_table`/`messages_table`/`session_id`/`pool` fields and
  promoted to `Implemented`. `truefix-config` `store_and_log_mapping.rs`.
- **Dictionary and codec model completeness (US9, GAP-22–29/32/33, FR-022–031)**: the largest single
  story — 11 new `FieldType` variants (`PriceOffset`/`LocalMktDate`/`DayOfMonth`/`UtcDate`/`Time`/
  `Currency`/`Exchange`/`MultipleValueString`/`MultipleStringValue`/`MultipleCharValue`/`Country`), open
  enums (`open_enum: bool`), per-group child dictionaries (`GroupDef.child`) enabling deep nested-group
  validation (also fixed a real, previously-undetected gap: group-member fields were never validated at
  all, since `FieldMap::fields()` skips group members by design), repeating-group
  `replace`/`remove`/`get`-by-index, header/trailer repeating-group decode, custom per-message field
  order (`encode_with_order`), structured dictionary version metadata + BeginString-match validation,
  value→label lookup. Closed GAP-33 (bundled dictionary content parity) via a new QFJ-XML→`.fixdict`
  converter (`crates/truefix-dict/src/qfj_xml.rs`, `--features dict-tooling`) that fully regenerated all
  8 non-Orchestra bundled dictionaries to real QFJ scale (FIX40: 139 fields/27 messages … FIX50SP2: 1610
  fields/110 messages, up from ~35/8) — chosen over a manual per-version diff after discovering the true
  scale of the gap, per explicit user direction. Found and fixed along the way: component-required
  semantics (a component's own `required` attribute means "the component as a whole," not "every field
  in it"), three Rust-identifier-safety bugs surfaced by real QFJ data (digit-leading enum labels, a
  `yield`-keyword collision, a `SecurityStatus` field/message name collision), and a QFJ upstream data
  quirk (`BeginString`/`CheckSum` misclassified as `CHAR` in FIX40/41's own XML). `truefix-dict`
  `field_types_extended.rs`, `version_meta.rs`, `dictionary_coverage.rs` (byte-for-byte regeneration
  regression test), extended `group_validation.rs`/`dual_track.rs`/`cli.rs`; `truefix-core`
  `groups.rs`/`field_order_encode.rs`/`header_trailer_groups.rs`.
- **AT suite growth (SC-013)**: the conformance suite grew from **353/353 to 373/373 scenario runs**
  passing (US3's 3 new scenario families across 9 versions + US4's chunked-resend scenario) — a
  deliberate, disclosed regression-floor bump in `truefix-at` `coverage.rs`
  (`server_suite_scenario_run_count_does_not_regress`), the opposite of 004's "stays unmodified" gate,
  per this feature's own Constitution Principle II framing.
- **Gate status**: `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings`
  clean across default, `--features sql`/`mssql`/`redb`/`mongodb`/`dict-tooling`; `cargo test --workspace
  --all-features` green (92/92 test binaries passing, 0 failures); `cargo test -p truefix-at --test
  conformance` confirms 373/373 scenario runs; `cargo test -p truefix-at --test coverage` confirms the
  floor bump is intentional and disclosed; `cargo deny check` clean.

## 006 — Audit remediation

Closes every P0/P1 item from the 2026-07-02/03 pre-production audit (`docs/todo/003.md`), produced
by 5 parallel deep-dive reviews (session/state-machine, transport+config, codec/dictionary,
store/log, Engine facade+AT harness) plus a supplementary Chinese-language pass (`B1`-`B30`). Six
fresh P0 bugs, six P1 bugs, five "closed-but-incomplete" re-openings from 005, seven of the twelve
P1 feature-completeness gaps carried forward from `002.md`, eleven P2 minor bugs/tooling gaps, two
AT-harness follow-ups, and every cited `B##` supplementary finding. Like 005, this feature touches
session-state-machine/codec/protocol behavior, so the AT suite grew rather than staying unmodified.
See `docs/todo/003.md` for the audit's own citations and `specs/006-audit-remediation/tasks.md`
(T001-T093) for full per-task disclosure, including test names and live-verification notes.

- **Session anti-replay/desync holes (US1, `BUG-05`/`BUG-06`/`BUG-22`/`B3`/`B5`/`B7`,
  FR-001–009)**: `on_logon` now rejects a Logon whose own `MsgSeqNum` is below `next_in_seq` with no
  PossDup justification (Logout+disconnect, before any state mutation); plain-mode `SequenceReset`
  now rejects a decreasing `NewSeqNo` and a missing `NewSeqNo`; `ResendRequest` missing
  `BeginSeqNo`/`EndSeqNo` now draws `RequiredTagMissing` (distinct from the retained
  begin>end silent no-op); an initiator with `ResetOnLogon` now performs its own reset proactively
  on connect instead of relying on a race with the acceptor's reply; a message drained from the
  out-of-order queue after a gap-fill is now validated identically to an in-order message; a
  dictionary-validation-failure disconnect now sends Logout first; an unparseable `SendingTime` now
  fails the latency check instead of silently passing. AT suite grew 8 new scenario families across
  `SUITE_VERSIONS` (373 → 403 runs).
- **Multi-session acceptor routing (US2, `BUG-07`, FR-010/011/012)**: `route_and_run`'s live-routing
  lookup key now extracts SubID/LocationID (tags 50/142/57/143) from the inbound Logon, not just the
  3 required fields — a session registered with those fields populated (005's `GAP-47`) was
  previously permanently unroutable. A grouped acceptor with a `SessionQualifier` collision is now
  rejected at config-resolve time; each statically-registered session in a group now gets its own
  independent `MessageStore` (discovered mid-implementation: sharing one store across concurrently
  connected sessions in a group corrupted each session's own sequence bookkeeping — a regression
  this fix's own bisection surfaced and closed). `AcceptorBuilder::bind` now always enables
  `SO_REUSEADDR`.
- **Silent store failures and spurious mid-window resets (US3, `BUG-08`/`BUG-09`/`BUG-14`/`BUG-15`/
  `GAP-39`/`GAP-48`/`GAP-49`, FR-013–017)**: every previously-swallowed store-operation failure in
  `truefix-transport` now routes through the log as an operator-visible event; a process restart
  landing inside an already-active schedule window now consults the store's persisted creation time
  instead of hardcoding "not yet in session," closing a spurious full-reset-on-every-restart bug;
  `FileStore`/`CachedFileStore` now override `save_and_advance_sender` with real seq-first-then-body
  atomicity (the two file-backed stores 005's `GAP-39` fix never reached); `BodyLog::reset()` now
  syncs when `FileStoreSync=Y`; `MssqlStore` now validates table identifiers before use, matching
  its SQL-store sibling.
- **QuickFIX/J JDBC URL grammar (US4, `BUG-10`/`GAP-55`, FR-018–020)**: `jdbc:h2:` is now a typed
  `UnsupportedBackend` error (never silently misrouted to a SQLite file named after the raw scheme
  string); `MssqlStore::parse_url` gained a second accepted grammar for real semicolon-delimited
  QuickFIX/J MSSQL URLs, additive to the existing path-based form (disclosed public-API surface
  growth); `JdbcUser`/`JdbcPassword` values are now percent-encoded before splicing into a URL.
- **Engine lifecycle (US5, `BUG-11`/`BUG-16`/`BUG-21`, FR-021–023)**: a partial multi-session
  `Engine::start` failure now cleanly stops every already-started acceptor/initiator instead of
  leaving them orphaned and running uncontrollably; a grouped acceptor's conflicting
  `ContinueInitializationOnError` values now resolve by strictest-member-wins (any `N` fails the
  whole group), not just the first member's flag; `Engine::shutdown()`'s doc comment now accurately
  discloses it doesn't touch plain (non-failover) initiators.
- **Network hardening (US6, `BUG-13`/`BUG-19`/`GAP-53`/`GAP-54`/`B14`/`B30`, FR-024–029)**: inbound
  frame assembly is now bounded (`MAX_BODY_LEN`, 16 MiB) instead of buffering an attacker-declared
  `BodyLength` unboundedly; a proxy connect attempt that never completes its handshake now times
  out (previously only plain/TLS direct-connects had a timeout); a `CipherSuites` value matching
  zero recognized suites is now a clean config-time error instead of an opaque handshake failure; a
  trusted-source PROXY-header peek now times out instead of hanging the connection indefinitely; a
  malformed/non-FIX prefix now discards only itself, not the legitimate message that followed it
  (previously cleared the whole buffer); the transport crate's own duplicate `framing.rs` was
  deleted in favor of the shared `truefix-core` implementation.
- **Dictionary/codec correctness (US7, `BUG-12`/`GAP-26`/`GAP-27`/`GAP-33`/`GAP-56`, FR-030–034)**:
  `FieldType::value_ok` now format-checks `UtcTimeOnly`/`UtcDate`; `tags.rs` gained the real header
  tags (627/628/629/630, `NoHops`'s group — the audit's own citation of 504 was independently
  verified wrong against the shipped dictionary and corrected) and 6 missing `EncodedXxxLen`↔`EncodedXxx`
  pairs (the audit's citations of 620→621/1039→1040 were also independently verified wrong and
  corrected); codegen now honors a message's declared `fieldOrder` on the real production encode
  path (previously parsed but never applied — `GAP-27`, entirely dormant); the production decode
  path now wires header/trailer repeating-group decode via a new `HeaderTrailerGroupsOnly` adapter
  (previously `decode_with_groups` was never called from any production path — `GAP-26`, entirely
  dormant; wiring it in surfaced and fixed a real pre-existing gap in the primitive itself, which
  never tracked `fields_out_of_order`); all 8 legacy `.fixdict` provenance headers now name the
  current `fix_repository.rs` tool instead of the deleted `qfj_xml.rs` (`GAP-56`/`GAP-33`'s stale
  citation).
- **Carried-forward feature-completeness gaps (US8, `GAP-10`/`11`/`12`/`18c`/`19`/`21`/`44`,
  FR-035–041)**: recurring daily `ResetSeqTime`/`EnableResetSeqTime` sequence reset, independent of
  Enter/Exit window transitions; `LogonTag`/`LogonTag1`/`LogonTag2`/… multi-tag support (was exactly
  one pair); inbound Logon's `DefaultApplVerID` (tag 1137) auto-extraction plus a real
  `FixtDictionaries` transport/application split reachable from `.cfg` (previously aliased to one
  dictionary) and selected per-message via tag 1128, falling back to the negotiated tag 1137, falling
  back to the dictionary's own default; dynamic-session templates now carry SubID/LocationID through
  from the inbound Logon, not just BeginString/SenderCompID/TargetCompID; `.cfg`-selectable
  `ScreenLog`/`TracingLog`/`CompositeLog` via a new `Log` key; IANA timezone names for `TimeZone`
  (new `time-tz` dependency, BSD-3-Clause, license-checked via `cargo deny`), DST-aware unlike the
  pre-existing fixed-offset form; `${var}` interpolation now falls back to environment variables.
- **AT harness coverage (US9, `BUG-17`, FR-042/043)**: a new `MinQty` (tag 110) scenario across
  FIX.4.2/FIX.4.4 proves the field — previously absent from every bundled dictionary, now present
  since 005's dictionary-coverage work — is accepted, not rejected as undefined; the regression
  floor was bumped twice (373 → 403 at US1 closeout, 403 → 405 at US9 closeout) to track the true
  count exactly at each stage rather than drifting stale again.
- **Tooling / latent-risk hygiene (US10, `GAP-50`/`BUG-18`/`GAP-51`/`BUG-20`, FR-044–046)**:
  `flatten_members` gained a depth-16 recursion guard matching its sibling `resolve_entries`
  (previously unbounded — latent, not live against the 9 real vendored sources, but a genuine risk
  for a future/different Repository edition); `parse_messages`'s doc comment corrected to describe
  its actual behavior (it never registered a synthetic `id_by_name`/`by_id` entry as previously
  claimed — the adjacent panicking-index concern the audit raised was independently traced and
  found provably safe by construction, requiring no code change); `BodyLog::append`'s offset
  determination now happens inside the same lock as the write and index-insert, closing a TOCTOU
  race between concurrent writers (verified as a genuine regression: reverting the fix reproduced
  concrete data corruption under 64 concurrent writers before being re-applied).
- **Gate status**: `cargo fmt --check` clean; `cargo clippy --workspace --all-targets --all-features
  -- -D warnings` clean; `cargo test --workspace --all-features` green (578 tests passed, 0
  failures — grown across feature 006); `cargo test -p truefix-at --test conformance` confirms
  405/405 scenario runs, including the new `MinQty` scenario; `cargo test -p truefix-at --test
  coverage` confirms the floor exactly matches; `cargo deny check` clean (covers the new `time-tz`
  dependency's full transitive license tree).

## 007 — Second audit remediation

Closes every confirmed defect from the 2026-07-03 second-pass audit (`docs/todo/004.md`), produced by
a pure per-crate code review plus a systematic QuickFIX/J + QuickFIX/Go source comparison, itself
carrying two internal self-verification passes — a per-item "Verification Pass" (which retracted 3
originally-cited `BUG-23`–`BUG-85` items outright, corrected 3, retracted 1 more) and a four-agent
"二次交叉验证" cross-check against the current working tree (which found 3 items — `BUG-37`/`BUG-98`/
`BUG-101` — already fixed as a side effect of feature 006). Numbering continues `BUG-23` onward from
`003.md`'s `BUG-05`–`BUG-22`. 13 must-fix-before-shipping items (US1), 16 narrower-blast-radius
defects (US2), and 20 low-priority hardening/hygiene items (US3) — 51 FRs total (`FR-001`–`FR-050` +
`FR-001a`). One pass, staged US1→US2→US3 (P1→P2→P3), all 119 tasks completed with TDD (failing test
first) + bisection-verified fixes throughout. See `docs/todo/004.md` for the audit's own citations and
`specs/007-second-audit-remediation/tasks.md` (T001-T119) for full per-task disclosure.

- **Store durability and crash-safety (US1, FR-001/001a/002-004)**: sender/target sequence numbers now
  persist in two separate files (`senderseqnums`/`targetseqnums`, matching QuickFIX/J's own layout)
  instead of one combined file whose partial write could silently desync both counters; a sequence file
  that fails to parse as valid recorded state now surfaces as a typed, catchable error instead of
  silently defaulting to `(1, 1)`; an existing single-file deployment auto-migrates transparently to
  the two-file layout on first open after upgrading (disclosed, additive on-disk format change);
  `SqlStore::ensure_schema` now applies its `creation_time` column migration before any row references
  it; `MssqlStore::save_and_advance_sender` now rolls back on a failed final commit instead of leaving
  the transaction open.
- **Protocol-breaking handshake and deadlock fixes (US1, FR-005-009)**: the `ResetSeqNumFlag` handshake
  now round-trips correctly on both acceptor (echoes `Y` iff the inbound Logon requested it) and
  initiator (verifies the echo, or infers a reset from `MsgSeqNum=1`) sides — previously a reconnecting
  counterparty's legitimate `Logon(MsgSeqNum=1, ResetSeqNumFlag=Y)` could be intercepted by the
  too-low-seq check before the reset was ever honored; a `ResendRequest`-vs-`ResendRequest` deadlock
  (both sides simultaneously waiting on their own outstanding request) is now answered immediately
  rather than queued; a second inbound connection presenting an already-active session identity is now
  refused; an unexpected TCP drop (not a graceful Logout) now honors `ResetOnDisconnect` identically to
  a Logout-driven disconnect.
- **Resource-leak and lifecycle fixes (US1, FR-012/013)**: a scheduled or reconnecting initiator whose
  active connection task drops now reconnects on its next eligible attempt instead of treating the slot
  as permanently occupied; `Engine::shutdown()` (and a new `impl Drop for Engine`, disclosed additive
  public-API surface growth) now stops every spawned session task including plain (non-failover)
  initiators, closing a leak where such tasks previously had no way to be reached once started.
  Disclosed additive public API: `SessionHandle::abort`/`is_finished` (sync, non-consuming), `impl Drop
  for Engine`, and a new `Event::Disconnected` variant.
- **Malformed-input and enforcement gaps (US1, FR-014-016)**: a message declaring `BodyLength=0` is now
  rejected as malformed rather than accepted; an acceptor with a configured trading-hours schedule now
  rejects a Logon outside that window and tears down an already-logged-on session that crosses out of
  it, matching the initiator-side schedule enforcement that already existed; admin-typed messages (a
  malformed Logon, etc.) are now dictionary-validated the same way application messages already were.
- **Callback-ordering restructure (US1, FR-010/011)**: a message is no longer delivered to the
  application's `from_admin`/`from_app` callback until after the session layer's own
  sequence/identity/latency/PossDup/dictionary checks have already passed — an application can no
  longer observe a message the session layer is about to reject.
- **Narrower-blast-radius defects (US2, FR-017-030)**: a stale `resend_requested`/queue-suppression
  state no longer survives a reconnect and masking a genuine new gap; a non-Logon message arriving
  before the session completes its Logon exchange (`validLogonState`) is now rejected rather than
  processed, including no longer drawing a pre-logon `ResendRequest`; an inbound Logon with an
  impossible `NextExpectedMsgSeqNum`, a missing `MsgSeqNum`, or a negative `HeartBtInt` is now rejected
  with a clear reason; a length-prefixed data field (tag 95/96 and friends) not followed by its
  matching data tag, or with a non-numeric length, now fails decoding cleanly instead of silently
  misparsing what follows; the plain reconnecting initiator path now applies the same socket options
  (`TcpNoDelay`, `KeepAlive`, buffer sizes) every other connection path already applies; an acceptor
  group sharing one listen port now resolves each session's own log/dictionary-validator/socket-options/
  TLS configuration consistently rather than pulling from whichever member happened to be first;
  outbound `Reject` now includes `RefMsgType(372)` on `FIX.4.2`+ sessions and omits an out-of-range
  `SessionRejectReason(373)` on `FIX.4.0`/`4.1`; the PossDup anti-replay check now also applies when an
  inbound sequence number equals (not just is below) the expected next-incoming number; any of
  `ResetOnLogon`/`ResetOnLogout`/`ResetOnDisconnect` (not only `ResetOnLogon`) now triggers
  `ResetSeqNumFlag=Y` on a first-ever connection; a scheduled initiator's retry now backs off across a
  small number of attempts instead of tight-looping sub-second reconnects; a processed gap-fill
  `SequenceReset` now discards now-superseded queued messages instead of retaining them indefinitely; a
  stray Logon in `AwaitingLogout`/`Disconnected` state is now rejected; a TCP drop mid-handshake (before
  either side reaches `LoggedOn`) now still fires the application's logout callback; an MSSQL
  store/log URL's credentials containing `@` now split correctly; `FileStore`/`CachedFileStore` opening
  against a long-running session's history no longer reads every stored message body into memory just
  to rebuild its offset index or warm its cache.
- **Low-priority hardening and hygiene (US3, FR-031-050)**: frame-length and checksum arithmetic no
  longer silently wraps on inputs exceeding `usize`/`u32` bounds (defense-in-depth atop the existing
  `MAX_BODY_LEN` bound); `HeartBtIntTimeoutMultiplier` now preserves fractional precision instead of
  truncating to an integer; repeating-group dictionary validation now recurses into required-field and
  field-type/enum checks inside each group entry, not only at the message's top level; `CHAR` fields on
  `FIX.4.0`/`4.1` now accept multi-character values (matching those versions' plain-text treatment);
  `.cfg` `TimeStampPrecision` parsing is now case-insensitive and `SocketUseSSL`/other Y/N switches now
  reject an unrecognized value instead of silently coercing to a default; a resent message's refreshed
  `SendingTime` now uses the session's configured timestamp precision instead of a hardcoded
  millisecond precision; an oversized peer `HeartBtInt` is now clamped instead of silently truncated;
  `ResetOnError` is now honored on the low-sequence-without-PossDup rejection path, matching the
  identity/latency paths; heartbeats continue during `AwaitingLogout` instead of stopping the moment
  logout begins; a `ResendRequest`'s `EndSeqNo=999999` on `FIX.4.2`-or-earlier sessions is now treated
  as the version-range's documented "resend to highest sent" infinity convention; the multi-endpoint
  reconnecting initiator now prefers its primary endpoint again after a successful backup-endpoint
  reconnect later drops; `ReconnectHandle::stop()` now tears down a currently-active connection instead
  of only suppressing future attempts; an acceptor now refuses a connection whose first message isn't a
  Logon; the transport's internal admin-message channel now applies bounded capacity/backpressure
  instead of growing unbounded; a timestamp's fractional-seconds digit count other than 0/3/6/9 is now
  rejected, and a leap second (`:60`) now maps to 59s + max sub-second fraction instead of being
  rejected outright; frame detection now confirms the checksum-position bytes actually look like a
  checksum field, and that the buffer's leading bytes form a recognizable `BeginString`, before
  accepting a frame boundary; a decoded message with no `MsgType` field is now rejected; the dictionary
  codegen path (`--features dict-tooling`) now handles a field type outside its explicit type table
  (`TIME`/`PRICEOFFSET`), a Rust-keyword-colliding enum-value label, and the `open` enum modifier
  consistently with the runtime (non-codegen) dictionary path.
- **AT harness growth**: the server-suite scenario-run regression floor grew from 405 (006's closing
  state) to 424 (`crates/truefix-at/tests/coverage.rs`'s `server_suite_scenario_run_count_does_not_regress`)
  — the +19 runs come from new scenarios across US1/US2 (admin-message-dictionary-validation,
  schedule-enforcement, and several `ResetSeqNumFlag`/resend/reject-correctness scenarios). A
  cross-cutting fix (`truefix_at::runner::wire_begin_string`) was needed alongside the new
  `BeginString`-format check (FR-048) because the AT harness's `"FIX.Latest"` testing sentinel is not a
  real wire `BeginString` value — it now translates to `"FIX.5.0SP2"` on the wire while dictionary
  selection logic is untouched.
- **Gate status**: `cargo fmt --check` clean; `cargo clippy --workspace --all-targets --all-features --
  -D warnings` clean (0 warnings); `cargo test --workspace --all-features` green (174 test-result
  blocks, 0 failures, up from 118/578 at 006's close — 716 tests passing); `cargo test -p truefix-at
  --test conformance` and `--test coverage` both green, confirming 424/424 scenario runs; `cargo deny
  check` clean (`advisories ok, bans ok, licenses ok, sources ok`) — no new external dependency was
  introduced anywhere in this feature.

## Not addressed by 007 — disclosed, not silently dropped

Per Constitution Principle VII (inventory-based completeness) and this feature's own
`spec.md` Assumptions section, the following items `docs/todo/004.md` raised are **not** closed by this
feature, called out explicitly rather than left to be discovered by a future reader assuming "007
closed everything the second-pass audit found":

- **`BUG-37`, `BUG-98`, `BUG-101`** — confirmed already fixed as a side effect of feature 006 (verified
  directly against current source before `spec.md` was written); no action needed.
- **The "Exchange field case-sensitivity" item** from `004.md`'s "Partially covered" table — fully
  retracted by the document's own second verification pass (neither QuickFIX/J nor QuickFIX/Go enforce
  case on that field; the original claim mischaracterized both references).
- **`BUG-108`** (heartbeat-timeout-multiplier default value differs from QuickFIX/J's `1.4` and
  QuickFIX/Go's `1.2`) — `004.md`'s own conclusion is this is an intentional difference (TrueFix's
  default is more lenient, not incorrect), not a defect. (The overflow/lost-precision defect in the
  *same area*, `BUG-36`, was still in scope and is closed — see US3 above.)
- **`BUG-104`** (no session enabled/disabled concept), **`BUG-110`** (`ClosedResendInterval` documented
  no-op), **`BUG-111`** (a new `Session` object per connection rather than one persisting across
  reconnects) — `004.md` itself classifies each as an intentional design choice or a documented,
  accepted no-op, not a correctness defect; `BUG-111` is explicitly the architectural root cause of
  several in-scope items (`BUG-32`/`BUG-42`/`BUG-93`, all closed above) rather than an independently
  actionable item itself.
- **`BUG-106`** — a literal duplicate of `BUG-83` (already covered under US2's `OrigSendingTime`
  scenario above); not separately tracked.
- **`BUG-65`** (`parse_utc_offset` overflow requiring a `.cfg` offset field value of `hours > ~596523`,
  far beyond any valid UTC offset) — a confirmed-correct code-level observation `004.md` itself assesses
  as having negligible practical risk; deferred, consistent with this project's practice of not
  hardening against inputs with no realistic trigger.
- **`BUG-73`** (codegen enum-label sanitization for a hypothetical custom dictionary using a
  Rust-keyword-named enum value, never observed in any bundled dictionary) — `spec.md`'s Assumptions
  section lists this as excluded/negligible-risk alongside `BUG-65`, but it was in fact bundled into and
  closed by US3's `T113`/`T114` (`FR-050`) as low-marginal-cost hardening alongside the in-scope
  `BUG-72`/`BUG-74` codegen fixes it shares a code path with. Flagged here as a disclosed
  spec-vs-implementation discrepancy in the more-thorough direction (closed despite being listed as
  deferred), not a gap.
- AT-harness scenario authoring for `1d_InvalidLogonNoDefaultApplVerID` remains unwritten (a 006
  follow-up item, unrelated to 007's own scope) and the harness-limited scenarios needing a
  multi-connection `Step` primitive remain blocked, as disclosed under 006 above — 007 did not revisit
  either.

## Not addressed by 006 — disclosed, not silently dropped

Per Constitution Principle VII (inventory-based completeness), the following items `docs/todo/003.md`
raised are **not** closed by this feature, called out explicitly rather than left to be discovered by
a future reader assuming "006 closed everything the audit found":

- **`GAP-28`/`GAP-32`** (dictionary version metadata / BeginString-match validation — "mechanism
  correct, but orphaned from real data"): zero shipped `.fixdict` files declare a `version-meta`
  directive, so the already-implemented validation logic never fires against any dictionary TrueFix
  actually ships. This did not end up covered by any of `spec.md`'s 46 FRs or US8's 7-item
  acceptance-scenario enumeration — an omission during specification, not a deliberate deferral with
  a stated reason (unlike the items below). Flagged here for a future feature to pick up: either add
  `version-meta` directives to the shipped `.fixdict` sources, or reconfirm whether the check itself
  still earns its keep with zero live callers.
- Explicitly low-priority/deferred-with-reason in `docs/todo/003.md` itself, not reconfirmed as
  requiring a fix: `GAP-13` (QFJ-only `RejectReason::code()` refinement), `GAP-17` (`AllowedRemoteAddresses`
  union-not-per-session — reconfirmed as intentional/documented behavior, not a defect), `GAP-20`
  (transport routing key scope — its unroutability *consequence* was `BUG-07`, now closed; the key's
  own scope is unchanged), `GAP-31` (timestamp precision truncation, confirmed numerically-safe),
  `GAP-34` (no `toXML` diagnostic dump), `GAP-35`/`GAP-46` (QuickFIX/Go-only optional behaviors),
  `GAP-36` (no offline FIX log-file batch parser), `GAP-37` (`MessageStore` lifecycle hooks,
  reconfirmed non-issue given TrueFix's sans-IO single-owner architecture), `GAP-40` (full `Log`
  trait severity-level support — the narrower, higher-priority slice of this concern that blocked
  `BUG-08` was closed by routing store-failure signals through the existing `on_event`, without the
  full trait change), `GAP-42` (unbounded `mpsc` channels in 4 background-writer log backends),
  `GAP-45` (`SessionSettings` immutability — architectural, not recommended as near-term work),
  `GAP-52` (O(fields × enums) enum-emission, build-time only, informational).
- AT-harness scenarios still blocked on harness limitations, not feature gaps: `1c_InvalidSenderCompID`/
  `1c_InvalidTargetCompID`/`1d_InvalidLogonWrongBeginString` (dynamic-template acceptor adopts
  whatever identity the first Logon claims); `1b_DuplicateIdentity`/`20_SimultaneousResendRequest`
  (the runner drives exactly one `TcpStream` per scenario, no multi-connection `Step` primitive).
  Note `1d_InvalidLogonNoDefaultApplVerID` is now *technically* unblockable-no-longer — US8's T079
  wired a real `FixtDictionaries` split reachable from `.cfg` — but no scenario was authored for it
  in this pass (US9's own scope was `MinQty` only); a real, actionable follow-up for a future
  iteration.
- Two purely-cosmetic documentation-accuracy notes from `docs/todo/003.md`'s own closing section
  (a `002.md` addendum pointing at `docs/todo/003.md` for the `GAP-33` citation; a doc-comment on
  `MongoStore` explaining its intentional `save_and_advance_sender` non-override) were not applied —
  low-value, no behavioral stakes either way.

## Outstanding before a v1 release claim

- Broaden the corpus from one representative scenario per behavior class toward the full Appendix B
  enumeration (additional permutations within already-covered classes).
- `GAP-28`/`GAP-32` (dictionary version-meta validation orphaned from real data — see "Not addressed
  by 006" above; the one item from `docs/todo/003.md` that fell through specification, not a
  deliberate deferral).
- The remaining low-priority/deferred-with-reason items from `docs/todo/003.md` not closed by 006:
  `GAP-13`/`17`/`20`/`31`/`34`/`35`/`36`/`37`/`40`/`42`/`45`/`46`/`52` (see "Not addressed by 006"
  above for why each is deferred, not overlooked).
- AT scenario authoring for `1d_InvalidLogonNoDefaultApplVerID` (unblocked by 006's US8 FIXT work,
  not yet written) and the harness-limited scenarios needing a multi-connection `Step` primitive.
- The remaining excluded/deferred items from `docs/todo/004.md` not closed by 007 (see "Not addressed
  by 007" above): `BUG-65` (unrealistic-trigger overflow), the design-choice items `BUG-104`/`108`/
  `110`/`111`, and the duplicate `BUG-106`.
- `GAP-28`/`GAP-32`'s dictionary version-metadata inertness (flagged again during 007's own `T091`/
  `T092`, since `.fixdict` files still only declare a plain `version` line, never the separate
  `version-meta` directive the check reads) remains unresolved — still the one item from `003.md` that
  fell through specification, now reconfirmed live in 007 too.
