# Quickstart: Validating Feature 003

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Runnable scenarios that prove each user story works end-to-end. Run from the repo root. These
supplement, not replace, the full `truefix-at` suite and unit/integration tests `/speckit-tasks` will
enumerate.

## Prerequisites

- Rust toolchain per `Cargo.toml` (`rust-version = "1.96"`).
- `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` green on the branch before
  validating any scenario below.
- Optional (only for backend-specific scenarios): a local PostgreSQL/MySQL/MSSQL instance reachable via
  `DATABASE_URL`-style env vars, matching the pattern feature 002 established for Postgres/MySQL CI
  service containers.

## US1/US2 — Conformance & durable resend

```bash
cargo test -p truefix-at                      # full AT suite, all targeted versions
cargo test -p truefix-session -- resend        # session-owned resend unit/integration tests
```
Expected: 100% pass across fix40/41/42/43/44/50/50SP1/50SP2 (+fixLatest once US9 lands) and the three
special-category suites; a scripted process-restart test replays a pre-restart ResendRequest range with
`PossDupFlag=Y`.

## US3/US4/US8 — Validation completeness

```bash
cargo test -p truefix-dict -- validate
cargo test -p truefix-session -- config_switches
```
Expected: field-order violations rejected only when `validate_fields_out_of_order=true`; each of the 12
session switches and 4 extra validation toggles has a passing dedicated test.

## US5/US6/US7/US9 — Dictionary model & FIX Latest

```bash
cargo test -p truefix-dict -- component
cargo test -p truefix-dict -- extend
cargo test -p truefix-core -- field_types
cargo test -p truefix-at -- fixlatest
```
Expected: component-using messages validate identically to hand-inlined equivalents; a runtime-loaded
extension dictionary validates custom fields; Data/UtcDateOnly/UtcTimeOnly round-trip exactly; a
FIX-Latest-configured session completes logon-to-heartbeat.

## US10 — Extended application hooks

```bash
cargo test -p truefix-session -- application_hooks
```
Expected: a logon predicate refusing a CompID with a chosen `SessionStatus` produces that value on the
outbound Logout/Reject; `on_before_reset` fires before every reset.

## US11 — Benchmark (observation only)

```bash
cargo bench -p truefix-session session
```
Expected: a reported latency distribution; no pass/fail — for manual regression comparison only.

## US12 — Network hardening

```bash
cargo test -p truefix-transport -- proxy_protocol
cargo test -p truefix-transport -- socks
cargo test -p truefix-transport -- http_connect
cargo test -p truefix-transport -- tls_inline_pem
cargo test -p truefix-transport -- cipher_suites
```
Expected: PROXY headers honored only from a configured trusted upstream; SOCKS4/SOCKS5(+auth)/
HTTP-CONNECT initiator connections succeed through a local test proxy; TLS establishes from inline PEM
bytes; a restricted cipher-suite list is enforced.

## US13 — Dictionary CLI

```bash
cargo run -p truefix-dict --bin truefix-dict -- validate --dict crates/truefix-dict/dict-src/normalized/FIX44.fixdict
```
Expected: prints "OK" plus the dual-track hash for a known-good dictionary file.

## US14 — Backpressure & additional SQL backends

```bash
cargo test -p truefix-transport -- backpressure
cargo test -p truefix-store --features mssql -- mssql
```
Expected: a saturated bounded application channel blocks the reader without dropping messages, while
concurrent admin traffic (heartbeat) is still processed; MSSQL store/log tests pass when a service
instance is reachable (skipped otherwise, matching the Postgres/MySQL precedent).

## Full gate

```bash
cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test --workspace && cargo test -p truefix-at
```
Expected: green — this is the release gate (SC-015).
