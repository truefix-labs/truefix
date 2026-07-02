# Implementation Plan: Engine Config-Wiring Completion + Additional Store Backends

**Branch**: `002-qfj-parity-completion` (feature tracked by directory, not branch name — mirroring
003's convention) | **Date**: 2026-07-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/004-engine-wiring-extra-backends/spec.md`

## Summary

A 2026-07-02 code-vs-code comparison (`docs/todo-gap-analysis.md`'s gap section) found six remaining
gaps against QuickFIX/J + QuickFIX/Go, none of them protocol-correctness defects: three capabilities
that work correctly at the Rust-API level but aren't reachable purely from a `.cfg` file (initiator
failover, dictionary/validator wiring, SQL-backend selection via `JdbcURL`), one multi-session
startup-robustness gap (`ContinueInitializationOnError`, recognized but inert), and two new optional
storage backends (`RedbStore`/`RedbLog` — an embedded, transactional replacement for QuickFIX/J's
obsolete `SleepycatStore`; `MongoStore`/`MongoLog` — matching QuickFIX/Go's MongoDB option). Phase 0
research (`research.md`) grounded every decision by reading the actual current code rather than
assuming the gap analysis's framing was complete, and surfaced one real complication the gap analysis
didn't anticipate: wiring initiator failover isn't a drop-in swap, because the multi-endpoint
reconnecting connector returns a different handle type (`ReconnectHandle`, missing `logout()`/`send()`)
than every other `connect_initiator*` variant (`SessionHandle`), and no TLS-aware or proxy-aware
reconnecting variant exists yet. The plan resolves this **additively** (a new `Engine.failover_initiators`
field/accessor, alongside the unchanged `initiators()`) rather than by changing `Engine::initiators()`'s
existing return type. The work is delivered **in one pass but internally staged (W1–W6, six stages)**
with a verifiable checkpoint per stage, mirroring 001/002/003's model. None of the six items touch the
session state machine, wire codec, or AT-scenario-tested protocol behavior — **no new AT scenarios are
needed**; the existing 353/353-scenario suite is expected to stay green and unmodified throughout
(FR-010/SC-006), which is itself the primary correctness gate for this feature (there being no new
protocol behavior to gate on).

## Technical Context

**Language/Version**: Rust, edition 2021, MSRV per workspace pin (1.96 — comfortably above `mongodb`
3.7.0's own 1.88 floor). `#![forbid(unsafe_code)]` and per-crate `deny(clippy::unwrap_used/expect_used/
panic/indexing_slicing)` on non-test code unchanged (Principle I).

**Primary Dependencies**:
- All existing workspace deps (tokio, tokio-rustls/rustls, thiserror, sqlx, tiberius, tracing, metrics)
  — used more, not replaced.
- **New** (each gated behind a Phase 0 license-audit task before first dependent line of code, per
  Principle III; research.md §7 has already informally verified both, `/speckit-tasks` re-confirms via
  `cargo deny check` before code lands, mirroring 003's `tiberius` precedent): `redb` 4.1.0+ (MIT OR
  Apache-2.0, embedded transactional KV store, US5) and `mongodb` 3.7.0+ (Apache-2.0, official async
  MongoDB driver, US6).
- No new dependency for US1–US4 — pure wiring using types/functions already present in
  `truefix-transport`/`truefix-dict`/`truefix-config`, plus one new function
  (`connect_initiator_reconnecting_multi_tls`) added to the existing `truefix-transport` crate.

**Storage**: `RedbStore`/`RedbLog` (US5) and `MongoStore`/`MongoLog` (US6) are new, independent,
off-by-default backends alongside the existing Memory/File/CachedFile/SQL(PG/MySQL/SQLite)/MSSQL
options — `MessageStore`/`Log` trait contracts unchanged, no new trait methods.

**Testing**: Table-driven unit/mapping tests for the new `.cfg` resolution paths (validator, `JdbcURL`
dispatch, `ContinueInitializationOnError`), `Engine::start`-level integration tests (multi-session
partial-failure, failover-over-TLS reconnect), and full `MessageStore`/`Log` conformance-test-pattern
suites for `RedbStore`/`RedbLog` (unconditional, like SQLite today — no external service) and
`MongoStore`/`MongoLog` (gated on `DATABASE_URL_MONGO` availability, mirroring the MSSQL precedent).
**No AT suite changes** — this feature adds no protocol behavior for the AT corpus to cover; the
existing 353/353-scenario suite (verified green at spec time) staying green and *unmodified in its
assertions* is itself part of this feature's Definition of Done (FR-010).

**Target Platform**: Linux/macOS servers (tokio). No platform-specific code added.

**Project Type**: Library/engine (existing cargo workspace). No new crates, no new binary targets.

**Performance Goals**: Correctness/wiring-completeness first; no new performance surface is introduced
(this feature adds no hot-path logic — `.cfg` resolution runs once at startup, and the two new store
backends are optional, off-by-default, and not on any existing hot path).

**Constraints**: No panic/unwrap/expect/unreachable on codec/session/I-O/timer paths (Principle I);
`redb`'s synchronous API MUST be wrapped in `tokio::task::spawn_blocking`, never called directly from
an async trait method (research.md §5) — a correctness constraint specific to this feature's US5.
`unsafe_code = forbid` remains workspace-wide for TrueFix's own crates. No reference source/data copied
(Principle III). This feature introduces **one disclosed, narrow deviation from strict
zero-new-surface-on-existing-types**: `ResolvedSession` and `ConfigError` (both `truefix-config`
public types) each gain new members (a field, a variant) — additive in the sense that no existing
field/variant is removed or retyped, but any code that pattern-matches `ConfigError` exhaustively
(without a wildcard arm) would need a new arm; grep-confirmed at plan time that no such exhaustive
match exists anywhere in this workspace today, so this lands as a non-breaking change in practice, not
just in principle.

**Scale/Scope**: 10 FRs across 6 user stories; two new off-by-default Cargo features (`redb`,
`mongodb`) on `truefix-store`/`truefix-log`; zero new AT scenarios; up to 6 previously-`Recognized`
config keys move to `Implemented` (`ContinueInitializationOnError`, plus whichever `Jdbc*`/dictionary
keys' stance the resolution work actually activates — see FR-009).

**Reference-only architecture inputs**: `thrdpty/quickfix` (Go, for `MongoStore`'s precedent) and
`thrdpty/quickfixj` (Java, for `ContinueInitializationOnError`'s and the `Jdbc*`/dictionary keys'
existing semantics) read for design/behaviour only, per Principle III — no source/data-file copying;
`redb` has no QuickFIX/J/Go precedent at all (a new-to-this-project design, not a ported one).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. Production-Ready First | Every new surface is additive; `ConfigError::UnsupportedBackend` replaces what would otherwise risk a silent fallback/panic (FR-004); `redb`'s sync API is explicitly required to route through `spawn_blocking`, never blocking the executor (research.md §5); multi-session partial-failure (US4) is logged via `tracing`, not silently swallowed. | ✅ PASS |
| II. Protocol Correctness First (NON-NEG) | Explicitly not applicable as a *new* gate — this feature touches no session-state-machine/codec/protocol behavior (FR-010). The existing AT suite (353/353) staying green and **unmodified** is this feature's substitute correctness gate, verified at the start and end of every stage. | ✅ PASS (no new protocol surface to gate; regression-freedom is the gate) |
| III. License & Provenance (NON-NEG) | Two new dependencies identified (`redb`, `mongodb`); each MUST pass a license-compatibility check before first use (research.md §7's audit table, re-confirmed via `cargo deny check` at task start) — flagged as a Phase 0/task-start condition, matching 002/003's precedent. | ✅ PASS (conditioned on Phase 0 dependency audit) |
| IV. Dual-Track Data Dictionary | Not implicated — US2's dictionary wiring reads existing `DataDictionary`/`load_fixXX`/`load_from_file` mechanisms as-is; no new dictionary content, codegen, or dual-track surface is introduced. | ✅ PASS (N/A — no dictionary content change) |
| V. Test Discipline | Table-driven mapping tests for every new `.cfg` resolution path; `Engine::start`-level integration tests for the two behaviorally-nontrivial stories (US1 failover-over-TLS, US4 partial-failure startup); full conformance-test-pattern coverage for both new store backends, matching the SQL/MSSQL precedent exactly. | ✅ PASS |
| VI. Acceptor/Initiator Parity | US1 (failover) is initiator-only by nature (failover endpoints are an initiator-side concept in FIX, matching QuickFIX/J's own `SocketConnectHost<n>` semantics — there is no acceptor-side "failover" to parallel it, same reasoning 003's plan already applied to PROXY-protocol-is-acceptor-side/SOCKS-is-initiator-side). US2/US3/US4 apply uniformly to acceptor and initiator sessions (dictionary validation, store/log selection, and multi-session startup all operate identically regardless of `ConnectionType`) — verified in each contract file's test-hook list. US5/US6 (new store backends) are role-agnostic by construction, same as every existing `MessageStore`/`Log` implementation. | ✅ PASS |
| VII. Inventory-Based Completeness | Every gap-analysis GAP item (GAP-01 through GAP-06, minus the two removed by user decision) traces to exactly one user story below; every config key this feature activates updates its Appendix A stance (FR-009); no gap is claimed closed without a test citation in its contract file. | ✅ PASS |

**Conflict-order reminder**: License → Protocol correctness → Production readiness → feature count. No
principle is traded off in this feature. Protocol correctness's usual gate (AT suite) is satisfied
vacuously-but-verifiably here: there is no new protocol behavior, so the gate is "the existing suite
doesn't regress," checked explicitly at every stage boundary rather than skipped.

**Initial gate result**: PASS (one documented condition: Phase 0 dependency/provenance audit for `redb`/
`mongodb`, mirroring 002/003's precedent). One Complexity Tracking entry (the `ResolvedSession`/
`ConfigError` additive-but-technically-new-members note above), verified non-breaking in practice.

## Project Structure

### Documentation (this feature)

```text
specs/004-engine-wiring-extra-backends/
├── plan.md              # This file
├── spec.md              # Feature spec (6 US, 10 FR, 0-question — no clarifications needed)
├── research.md          # Phase 0 output — 7 grounded decisions + dependency license-audit table
├── data-model.md        # Phase 1 output — concrete shapes for every spec Key Entity + research findings
├── quickstart.md        # Phase 1 output — per-user-story validation commands
├── contracts/           # Phase 1 output (2 theme files + index)
│   ├── README.md
│   ├── engine-cfg-wiring.md    # US1, US2, US3, US4
│   └── additional-backends.md  # US5, US6
└── checklists/
    └── requirements.md  # spec quality (from /speckit-specify)
```

### Source Code (repository root) — files touched/added per stage

No new crates, no new binary targets. Changes land in the existing layered workspace, dependency
direction unchanged (strictly downward):

```text
crates/
├── truefix-transport/     # W1: connect_initiator_reconnecting_multi_tls (new fn); no changes to
│   │                      # existing connect_initiator* signatures
│   └── src/lib.rs
├── truefix-config/        # W1: resolve_failover_addresses read by a new Engine-side branch (no
│   │                      # builder.rs change needed — failover_addresses already resolved, just
│   │                      # unread downstream); W2: resolve_validator (new) + ResolvedSession.validator
│   │                      # (new field); W3: JdbcURL dispatch in resolve_store/resolve_log +
│   │                      # ConfigError::UnsupportedBackend (new variant); W4: read
│   │                      # continue_initialization_on_error (already a SessionConfig field, from 003)
│   └── src/builder.rs, src/keys.rs (stance updates, FR-009)
├── truefix/                # W1: Engine.failover_initiators (new field) + failover_initiators()
│   │                        # (new accessor) + shutdown() extended; W2: Services.validator wired from
│   │                        # rs.validator; W4: multi-session loop restructured for partial-failure
│   │                        # continuation
│   └── src/lib.rs
├── truefix-store/          # W5: new src/redb.rs (RedbStore/RedbStoreConfig, `redb` feature);
│   │                        # W6: new src/mongo.rs (MongoStore/MongoStoreConfig, `mongodb` feature);
│   │                        # StoreConfig gains Redb/Mongo variants
│   └── src/lib.rs, src/redb.rs (new), src/mongo.rs (new), Cargo.toml (redb/mongodb features)
└── truefix-log/            # W5: new src/redb.rs (RedbLog/RedbLogConfig); W6: new src/mongo.rs
    │                        # (MongoLog/MongoLogConfig)
    └── src/lib.rs, src/redb.rs (new), src/mongo.rs (new), Cargo.toml (redb/mongodb features)
.github/workflows/
└── ci.yml                  # W6: new `mongo` job (service container), mirroring the `mssql` job
docs/
├── parity-matrix.md        # stance updates (FR-009)
└── todo-gap-analysis.md    # GAP items checked off as each stage closes
```

**Structure Decision**: Reuse the existing layered workspace unchanged; zero new crates, zero new
binary targets. `truefix-store`/`truefix-log` each gain two new sibling modules (`redb.rs`, `mongo.rs`)
alongside the existing `sql.rs`/`mssql.rs`, following the exact same independent-off-by-default-feature
pattern feature 003 established for `mssql`.

### Internal Delivery Stages (single release, dependency-ordered, each with a checkpoint)

> One-pass gap closure, **internally staged**. `/speckit-tasks` expands each stage into tasks; these
> are NOT separate releases. W1–W4 have no dependency ordering among themselves (four independent
> `.cfg`-wiring/robustness slices touching disjoint code paths) and could be done in any order or in
> parallel; W5/W6 are also mutually independent. Listed in spec priority order (P1 → P2 → P3).

| Stage | Scope (US / FR) | Checkpoint (verifiable exit) |
|-------|-----------------|------------------------------|
| **W1** | Initiator failover wiring (US1; FR-001) | A `.cfg` initiator with backup endpoints reconnects to a backup after the primary fails, over both plain TCP and TLS, zero additional Rust code (SC-001). |
| **W2** | Dictionary/validator `.cfg` wiring (US2; FR-002) | A `.cfg`-only session with `UseDataDictionary=Y` rejects a dictionary-invalid message identically to a programmatically-wired validator (SC-002). |
| **W3** | SQL backend `.cfg` dispatch (US3; FR-003/004) | A `.cfg`-only session with `JdbcURL` set persists to the named backend and survives a restart; an unsupported scheme/uncompiled feature produces a typed error, never a panic (SC-003). |
| **W4** | `ContinueInitializationOnError` (US4; FR-005) | A multi-session `.cfg` acceptor with one misconfigured session and the flag set starts every other session; unset, it fails entirely as before (SC-004). |
| **W5** | `RedbStore`/`RedbLog` (US5; FR-006/007) | Sequence numbers/messages persist across a restart against the same `redb` file; `reset()` is atomic; two session identities stay isolated (SC-005, embedded half). |
| **W6** | `MongoStore`/`MongoLog` (US6; FR-008) | Same conformance contract as W5, gated on `DATABASE_URL_MONGO`, skipping cleanly when unavailable; new CI `mongo` service-container job (SC-005, MongoDB half). |

**Cross-cutting**: FR-009 (Appendix A stance updates) is applied as each key is activated per stage,
finalized at W6. FR-010/SC-006 (AT suite + full workspace gate stay green, unmodified) is a per-stage
exit criterion for every W, verified by running `cargo test -p truefix-at --test conformance` at the
end of each stage and confirming the scenario-run count is unchanged from this plan's baseline
(353/353, confirmed green at plan time). The Phase 0 dependency/provenance audit (`redb`, `mongodb`) is
a prerequisite check performed at the start of `/speckit-tasks`, before W5/W6 begin.

## Complexity Tracking

> One documented, non-blocking note (not a Constitution violation): `ResolvedSession`/`ConfigError`
> (both `truefix-config` public types) gain new members. This is additive in the sense every existing
> field/variant is untouched, but is flagged here for transparency since a hypothetical downstream
> exhaustive `match` on `ConfigError` without a wildcard arm would need updating — grep-confirmed at
> plan time that no such match exists in this workspace, so no actual break occurs, but the risk class
> is disclosed rather than silently assumed away, matching this project's established disclosure
> discipline (e.g. 003's `Reject.session_status`/`TlsSpec.key_store_path` precedents, which were
> genuine breaks; this one is confirmed not to be, but gets the same scrutiny).

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|---------------------------------------|
| (none — see note above) | — | — |
