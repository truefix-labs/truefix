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

- Workspace tests: **green** (212 passing, default features).
- SQL feature tests: **green** (`cargo test -p truefix-store -p truefix-log --features sql`; SQLite
  cases run unconditionally, PostgreSQL/MySQL cases run when `DATABASE_URL_PG`/`DATABASE_URL_MYSQL`
  are set — CI's `sql` job provides both via service containers, see `.github/workflows/ci.yml`).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: **clean** (with and
  without `--features sql`).
- AT conformance suite: **green** (`cargo test -p truefix-at --test conformance`; see corpus below).
- Benchmarks: `cargo bench -p truefix-core` (codec throughput; visibility only, no numeric gate).

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

## Outstanding before a v1 release claim

- Broaden the corpus from one representative scenario per behavior class toward the full Appendix B
  enumeration (additional permutations within already-covered classes).
- Expand the bundled dictionaries from representative subsets to full FIX Orchestra coverage.
