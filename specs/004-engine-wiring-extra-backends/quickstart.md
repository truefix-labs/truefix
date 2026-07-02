# Quickstart: Validating Feature 004

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Runnable scenarios that prove each user story works end-to-end. Run from the repo root.

> **Lesson from feature 003's own quickstart** (found and fixed during that feature's Polish phase):
> `cargo test -- <substring>` filters match against *test function names*, not file names — a filter
> that doesn't match any `#[test] fn` silently reports `0 passed; ... ok`, a false-green. Every command
> below uses `--test <file>` (runs the named integration-test binary in full) instead, to avoid
> repeating that mistake.

## Prerequisites

- Rust toolchain per `Cargo.toml` (`rust-version` pin).
- `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` green on the
  branch before validating any scenario below.
- Optional (US6 only): a local MongoDB instance reachable via `DATABASE_URL_MONGO`, matching the
  pattern feature 003 established for MSSQL's CI service container.

## US1 — Failover-capable initiator purely from `.cfg`

```bash
cargo test -p truefix-transport --test failover_tls
cargo test -p truefix --test failover_engine
```
Expected: an initiator configured with `.cfg` numbered backup endpoints reconnects to a backup after
the primary is killed, with no Rust code beyond `Application` callbacks.

## US2 — Dictionary validation purely from `.cfg`

```bash
cargo test -p truefix --test config_start
cargo test -p truefix-config --test validator_mapping
```
Expected: a `.cfg`-only session with `UseDataDictionary=Y` rejects a dictionary-invalid message
identically to a session whose `Services.validator` was wired programmatically.

## US3 — SQL backend selection purely from `.cfg`

```bash
cargo test -p truefix-config --test store_and_log_mapping
cargo test -p truefix --test config_start --features sql
```
Expected: a `.cfg`-only session with `JdbcURL=sqlite:...` (or `postgres://`/`mysql://`/`mssql://` when
reachable) persists sequence numbers/messages and survives a restart.

## US4 — Continue starting other sessions on a configuration error

```bash
cargo test -p truefix --test continue_on_error
```
Expected: a multi-session `.cfg` acceptor with `ContinueInitializationOnError=Y` and one misconfigured
session starts every other, validly-configured session; the same file with the flag unset fails
startup entirely.

## US5 — Embedded transactional store (`redb`)

```bash
cargo test -p truefix-store -p truefix-log --features redb
```
Expected: sequence numbers and message bodies persist across a process restart against the same file;
`reset()` atomically clears both; two session identities sharing one file stay isolated.

## US6 — MongoDB store

```bash
cargo test -p truefix-store -p truefix-log --features mongodb
```
Expected: the same conformance contract as US5, skipping cleanly when `DATABASE_URL_MONGO` is unset.

## Full gate

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo test -p truefix-store -p truefix-log --features redb
cargo test -p truefix-store -p truefix-log --features mongodb
cargo test -p truefix-at --test conformance
```
Expected: green — AT suite stays at its current scenario count, unmodified (SC-006); this feature adds
no protocol behavior for it to cover.
