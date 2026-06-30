# Implementation Plan: Complete Remaining QuickFIX/J Parity (excl. QuickFIX/Go-only)

**Branch**: `002-qfj-parity-completion` | **Date**: 2026-06-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/002-qfj-parity-completion/spec.md`

## Summary

Feature 001 delivered a structurally-complete engine; a 2026-06-30 audit (`docs/todo-gap-analysis.md`)
found 21 remaining QuickFIX/J-parity gaps. This feature closes **all of them except QuickFIX/Go-only
extras**, organised so the highest-value correctness and usability gaps land first. The work is
additive to the existing layered cargo workspace — no new architecture — and is **delivered in one pass
but internally staged (G1–G10) with a verifiable checkpoint per stage**, mirroring 001's S0–S9 model.
Two stages close partially-met constitution principles: typed codegen of **all** messages + a working
MessageCracker (Principle IV) and a `metrics`-facade observability export (Principle I). The ported AT
suite remains the release gate; every correctness behaviour is proven by an AT scenario or session-level
test before its stage is "done".

## Technical Context

**Language/Version**: Rust, edition 2021, MSRV per workspace pin. `#![forbid(unsafe_code)]` retained;
per-crate `deny(clippy::unwrap_used/expect_used/panic/indexing_slicing)` on non-test code (Principle I).

**Primary Dependencies** (all already in the workspace; this feature adds usage, not new architecture):
- tokio + tokio-rustls/rustls (+ **rustls-pemfile** for loading key/cert from `.cfg` paths — new usage).
- thiserror (new typed error enums: `ConfigError`, callback `Reject`/`DoNotSend`/`BusinessReject`).
- **sqlx** with Postgres + MySQL + SQLite features (per Clarifications) behind the `sql` feature.
- **metrics** facade (already a workspace dep; now actually emitted to — Principle I).
- time crate (extend UTCTimestamp formatting to SECONDS/MILLIS/MICROS/NANOS — FR-009).
- Dictionary source: existing normalized `dict-src/normalized/*.fixdict` (no new third-party data).

**Storage**: `MessageStore` gains a **session-owned** persistence path (FR-001/002): the `Session`
resend path reads/writes the store instead of an in-memory map. SQL store/log extend SQLite → +Postgres
+MySQL. `CachedFileStore` gains a real in-memory cache + `FileStoreSync` fsync toggle (FR-025).

**Testing**: Table-driven unit tests (codec/group-parsing/validation/config-mapping), two-process
integration (config-driven start, restart-resend, TLS/mTLS, schedule-reset), and **new AT scenarios**
in `truefix-at` for the integrity/group/reverse-route classes the audit lists as uncovered. SQL
multi-backend tests gate on DB availability (Postgres/MySQL service containers in CI; SQLite always).

**Target Platform**: Linux/macOS servers (tokio). No platform-specific code.

**Project Type**: Library/engine (existing cargo workspace) + example binaries. Async-first public API.

**Performance Goals**: Correctness-first; no numeric gate. Maintain codec/round-trip benchmarks for
regression visibility (existing).

**Constraints**: No panic/unwrap/expect/unreachable on codec/session/I-O/timer paths (Principle I).
BodyLength/CheckSum byte-identical and codegen↔runtime dual-track hash identical (Principle IV). No
reference source/data copied (Principle III). The typed-callback change is a **deliberate breaking API
change** (pre-1.0) and MUST ship with migration notes + a semver bump (Principle I).

**Scale/Scope**: 27 FRs across 12 user stories; ~30 config keys move recognised→implemented (Appendix A
stance updates, FR-027); new AT scenarios for the audit's uncovered integrity/group/reverse-route set;
typed codegen for **every** message in the bundled dictionary subsets across targeted versions.

