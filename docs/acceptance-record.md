# Acceptance Record

Maps the [001 quickstart](../specs/001-fix-engine-parity/quickstart.md) (V1–V9) and
[002 quickstart](../specs/002-qfj-parity-completion/quickstart.md) (V1–V10) validation scenarios to
the automated tests that verify them. All run under `cargo test --workspace` unless noted.

## 001 — FIX engine parity foundation

| Quickstart | What it proves | Verified by |
|------------|----------------|-------------|
| Build & gate | fmt / clippy `-D warnings` / build clean | CI `check` job; local `cargo fmt --check && cargo clippy --workspace --all-targets -D warnings` |
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

- Workspace tests: **green** (329 passing, default features — grown substantially across feature 003;
  see the "003" section below for what drove the growth).
- SQL feature tests: **green** (`cargo test -p truefix-store -p truefix-log --features sql`; SQLite
  cases run unconditionally, PostgreSQL/MySQL cases run when `DATABASE_URL_PG`/`DATABASE_URL_MYSQL`
  are set — CI's `sql` job provides both via service containers, see `.github/workflows/ci.yml`).
- MSSQL feature tests: **green** (`cargo test -p truefix-store -p truefix-log --features mssql`;
  cases skip when `DATABASE_URL_MSSQL` is unset, run for real against CI's new `mssql` job service
  container, see `.github/workflows/ci.yml`).
- `dict-tooling` feature tests: **green** (`cargo test -p truefix-dict --features dict-tooling`; the
  Orchestra XML → normalized-`.fixdict` conversion tool, off by default — CI's `dict-tooling` job).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: **clean** (with and
  without `--features sql`/`dict-tooling`).
- AT conformance suite: **green** (`cargo test -p truefix-at --test conformance`; 353/353 scenario
  runs across all 9 targeted versions, plus 3 independently-gated special-category suites and a
  regression-floor test — see corpus below and the "003" section's US1-closeout entry).
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
  `docs/todo-gap-analysis.md`'s TODO-01 remains the authoritative, item-by-item record of every
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

## Outstanding before a v1 release claim

- Broaden the corpus from one representative scenario per behavior class toward the full Appendix B
  enumeration (additional permutations within already-covered classes).
- Expand the bundled dictionaries from representative subsets to full FIX Orchestra coverage.
