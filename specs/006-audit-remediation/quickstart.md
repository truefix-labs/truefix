# Quickstart: Validating Feature 006

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Runnable scenarios that prove each user story works end-to-end. Run from the repo root.

> **Lesson from feature 003/005's own quickstarts** (re-applied here): `cargo test -- <substring>`
> filters match against *test function names*, not file names — a filter that doesn't match any
> `#[test] fn` silently reports `0 passed; ... ok`, a false-green. Every command below uses `--test
> <file>` (runs the named integration-test binary in full) instead.
>
> **T092 update (implementation-time correction)**: this file's commands were originally written
> speculatively during `/speckit-plan`, before `/speckit-tasks`/`/speckit-implement` had chosen
> concrete test file names — every "# new" file name below was a placeholder guess. All commands
> have now been corrected to the actual file/module names landed during implementation, and each
> one was re-run live to confirm it passes before this file was updated (not just renamed on
> paper).

## Prerequisites

- Rust toolchain per `rust-toolchain.toml` (1.96.0 pin).
- `cargo test --workspace --all-features` and `cargo clippy --workspace --all-targets --all-features
  -- -D warnings` green before validating any scenario below (confirmed live at Polish-phase
  closeout: 578 tests passed, 0 failed; clippy clean).
- Baseline at `/speckit-plan` time (2026-07-03): `cargo run -p truefix-at --example count_check` (a
  throwaway, uncommitted example) → **373/373 AT scenario-runs**, matching the audit's claimed count
  exactly. Final count after this feature: **405** (see US9 below).
