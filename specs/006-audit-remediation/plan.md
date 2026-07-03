# Implementation Plan: Audit Remediation (docs/todo/003.md)

**Branch**: `main` (feature tracked by directory, not a dedicated branch — mirroring 004/005's
convention; no `before_plan`/branch-creation hook is registered) | **Date**: 2026-07-03 | **Spec**:
[spec.md](./spec.md)

**Input**: Feature specification from `specs/006-audit-remediation/spec.md`

## Summary

A 2026-07-02/03 audit (`docs/todo/003.md`), produced by five independent deep-dive reviews plus a
supplementary Chinese-language pass (`B1`-`B30`), found 6 fresh P0 protocol-correctness/data-loss/DoS
bugs (`BUG-05`/`BUG-06` session anti-replay+desync holes, `BUG-07` SubID/LocationID/Qualifier routing
break, `BUG-08`/`BUG-09` silent store failures + spurious mid-window resets, `BUG-11` orphaned-task
leak), 6 P1 bugs (`BUG-10` JDBC grammar, `BUG-12`-`BUG-17`), 5 "closed-but-incomplete" re-openings
(`GAP-26`/`GAP-27` dormant dictionary features, `GAP-28`/`GAP-32` orphaned version-meta, `GAP-33`/
`GAP-56` stale provenance, `GAP-39`/`GAP-49` FileStore atomicity gap), 12 carried-forward
feature-completeness gaps, 11 new P2 minor bugs/tooling gaps (`GAP-50`-`GAP-56`, `BUG-18`-`BUG-22`),
2 AT-harness follow-ups, and 12 supplementary Chinese-pass findings (`B1`-`B30`, mostly session/
transport/store edge cases the five-review pass's English writeups didn't independently re-derive).
Phase 0 research (below) re-verified every citation this plan relies on by reading current source
directly — every one checked out exactly as the audit described, including running the AT suite live
to confirm its actual scenario-run count (**373**, not the stale asserted floor of 353). Three
scope-defining decisions were made via `/speckit-clarify` rather than assumed: `jdbc:h2:` gets removed
from the recognized-scheme table rather than gaining a real H2 backend (no new dependency); a
`SessionQualifier`-distinguished session must bind its own distinct listener/template rather than
relying on a wire-field guess; and a grouped-acceptor's conflicting `ContinueInitializationOnError`
values resolve by strictest-member-wins. The work spans 10 user stories / 46 functional requirements
and is delivered **in one pass, internally staged (R1-R10, one stage per user story)** with a
verifiable checkpoint per stage, mirroring 001-005's model.

## Technical Context

**Language/Version**: Rust, edition 2021, MSRV per workspace pin (`rust-toolchain.toml`, currently
1.96.0). `unsafe_code = "forbid"` (workspace-wide) and per-crate
`#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic,
clippy::indexing_slicing))]` on `truefix-session`/`truefix-core`/`truefix-dict` (and equivalently
strict discipline elsewhere on critical paths) remain unchanged (Principle I) — every fix in this
feature touches code already under that discipline (`truefix-session::state`,
`truefix-transport::lib`/`framing`/`proxy`, `truefix-store::{file,mssql}`, `truefix-config::builder`,
`truefix-dict::{model,fix_repository}`, `truefix::lib`).

**Primary Dependencies**: No new external crates. Every FR is implemented against already-present
workspace dependencies (`tokio`, `tokio-rustls`/`rustls`, `thiserror`, `sqlx`, `tiberius`, `redb`,
`mongodb`, `socket2`, `time`, `tracing`). The one dependency question this audit raised —
whether to add an H2-compatible driver for `jdbc:h2:` (US4) — was resolved by `/speckit-clarify`:
**no**, the scheme is rejected cleanly instead (FR-018), so no new dependency is introduced anywhere
in this feature.

