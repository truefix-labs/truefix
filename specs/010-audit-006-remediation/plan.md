# Implementation Plan: Audit 006 Remediation

**Branch**: `010-audit-006-remediation` | **Date**: 2026-07-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/010-audit-006-remediation/spec.md`

## Summary

`docs/todo/006.md` is the next audit-remediation wave after feature 009, covering confirmed
`NEW-97`-`NEW-155` items that were not already tracked in `docs/todo/005.md`. This plan implements
the clarified scope: all non-retired findings are either fixed, refuted with evidence, or recorded
with a disposition; clarified feature gaps (`NEW-105`, `NEW-106`, `NEW-108`, `NEW-134`, `NEW-135`,
`NEW-137`, `NEW-146`, `NEW-148`) are in implementation scope. The work is organized by ownership
domain rather than by source-audit order: session protocol/lifecycle, transport/engine lifecycle,
store/log durability, config/defaults, dictionary/codec validation, and public API parity.

The technical approach is conservative: no new crates, no source copying from QuickFIX/J or
QuickFIX/Go, and no wholesale state-machine rewrite. Each fix lands in the crate that already owns
the behavior, with failing-before/passing-after tests and integration/AT coverage for role-sensitive
session semantics.

## Technical Context

**Language/Version**: Rust edition 2024, workspace MSRV `1.96`, `unsafe_code = "forbid"`.

**Primary Dependencies**: Existing workspace dependencies: `tokio`, `tokio-rustls`, `rustls`,
`rustls-native-certs`, `mongodb`, `sqlx`, `tiberius`, `redb`, `tracing`, `metrics`, `time`,
`thiserror`, `memchr`, and existing transport buffer primitives where already available. New
external dependencies are not planned; if a size-limit/rotation implementation needs a helper, the
tasks phase must first prove existing std/tokio file primitives are insufficient and check license
compatibility.

**Storage**: Existing message stores (`FileStore`, `CachedFileStore`, `MemoryStore`, `NoopStore`,
`MongoStore`, `SqlStore`, `MssqlStore`, `RedbStore`) plus file log backends. This feature changes
store semantics for transactionality, crash safety, duplicate detection, per-dynamic-session store
factories, and bounded file growth.

**Testing**: `cargo test` for unit/integration coverage; targeted crate tests under
`crates/*/tests`; AT-harness scenarios for session protocol behavior where applicable. Feature-gated
backend tests remain behind existing backend features and may be marked ignored when they require
external services.

**Target Platform**: Server-side Rust library/engine on Linux/macOS development targets; transport
and TLS behavior remains tokio-based and cross-platform.

**Project Type**: Cargo workspace library/engine with CLI/test harness support; no frontend or web
service.

**Performance Goals**: Preserve behavior while bounding memory/file growth for `NEW-108`,
`NEW-150`, and `NEW-153`; reduce allocation pressure for `NEW-133`; no broad throughput target is
introduced beyond not regressing existing tests and benchmarks for decode/transport paths.

**Constraints**: Critical paths must avoid panic/unwrap/expect/unreachable and unchecked indexing;
errors remain typed and observable; public API additions require docs and compatibility notes; all
reference-implementation comparisons must remain behavioral/prose-level under the constitution.

**Scale/Scope**: 3 user stories, 15 functional requirement groups, 50+ audit findings across all 9
crates. Implementation is expected to be task-batched by risk tier and crate, not completed as one
monolithic patch.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. Production-Ready First | P1 work closes data-loss, lifecycle, resource-bound, and shutdown gaps; plan requires typed errors, observability, and no critical-path panic/unwrap patterns. | PASS |
| II. Protocol Correctness First | Session lifecycle, sequence, logout, validation, `SequenceReset`, and `PossResend` items are treated as protocol behavior and require FIX/behavioral reference evidence plus role-sensitive tests. | PASS |
| III. License & Provenance | QuickFIX/J and QuickFIX/Go remain behavioral references only; no source translation or copied private data. New dependencies are avoided unless license review is recorded. | PASS |
| IV. Dual-Track Data Dictionary | Dictionary/codegen/validation work keeps runtime dictionary and generated behavior aligned; section-order enforcement is clarified as validation-layer policy. | PASS |
| V. Test Discipline | Plan requires targeted unit tests, integration tests, and AT additions where protocol semantics are externally observable. | PASS |
| VI. Acceptor/Initiator Parity | Role-sensitive protocol items must cover both roles; acceptor-only findings such as dynamic identity and acceptor handles are explicitly scoped. | PASS |
| VII. Inventory-Based Completeness | Every requirement traces to `docs/todo/006.md` `NEW-*` IDs with retired/false-positive IDs excluded. | PASS |

**Result**: No gate failures. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/010-audit-006-remediation/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── README.md
│   ├── session-lifecycle.md
│   ├── transport-engine.md
│   ├── store-log.md
│   ├── config-defaults.md
│   └── dictionary-codec-api.md
├── checklists/
│   └── requirements.md
└── tasks.md              # created by /speckit-tasks, not by this command
```

### Source Code (repository root)

```text
crates/
├── truefix/              # Engine facade, dynamic acceptor services, session registry/shutdown
├── truefix-at/           # Acceptance-test harness and protocol scenarios
├── truefix-config/       # .cfg parsing, defaults, strict booleans/TLS protocol values
├── truefix-core/         # Field parsing, codec/framing checksum and section diagnostics
├── truefix-dict/         # Runtime validation, section-order policy, required fields, groups
├── truefix-log/          # File log options, timestamp/error behavior, bounded/rotated logs
├── truefix-session/      # Session state machine, logout/logon/SequenceReset/PossResend/cancel
├── truefix-store/        # Store transactionality, crash safety, duplicate detection, file bounds
└── truefix-transport/    # Accept loops, TLS handshakes, bounded staging, scheduled TLS, backlog
```

**Structure Decision**: Use the existing cargo workspace and crate boundaries. No new crates,
top-level apps, or service layers are introduced.

## Phase 0: Research Summary

See [research.md](./research.md). All plan-time technical unknowns are resolved:

- dynamic acceptor persistence uses a per-identity store/services factory;
- cancel-on-disconnect and `PossResend(97)` are implemented in this feature;
- file log/store growth receives built-in configurable size-limit or rotation behavior;
- duplicate sequence detection is added as a separate capability without changing the core save
  contract;
- section-order enforcement stays in validation policy;
- TLS scheduled initiator and configurable listener backlog are in scope.

## Phase 1: Design Summary

See [data-model.md](./data-model.md) for entities/state transitions and [contracts/](./contracts/)
for API/config/protocol contracts. The design keeps public API changes additive except where config
defaults intentionally change for parity.

## Post-Design Constitution Check

| Principle | Re-check | Status |
|---|---|---|
| I | Contracts require typed errors, bounded resources, and observable shutdown/log failures. | PASS |
| II | Session and validation contracts map protocol findings to tests and AT candidates. | PASS |
| III | Research records behavioral-reference-only discipline and dependency caution. | PASS |
| IV | Dictionary/codec contract keeps runtime validation as the enforcement point for section order. | PASS |
| V | Quickstart defines crate tests, integration tests, and AT validation slices. | PASS |
| VI | Contracts call out acceptor/initiator coverage for role-sensitive lifecycle items. | PASS |
| VII | Contracts and data model preserve `NEW-*` traceability. | PASS |

## Complexity Tracking

No constitution violations require justification.
