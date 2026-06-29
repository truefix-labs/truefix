# Implementation Plan: TrueFix — Full QuickFIX/J-Parity Production FIX Engine

**Branch**: `001-fix-engine-parity` | **Date**: 2026-06-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/001-fix-engine-parity/spec.md`

## Summary

TrueFix is a production-grade FIX protocol engine in Rust at feature-parity with QuickFIX/J, with three
differentiators: production-readiness, first-class acceptor/sell-side support, and FIX conformance (the
ported Acceptance Test suite) as the release gate. The technical approach is an **async-first** engine on
**tokio** (TLS via **rustls**), organized as a layered **cargo workspace** of focused crates, with a
**dual-track data dictionary** (build-time codegen of strongly-typed messages + a runtime
`DataDictionary` validator, both fed from one normalized dictionary derived from FIX Orchestra/Repository).
Although delivered in one pass to full parity, the plan and downstream tasks are **internally staged by
dependency** with a checkpoint per stage so every increment is independently verifiable.

## Technical Context

**Language/Version**: Rust, edition 2021. MSRV = latest stable at implementation start (pin in CI;
document in workspace README). `#![forbid(unsafe_code)]` by default in every crate; any `unsafe` requires
a performance rationale + safety comment + localized `#[allow]` (Constitution Principle I).

**Primary Dependencies**:
- Async runtime / IO / timers / concurrency: **tokio**.
- TLS: **rustls** (+ `tokio-rustls`), `rustls-pemfile` for key/cert loading.
- Typed errors: **thiserror** (named error enums; no `Box<dyn Error>` on critical paths).
- Decimal/price precision: **rust_decimal** (fixed-precision; never f64 for FIX Price/Qty/Amt).
- Time / nanosecond timestamps: **time** crate (UTCTimestamp with nanosecond storage, ≤ picosecond
  parse-accept then truncate) — `chrono` is an acceptable alternative; decision recorded in research.md.
- Serde + config: **serde**; `.cfg` parser is hand-written (QuickFIX `[DEFAULT]`/`[SESSION]` INI-like
  with `${name}` interpolation) since the format is not standard TOML/INI.
- SQL store/log backend: **sqlx** (JDBC-equivalent). Per clarification, SQL backend **is in v1 scope**.
- Observability: **tracing** (structured events/logs) + **metrics** facade (JMX-equivalent monitoring).
- Dictionary source: FIX Trading Community **Orchestra/Repository** machine-readable specs → normalized
  TrueFix dictionary format at build time (FR-A2; License-safe).

**Storage**: Pluggable `MessageStore` (memory, file, cached-file, SQL via sqlx); `NoopStore`.
Sleepycat/JE-equivalent KV store is **deferred for v1** (documented unsupported-with-reason, FR-G1/I2).

**Testing**: Table-driven unit tests (codec + state machine), two-process integration tests, and the
ported **AT suite** (`truefix-at`). CI runs `cargo fmt --check`, `cargo clippy -D warnings`,
`cargo test`, plus AT and benchmarks (visibility-only).

**Target Platform**: Linux/macOS servers (tokio). No platform-specific code expected.

**Project Type**: Library/engine (cargo workspace) + example binaries. Async-first public API (FR-F7).

**Performance Goals**: No numeric gate for v1 (SC-008). Maintain reproducible benchmarks (codec
throughput, session round-trip latency) for regression visibility only. Correctness-first.

**Constraints**: No panic/unwrap/expect/unreachable on codec, session state machine, I/O, or timer paths
(Constitution I; SC-005). All recoverable errors typed. BodyLength/CheckSum byte-identical to QuickFIX/J
for the canonical message set (SC-002). License discipline: no reference source copied (Constitution III).

**Scale/Scope**: 9 FIX versions (4.0/4.1/4.2/4.3/4.4/5.0/5.0SP1/5.0SP2) + FIXT 1.1; ~150 config keys
(Appendix A); 73 distinct server AT scenarios + special-category suites (Appendix B), all targeted
versions are the hard release gate (FR-M3).

**Reference-only architecture inputs**: `thrdpty/quickfix` (Go) and `thrdpty/quickfixj` (Java) read for
design/behavior understanding only; no line-by-line translation or source/data-file copying.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. Production-Ready First | `forbid(unsafe_code)`; clippy `-D warnings` bans unwrap/expect on critical paths (lint config); thiserror typed errors; tracing+metrics observability. Public API async + documented. | ✅ PASS |
| II. Protocol Correctness First (NON-NEG) | AT suite (`truefix-at`) is the acceptance gate; protocol decisions cite FIX spec/reference behavior in research.md & code docs. | ✅ PASS |
| III. License & Provenance (NON-NEG) | Dictionaries from FIX Orchestra/Repository, normalized; reference repos design-only; AT ported as black-box fixtures; all deps verified Apache-2.0/MIT-compatible. | ✅ PASS (dep audit is a Phase 0 task) |
| IV. Dual-Track Data Dictionary | `truefix-dict` provides build.rs codegen **and** runtime `DataDictionary` from one normalized source; research.md argues coexistence. | ✅ PASS |
| V. Test Discipline | Table-driven unit + two-process integration + ported AT; test-first encouraged; CI enforces. | ✅ PASS |
| VI. Acceptor/Initiator Parity | `truefix-transport` designs Acceptor and Initiator as peers; dynamic sessions first-class; AT run against acceptor. | ✅ PASS |
| VII. Inventory-Based Completeness | Appendix A/B are the baselines; tasks trace per-key/per-scenario; `parity.md` checklist gates requirement quality. | ✅ PASS |

