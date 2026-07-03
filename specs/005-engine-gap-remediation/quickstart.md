# Quickstart: Validating Feature 005

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Runnable scenarios that prove each user story works end-to-end. Run from the repo root. Test file
names below are the target shape `/speckit-tasks`/`/speckit-implement` should produce (per each
`contracts/*.md`'s "Test hooks" section) — none exist yet as of this plan; update this file if
`/speckit-tasks` lands a different concrete file name for any of them.

> **Lesson from feature 003's own quickstart** (re-applied by every feature since): `cargo test --
> <substring>` filters match against *test function names*, not file names — a filter that doesn't
> match any `#[test] fn` silently reports `0 passed; ... ok`, a false-green. Every command below uses
> `--test <file>` (runs the named integration-test binary in full) instead.

## Prerequisites

- Rust toolchain per `Cargo.toml` (`rust-version` pin).
- `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` green before
  validating any scenario below (baseline: 351 tests passing, 353/353 AT scenarios, confirmed green at
  plan time — this feature grew both numbers: 92/92 test binaries passing workspace-wide and 373/373
  AT scenario runs at completion).
- No new external services required (research.md §18 — no new dependency; the `sql`/`mssql`/`redb`/
  `mongodb` feature-gated backends this feature touches (US7) already have their own CI service
  containers from prior features).

## US1 — Correctness bugs: `#` truncation, Signature/93 mapping

```bash
cargo test -p truefix-config --lib
cargo test -p truefix-core --test signature_length
```
Expected: a `.cfg` `Password` value containing `#` round-trips exactly (`crates/truefix-config/src/
lib.rs`'s inline unit test `hash_immediately_after_a_value_is_not_treated_as_a_comment`, not a separate
integration-test file); a `Signature`/`SignatureLength` field pair with an embedded SOH byte decodes
byte-identically.

## US2 — `.cfg` keys that misrepresent behavior: `AcceptorBuilder` wiring + `JdbcURL`

```bash
cargo test -p truefix-config --test store_and_log_mapping
cargo test -p truefix --test multi_session_acceptor_cfg
cargo test -p truefix --test config_start --features sql
```
Expected: a `.cfg`-only multi-session acceptor with `AllowedRemoteAddresses`/`DynamicSession`/
`AcceptorTemplate` set actually governs connection acceptance and template resolution; an unmodified
QuickFIX/J-style `JdbcURL=jdbc:...` + separate `JdbcUser`/`JdbcPassword` starts a working SQL session.

## US3 — Session protocol-correctness safeguards

```bash
cargo test -p truefix-session --test state_machine
cargo test -p truefix-transport --test resend_veto
cargo test -p truefix-at --test conformance
```
Expected: a vetoed resend message is replaced by a GapFill on the wire; a PossDup message with a
falsified earlier `OrigSendingTime` and a duplicate Logon both trigger logout+disconnect; **the AT
suite's scenario-run count is higher than this plan's 353 baseline** (373 at completion), not merely
unchanged.

## US4 — Automatic inbound resend chunk continuation

```bash
cargo test -p truefix-session --test recovery
cargo test -p truefix-at --test conformance
```
Expected: a resend spanning more than one chunk completes without a manual re-request; the AT suite
gains a scenario covering it.

## US5 — Session identity completeness

```bash
cargo test -p truefix-config --test key_coverage
cargo test -p truefix --test session_qualifier
```
Expected: two `.cfg` sessions sharing BeginString/SenderCompID/TargetCompID but distinct
`SessionQualifier` both start as distinct, independently addressable sessions.

## US6 — Initiator connection robustness

```bash
cargo test -p truefix-transport --test reconnect
cargo test -p truefix-transport --lib
```
Expected: a stepped reconnect interval reaches and sticks at its final value (`reconnect.rs`); a
configured local bind address is honored (`reconnect.rs`'s
`initiator_connects_from_the_configured_local_bind_address`); a short connect timeout against an
unresponsive peer fails within bound (`lib.rs`'s inline `connect_timeout_tests`/
`reconnect_delay_tests` modules, using `tokio::time::pause()` for deterministic timing rather than
real network delays).

## US7 — Store/log persistence hardening

```bash
cargo test -p truefix-store --test redb_backend
cargo test -p truefix-store --test sql_backends --features sql
cargo test -p truefix-log --test sql_log --features sql
```
Expected: a store's creation time is queryable and updates on reset; a SQL-family store's save +
sequence-increment is atomic; every structured log entry carries a timestamp and session-identity
column.

## US8 — Config-key stance registry accuracy sweep

```bash
cargo test -p truefix-config --test key_coverage
cargo test -p truefix-config --test store_and_log_mapping --features sql
```
Expected: `SocketAcceptProtocol`/`SocketConnectProtocol`/`JdbcDataSourceName`/`JdbcConnectionTestQuery`
read `Unsupported`; the JDBC pool-tuning/table-name keys apply to a `.cfg`-resolved SQL store.

## US9 — Dictionary and codec model completeness

```bash
cargo test -p truefix-dict --test field_types_extended
cargo test -p truefix-dict --test group_validation
cargo test -p truefix-dict --test version_meta
cargo test -p truefix-core --test groups
cargo test -p truefix-core --test header_trailer_groups
cargo test -p truefix-core --test field_order_encode
cargo test -p truefix-dict --test dual_track
cargo test -p truefix-dict --features dict-tooling --test dictionary_coverage
```
Expected: all 11 new field types validate per-format (`field_types_extended.rs`); open enums and
per-group child dictionaries pass their scenarios (`field_types_extended.rs`/`group_validation.rs`);
dictionary version metadata matches/mismatches `BeginString` correctly (`version_meta.rs`); group CRUD
(`replace_group`/`remove_group`/`get_group`) passes (`truefix-core`'s `groups.rs`); header/trailer
repeating-group decode passes (`header_trailer_groups.rs`); custom field order round-trips
byte-for-byte (`field_order_encode.rs`); the dual-track content-hash equality test stays green
throughout every model change (`dual_track.rs`); bundled dictionary coverage matches QuickFIX/J's own
per version, enforced as a continuous byte-for-byte regeneration regression test
(`dictionary_coverage.rs`).

## Full gate

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo test -p truefix-store -p truefix-log --features sql
cargo test -p truefix-store -p truefix-log --features mssql
cargo test -p truefix-store -p truefix-log --features redb
cargo test -p truefix-store -p truefix-log --features mongodb
cargo test -p truefix-dict --features dict-tooling
cargo test -p truefix-at --test conformance
cargo test -p truefix-at --test coverage
cargo deny check
```
Expected: green throughout; `cargo test -p truefix-at --test coverage`'s
`server_suite_scenario_run_count_does_not_regress` test's floor value is expected to require a
deliberate, disclosed bump once US3/US4's new AT scenarios land (SC-013) — this is the one test in the
whole workspace this feature is *expected* to require editing the assertion of, not just adding new
passing tests around.
