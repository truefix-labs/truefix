# Quickstart & Validation Guide: TrueFix

This guide proves the engine works end-to-end. It is a run/validation guide — implementation lives in
`tasks.md` and the crates. Commands assume the cargo workspace from [plan.md](./plan.md).

## Prerequisites
- Rust stable toolchain (MSRV pinned in CI); `cargo`, `cargo-deny`, `cargo-nextest` (optional).
- No external services required for core validation. The SQL store/log validation needs a database
  (SQLite file is sufficient for tests).

## Build & gate checks
```bash
cargo build --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check            # license/advisory gate (Constitution III)
cargo test --workspace
```
Expected: all green; clippy denies any `unwrap`/`expect` on critical paths via lint config.

## Validation scenarios (map to user stories)

### V1 — Codec round-trip & byte-exact framing (US2 / SC-002)
```bash
cargo test -p truefix-core
```
Expected: round-trip vectors pass; BodyLength/CheckSum match the reference vectors for NewOrderSingle,
ExecutionReport, Logon, Heartbeat, ResendRequest, SequenceReset, Reject. Garbled/truncated inputs return
typed errors (no panic).

### V2 — Live session: logon → heartbeat → logout (US1)
```bash
cargo test -p truefix-transport --test integration_logon   # two in-process peers
# or run the examples in two terminals:
cargo run -p executor -- examples/executor/executor.cfg
cargo run -p banzai   -- examples/banzai/banzai.cfg
```
Expected: Logon exchange with matching HeartBtInt; sustained heartbeats; TestRequest/Heartbeat round
trip; clean bilateral Logout. Validated on FIX 4.2 and 4.4/FIXT 1.1.

### V3 — Sequence recovery & resend (US3)
```bash
cargo test -p truefix-session --test recovery
```
Expected: high-seq triggers ResendRequest + queue; ResendRequest reply uses PossDupFlag=Y +
OrigSendingTime and collapses admin gaps to SequenceReset-GapFill; low-seq w/o PossDup disconnects.

### V4 — Acceptor multi-session + dynamic session (US4)
```bash
cargo test -p truefix-transport --test multi_dynamic
```
Expected: two static sessions + one template-matched dynamic session all reach logged-on; connection from
a disallowed remote is refused.

### V5 — Dictionary validation, dual-track (US5)
```bash
cargo test -p truefix-dict
```
Expected: per-toggle accept/reject outcomes correct; FIXT 1.1 transport/application split + DefaultApplVerID
resolution; codegen/runtime same-source assertion passes.

### V6 — Store/log persistence across restart (US7 / SC-006)
```bash
cargo test -p truefix-store --test restart_resend
```
Expected: after simulated restart, sequence numbers and resend capability recovered (file/cached/SQL).

### V7 — Config: full Appendix A key coverage (US8 / SC-004)
```bash
cargo test -p truefix-config --test key_coverage
```
Expected: every Appendix A key resolves to "implemented" or "documented-unsupported-with-reason";
`${name}` interpolation and DEFAULT/SESSION precedence verified.

### V8 — Acceptance Test suite (US12 / FR-M3) — RELEASE GATE
```bash
cargo test -p truefix-at                 # full matrix
cargo run  -p truefix-at -- --report     # scenario × version pass/deferred report
```
Expected: all targeted FIX versions pass their in-scope AT scenarios; any deferral explicitly listed with
a reason. This is the conformance gate.

### V9 — Observability (US11 / SC-007)
While V2/V4 run, the monitoring surface reports each session's logged-on status, next sender/target
sequence numbers, and connection health; an operational reset action is reflected in that state.

## Done signal
All of V1–V9 green, `cargo deny` clean, and the AT report showing every targeted version passing (modulo
explicitly-justified per-scenario deferrals) constitutes the v1 parity acceptance per the spec Success
Criteria and the `parity.md` checklist.