- No new external services required — the `sql`/`mssql`/`redb`/`mongodb` feature-gated backends this
  feature touches (US3, US4) already have their own CI service containers from prior features
  (001-005). One new dependency was added: `time-tz` (US8/T082, IANA timezone names — see research.md
  §R8 and this feature's completion disclosure for the license-compliance check).

## US1 — Session protocol correctness

```bash
cargo test -p truefix-session --lib
cargo test -p truefix-session --test state_machine
cargo test -p truefix-session --test recovery
cargo test -p truefix-session --test timeouts
cargo test -p truefix-session --test validation_hook
cargo test -p truefix-at --test coverage
```
Expected: a Logon with `MsgSeqNum` below `next_in_seq` (no PossDup) is rejected with Logout+disconnect,
not accepted; a decreasing plain-mode `SequenceReset` is rejected, not applied; a `ResendRequest`
missing `BeginSeqNo`/`EndSeqNo` gets a required-tag-missing response; an initiator with `ResetOnLogon`
reconnects cleanly on every attempt (no repeating gap→resend→reject-duplicate-Logon loop); a message
drained after a gap-fill is validated like any in-order message; reconnecting resets per-connection
timers; a dictionary-validation-failure disconnect sends Logout first; an unparseable `SendingTime`
fails the latency check. AT suite scenario-run count grows past 373 (to 403 after this story).

## US2 — Multi-session acceptor routing

```bash
cargo test -p truefix-transport --test multi_dynamic
cargo test -p truefix --test session_qualifier
cargo test -p truefix-transport --test socket_options
```
Expected: two acceptor sessions sharing one `SocketAcceptPort`, distinguished by SubID/LocationID/
SessionQualifier, each correctly route a live Logon from their respective identity; a
SessionQualifier-ambiguous group configuration is rejected at config-resolve time with a clear error;
an `AcceptorBuilder`-bound listener rebinds immediately after a prior process releases the port.

## US3 — Store/log persistence and durability

```bash
cargo test -p truefix-transport --test store_error_logging
cargo test -p truefix-transport --test scheduled
cargo test -p truefix-store --test restart_resend
cargo test -p truefix-store --test reset_ordering
cargo test -p truefix-store --features mssql --test mssql_backend
```
Expected: a simulated store failure produces a log event; a process restart landing inside an active
schedule window preserves sequence numbers/history (no spurious reset); `FileStore`/`CachedFileStore`
survive a simulated crash between body-write and seq-advance without desync (seq-first-then-body
ordering); an invalid MSSQL table identifier fails cleanly at config time.

## US4 — QuickFIX/J JDBC URL grammar compatibility

```bash
cargo test -p truefix-config --test store_and_log_mapping --features sql,mssql
```
Expected: `JdbcURL=jdbc:h2:mem:quickfixj` fails with a clear unsupported-backend error (never opens a
SQLite file); `JdbcURL=jdbc:sqlserver://localhost:1433;databaseName=quickfixj` (real semicolon
grammar) parses and resolves; `JdbcUser`/`JdbcPassword` values containing `@`/`:`/`/` splice into a
valid URL.

## US5 — Engine lifecycle

```bash
cargo test -p truefix --test continue_on_error
```
Expected: a `.cfg` where one grouped acceptor member fails to bind leaves the already-started
sessions cleanly stopped, not orphaned, once `start()` returns its error; two grouped sessions with
conflicting `ContinueInitializationOnError` values resolve to strictest-wins (any `N` makes the group
fail-fast).

## US6 — Network hardening

```bash
cargo test -p truefix-transport --test framing_bounds
cargo test -p truefix-transport --test proxy_client
cargo test -p truefix-transport --test proxy_protocol
cargo test -p truefix-transport --test tls_hardening
```
Expected: a connection declaring an oversized `BodyLength` is closed, not buffered unboundedly; a
proxy connect attempt that never completes its handshake times out; a misconfigured `CipherSuites`
value fails at config time; a trusted-source connection with an incomplete PROXY header times out
instead of hanging; a legitimate message following a malformed prefix is preserved, not discarded.

## US7 — Dictionary/codec correctness

```bash
cargo test -p truefix-dict --test field_types_extended
cargo test -p truefix-dict --test field_order_production
cargo test -p truefix-transport --test header_trailer_groups_production
cargo test -p truefix-core --test encoded_leg_pairs
cargo test -p truefix-dict --test provenance_headers
```
Expected: a malformed `UtcTimeOnly`/`UtcDate` value fails dictionary validation; a message with a
declared `fieldOrder` is encoded in that order on the real (codegen) encode path; a `NoHops`
header/trailer group decodes and validates correctly on the real production decode path; embedded SOH
bytes in `EncodedLeg*` fields don't corrupt framing; no shipped `.fixdict` still names the deleted
`qfj_xml`-era conversion tool in its provenance header.

## US8 — Carried-forward feature-completeness gaps

```bash
cargo test -p truefix-session --lib schedule_reset   # T076: recurring ResetSeqTime
cargo test -p truefix-session --test schedule        # T076: recurring ResetSeqTime (.cfg-level)
cargo test -p truefix-session --test config_switches  # T077: multi-tag LogonTag
cargo test -p truefix-config --test session_switches_mapping  # T077: LogonTag/LogonTag1/…
cargo test -p truefix-session --test state_machine    # T078: inbound DefaultApplVerID (tag 1137)
cargo test -p truefix-session --test validation_hook  # T079: real FixtDictionaries, per-message
cargo test -p truefix-config --test validator_mapping # T079: FixtDictionaries resolution from .cfg
cargo test -p truefix-transport --test multi_dynamic  # T080: wildcard SubID/LocationID templates
cargo test -p truefix-config --test store_and_log_mapping  # T081: Log=Screen/Tracing/Composite
cargo test -p truefix --test config_start             # T081: Log backend selection, live engine
cargo test -p truefix-session --lib "schedule::"      # T082: IANA timezone DST-awareness
cargo test -p truefix-config --test schedule_mapping  # T082: TimeZone=America/New_York etc.
cargo test -p truefix-config --lib                    # T083: ${var} env-var fallback
```
Expected: each targeted test demonstrates the specific previously-missing behavior now works
end-to-end (see spec.md US8 acceptance scenario 1's full list).

## US9 — AT harness coverage

```bash
cargo test -p truefix-at --test coverage
cargo test -p truefix-at --test conformance
```
Expected: `coverage.rs`'s asserted floor matches the suite's actual `server_suite()` scenario-run
count exactly (405); `conformance.rs`'s `server_acceptance_suite_passes` actually executes every one
of those 405 runs live, including the new `MinQty` (tag 110) scenario across FIX.4.2 and FIX.4.4.

## US10 — Tooling / latent-risk hygiene

```bash
cargo test -p truefix-dict --features dict-tooling --lib fix_repository
cargo test -p truefix-store --test body_log_concurrency
```
Expected: a self-referencing component chain fails with a `FlattenTooDeep` error instead of
recursing unboundedly; 64 concurrent `BodyLog`-backed `save()` callers (via a shared `FileStore`) no
longer race on the recorded byte offset — every message round-trips with its correct content.

## Full regression pass

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p truefix-at --test coverage
cargo test -p truefix-at --test conformance
cargo deny check   # confirms the new time-tz dependency's license tree (US8/T082)
```
Expected: zero regressions against the pre-feature baseline (405/405 AT scenario-runs, up from 373 —
strictly grew, never shrank; no test-count decrease anywhere in the workspace — SC-007/SC-008).
