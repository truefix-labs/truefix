# Implementation Plan: Close Remaining QuickFIX/J Parity Gaps (Post-002 Audit)

**Branch**: `003-qfj-full-parity-closure` | **Date**: 2026-07-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/003-qfj-full-parity-closure/spec.md`

## Summary

A 2026-07-01 code audit (`docs/todo-gap-analysis.md`), run after features 001 and 002, found 14
remaining QuickFIX/J-parity gaps across three tiers: P0 (AT scenario coverage still ~30%/2-of-6 versions;
session-owned resend still reads an in-memory map instead of the durable store 002 already wired at the
transport layer), P1 (field-order validation, a dozen inert session switches, dictionary components,
runtime dictionary loading, field-type completeness, four extra validation toggles, FIX Latest), and P2
(a session benchmark, network hardening, a dictionary CLI, inbound backpressure, MSSQL/Oracle). Per user
decision, this feature closes **all three tiers** plus two QuickFIX/J-only extras the audit marks
suitable (`ApplicationExtended`-style hooks, `RejectLogon`/`SessionStatus`). The work is **additive to
the existing layered cargo workspace** — no new crates, no breaking changes to existing public
signatures — and is delivered **in one pass but internally staged (G1a/G1b, G2–G14 — 15 stages total)**
with a verifiable checkpoint
per stage, mirroring 001's S0–S9 and 002's G1–G10 models. Phase 0 research (see `research.md`) grounded
every technical decision in the actual codebase, correcting one audit inaccuracy (sqlx has no MSSQL
driver; `tiberius` is the correct choice) and resolving five ambiguities the `/speckit-clarify` session
surfaced (PROXY-protocol trust boundary, proxy-type scope, benchmark non-gating status, admin/application
channel separation, deferred Oracle driver decision).

## Technical Context

**Language/Version**: Rust, edition 2021, MSRV per workspace pin (1.96). `#![forbid(unsafe_code)]`
retained workspace-wide; per-crate `deny(clippy::unwrap_used/expect_used/panic/indexing_slicing)` on
non-test code unchanged (Principle I).

**Primary Dependencies**:
- All existing workspace deps (tokio, tokio-rustls/rustls, rustls-pki-types, thiserror, rust_decimal,
  time, sqlx, tracing, metrics) — used more, not replaced.
- **New** (each gated behind a Phase 0 license-audit task before first dependent line of code, per
  Principle III; see `research.md` §"Summary of new dependencies"): `quick-xml` (FIX Orchestra parsing,
  build/tooling-time only), `ppp` (PROXY protocol v1/v2 parsing), `tokio-socks` (SOCKS4/5 client),
  `tiberius` (pure-Rust MSSQL driver — **not** sqlx, which has no MSSQL feature; research.md §8 corrects
  the audit's assumption), and tentatively `oracle` (deferred pending legal review, per spec
  Clarifications — may ship as a documented-interface-only stub instead of real code).
- HTTP CONNECT proxying is hand-rolled (no new dependency — a CONNECT handshake is a single request/
  response, not worth a full HTTP client crate).

**Storage**: `MessageStore` trait unchanged (7 async methods, already sufficient — research.md §5);
`Session` gains a `with_store` constructor that reads/writes through it directly instead of relying
solely on transport-layer seeding. `SqlStore`/`SqlLog` gain `mssql://`/`sqlserver://` URL-scheme branches
(via `tiberius`) alongside the existing postgres/mysql/sqlite branches; an `oracle://` branch is
implemented only if the Phase 0 license review clears it.

**Testing**: Table-driven unit tests (dictionary parsing/component-expansion/merge, field round-trips,
validation toggles), session/process-level integration tests (crash-restart resend, channel-saturation
backpressure, proxy/PROXY-protocol/TLS handshakes), and **AT suite expansion** in `truefix-at` — from
~40 entries covering FIX.4.2/4.4 to the full 73-scenario + 3-special-suite catalogue across all 8 legacy
versions (fixLatest as a 9th once its dictionary lands). SQL multi-backend tests gate on service
availability (extends the Postgres/MySQL CI-container precedent from feature 002 to MSSQL).

**Target Platform**: Linux/macOS servers (tokio). No platform-specific code added.

**Project Type**: Library/engine (existing cargo workspace) + example binaries + one new CLI binary
target (`truefix-dict`).

**Performance Goals**: Correctness-first; the new `benches/session.rs` benchmark is an observation/
regression-comparison tool only, explicitly **not** a CI-gating numeric SLO (per Clarifications).