**Storage**: All 7 existing `MessageStore` backends (Memory/File/CachedFile/SQL(PG/MySQL/SQLite)/
MSSQL/Redb/Mongo) are touched by US3: `FileStore`/`CachedFileStore` gain an atomic
`save_and_advance_sender` override (closing `GAP-49`, the one gap 005's `GAP-39` atomicity work left
open); `MssqlStore` gains identifier validation (`BUG-14`); `BodyLog::reset()` (used by both file
backends) gains an `fsync` call gated on `self.sync` (`BUG-15`); `FileStore`/`CachedFileStore::reset()`
gains crash-safe ordering between body-clear and seqnum-clear (`B17`). `MessageStore::creation_time()`
(already persisted correctly by all backends since `GAP-38`, feature 005) gains its first real
consumer: `truefix-transport::run_scheduled_initiator` (`BUG-09`/`GAP-48`). No new backend, no
trait-contract-breaking change — every store-layer change is either an added trait-method override
(same signature) or an internal-ordering/validation fix.

**Testing**: Table-driven unit tests for every codec/config-parsing/dictionary fix (US4, US7, parts of
US6/US10); two-process integration tests for every session-state-machine change (US1's 8 scenarios —
low-seq Logon rejection, SequenceReset anti-replay, malformed ResendRequest, `ResetOnLogon`
reconnect-loop fix, gap-fill validation-bypass fix, per-connection timer reset, pre-disconnect Logout,
unparseable-`SendingTime` latency fix — each MUST land as both a `truefix-session` state-machine test
and, per Constitution Principle II, a **new AT scenario** since these are genuine protocol-behavior
corrections, not just internal refactors); live-connection integration tests for US2's SubID/
LocationID/`SessionQualifier` routing and US6's DoS-hardening scenarios; store/log conformance-suite
extensions for US3; `.cfg`-mapping tests for US4/US8's config-key work; AT-suite tests for US9.
**AT suite**: currently **373/373 passing** (verified live during Phase 0, not merely read from the
audit) — `BUG-17`'s fix bumps the stale `>= 353` regression-floor literal
(`crates/truefix-at/tests/coverage.rs:41`) to `>= 373` as its own first, trivial sub-task, and the
suite is expected to **grow further** as US1's 8 new protocol-behavior scenarios land (part of this
feature's Definition of Done, not merely non-regression — mirrors 005's SC-013 precedent).

**Target Platform**: Linux/macOS servers (tokio). No platform-specific code added.

**Project Type**: Library/engine (existing cargo workspace: `truefix`, `truefix-at`, `truefix-config`,
`truefix-core`, `truefix-dict`, `truefix-log`, `truefix-session`, `truefix-store`, `truefix-transport`).
No new crates, no new binary targets — `truefix-dict`'s existing `truefix-dict-cli` binary target is
unaffected (US7's `GAP-56` fix only touches shipped `.fixdict` header comments, not the tool that
would regenerate them).

**Performance Goals**: Correctness/completeness first, per Constitution Principle II's conflict-order
(protocol correctness > production readiness > feature count > performance). No specific new
performance target — US6's DoS-hardening (bounded frame size, proxy connect timeout, PROXY-header
peek timeout) is a reliability/availability feature, not a throughput/latency target, and US10's
`GAP-52` (O(fields × enums) → O(log n + k) enum-emission) is a build-time-only improvement with no
runtime-facing performance claim.