**Conflict-order reminder** (governance): License (absolute) → Protocol correctness → Production
readiness → feature count. No current design choice trades against a higher principle.

**Initial gate result**: PASS. No violations requiring Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/001-fix-engine-parity/
├── plan.md              # This file
├── spec.md              # Feature spec (Appendix A/B baselines)
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (public API/behavior contracts)
│   ├── README.md
│   ├── core-codec.md
│   ├── dictionary.md
│   ├── session.md
│   ├── store-log.md
│   ├── transport.md
│   ├── config-keys.md
│   ├── application-api.md
│   └── at-runner.md
└── checklists/
    ├── requirements.md  # spec quality (from /speckit-specify)
    └── parity.md        # config-key + AT parity coverage (from /speckit-checklist)
```

### Source Code (repository root)

Cargo workspace, layered by dependency (lower depends on nothing above it):

```text
Cargo.toml                  # [workspace] members + shared lints/deps
crates/
├── truefix-core/           # Field, FieldMap, Group (nested), Message(header/body/trailer),
│                           #   field types & converters, SOH codec, BodyLength/CheckSum, typed errors
├── truefix-dict/           # dictionary model + parser (normalized format), runtime DataDictionary
│   └── build.rs            #   codegen of strongly-typed messages from the same normalized source
├── truefix-session/        # session state machine, seq mgmt, admin msgs (Logon/Logout/Heartbeat/
│                           #   TestRequest/ResendRequest/SequenceReset/Reject), scheduling, resend,
│                           #   789 NextExpectedMsgSeqNum, chunked resend, latency checks
├── truefix-store/          # MessageStore trait + Memory/File/CachedFile, sqlx SQL backend, Noop
├── truefix-log/            # Log trait + Screen/File/Tracing/Composite; message vs event separation
├── truefix-transport/      # Initiator + Acceptor (single/multi-thread, multi-session, dynamic
│                           #   sessions, reconnect), rustls TLS, socket options
├── truefix-config/         # SessionSettings, .cfg parser + ${name} interpolation, programmatic config
├── truefix/                # Facade: re-exports + Application trait + MessageCracker
└── truefix-at/             # Acceptance Test suite runner + ported scenario fixtures
examples/
├── executor/               # acceptor that executes orders (QFJ executor-equivalent)
├── banzai/                 # initiator client (QFJ banzai-equivalent)
└── ordermatch/             # order-matching acceptor (QFJ ordermatch-equivalent)
tests/                      # cross-crate two-process integration tests
benches/                    # codec + round-trip benchmarks (visibility-only)
```

**Structure Decision**: Layered cargo workspace. Dependency direction is strictly downward:
`core` ← `dict` ← `session`; `store`, `log`, `config` are leaf services consumed by `session`/`transport`;
`transport` depends on `session`; `truefix` facade depends on all; `truefix-at` depends on the facade.
This enforces the staged build order and keeps each layer independently testable (Constitution V/VII).

### Internal Delivery Stages (single release, dependency-ordered, each with a checkpoint)

> One-pass full parity (Constitution: one-pass goal) but **internally staged** so each increment is
> verifiable. `/speckit-tasks` will expand each stage into tasks; these are NOT separate releases.

| Stage | Scope | Checkpoint (verifiable exit) |
|-------|-------|------------------------------|
| S0 | Workspace skeleton, CI (fmt/clippy -D warnings/test), lint policy, dep license audit | CI green on empty crates; license audit clean |
| S1 | `truefix-core`: model + SOH codec + field types + BodyLength/CheckSum + typed errors | Table-driven codec round-trip; byte-exact BL/CS vs reference for canonical msgs (SC-002) |
| S2 | Minimal `truefix-session` + `truefix-transport`: Logon/Heartbeat/TestRequest/Logout over TCP | Two-process integration: US1 logon→heartbeat→logout on FIX 4.2 & 4.4 |
| S3 | Session recovery: seq mgmt, ResendRequest, SequenceReset (GapFill/Reset), PossDup, chunked resend, 789 | US3 scenarios; relevant server AT sequence cases pass |
| S4 | `truefix-dict`: normalized dict + parser + runtime DataDictionary + build.rs codegen (dual-track) | Validation toggles produce correct accept/reject; codegen+runtime agree on one source (Principle IV) |
| S5 | `truefix-store` + `truefix-log`: memory/file/cached + sqlx; screen/file/tracing/composite | Restart-survivable resend (SC-006); message/event log separation |
| S6 | Multi-session, scheduling, dynamic sessions, reconnect, allow-listing | US4/US6 integration; dynamic-session acceptance |
| S7 | All 9 FIX versions + full codegen breadth | Round-trip + validation across every version |
| S8 | TLS (rustls), Logon auth, monitoring (tracing+metrics) | TLS handshake both roles; auth accept/reject; monitoring surface (SC-007) |
| S9 | `truefix-at`: full ported AT suite across all targeted versions | **Release gate**: FR-M3 all-versions AT green; examples runnable |

## Complexity Tracking

> No Constitution violations. The large surface (9 versions, ~150 keys, ~480 AT defs) is inherent to the
> parity goal (Principle VII), not accidental complexity; it is managed by the staged structure above and
> the inventory checklists rather than by deviating from any principle. No justification entries required.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none) | — | — |