**Constraints**: No panic/unwrap/expect/unreachable on codec/session/I-O/timer paths (Principle I).
`unsafe_code = forbid` remains workspace-wide for TrueFix's own crates (does not constrain what an
external dependency like `tiberius`/`oracle` does internally). No reference source/data copied
(Principle III) — FIX Latest content derives from FIX Orchestra XML, not vendored QuickFIX/J `.def`
files. Every addition in this feature is backward-compatible (new methods with no-op defaults, new
optional config fields defaulting to today's behavior, new constructors alongside existing ones) — no
semver-breaking change, unlike 002's deliberate callback-signature break.

**Scale/Scope**: 21 FRs across 14 user stories; AT catalogue grows from ~40 entries/2 versions to 73
scenarios + 3 special suites across up to 9 versions; ~14 previously-"recognised" config keys move to
"implemented" (Appendix A stance updates, FR-021).

**Reference-only architecture inputs**: `thrdpty/quickfix` (Go) and `thrdpty/quickfixj` (Java) read for
design/behaviour only; AT ported as black-box fixtures; no source/data-file copying (Principle III). FIX
Orchestra (FPL's own published specification) is the source for FIX Latest, not either reference
implementation's vendored copy.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. Production-Ready First | Every new/changed surface is additive (no breaking changes this time); new config/hooks/errors are typed; `SocketSynchronousWrites` timeout and `in_chan_capacity` backpressure are typed-error/blocking behaviors, not panics; the benchmark is explicitly non-gating so it can't destabilize CI. | ✅ PASS |
| II. Protocol Correctness First (NON-NEG) | AT coverage expansion (FR-001/002) is the single largest deliverable; field-order/extra-validation toggles (FR-006/007) and every other correctness behavior are gated by FR-005 (AT/session-test proof requirement). | ✅ PASS |
| III. License & Provenance (NON-NEG) | Five new dependencies identified (`quick-xml`, `ppp`, `tokio-socks`, `tiberius`, tentatively `oracle`); each MUST pass a license-compatibility check before first use (research.md's audit table) — **flagged as a Phase 0/task-start condition**, matching how 002 treated its own new dependencies. FIX Orchestra provenance (FPL's open spec, not a vendored reference-implementation copy) MUST also be confirmed at task start. | ✅ PASS (conditioned on Phase 0 dependency/provenance audit, same pattern as 002) |
| IV. Dual-Track Data Dictionary | Components (FR-009) and FIX Latest (FR-012) extend — not fork — the single normalized-dictionary source; codegen and runtime dictionary continue sharing one source across all versions including the 10th (dual-track hash extended, not replaced). | ✅ PASS |
| V. Test Discipline | Table-driven unit tests for every new toggle/type; session/process-level integration tests for resend, backpressure, and network hardening; AT suite is the largest single test-discipline deliverable in this feature. | ✅ PASS |
| VI. Acceptor/Initiator Parity | AT scenarios exercise the acceptor side (as today); durable resend, extended hooks, and network hardening are designed for both initiator and acceptor roles (proxy/SOCKS is initiator-side by nature — PROXY protocol is acceptor-side by nature; both sides of the parity requirement are covered, each by the role that actually needs it in QuickFIX/J terms). | ✅ PASS |
| VII. Inventory-Based Completeness | Every audit TODO (01–14) traces to a stage below; config keys moved from recognised→implemented update their Appendix A stance (FR-021); no gap is claimed closed without an AT/session-test/unit-test citation. | ✅ PASS |

**Conflict-order reminder**: License → Protocol correctness → Production readiness → feature count. No
principle is traded off in this feature; the Oracle driver's deferred-if-necessary framing (spec
Clarifications) is itself an application of Principle III's non-negotiable status — if legal review
fails, the spec already permits scoping Oracle down rather than compromising license cleanliness.

**Initial gate result**: PASS (one documented condition: Phase 0 dependency/provenance audit for the 5
new crates + FIX Orchestra source, mirroring 002's precedent). No Complexity Tracking entries.

## Project Structure

### Documentation (this feature)

```text
specs/003-qfj-full-parity-closure/
├── plan.md              # This file
├── spec.md              # Feature spec (14 US, 21 FR, 5-question Clarifications session)
├── research.md          # Phase 0 output — 11 grounded decisions + dependency license-audit table
├── data-model.md        # Phase 1 output — concrete shapes for every spec Key Entity
├── quickstart.md         # Phase 1 output — per-user-story validation commands
├── contracts/           # Phase 1 output (6 theme files + index)
│   ├── README.md
│   ├── correctness-foundation.md   # US1, US2
│   ├── validation-completeness.md  # US3, US4, US8
│   ├── dictionary-model.md         # US5, US6, US7, US9
│   ├── application-hooks.md        # US10
│   ├── network-hardening.md        # US12
│   └── tooling-and-ops.md          # US11, US13, US14
└── checklists/
    └── requirements.md  # spec quality (from /speckit-specify, re-validated by /speckit-clarify)
```

### Source Code (repository root) — files touched/added per stage

No new crates except one new CLI binary target inside the existing `truefix-dict` crate. Changes land in
the existing layered workspace, dependency direction unchanged (strictly downward):

```text
crates/
├── truefix-core/        # G7: Data/UtcDateOnly/UtcTimeOnly Field constructors/accessors
│   └── src/field.rs
├── truefix-dict/        # G5: component directive+ComponentDef+expansion; G6: load_from_file/extend;
│   │                    # G8: FIX Latest normalized source + load_fixlatest + Orchestra conversion tool;
│   │                    # G13: new `bin/truefix-dict` CLI wrapping existing parse/codegen/build.rs logic
│   ├── src/parser.rs, src/model.rs, src/lib.rs, src/error.rs (new), src/bin/truefix-dict.rs (new), build.rs
│   └── dict-src/normalized/FIXLATEST.fixdict (new, generated by the Orchestra conversion tool)
├── truefix-session/      # G2: Session::with_store + store-backed build_resend/reset; G3: field-order +
│   │                     # extra validation wiring; G4: 12 config switches; G9: extended app hooks
│   │                     # (on_before_reset, Reject.session_status)
│   └── src/state.rs, src/config.rs, src/application.rs
├── truefix-store/        # G14: mssql://(tiberius)/oracle:// URL-scheme branches on SqlStore
│   └── src/sql.rs, Cargo.toml (mssql feature)
├── truefix-log/          # G14: matching MSSQL/Oracle branches on SqlLog
│   └── src/sql.rs
├── truefix-transport/     # G12: PROXY-protocol (trusted-upstream) + SOCKS4/5 + HTTP CONNECT +
│   │                      # inline-PEM + cipher-suites + sync-writes; G14: admin/application dual-channel
│   │                      # split + in_chan_capacity backpressure
│   └── src/lib.rs, src/tls_config.rs, src/framing.rs, src/proxy.rs (new)
├── truefix-config/        # G3/G4/G12/G14: new recognised→implemented keys (stance updates, FR-021)
│   └── src/keys.rs, src/builder.rs
└── truefix-at/            # G1: AT scenario expansion to 73+3 across all versions (split across an
    │                       # early stage and a closeout stage, per research.md §11)
    └── src/scenarios.rs, src/runner.rs
benches/
└── session.rs            # G10: new — round-trip latency benchmark (observation-only)
docs/
├── parity-matrix.md       # stance updates (FR-021)
└── todo-gap-analysis.md   # TODO items checked off as each stage closes
```

**Structure Decision**: Reuse the existing layered workspace unchanged; no new crates, one new binary
target inside `truefix-dict`. The only structurally-new module is `truefix-transport::proxy` (PROXY
protocol + SOCKS/HTTP-CONNECT client logic), which sits in its existing layer — dependency direction and
independent testability (Principle V) are preserved.

### Internal Delivery Stages (single release, dependency-ordered, each with a checkpoint)

> One-pass gap closure, **internally staged**. `/speckit-tasks` expands each stage into tasks; these are
> NOT separate releases. Priority tiers from the spec: G2 = P0/P1 foundation, G3–G9 = P1/P2 completeness,
> G10–G14 = P2 tooling/ops. G1 (AT coverage) is split: an early slice (G1a) lands after G2/G3, a closeout
> slice (G1b) after G8 and G9 both complete, so AT proof accrues incrementally rather than gating on the
> very last stage.

| Stage | Scope (US / FR) | Checkpoint (verifiable exit) |
|-------|-----------------|------------------------------|
| **G2** | Session-owned durable resend (US2; FR-003/004) | Kill and restart the process against the same store; a post-restart ResendRequest replays exact PossDup bodies; `reset()` clears store state consistently (SC-002). |
| **G3** | Field-order validation + extra validation toggles (US3, US8; FR-006/007) | `validate_fields_out_of_order`/`validate_checksum`/`validate_incoming_message`/`allow_pos_dup`/`requires_orig_sending_time` each independently gate their message class; AT scenarios `14g`/`15`/`2t` pass (SC-003). |
| **G1a** | AT coverage, early slice (US1 partial; FR-001/002) — scenarios needing only existing behavior, wired across fix40/41/43/50/50SP1/50SP2 | New scenarios (integrity/CompID/sequence/PossDup/reject-layer classes) pass across all non-fixLatest versions. |
| **G4** | Remaining 12 session config switches (US4; FR-008) | Each switch produces its documented behavioral change in a dedicated test (SC-004). |
| **G5** | Dictionary `component` model (US5; FR-009) | A component referenced by two messages validates identically to a hand-inlined equivalent (SC-005). |
| **G6** | Runtime dictionary loading + extend (US6; FR-010) | A custom dictionary loaded via `load_from_file`/`extend` validates a message using its custom fields, no rebuild (SC-006). |
| **G7** | Field-type completeness: Data/UtcDateOnly/UtcTimeOnly (US7; FR-011) | Each type round-trips byte-identically through its constructor/accessor pair (SC-007). |
| **G8** | FIX Latest support (US9; FR-012) — depends on G5 (components) | A FIX-Latest-configured session completes logon-to-heartbeat; typed codegen + `crack_fixlatest` generated; dual-track hash holds across 10 versions (SC-008). |
| **G9** | Extended application hooks: logon predicate documentation, `on_before_reset`, `Reject.session_status` (US10; FR-013) | A logon predicate refuses with a chosen `SessionStatus` on the outbound Logout/Reject; `on_before_reset` fires before every reset (SC-009). |
| **G1b** | AT coverage, closeout slice (US1 complete; FR-001/002/005) — fixLatest-targeted + field-order-dependent scenarios, full 73+3 matrix across all 9 versions | 100% of the 73 server scenarios + 3 special-category suites pass across every targeted version, including fixLatest (SC-001). This is the feature's release-gate checkpoint. |
| **G10** | Session round-trip latency benchmark (US11; FR-014) | `cargo bench` reports a stable, reproducible distribution; confirmed non-gating (no threshold enforced) (SC-010). |
| **G11** | Network hardening: PROXY protocol (trusted-upstream), SOCKS4/5+HTTP CONNECT, inline PEM, cipher suites, sync writes (US12; FR-015/016/017) | PROXY header trusted only from configured upstreams; proxy-type connections succeed; inline-PEM TLS establishes; restricted cipher suite enforced; stalled sync write times out (SC-011). |
| **G12** | `truefix-dict` CLI (US13; FR-018) | CLI generates a `.fixdict` from an Orchestra/FPL source, generates matching typed code, and validates a dictionary with its dual-track hash (SC-012). |
| **G13** | Inbound backpressure: admin/application dual-channel split + `in_chan_capacity` (US14 part 1; FR-019) | A saturated bounded application channel blocks the reader without dropping messages; concurrent admin traffic is unaffected; `None` (default) behaves as today (SC-013). |
| **G14** | Additional SQL backends: MSSQL via `tiberius`, Oracle per Phase 0 legal review (US14 part 2; FR-020/021) | SQL store/log suite passes equivalently against MSSQL wherever available (gated on service availability); Oracle either passes equivalently or is documented as deferred, per the spec's explicit allowance (SC-014). |

**Cross-cutting**: FR-005 (AT/session-test proof) is satisfied incrementally inside G2–G9, finalized at
G1b; FR-021 (Appendix A stance updates) is applied as each key is implemented, finalized at G14; SC-015
(full workspace gate green) is a per-stage exit criterion for every G; the Phase 0 dependency/provenance
audit (5 new crates + FIX Orchestra source) is a prerequisite check performed at the start of `/speckit-
tasks`, before any G that depends on a new crate begins (G8/G11/G12/G14).

## Complexity Tracking

> No Constitution violations. The breadth (14 audit TODOs, 5 new dependencies, one new CLI binary) is
> inherent to the parity-closure goal (Principle VII), managed by the staged structure and the AT/
> inventory gates rather than by deviating from any principle. Unlike feature 002, this feature
> introduces **no breaking API change** — every addition is backward-compatible, so there is no semver/
> migration-note condition to track here.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none) | — | — |