**Constraints**: No panic/unwrap/expect/unreachable on codec/session/I-O/timer paths (Principle I),
unchanged. `unsafe_code = forbid` remains workspace-wide. No reference source/data copied — all fix
directions in this plan were derived by reading `docs/todo/003.md`'s **behavioral description** of
each gap plus TrueFix's own current source, cross-checked against the FIX protocol specification and
QuickFIX/J's **documented/observed behavior** (never its source), per Principle III's established
boundary (same boundary 001-005 already established for AT-scenario porting and gap remediation).
This feature introduces one disclosed, additive public-API surface growth: `MssqlStore`'s MSSQL URL
parser (`crates/truefix-store/src/mssql.rs::parse_url`) grows a second accepted grammar (real
QuickFIX/J semicolon-delimited `jdbc:sqlserver://host[:port][;databaseName=X;user=Y;password=Z]`,
additive to the existing path-based `user:password@host[:port]/database` form — FR-019); no other
public type gains, loses, or narrows a field/variant. `FileStore`/`CachedFileStore` gaining a
`save_and_advance_sender` override is **not** a public-API change (the trait method and its signature
already exist from feature 005; only the two backends' internal implementation gains an override).

**Scale/Scope**: 10 user stories, 46 functional requirements, ~20 source files across 6 crates
(`truefix-session`, `truefix-transport`, `truefix-store`, `truefix-config`, `truefix-dict`,
`truefix-core`, `truefix`), staged R1-R10 (one stage per user story, ordered P1→P2→P3 matching the
spec's own priority ordering and the audit's own suggested triage order).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design (below).*

| Principle | Check | Status |
|---|---|---|
| I. 生产就绪优先 (Production-Ready First) | Every fix in US1-US3/US6 replaces a silent-failure/panic-adjacent/unbounded-resource path with a typed error, an operator-visible log event, or a bounded resource — the entire feature *is* a production-readiness remediation pass. `unwrap`/`expect`/`panic`/indexing-slicing denies stay in force; no fix requires loosening them (verified per-file during Phase 0 reads — every touched function already returns `Result`/`Vec<Action>`, no new panic surface introduced). | **PASS** |
| II. 协议正确性优先 (Protocol Correctness First) — NON-NEGOTIABLE | US1's 8 scenarios are each grounded in FIX-spec-equivalent QuickFIX/J/Go **behavior** (verified during Phase 0: `Session.java`'s `verify`/`isTargetTooLow`/`doTargetTooLow` gate cited for `BUG-05`; three-way `newSequence` branch cited for `BUG-06`; `handleResendRequest`'s required-tag rejection cited for `BUG-22`) and each MUST land a new/extended AT scenario (research.md §R1). US2's routing fix is a wire-protocol-adjacent transport-layer correctness fix, not itself session-semantic — no AT scenario required, but a live-connection integration test is (spec's own Independent Test). | **PASS** |
| III. License 与来源纪律 (License & Provenance) — NON-NEGOTIABLE | Every fix direction in this plan is derived from `docs/todo/003.md`'s prose description of the *gap* plus TrueFix's own current source — never from reading `thrdpty/quickfixj`/`thrdpty/quickfix` source during this planning pass (Phase 0 research cites QFJ/QFGo class/method **names and documented behavior** only, exactly as `docs/todo/003.md` itself already did, not new source reads). `jdbc:h2:` removal (US4) avoids the alternative (implementing H2 support) partly *because* it would require deeper study of a third reference implementation's JDBC layer — the clarify-resolved scope avoids that risk entirely. | **PASS** |
| IV. 数据字典双轨 (Dual-Track Data Dictionary) | US7 (`GAP-26`/`GAP-27` dormant-feature wiring) is precisely a dual-track-integrity fix: today codegen's `encode()` and the production decode path silently diverge from the runtime `DataDictionary`'s own `field_order`/group-decode capabilities. Wiring them in restores the "MUST NOT diverge" invariant Principle IV already requires — this feature is closing a pre-existing constitutional gap, not introducing risk of one. | **PASS** |
| V. 测试纪律 (Test Discipline) | Table-driven + integration + AT-port discipline is designed into every user story's Independent Test (spec.md) and carried through into this plan's Testing section above. US1 in particular adds 8 new AT scenarios as first-class deliverables, not afterthoughts. | **PASS** |
| VI. Acceptor/Initiator 对等 (Parity) | US1's session-layer fixes are verified on both roles where the underlying behavior applies to both (spec Assumptions): `BUG-05` (Logon low-seq) is acceptor-and-initiator-symmetric (either role can receive a Logon); `BUG-06`/`BUG-22` (SequenceReset/ResendRequest) apply to both; `B1` (`ResetOnLogon`) is initiator-specific by its very nature (only initiators send the first Logon) but its *fix* must be verified against an acceptor counterpart reconnecting correctly, not in isolation. US2's routing fix is acceptor-only by construction (only acceptors route inbound connections by SessionID) — correctly asymmetric, not a Principle VI violation (mirrors 005's precedent for acceptor-specific routing work). | **PASS** |
| VII. 基于清点的完整性 (Inventory-Based Completeness) | This entire feature *is* the inventory-based remediation of `docs/todo/003.md`'s clean-room re-audit — every FR traces to a specific `BUG-NN`/`GAP-NN`/`B-NN` citation with a verified file:line source (spec.md's Source-document framing + this plan's research.md). | **PASS** |

**Result**: No violations. No Complexity Tracking entries required (see below — table intentionally
empty).

## Project Structure

### Documentation (this feature)

```text
specs/006-audit-remediation/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md         # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── README.md
│   ├── session-protocol.md      # US1 — protocol-behavioral, grows the AT suite
│   ├── routing-and-lifecycle.md # US2, US5
│   ├── store-persistence.md     # US3
│   ├── config-compat.md         # US4
│   ├── network-hardening.md     # US6
│   ├── dictionary-codec.md      # US7
│   ├── completeness-and-harness.md # US8, US9
│   └── tooling-hygiene.md       # US10
├── checklists/
│   └── requirements.md
└── tasks.md              # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
# Existing cargo workspace (001-005) — no new crates, no new binary targets.
crates/
├── truefix/               # Engine facade — US5 (Engine::start partial-failure cleanup, shutdown doc)
├── truefix-at/             # AT harness — US1 (8 new scenarios), US9 (regression floor, MinQty scenario)
├── truefix-config/         # .cfg parsing/resolution — US4 (JdbcURL grammar), US8 (TimeZone, ${var} env
│                            # fallback, log-backend selection)
├── truefix-core/           # wire codec, tags — US7 (production encode/decode wiring, EncodedLeg* pairs)
├── truefix-dict/           # DataDictionary, codegen, fix_repository — US7 (UtcTimeOnly/UtcDate
│                            # validation, dormant-feature wiring, provenance headers), US10 (cycle
│                            # guard, doc-accuracy, enum-emission complexity)
├── truefix-log/            # (no changes — MssqlLog/SqlLog identifier validation already correct)
├── truefix-session/        # session state machine — US1 (8 protocol-correctness fixes)
├── truefix-store/          # MessageStore backends — US3 (FileStore/CachedFileStore atomicity, MSSQL
│                            # identifier validation, BodyLog fsync/reset ordering, TOCTOU close)
└── truefix-transport/      # session I/O, framing, proxy, Engine wiring — US2 (routing key, SO_REUSEADDR),
                             # US3 (swallowed-error logging, creation_time consumption), US6 (frame-size
                             # bound, proxy timeout, CipherSuites validation, PROXY-header timeout/sizing,
                             # framing recovery)

# Existing top-level docs (unaffected structurally)
docs/todo/003.md            # Source audit for this feature (read-only input)
```

**Structure Decision**: No new crates or directories. This feature is a remediation pass entirely
within the existing 9-crate layered workspace established by 001-005 (`specs/001-fix-engine-parity/
plan.md`'s layering: `truefix-core` → `truefix-dict`/`truefix-store`/`truefix-log` →
`truefix-session` → `truefix-transport` → `truefix`/`truefix-at`). Every fix lands in the crate that
already owns the affected behavior; no fix requires introducing a new cross-crate dependency edge.

## Complexity Tracking

*No entries — Constitution Check above found no violations requiring justification.*
