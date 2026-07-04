# Implementation Plan: Second-Pass Audit Remediation

**Branch**: `007-second-audit-remediation` | **Date**: 2026-07-04 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/007-second-audit-remediation/spec.md`

## Summary

`docs/todo/004.md` is a second-pass audit covering defects **not** already tracked in
`docs/todo/003.md` (closed by feature 006): a pure per-crate code review plus a systematic
QuickFIX/J/Go source comparison, itself carrying two internal self-verification passes before this
feature started (one confirmed/corrected/retracted individual findings, a second four-agent
cross-check against the current working tree found 3 items already fixed as a side effect of 006).
This plan's own Phase 0 research (below) independently re-read current source for every P1 item and
a representative sample of rescoped P2/P3 items — all checked out exactly as `004.md`'s own
corrected framing describes. The work spans 3 priority-ordered user stories / 51 functional
requirements (`FR-001`-`FR-050` plus `FR-001a`): 13 must-fix-before-shipping items (data loss,
protocol-breaking handshakes, resource leaks, a DoS-adjacent malformed-input acceptance, and an
acceptor with zero trading-hours schedule awareness), 16 narrower-blast-radius defects, and 20
low-priority hardening/hygiene items. Three clarifications were resolved via `/speckit-clarify`
rather than assumed: the sequence-number store adopts QuickFIX/J-Go's split two-file layout (not the
minimal atomic-single-file fix), all three priority tiers are addressed in one pass (not split
across features), and existing single-file deployments auto-migrate transparently on first open
after upgrading.

## Technical Context

**Language/Version**: Rust, edition 2021, MSRV per workspace pin (`rust-toolchain.toml`, currently
1.96.0). `unsafe_code = "forbid"` (workspace-wide) and per-crate
`#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic,
clippy::indexing_slicing))]` remain unchanged (Principle I) — every fix touches code already under
that discipline (`truefix-session::state`, `truefix-transport::lib`, `truefix-store::{file,sql,
mssql}`, `truefix-core::framing`, `truefix::lib`).

**Primary Dependencies**: No new external crates. Every FR is implemented against already-present
workspace dependencies (`tokio` — specifically `tokio::task::JoinHandle`'s existing synchronous
`abort()`/`is_finished()` methods, already available, just not yet exposed through
`SessionHandle`/`ReconnectHandle` — `thiserror`, `sqlx`, `tiberius`, `time`, `tracing`).

**Storage**: The sequence-number half of `FileStore`/`CachedFileStore` (`crates/truefix-store/src/
file.rs`'s `SeqFile`) changes its on-disk shape from one combined `seqnums` file to two separate
files (`senderseqnums`/`targetseqnums`), each atomically written and wholesale-deleted-and-recreated
on reset, with transparent auto-migration from the legacy format on open (research.md §R1.1,
Clarifications). `SqlStore`'s `ensure_schema` migration-ordering bug (research.md §R1.4) and
`MssqlStore`'s missing commit-failure rollback (§R1.5) are both touched; no other backend
(Memory/Redb/Mongo) is affected — `004.md` raised no P0/P1 findings against them.

**Testing**: Table-driven unit tests for every codec/config/store-format fix; two-process integration
tests for every session-state-machine change (the `ResetSeqNumFlag` handshake, duplicate-connection
refusal, `Event::Disconnected` plumbing, callback-ordering restructure, and the new acceptor-side
schedule enforcement all require live acceptor/initiator pairs, not just unit-level state-machine
tests, since several of them are inherently about *inter-process* protocol behavior). Per Constitution
Principle II, genuine protocol-behavior corrections among these MUST also land as AT scenarios — this
plan's own assessment (research.md, Constitution Check below) is that most User Story 1 items are
session-*internal* correctness fixes reachable by the existing `truefix-session`/`truefix-transport`
integration-test patterns already used for 005/006's equivalent items, with the `ResetSeqNumFlag`
handshake (`BUG-28`) and acceptor schedule enforcement (`BUG-86`/`87`) as the two most likely
candidates for new AT scenarios (to be confirmed per-item during `/speckit-tasks`, matching how 006's
task breakdown made this determination per-FR rather than pre-committing here). **AT suite**:
currently 405/405 passing (feature 006's closing state, reconfirmed live during this plan's Phase 0)
— this feature's regression floor must not decrease, and any new AT scenario bumps it upward,
mirroring 005/006's precedent.

**Target Platform**: Linux/macOS servers (tokio). No platform-specific code added.

**Project Type**: Library/engine (existing cargo workspace: `truefix`, `truefix-at`, `truefix-config`,
`truefix-core`, `truefix-dict`, `truefix-log`, `truefix-session`, `truefix-store`,
`truefix-transport`). No new crates, no new binary targets.

**Performance Goals**: Correctness/completeness first, per Constitution Principle II's conflict-order.
No specific new performance target — `FR-031`'s `FileStore`/`CachedFileStore` bounded-open-memory fix
and `FR-045`'s bounded admin channel are reliability/resource-bound features, not throughput/latency
targets.

**Constraints**: No panic/unwrap/expect/unreachable on codec/session/I-O/timer paths (Principle I),
unchanged. `unsafe_code = forbid` remains workspace-wide. No reference source read during this
planning pass beyond `docs/todo/004.md`'s own prose citations of QuickFIX/J/Go class/method names and
documented behavior (Principle III's established boundary, same as 006). This feature discloses three
additive public-API surface growths, none breaking: `SessionHandle` gains `abort(&self)` (sync) and
`is_finished(&self) -> bool` (sync, non-consuming); `Engine` gains `impl Drop`; `truefix_session::
state::Event` gains a new variant, `Disconnected` (research.md's "Summary of decisions requiring
disclosure" section has the full list with rationale for each).

**Scale/Scope**: 3 user stories, 51 functional requirements (`FR-001`-`FR-050` + `FR-001a`), touching
~7 source files across 5 crates (`truefix-session`, `truefix-transport`, `truefix-store`,
`truefix-core`, `truefix`), staged R1→R2→R3 matching the spec's own priority ordering.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design (below).*

| Principle | Check | Status |
|---|---|---|
| I. 生产就绪优先 (Production-Ready First) | Every User Story 1 item replaces a silent-data-loss, protocol-desync, or resource-leak path with a typed error, an explicit rejection, or a bounded/cleanly-stoppable resource. `unwrap`/`expect`/`panic`/indexing-slicing denies stay in force — research.md's per-item reads confirm every touched function already returns `Result`/`Vec<Action>`; no new panic surface is introduced (e.g. `SeqFile`'s new atomic-write path uses the same `Result`-returning `io_err` pattern its existing writes already use). | **PASS** |
| II. 协议正确性优先 (Protocol Correctness First) — NON-NEGOTIABLE | `BUG-28` (`ResetSeqNumFlag` handshake), `BUG-29` (`ResendRequest` deadlock), `BUG-32` (duplicate connections), `BUG-33` (`Event::Disconnected`), `BUG-34` (callback ordering), `BUG-86`/`87`/`88` (schedule enforcement), and `BUG-89` (admin dictionary validation) are all genuine session-protocol-behavior corrections grounded in QuickFIX/J/Go's documented behavior (research.md §R1.2-R1.4, R1.8-R1.13) — each is a Test Discipline candidate for a new AT scenario, to be finalized per-item at `/speckit-tasks` time (Testing section above). | **PASS** |
| III. License 与来源纪律 (License & Provenance) — NON-NEGOTIABLE | Every decision in research.md is derived from `docs/todo/004.md`'s prose description of the gap plus TrueFix's own current source (this plan's Phase 0 re-reads) — QuickFIX/J/Go citations throughout are class/method **names and documented behavior** only, exactly as `004.md` itself already cites them, never new reference-source reads. | **PASS** |
| IV. 数据字典双轨 (Dual-Track Data Dictionary) | Not applicable — no dictionary/codegen changes in this feature (User Story 3's codegen items, `FR-050`, are about codegen *robustness*, not dual-track divergence; they don't touch the runtime `DataDictionary` path at all). | **N/A** |
| V. 测试纪律 (Test Discipline) | Table-driven + integration + (where applicable) AT-port discipline designed into every User Story's Independent Test (spec.md) and this plan's Testing section. The store-format migration (`FR-001a`) additionally requires a dedicated fixture-based test (a pre-existing legacy-format file), not just the standard crash-safety unit tests — called out explicitly in research.md §R1.1 so it isn't missed at `/speckit-tasks` time. | **PASS** |
| VI. Acceptor/Initiator 对等 (Parity) | Spec Edge Cases explicitly requires every session-protocol item to be verified against both roles. `BUG-86`/`87` (schedule enforcement) is acceptor-only by construction (only acceptors currently lack any schedule awareness at all — initiators already have it via `run_scheduled_initiator`), correctly asymmetric per 005/006's own precedent for acceptor-specific gaps; `BUG-32` (duplicate-connection refusal) is likewise acceptor-only by construction (only acceptors route inbound connections by SessionID). Every other User Story 1/2 protocol item (`BUG-28`, `BUG-33`, `BUG-34`, `BUG-89`, etc.) applies symmetrically and must be tested on both roles. | **PASS** |
| VII. 基于清点的完整性 (Inventory-Based Completeness) | This entire feature is the inventory-based remediation of `docs/todo/004.md`'s own already-triple-verified findings — every FR traces to a specific `BUG-NN` citation with a verified file:line source (research.md). The spec's own Assumptions section additionally discloses every item `004.md` raised that this feature does **not** close, with a stated reason each — no silent scope-narrowing. | **PASS** |

**Result**: No violations. No Complexity Tracking entries required (table intentionally empty).

## Project Structure

### Documentation (this feature)

```text
specs/007-second-audit-remediation/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── README.md
│   ├── store-durability.md      # US1 — sequence-number store format/migration, SQL/MSSQL fixes
│   ├── session-protocol.md      # US1/US2 — ResetSeqNumFlag, ResendRequest, duplicate connections,
│   │                             # Event::Disconnected, callback ordering, schedule enforcement
│   ├── engine-lifecycle.md      # US1 — Engine::shutdown/Drop, scheduled-initiator reconnect
│   └── hardening.md             # US2/US3 — narrower-impact and low-priority items
├── checklists/
│   └── requirements.md
└── tasks.md              # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
# Existing cargo workspace (001-006) — no new crates, no new binary targets.
crates/
├── truefix/               # Engine facade — US1 (Engine::shutdown/Drop stopping plain initiators)
├── truefix-core/           # wire codec, framing — US1 (BodyLength=0 rejection), US2/US3
│                            # (checksum-position verification, BeginString format, overflow hardening)
├── truefix-session/        # session state machine — US1 (ResetSeqNumFlag handshake, ResendRequest
│                            # deadlock, Event::Disconnected, admin dictionary validation, schedule
│                            # enforcement), US2/US3 (resend_requested reset, validLogonState,
│                            # SubID/LocationID on send_app, and most of the narrower-impact tier)
├── truefix-store/          # MessageStore backends — US1 (sequence-number store format/migration,
│                            # SqlStore migration ordering, MssqlStore commit rollback), US2/US3
│                            # (FileStore/CachedFileStore open-time memory, MSSQL URL parsing)
├── truefix-transport/      # session I/O — US1 (duplicate-connection refusal, scheduled-initiator
│                            # reconnect detection, callback-ordering restructure), US2/US3 (socket
│                            # options on the plain reconnecting path, multi-endpoint rotation reset,
│                            # ReconnectHandle::stop, admin-channel backpressure)
└── truefix-dict/           # codegen only (dict-tooling feature) — US3 (FR-050: codegen type-mapping
                             # gaps, Rust-keyword enum-label collisions, `open` modifier divergence
                             # from the runtime parser)

# Existing top-level docs (unaffected structurally)
docs/todo/004.md            # Source audit for this feature (read-only input)
```

**Structure Decision**: No new crates or directories. This feature is a remediation pass entirely
within the existing layered workspace established by 001-006. Every fix lands in the crate that
already owns the affected behavior; no fix requires introducing a new cross-crate dependency edge.
`truefix-config`, `truefix-log`, and `truefix-at` are untouched by this feature — `004.md` raised no
findings against those crates' own logic. `truefix-dict` is touched narrowly: `BUG-72`/`73`/`74`
(codegen type-mapping gaps, enum-label keyword collisions, and `open`-modifier dual-track
divergence, `FR-050`) are all inside `codegen.rs`, gated behind the existing `dict-tooling` feature
— no change to the runtime (non-codegen) `DataDictionary` path, so Constitution Principle IV's
dual-track-divergence concern doesn't apply here (these are codegen *robustness* fixes making its
output match what the runtime path already correctly does, not a new divergence).

## Complexity Tracking

*No entries — Constitution Check above found no violations requiring justification.*