**Reference-only architecture inputs**: `thrdpty/quickfix` (Go) and `thrdpty/quickfixj` (Java) read for
design/behaviour only; AT ported as black-box fixtures; no source/data-file copying (Principle III).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. Production-Ready First | Existing lint policy retained; new error types are typed (`thiserror`); metrics export added (FR-023) — **completes** the observability requirement. **Condition**: the typed-callback breaking change (FR-016) MUST carry migration notes + semver bump. | ✅ PASS (with documented condition) |
| II. Protocol Correctness First (NON-NEG) | Every correctness FR (001-011) is gated by a new/existing AT scenario or session test (FR-012); group/integrity/reverse-route behaviours cite FIX-spec/reference behaviour in research.md. | ✅ PASS |
| III. License & Provenance (NON-NEG) | New AT scenarios authored as black-box fixtures; codegen from the existing normalized dict (no copied data); rustls-pemfile/sqlx-Postgres/MySQL license-audited (Phase 0 task). | ✅ PASS (dep audit is a Phase 0 task) |
| IV. Dual-Track Data Dictionary | Typed codegen for **all** messages + MessageCracker (FR-020/022) **completes** the unmet half; dual-track hash still proves one source (FR-021). | ✅ PASS (completes principle) |
| V. Test Discipline | Table-driven unit + two-process integration + new AT scenarios; test-first for each stage; CI enforces. | ✅ PASS |
| VI. Acceptor/Initiator Parity | Config-driven start, TLS/mTLS, schedule-reset, and metrics designed and tested for both roles; AT runs against the acceptor, config-start exercises both. | ✅ PASS |
| VII. Inventory-Based Completeness | Appendix A keys moved recognised→implemented update their registered stance (FR-027); audit TODO IDs trace to stages; AT corpus extended for the uncovered set. | ✅ PASS |

**Conflict-order reminder**: License → Protocol correctness → Production readiness → feature count. The
only trade is the breaking callback API, justified pre-1.0 under Principle I's own change rule (semver +
migration notes); no higher principle is traded.

**Initial gate result**: PASS (one documented condition on FR-016). No Complexity Tracking entries.

## Project Structure

### Documentation (this feature)

```text
specs/002-qfj-parity-completion/
├── plan.md              # This file
├── spec.md              # Feature spec (12 US, 27 FR, Clarifications)
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (changed/new public-surface contracts)
│   ├── README.md
│   ├── config-mapping.md
│   ├── application-api.md          # typed callback outcomes (supersedes 001)
│   ├── typed-codegen.md            # all-messages codegen + MessageCracker
│   ├── group-parsing-validation.md
│   ├── validation-integrity.md     # integrity checks, two reject layers, reverse route
│   └── operational.md              # TLS/socket/failover, schedule, metrics, storage/log
└── checklists/
    └── requirements.md  # spec quality (from /speckit-specify, re-validated by /speckit-clarify)
```

### Source Code (repository root) — files touched/added per stage

No new crates. Changes land in the existing layered workspace:

```text
crates/
├── truefix-core/       # G2: dictionary-driven group/component decode; reverse-route header helper (G4)
│   └── src/codec/decode.rs, src/group.rs, src/message.rs
├── truefix-dict/       # G2: group/component validation toggles; G6: build.rs typed codegen (all msgs)
│   ├── build.rs, src/codegen/ (new), src/validate.rs, src/model.rs
├── truefix-session/    # G1: store-backed resend; G4: integrity checks + switches + timestamp precision;
│   │                   #   G5: typed callbacks plumbed; G8: schedule_reset (new)
│   └── src/state.rs, src/config.rs, src/schedule.rs, src/schedule_reset.rs (new), src/application.rs
├── truefix-store/      # G1: session store contract; G10: +Postgres/+MySQL; CachedFileStore cache+fsync
│   └── src/sql.rs, src/file.rs
├── truefix-log/        # G10: log output switches (heartbeat filter, ms, visibility); +SQL backends
│   └── src/*.rs
├── truefix-transport/  # G3 wiring point; G7: TLS-from-config + mTLS/SNI/min-ver + socket opts + failover;
│   │                   #   G9: metrics export
│   └── src/lib.rs, src/framing.rs
├── truefix-config/     # G3: settings→engine mapping (new builder/mapping module) + typed ConfigError
│   └── src/lib.rs, src/builder.rs (new), src/keys.rs (stance updates, FR-027)
├── truefix/            # G3: one-shot "start engine from .cfg"; G5: typed Application/MessageCracker re-export
│   └── src/lib.rs, src/application.rs, src/cracker.rs
└── truefix-at/         # G4: new integrity/group/reverse-route AT scenarios; G1/G3/G7 integration scenarios
    └── src/scenarios.rs, src/runner.rs
docs/
├── parity-matrix.md    # update stances (FR-027); MIGRATION note for the callback change (Principle I)
└── acceptance-record.md
```

