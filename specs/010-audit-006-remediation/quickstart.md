# Quickstart: Audit 006 Remediation Validation

## Prerequisites

- Rust toolchain matching workspace MSRV.
- Optional external services only for backend-gated Mongo/SQL/MSSQL tests.
- No network access to QuickFIX/J or QuickFIX/Go is required; reference behavior is recorded in
  `docs/todo/006.md` and this feature's contracts.

## Core Validation

```bash
cargo test
```

Expected outcome: all default-feature unit and integration tests pass.

## Focused Crate Slices

```bash
cargo test -p truefix-session
cargo test -p truefix-transport
cargo test -p truefix-store
cargo test -p truefix-log
cargo test -p truefix-config
cargo test -p truefix-core
cargo test -p truefix-dict
cargo test -p truefix
cargo test -p truefix-at
```

Expected outcome: each crate's targeted tests for its `NEW-*` findings pass.

## Protocol And Lifecycle Checks

Validate the scenarios from [session-lifecycle.md](./contracts/session-lifecycle.md):

- pre-logon logout request is not dropped;
- invalid Logon sequence handling consumes sequence correctly;
- `SequenceReset` validation/queue behavior is correct;
- schedule-driven logout waits for peer response or timeout;
- `PossResend(97)` is observable;
- cancel-on-disconnect emits safe application-provided cancel requests.

Expected outcome: unit/integration/AT tests cover the relevant acceptor and initiator roles.

## Transport And Engine Checks

Validate the scenarios from [transport-engine.md](./contracts/transport-engine.md):

- shutdown stops active accepted sessions;
- TLS handshake timeout releases active state;
- inbound staging remains bounded;
- dynamic identities get isolated services;
- TLS scheduled initiator and configurable backlog are available.

Expected outcome: integration tests demonstrate bounded resources and addressable session handles.

## Store And Log Checks

Validate the scenarios from [store-log.md](./contracts/store-log.md):

- crash/error simulations do not advance sequence without recoverable messages;
- corrupt-tail recovery does not hide later valid appends;
- duplicate detection is available without changing `save()`;
- file growth policy bounds logs/stores without breaking resend/recovery.

Expected outcome: store/log tests pass; backend-gated tests are run where services are available.

## Quality Gates

```bash
cargo clippy -p truefix-config -p truefix-transport -p truefix --all-targets -- -D warnings
cargo clippy -p truefix-dict -p truefix-at --all-targets -- -D warnings
```

Expected outcome: clippy passes for the planned touched crates covered by existing approved gates.

## Validation Results (T099-T101)

- `cargo test --workspace --all-features --no-fail-fast`: **PASS** — 275 test binaries, 971 tests
  passed, 0 failed. No backend service (Mongo/SQL/MSSQL) was live in this run; those backend
  tests remain gated by their respective Cargo features and pass or are skipped consistently with
  the feature-gate notes in [store-log.md](./contracts/store-log.md).
- Focused crate slices (`cargo test -p <crate>` for each crate listed under
  [Focused Crate Slices](#focused-crate-slices)): all pass as part of the workspace run above.
- `cargo clippy -p truefix-config -p truefix-transport -p truefix --all-targets -- -D warnings`:
  **PASS**. This run surfaced and fixed one real finding: the NEW-133 buffered-decode
  optimization (`classify_buffered` in `crates/truefix-transport/src/lib.rs`) used direct
  `&buf[..total]` slicing, which trips this crate's `clippy::indexing_slicing` deny-by-default
  lint (Constitution Principle I: no panicking indexing on critical paths). Fixed by switching to
  `buf.get(..total).unwrap_or(&[])`, matching the fallback style already used throughout
  `truefix-core`'s framing code — `total` is always in-bounds by `frame_length`'s own contract, so
  the fallback is defense-in-depth, not a behavior change.
- `cargo clippy -p truefix-dict -p truefix-at --all-targets -- -D warnings`: **PASS**, no findings.
- AT protocol scenario regression found and fixed during this validation pass: `0_IdleHeartbeatEmitted`
  and `4_TestRequestOnSilence` in `crates/truefix-at/src/scenarios.rs` encoded the *pre-NEW-103*
  message ordering (idle Heartbeat observed before any TestRequest). Once NEW-103's corrected
  `TestRequestDelayMultiplier` default (`0.5`, matching QFJ parity) was implemented, a totally
  silent connection now always draws the liveness-probing TestRequest *before* the next idle
  Heartbeat (since `0.5 * HeartBtInt < 1.0 * HeartBtInt` for any interval) — the scenarios were
  updated to assert the corrected order instead of reverting the parity fix.
- A test-fixture bug was also found and fixed in
  `crates/truefix-dict/tests/audit006_validation.rs`: the section-order-violation fixture reused
  `SenderCompID(49)` for its out-of-order tag, which is also its own genuine duplicate-tag
  violation independent of section order — masking the "`ValidateFieldsOutOfOrder=false`" case.
  Fixed by using a distinct, not-yet-used header tag (`PossDupFlag(43)`) for the violation.

## Configuration/Default Compatibility Notes

Operators upgrading past this feature should be aware of these default-value and strictness
changes (all intentional QFJ-parity fixes, not regressions):

- `heartbeat_timeout_multiplier` default changed `2.0` → `1.4` (NEW-102): a silent peer is now
  disconnected sooner (~44s vs ~62s at the default 30s `HeartBtInt`).
- `test_request_delay_multiplier` default changed `1.0` → `0.5` (NEW-103): the liveness-probing
  TestRequest now fires sooner (~15s vs ~30s at the default 30s `HeartBtInt`) — and, since
  `0.5 * HeartBtInt` is always less than `1.0 * HeartBtInt`, a totally silent connection now
  always observes the TestRequest *before* the next idle Heartbeat, not after.
- Boolean `.cfg` values are now parsed strictly for every boolean key (NEW-104): a value other
  than `Y`/`N` is a typed config error instead of silently defaulting.
- `EnabledProtocols` values unrecognized by this engine's TLS backend are now rejected with a
  typed config error instead of being silently ignored (NEW-111/NEW-112/NEW-113 family).
- Strict field/checksum parsing (`Field::as_int`/`as_decimal` reject whitespace padding, direct
  `Message::decode` requires an exactly-three-digit checksum) may reject wire messages a previous
  build accepted leniently — this is intentional protocol-parity strictness (NEW-101, NEW-155),
  per [dictionary-codec-api.md](./contracts/dictionary-codec-api.md)'s Compatibility Notes.

## Traceability

Every new test should name or comment the `NEW-*` finding it covers, matching `spec.md`,
`data-model.md`, and the contracts.

## Implementation Evidence Checklist

- [X] `NEW-97`-`NEW-100`: session lifecycle and `SequenceReset` P1 tests added and passing.
- [X] `NEW-101`-`NEW-108`: core strictness, application parity, and file growth tests added and passing.
- [X] `NEW-111`-`NEW-127`: config/default/store/log/dictionary operational tests added and passing.
- [X] `NEW-128`-`NEW-140`: parser, transport, store, schedule, and metadata tests added and passing.
- [X] `NEW-142`-`NEW-155`: engine/transport/store/dictionary critical-path tests added and passing.
- [X] Retired IDs `NEW-109`, `NEW-110`, `NEW-114`, `NEW-131`, `NEW-141`, `NEW-144` remain excluded with evidence.

## Test Naming Convention

New tests for this feature should include the `audit006` prefix in the file name and mention the
covered `NEW-*` finding in the test name or a short nearby comment. This keeps task evidence
searchable across crates without depending on implementation details.
