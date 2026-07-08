# Implementation Plan: Audit 007 Remediation

**Branch**: `012-audit-007-remediation` | **Date**: 2026-07-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/012-audit-007-remediation/spec.md`

## Summary

`docs/todo/007.md` is a scoped follow-up audit covering three findings in `truefix-log`'s `FileLog`
and the `.cfg`-driven `Engine::start` construction path: `NEW-156` (blocking file I/O on the tokio
executor), `NEW-157` (no multi-generation/time-based log rotation), and `NEW-158` (no way to inject
a custom `MessageStore`/`Log` through `Engine::start`, unlike the lower-level
`truefix::transport::Services` path). This plan implements the fully-clarified scope from
`spec.md`: `FileLog` is rebuilt on the same bounded-channel + background-task shape already used by
`SqlLog`/`MssqlLog`/`MongoLog`/`RedbLog`; `RotatingFile`/`FileLogOptions` gain a composable
generation-count and/or time-based retention policy with crash-safe, restart-safe rotation; and
`StoreConfig`/`ResolvedSession` gain both a `Custom(...)`-style config surface and a builder-level
override entry point on `Engine`, with the builder override taking precedence and fail-fast
validation against nonsensical combinations. No new external dependencies, no protocol/wire-level
changes, and no changes to the `MessageStore`/`Log` trait definitions themselves — this is
entirely a construction/plumbing feature.

## Technical Context

**Language/Version**: Rust edition 2024, workspace MSRV `1.96`, `unsafe_code = "forbid"`.

**Primary Dependencies**: Existing workspace dependencies only — `tokio` (`mpsc` channel,
`task::spawn`/`spawn_blocking`, already used by `SqlLog`/`MssqlLog`/`MongoLog`/`RedbLog`), `tracing`
and `metrics` (structured observability for write/flush failures, FR-009), `time` (date-stamped
rotation file naming), `thiserror` (new `ConfigError`/`LogError` variants). No new external crate
is needed for any of the three findings (`research.md`).

**Storage**: File-based logs (`FileLog`/`RotatingFile` in `crates/truefix-log/src/file.rs`) and the
existing `MessageStore` implementations (`MemoryStore`, `FileStore`, `CachedFileStore`, `NoopStore`,
plus `Sql`/`Mssql`/`Redb`/`Mongo` behind their Cargo features) in `crates/truefix-store`. This
feature adds a `StoreConfig::Custom(Arc<dyn MessageStore>)` pass-through variant and a retention
policy for file-based logs; it does not add a new storage backend.

**Testing**: `cargo test` for unit/integration coverage; targeted crate tests under
`crates/truefix-log/tests/` (mirroring the existing `async_writer_flush_on_shutdown.rs`,
`bounded_async_channel.rs`, `audit006_file_log.rs`, `audit006_file_growth.rs` patterns) and
`crates/truefix/tests/` (mirroring `config_start.rs`, `audit006_engine_api.rs`). Each of
`NEW-156`/`NEW-157`/`NEW-158` needs at least one failing-before/passing-after test (SC-005).
Backend-gated tests (`sql`/`mssql`/`mongodb`/`redb` features) are unaffected — `FileLog` and the
`StoreConfig::Custom`/`Engine` override surface require no Cargo feature gate.

**Target Platform**: Same as the existing workspace — a Rust library/engine embedded by a host
process (server-side, Linux/macOS/Windows via `tokio`), not a standalone deployable service.

**Project Type**: Library (Cargo workspace of crates) — `truefix-log`, `truefix-store`,
`truefix-config`, `truefix-transport`, `truefix` (facade). No frontend/mobile component.

**Performance Goals**: SC-001 — under a simulated slow/blocked disk write, other scheduled session
work (e.g. heartbeat timers) MUST still fire within the session's configured heartbeat tolerance;
no numeric latency target beyond that existing, per-session-configurable bound.

**Constraints**: Non-blocking `FileLog` writes (FR-001); bounded backpressure, not unbounded
queuing, under sustained write pressure (FR-003); zero log-line loss across a normal shutdown/drain
(FR-002, SC-002) and across any rotation event (FR-005) or crash mid-rotation (FR-011); default
behavior unchanged when the new retention config is unset (FR-004); acceptor/initiator parity for
every requirement (FR-008, Constitution Principle VI).

**Scale/Scope**: Single feature confined to `truefix-log` (`FileLog`/`RotatingFile`),
`truefix-store` (`StoreConfig`), `truefix-config` (`ResolvedSession`/`ConfigError`), and `truefix`
(`Engine`) — three findings from one scoped audit document, not a crate-wide rewrite.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. 生产就绪优先 (Production-Ready First)** — PASS. All new code stays off `panic!`/`unwrap()`/
  `expect()`/`unreachable!()` on the critical path (`FileLogWriter` background task, `RotatingFile`
  rotation, `resolve_store`/`resolve_log` validation), matching the crate's existing
  `#![cfg_attr(not(test), deny(clippy::unwrap_used, ...))]` lints. New public items get doc
  comments (Principle I requires every public type/trait/fn to be documented). Write/flush
  failures are surfaced via structured `tracing`/`metrics` (FR-009), not silently dropped.
  Additive-but-match-breaking changes (`StoreConfig::Custom`, `ResolvedSession`'s new field,
  `FileLogOptions`'s new field) are documented as migration notes per `contracts/
  public-api-changes.md`, satisfying "breaking changes MUST follow semver and record migration
  notes."
- **II. 协议正确性优先 (Protocol Correctness First)** — N/A for this feature. None of `NEW-156`/
  `NEW-157`/`NEW-158` touch session-layer FIX protocol semantics (logon, sequence numbers,
  resend/gap-fill, heartbeats as protocol behavior) — they change logging/storage construction and
  file rotation. No FIX spec citation is required per Principle II's own scope (protocol
  *behavior* decisions); `research.md` explicitly notes this non-applicability.
- **III. License 与来源纪律 (License & Provenance)** — PASS. Every design decision in
  `research.md` is derived from this repository's own existing patterns (`SqlLog`/`MssqlLog`/
  `MongoLog`/`RedbLog`'s channel+background-task shape, `RedbLog`'s `spawn_blocking` wrap,
  `AcceptorBuilder`'s override-layering convention) — no `thrdpty/quickfix(j)` source consulted for
  this feature (QFJ's `SLF4JLogFactory`/Logback precedent, cited in `docs/todo/007.md` as *prior
  art motivating the capability*, is not a source of any code or design here — the chosen fix
  direction explicitly rejected the Logback-equivalent `tracing-appender` sink path). No new
  external dependency is introduced (`research.md`), so no new license to vet.
- **IV. 数据字典双轨 (Dual-Track Data Dictionary)** — N/A. This feature does not touch message
  field/dictionary definitions, codegen, or the runtime `DataDictionary`.
- **V. 测试纪律 (Test Discipline)** — PASS (planned). Table-driven unit tests for `RetentionPolicy`
  validation and `RotatingFile` generation/interval rotation; integration tests for
  `FileLog::shutdown()` flush-on-drain (extending the existing `async_writer_flush_on_shutdown.rs`
  pattern) and for `Engine::start`/override precedence (extending `config_start.rs`); no AT
  (Acceptance Test) port is applicable since this feature has no session-protocol behavior (see
  Principle II above) — `quickstart.md` enumerates the concrete `cargo test` invocations. SC-005
  requires every accepted finding to have a failing-before/passing-after test.
- **VI. Acceptor/Initiator 对等 (Parity)** — PASS. FR-008 explicitly requires identical behavior
  for acceptor and initiator sessions across all three findings; every user story's acceptance
  scenarios include an explicit acceptor+initiator scenario (spec.md Story 1-3, scenario 4 each).
- **VII. 基于清点的完整性 (Inventory-Based Completeness)** — N/A. This feature is scoped
  audit-remediation from `docs/todo/007.md`, not a QuickFIX/J feature-parity inventory item.

No violations requiring `Complexity Tracking` justification.

## Project Structure

### Documentation (this feature)

```text
specs/012-audit-007-remediation/
├── plan.md                        # This file (/speckit-plan command output)
├── research.md                    # Phase 0 output (/speckit-plan command)
├── data-model.md                  # Phase 1 output (/speckit-plan command)
├── quickstart.md                  # Phase 1 output (/speckit-plan command)
├── contracts/
│   └── public-api-changes.md      # Phase 1 output (/speckit-plan command)
├── checklists/
│   └── requirements.md            # /speckit-specify + /speckit-clarify output
└── tasks.md                       # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

Existing Cargo workspace (`Cargo.toml` members = `crates/*`); this feature touches four existing
crates, adding no new crate:

```text
crates/
├── truefix-log/
│   ├── src/
│   │   ├── file.rs        # NEW-156 (async writer) + NEW-157 (RetentionPolicy, rotation) — primary change site
│   │   ├── redb.rs        # Reference pattern only (channel+background-task+spawn_blocking shape); not modified
│   │   ├── sql.rs         # Reference pattern only; not modified
│   │   └── lib.rs         # Log trait unchanged; doc comments updated for FileLog's new shutdown() behavior
│   └── tests/
│       ├── async_writer_flush_on_shutdown.rs   # Extended with a FileLog case (NEW-156)
│       ├── audit006_file_log.rs                # Existing FileLog coverage; must keep passing unmodified-behavior-wise
│       ├── audit006_file_growth.rs             # Existing MaxFileLogSize coverage; must keep passing
│       └── file_log_retention.rs               # New (NEW-157)
├── truefix-store/
│   └── src/lib.rs         # StoreConfig::Custom variant + build_store pass-through (NEW-158)
├── truefix-config/
│   └── src/builder.rs     # ResolvedSession custom-log field, resolve_store/resolve_log validation, new ConfigError variant (NEW-158)
└── truefix/
    ├── src/lib.rs         # Engine override entry point, precedence resolution (NEW-158)
    └── tests/
        ├── config_start.rs                # Existing; must keep passing
        └── custom_backend_injection.rs     # New (NEW-158)

README.md                  # FR-007: document the custom store/log injection path
```

**Structure Decision**: Single Cargo workspace, existing crate layout unchanged (no new crate). All
three findings are implemented in-place in the crates that already own the affected behavior
(`truefix-log` owns `FileLog`; `truefix-store` owns `StoreConfig`; `truefix-config` owns
`.cfg`-resolution; `truefix` owns `Engine`), per the codebase's established layering
(`specs/001-fix-engine-parity/plan.md`) and this feature's own Assumption that no new crate is
warranted for a scoped audit-remediation.

## Complexity Tracking

*No violations — Constitution Check above passed without exceptions.*
