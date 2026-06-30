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
| V8 Acceptance Test suite (US12, gate) | scripted scenarios pass across versions | `truefix-at` `conformance.rs` |
| V9 Observability (US11, SC-007) | session state/seq/health + reset/force-logout | `truefix-transport` `monitor.rs` |
| TLS / auth / timeouts (US10) | TLS handshake; auth accept/reject; timeouts/latency | `truefix-transport` `tls.rs`, `auth.rs`; `truefix-session` `timeouts.rs` |
| Examples (US13) | executor↔banzai order→ExecutionReport; multi-session | `truefix` `examples_smoke.rs` |

## Current gate status

- Workspace tests: **green** (113 passing, default features; plus the SQL store test under
  `--features sql`).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: **clean**.
- Benchmarks: `cargo bench -p truefix-core` (codec throughput; visibility only, no numeric gate).

## Outstanding before a v1 release claim

- Port the full Appendix B AT corpus (67 remaining server scenarios + special suites) onto the
  runner (T085/T086) and run the all-versions gate (FR-M3).
- Finish deferred items: full 789 sync + LastMsgSeqNumProcessed (T040), SQL log (T059), the
  schedule-driven connector loop (T070).
- Expand the bundled dictionaries from representative subsets to full FIX Orchestra coverage.