**Structure Decision**: Reuse the 001 layered workspace unchanged; dependency direction stays strictly
downward. The only structural additions are a `truefix-config` mapping/builder module, a
`truefix-session` `schedule_reset` module, and a `truefix-dict` `codegen` module — each sits in its
existing layer, preserving independent testability (Principle V).

### Internal Delivery Stages (single release, dependency-ordered, each with a checkpoint)

> One-pass parity completion, **internally staged**. `/speckit-tasks` expands each stage into tasks;
> these are NOT separate releases. Priority tiers from the spec: G1–G3 = P1, G4–G6 = P2, G7–G10 = P3.

| Stage | Scope (US / FR) | Checkpoint (verifiable exit) |
|-------|-----------------|------------------------------|
| **G1** | Session-owned persistent resend (US2; FR-001/002/003/010 PersistMessages) | Restart a process against the same file store; a post-restart ResendRequest replays exact PossDup bodies; `PersistMessages=N` gap-fills (SC-002). |
| **G2** | Dictionary-driven group/component decode + group validation (US3; FR-004/005) | Correct group accepted; wrong-count / missing-delimiter / out-of-order / nested / zero-count rejected per toggle (SC-003). |
| **G3** | Settings→engine mapping + one-shot start + typed `ConfigError` (US1; FR-013/014/015) | Engine starts an acceptor **and** an initiator from one `.cfg`, completes logon→heartbeat; bad key fails fast with a typed error naming key+session (SC-001). |
| **G4** | Inbound integrity validation, two reject layers, RejectGarbledMessage, reverse routing, correctness switches, timestamp precision (US4/US11; FR-006/007/008/009/010/011/012) | New AT scenarios for bad-checksum/body-length/begin-string/CompID/sending-time/field-order/repeated-tag/reverse-route reach FIX-correct outcomes (SC-004/SC-005). |
| **G5** | Typed Application callback outcomes — breaking change (US5; FR-016) | Callbacks return `Reject`/`DoNotSend`/`BusinessReject`; engine refuses logon, suppresses send, emits 35=j with reason+ref tag; migration note recorded (SC-006). |
| **G6** | Typed codegen for **all** messages + MessageCracker (US6; FR-020/021/022) | Generated typed message/group/component structs round-trip byte-identically; cracker dispatches by MsgType/version; dual-track hash holds (SC-007). |
| **G7** | TLS-from-config + mTLS/SNI/min-version + full socket options + multi-endpoint failover (US7/US10; FR-017/019) | mTLS session from `.cfg` alone; un-credentialed client refused; initiator fails over to a backup endpoint; socket options applied (SC-008/SC-011). |
| **G8** | Schedule reset semantics + weekly StartDay/EndDay, session-layer (US8; FR-018) | Boundary crossing performs disconnect→reset→clear-store→reconnect; weekly windows correct across days; non-stop unaffected (SC-009). |
| **G9** | Observability metrics export via `metrics` facade (US9; FR-023) | State gauge, sender/target seq gauges, sent/received counters, reconnect counter reflect a logon-traffic-reconnect cycle (SC-010). |
| **G10** | Storage/logging completeness: SQL Postgres/MySQL/SQLite, CachedFileStore cache+fsync, log switches (US12; FR-024/025/026) + Appendix A stance updates (FR-027) | SQL suite green across the three DBs (gated on availability); cached store caches/flushes+survives restart; log switches change output; stances updated (SC-012/SC-014). |

**Cross-cutting**: FR-012 (AT proof) is satisfied incrementally inside G1–G4/G8; FR-027 (stance updates)
is applied as each key is implemented and finalised in G10; SC-013 (full gate green) is a per-stage exit
criterion for every G.

## Complexity Tracking

> No Constitution violations. The breadth (multi-DB, all-message codegen, integrity matrix) is inherent
> to the parity goal (Principle VII), managed by the staged structure and the AT/inventory gates rather
> than by deviating from any principle. The one breaking API change is explicitly sanctioned by Principle
> I's own change rule (semver + migration notes), recorded as a Constitution-Check condition above.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none) | — | — |
