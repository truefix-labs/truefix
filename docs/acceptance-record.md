# Acceptance Record

Maps the [quickstart](../specs/001-fix-engine-parity/quickstart.md) validation scenarios (V1–V9)
to the automated tests that verify them (T098). All run under `cargo test --workspace`.

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
| V8 Acceptance Test suite (US12, gate) | 21 scripted scenarios / 35 runs across versions | `truefix-at` `conformance.rs` |
| V9 Observability (US11, SC-007) | session state/seq/health + reset/force-logout | `truefix-transport` `monitor.rs` |
| TLS / auth / timeouts (US10) | TLS handshake; auth accept/reject; timeouts/latency | `truefix-transport` `tls.rs`, `auth.rs`; `truefix-session` `timeouts.rs` |
| Examples (US13) | executor↔banzai order→ExecutionReport; multi-session | `truefix` `examples_smoke.rs` |

## Current gate status

- Workspace tests: **green** (123 passing, default features; plus the SQL store test under
  `--features sql`).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: **clean**.
- Benchmarks: `cargo bench -p truefix-core` (codec throughput; visibility only, no numeric gate).

## AT corpus coverage (T085/T086)

The runner exercises every distinct server behavior class across FIX.4.2/4.4:

- **Logon & sequencing**: valid logon, logon seq-too-high (→ResendRequest), MsgSeqNum
  too-high/too-low, PossDup-too-low (ignored).
- **Admin**: TestRequest→Heartbeat, unsolicited Logout, SequenceReset-Reset, ResendRequest→GapFill.
- **Field validation (14a–14f + 2r)**: InvalidTagNumber, RequiredTagMissing,
  TagNotDefinedForMessageType, TagSpecifiedWithoutValue, IncorrectEnumValue, IncorrectDataFormat,
  and the business-level UnregisteredMsgType reject.
- **Special suites**: NextExpectedMsgSeqNum (789), LastMsgSeqNumProcessed (369), CheckLatency/
  timestamps, validateChecksum / rejectGarbledMessages (default-drop), resendRequestChunkSize.
  `resynch` is covered at the transport level by `reconnect.rs` and `restart_continuity.rs`.

The 789/369 work surfaced and fixed a real conformance bug (acceptor Logon stamped sequence state
before consuming the inbound Logon); see `truefix-session` `state_machine.rs`.

## Outstanding before a v1 release claim

- Broaden the corpus from one representative scenario per behavior class toward the full Appendix B
  enumeration (additional permutations within already-covered classes).
- Expand the bundled dictionaries from representative subsets to full FIX Orchestra coverage.
