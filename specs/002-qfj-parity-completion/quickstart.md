# Quickstart / Validation Guide: QuickFIX/J Parity Completion

Runnable checks proving each stage's behaviour. References the [contracts](./contracts/) and
[data-model](./data-model.md); does not duplicate implementation. The full gate
(`cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`)
plus the AT conformance suite must stay green at every stage (SC-013).

## Prerequisites

- Rust toolchain (workspace MSRV); `cargo`.
- For SQL multi-backend checks: `DATABASE_URL_PG` / `DATABASE_URL_MYSQL` env vars pointing at reachable
  Postgres/MySQL instances (CI service containers); SQLite needs nothing. Tests skip a backend whose URL
  is absent.
- For TLS/mTLS checks: test certs are generated in-test (`rcgen`), as in the existing `tls.rs`.

## V1 — Config-driven start (US1 / G3 / SC-001)

- Run the two-process integration test that loads a fixture `.cfg` with one acceptor + one initiator.
- Expected: initiator connects, both reach logon→heartbeat; store/log/schedule reflect the file.
- Negative: a fixture with a bad recognised-key value ⇒ `ConfigError{key,session,kind}`, nothing started.

```bash
cargo test -p truefix --test config_start
```

## V2 — Restart-survivable resend (US2 / G1 / SC-002)

- Test sends app messages to a file store, drops the process, restarts against the same store, then a
  counterparty ResendRequest replays exact PossDup bodies; admin ranges gap-fill; `PersistMessages=N`
  gap-fills entirely.

```bash
cargo test -p truefix-session --test restart_resend
cargo test -p truefix-transport --test restart_continuity
```

## V3 — Repeating-group decode + validation (US3 / G2 / SC-003)

- Core decode tests cover correct + wrong-count + missing-delimiter + out-of-order + nested + zero-count.
- AT scenarios assert the FIX-correct reject for each malformed group.

```bash
cargo test -p truefix-core --test groups
cargo test -p truefix-at --test conformance     # 14i/14j/21/QFJ934 + existing
```

## V4 — Inbound integrity, reject layers, garbled, reverse route, precision (US4/US11 / G4 / SC-004/005)

- New AT scenarios for bad-checksum, body-length, begin-string, CompID, sending-time, field-order,
  repeated-tag, garbled (RejectGarbledMessage on/off), reverse-route.
- Session tests for timestamp precision round-trip and the correctness switches.

```bash
cargo test -p truefix-at --test conformance
cargo test -p truefix-session
```

## V5 — Typed callbacks (US5 / G5 / SC-006)

- Tests with an `Application` returning `Reject` / `DoNotSend` / `BusinessReject`; assert refused logon,
  suppressed send (seq not consumed), and an emitted 35=j carrying reason+ref tag.

```bash
cargo test -p truefix-at --test conformance
```

## V6 — Typed codegen + cracker (US6 / G6 / SC-007)

- Build emits typed artifacts; round-trip equality (typed vs generic bytes); cracker dispatch; dual-track
  hash holds.

```bash
cargo build -p truefix-dict
cargo test -p truefix-dict        # codegen golden + dual-track hash
cargo test -p truefix --test cracker
```

## V7 — TLS/mTLS + socket/failover (US7/US10 / G7 / SC-008/011)

```bash
cargo test -p truefix-transport --test tls        # incl. mTLS from config + min-version refusal
cargo test -p truefix-transport --test failover   # backup-endpoint rotation; socket options applied
```

## V8 — Schedule reset + weekly windows (US8 / G8 / SC-009)

```bash
cargo test -p truefix-session --test schedule
cargo test -p truefix-session --test schedule_reset
```

## V9 — Metrics export (US9 / G9 / SC-010)

- Run a logon→traffic→reconnect cycle and assert the exported gauges/counters via a test metrics
  recorder.

```bash
cargo test -p truefix-transport --test metrics
```

## V10 — Storage/logging completeness (US12 / G10 / SC-012/014)

```bash
cargo test -p truefix-store --features sql        # PG/MySQL gated on URL; SQLite always; cached store
cargo test -p truefix-log   --features sql
cargo test -p truefix-config --test key_coverage  # stances updated (SC-014)
```

## Full gate (every stage exit; SC-013)

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p truefix-at --test conformance
```
