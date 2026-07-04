# Implementation Plan: Third-Pass Audit Remediation

**Branch**: `008-third-audit-remediation` | **Date**: 2026-07-04 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/008-third-audit-remediation/spec.md`

## Summary

`docs/todo/005.md` is a third-pass audit covering defects **not** already tracked in
`docs/todo/003.md` (closed by feature 006) or `docs/todo/004.md` (in progress on feature 007): four
successive independent full-codebase reviews of all 9 crates, each cross-referenced against
QuickFIX/J/Go, with each later pass re-verifying (and refuting, downgrading, or folding together)
findings from the passes before it. This plan's own Phase 0 research (below) independently re-read
current source for every User Story 1 item (all 21) — every one confirmed exactly as `005.md`'s own
final, self-corrected framing describes; no further correction was needed. The work spans 3
priority-ordered user stories / 83 tracked findings (`FR-001`-`FR-083`): 21 protocol-correctness/
data-loss/security defects (a total-functional-failure Mongo persistence bug, an unauthenticated
pre-Logon DoS vector, several protocol-handshake and FIXT 1.1 gaps), 30 narrower-blast-radius
defects, and 32 hardening/dictionary-tooling/test-harness items. Four clarifications were resolved
via `/speckit-clarify` rather than assumed: all three priority tiers are addressed in this one pass
(matching feature 007's precedent); the TLS default-trust-store fix may add one narrowly-scoped new
dependency (`rustls-native-certs`); the Mongo sequence-store fix is forward-only (no migration of
pre-existing corrupted rows); and the pre-Logon DoS timeout reuses the existing `logon_timeout`
setting rather than introducing a new config key.

## Technical Context

**Language/Version**: Rust, edition 2021, MSRV per workspace pin (`rust-toolchain.toml`, currently
1.96.0). `unsafe_code = "forbid"` (workspace-wide) and per-crate
`#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic,
clippy::indexing_slicing))]` remain unchanged (Principle I) — every fix touches code already under
that discipline (`truefix-session::state`, `truefix-transport::{lib,proxy}`, `truefix-store::mongo`,
`truefix-dict::{codegen,model,validate}`, `truefix-core::tags`, `truefix-config::builder`,
`truefix-log::mssql`, `truefix-transport::tls_config`).

**Primary Dependencies**: One new dependency: `rustls-native-certs` (resolved via Clarifications,
Session 2026-07-04), added specifically to close `NEW-11`/`FR-049`'s TLS default-trust-store gap.
License-compatible (MIT/Apache-2.0 dual license, matching TrueFix's own — Principle III). Every
other FR is implemented against already-present workspace dependencies (`tokio`, `mongodb`,
`tiberius`, `bson` (via `mongodb`), `rustls`/`tokio-rustls`, `ppp`, `thiserror`, `time`, `tracing`).

**Storage**: `MongoStore` (`crates/truefix-store/src/mongo.rs`) receives its two P0/P1 fixes in this
feature: `set_seq`'s literal-key bug (`FR-001`, forward-fix only per Clarifications — no migration
of existing corrupted rows) and a new transactional `save_and_advance_sender` override (`FR-002`),
plus a uniqueness fix for `ensure_session_row` (`FR-003`). `MssqlStore`/`MssqlLog` both gain an
opt-in TLS-certificate-validation config surface (`FR-010`). No other backend's on-disk/wire format
changes.

**Testing**: Table-driven unit tests for every codec/dictionary/config-key fix; two-process
integration tests for every session-state-machine change (`NEW-03`'s acceptor `ResetOnLogon`,
`NEW-56`'s reason-aware teardown, `NEW-62`'s schedule-exit Logout, `NEW-63`'s dictionary-invalid-Logon
routing, `NEW-84`'s gap-fill verification, and `NEW-93`'s pre-Logon DoS timeout all require live
acceptor/initiator pairs, not just unit-level state-machine tests, since they are inherently about
*inter-process* protocol behavior or network-level resource bounds). Per Constitution Principle II,
genuine protocol-behavior corrections among these MUST also land as AT scenarios where a suitable
scenario category exists; `/speckit-tasks` determines this per-FR (matching 006/007's precedent
rather than pre-committing here). **AT suite regression floor**: this feature's own baseline is
whatever feature 007 leaves passing when 008 branches from it — this feature MUST NOT decrease that
count, and any new AT scenario (e.g. for `NEW-83`'s fixed-identity acceptor mode, itself one of this
feature's own FRs, `FR-082`) bumps it upward.

**Target Platform**: Linux/macOS servers (tokio). No platform-specific code added.

**Project Type**: Library/engine (existing cargo workspace: `truefix`, `truefix-at`,
`truefix-config`, `truefix-core`, `truefix-dict`, `truefix-log`, `truefix-session`, `truefix-store`,
`truefix-transport`). No new crates, no new binary targets.

**Performance Goals**: Correctness/completeness first, per Constitution Principle II's conflict
order. No specific new performance target — `FR-007`'s bounded log-writer channels and `FR-044`'s
capped pre-Logon read buffer are reliability/resource-bound fixes, not throughput/latency targets.

**Constraints**: No panic/unwrap/expect/unreachable on codec/session/I-O/timer paths (Principle I),
unchanged. `unsafe_code = forbid` remains workspace-wide. No reference source read during this
planning pass beyond `docs/todo/005.md`'s own prose citations of QuickFIX/J/Go class/method names
and documented behavior (Principle III's established boundary, same as 006/007). This feature
discloses one new dependency (`rustls-native-certs`, `FR-049`) and one new internal (non-public)
enum shape for disconnect-reason threading (`FR-014`) — see research.md's "Summary of decisions
requiring disclosure" for full rationale. No new public API surface (no new `pub` types/traits/
breaking signatures).

**Scale/Scope**: 3 user stories, 83 functional requirements (`FR-001`-`FR-083`), touching ~20 source
files across 8 crates (`truefix-session`, `truefix-transport`, `truefix-store`, `truefix-core`,
`truefix-dict`, `truefix-config`, `truefix-log`, `truefix-at`), staged US1→US2→US3 matching the
spec's own priority ordering.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design (below).*

| Principle | Check | Status |
|---|---|---|
| I. 生产就绪优先 (Production-Ready First) | Every User Story 1 item replaces a silent-data-loss (`NEW-54`), protocol-desync (`NEW-02/03/84`), or unauthenticated-resource-exhaustion (`NEW-93`) path with a typed error, an explicit rejection, or a bounded/timed-out resource. `unwrap`/`expect`/`panic`/indexing-slicing denies stay in force — research.md's per-item reads confirm every touched function already returns `Result`/`Vec<Action>`; no new panic surface is introduced. | **PASS** |
| II. 协议正确性优先 (Protocol Correctness First) — NON-NEGOTIABLE | `NEW-02` (`BeginSeqNo=0`), `NEW-03` (acceptor `ResetOnLogon`), `NEW-56` (teardown reset-flag scoping), `NEW-58`/`NEW-55` (FIXT 1.1 header/group-decode), `NEW-62` (schedule-exit Logout), `NEW-63` (dictionary-invalid Logon routing), `NEW-84` (gap-fill verification), and `NEW-06` (FIX 5.x sub-version dispatch) are all genuine session-protocol-behavior corrections grounded in QuickFIX/J/Go's documented behavior (research.md §R1.2-R1.13) — each is a Test Discipline candidate for a new AT scenario, finalized per-item at `/speckit-tasks` time. | **PASS** |
| III. License 与来源纪律 (License & Provenance) — NON-NEGOTIABLE | Every decision in research.md is derived from `docs/todo/005.md`'s prose description of the gap plus TrueFix's own current source (this plan's Phase 0 re-reads) — QuickFIX/J/Go citations throughout are class/method **names and documented behavior** only. The one new dependency (`rustls-native-certs`) is MIT/Apache-2.0 dual-licensed, compatible with TrueFix's own release license and the dependency-discipline constraint in "技术与实现约束". | **PASS** |
| IV. 数据字典双轨 (Dual-Track Data Dictionary) | `NEW-55`/`NEW-58` (FIXT header tags + group-aware decode) and `NEW-06` (FIX 5.x `crack_*` dispatch) touch both the runtime `DataDictionary` path (`tags::is_header`, `classify_buffered`) and codegen (`crack_*` generation) — both tracks are updated together, no divergence introduced. `NEW-05`/`NEW-60`/`NEW-61`/`NEW-78`/`NEW-79`/`NEW-82` (User Story 3's dictionary-tooling items) are codegen/converter-only robustness fixes that make codegen's behavior match what the runtime path already correctly does — not a new dual-track divergence. | **PASS** |
| V. 测试纪律 (Test Discipline) | Table-driven + integration + (where applicable) AT-port discipline designed into every User Story's Independent Test (spec.md) and this plan's Testing section. `NEW-93`'s pre-Logon DoS fix and `NEW-84`'s gap-fill verification fix each additionally require a dedicated network-level/two-process test (not just state-machine unit tests), called out explicitly in research.md so it isn't missed at `/speckit-tasks` time. User Story 3's AT-harness items (`FR-076`-`FR-082`) directly improve this same test-discipline infrastructure. | **PASS** |
| VI. Acceptor/Initiator 对等 (Parity) | `NEW-03` (acceptor `ResetOnLogon`) and `NEW-93` (multi-session acceptor pre-Logon DoS) are acceptor-only by construction (only acceptors have this local-reset-on-Logon gap / only the multi-session acceptor path routes by Logon content before a `Session` exists) — correctly asymmetric per prior features' precedent for acceptor-specific gaps. Every other User Story 1/2 protocol item (`NEW-02`, `NEW-56`, `NEW-62`, `NEW-63`, `NEW-84`, `NEW-85`, `NEW-86`) applies symmetrically and must be tested on both roles. | **PASS** |
| VII. 基于清点的完整性 (Inventory-Based Completeness) | This entire feature is the inventory-based remediation of `docs/todo/005.md`'s own four-pass-verified findings — every FR traces to a specific `NEW-NN` citation with a verified file:line source (research.md). The spec's own Assumptions section discloses every item `005.md` raised that this feature does **not** close (refuted findings, the Parity-Note item, the missing-features table, the `BUG-92` cross-reference), with a stated reason each — no silent scope-narrowing. | **PASS** |

**Result**: No violations. No Complexity Tracking entries required (table intentionally empty).

## Project Structure

### Documentation (this feature)

```text
specs/008-third-audit-remediation/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── README.md
│   ├── session-protocol.md      # US1/US2 — ResetOnLogon, gap-fill verification, teardown
│   │                             # reset-reason scoping, schedule-exit Logout, Logon rejection
│   │                             # routing, Logout/ResendRequest too-high handling
│   ├── fixt-and-dictionary.md   # US1/US2/US3 — FIXT 1.1 header/group-decode, ApplVerID dispatch,
│   │                             # validation-key wiring, dictionary/codegen correctness
│   ├── store-and-transport.md   # US1/US2 — Mongo persistence/atomicity, MSSQL TLS opt-out,
│   │                             # pre-Logon DoS, PROXY header handling, TLS default trust store
│   └── hardening-and-tooling.md # US3 — hygiene, dictionary-tooling, AT harness coverage
├── checklists/
│   └── requirements.md
└── tasks.md              # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
# Existing cargo workspace (001-007) — no new crates, no new binary targets.
crates/
├── truefix-core/           # tags::is_header (US1), decode MAX_BODY_LEN consistency, encode
│                            # duplicate-member/field-map fixes (US2/US3)
├── truefix-session/         # session state machine — US1 (BeginSeqNo=0, acceptor ResetOnLogon,
│                             # gap-fill verification, teardown reset-reason scoping, schedule-exit
│                             # Logout, dictionary-invalid Logon routing), US2 (pre-logon admin-msg
│                             # resend, NewSeqNo=0/missing handling, Logout/ResendRequest too-high
│                             # handling), US3 (DefaultApplVerID validation, Reject-before-Logout)
├── truefix-store/           # MessageStore backends — US1 (MongoStore set_seq fix, transactional
│                             # save_and_advance_sender), US2 (Mongo unique index, BodyLog::reset
│                             # lock ordering, creation-time error handling), US3 (CachedFileStore
│                             # read-through)
├── truefix-transport/       # session I/O — US1 (multi-session acceptor pre-Logon DoS timeout/cap,
│                             # classify_buffered fixt_dictionaries arm, PROXY header buffer/timeout,
│                             # scheduled-initiator connect-blocking, TLS default trust store), US2
│                             # (cipher-suite typo detection, DNS resolution timing, SO_REUSEADDR,
│                             # frame-resync, MSSQL TLS opt-out plumbing, PROXY Unknown/Unspecified
│                             # header consumption, LogMessageWhenSessionNotFound wiring), US3
│                             # (Monitor cleanup, HTTP CONNECT status parsing, log-writer shutdown
│                             # flush, HashMap iteration determinism)
├── truefix-dict/            # US1 (MultipleCharValue allows(), UDF validation ordering, FIX 5.x
│                             # crack_* ApplVerID dispatch, codegen BeginString stamping, 7 validation
│                             # config keys' ValidationOptions fields), US2 (header-group validation,
│                             # MonthYear/LocalMktDate format, tag-0 rejection, group-count fidelity,
│                             # DayOfMonth/Length/SeqNum/NumInGroup range checks, cyclic-group guard,
│                             # empty-group-value rejection, application_for empty-ApplVerID
│                             # fallback), US3 (fix-repository/orchestra converter correctness,
│                             # parser.rs duplicate detection, codegen dedup/rerun-if-changed)
├── truefix-config/          # US1 (7 validation-key wiring), US2 (LogMessageWhenSessionNotFound,
│                             # DNS-resolution timing), US3 (config-var circular-reference detection,
│                             # UnresolvedVariable line numbers, reconnect_interval/logout_timeout
│                             # bare-struct defaults)
├── truefix-log/             # US1 (MssqlLog::parse_url semicolon-form + percent-decode), US2 (MSSQL
│                             # TLS opt-out, unbounded async-channel bounding), US3 (log-writer
│                             # JoinHandle/flush-on-shutdown)
└── truefix-at/              # US3 only — dictionary validators for all 9 versions, Logout-reason
                              # assertions, buffer-empty-at-end assertion, check_match strictness,
                              # MsgSeqNum ordering assertions, per-scenario conformance reporting,
                              # fixed-identity acceptor mode

# Existing top-level docs (unaffected structurally)
docs/todo/005.md            # Source audit for this feature (read-only input)
```

**Structure Decision**: No new crates or directories. This feature is a remediation pass entirely
within the existing layered workspace established by 001-007. Every fix lands in the crate that
already owns the affected behavior; no fix requires introducing a new cross-crate dependency edge.
`truefix` (the engine facade crate) is touched narrowly, if at all — `005.md` raised no findings
directly against `truefix::lib` beyond what's already covered by `truefix-transport`'s log-writer
shutdown-flush item (`NEW-91`/`FR-008`), which may require `Engine::shutdown`/`Drop` to await new
log-backend drain handles. `truefix-dict` is touched across both its runtime-validation modules
(`model.rs`, `validate.rs`) and its codegen/tooling modules (`codegen.rs`, `fix_repository.rs`,
`orchestra.rs`, `parser.rs`) — the runtime-vs-codegen distinction within this one crate is preserved
exactly as Constitution Principle IV requires (dual-track, not divergent).

## Complexity Tracking

*No entries — Constitution Check above found no violations requiring justification.*
