# Implementation Plan: Third/Fourth-Pass Audit Remediation

**Branch**: `009-audit-005-remediation` | **Date**: 2026-07-05 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/009-audit-005-remediation/spec.md`

## Summary

`docs/todo/005.md` is a four-pass audit of all 9 crates, each pass cross-referencing QuickFIX/J/Go
and re-verifying (refuting 4, downgrading 3, folding 1 into another) the passes before it. This plan
covers the 90 in-scope items (`NEW-02` through `NEW-96`, excluding 4 refuted, 1 parity-note, and 6
feature-gap rows) across 3 priority-ordered user stories / 83 functional requirements: 21
protocol-correctness/data-loss/security defects (US1 — a Mongo sequence-persistence total failure,
an unauthenticated multi-session-acceptor pre-Logon DoS, `BeginSeqNo=0`, acceptor `ResetOnLogon`,
gap-fill `SequenceReset` verification bypass, FIXT 1.1 header/decode gaps, teardown reset-reason
conflation, TLS default trust store, and more), 30 narrower-blast-radius defects (US2), and 32
hardening/dictionary-tooling/AT-harness-coverage items (US3, including — per `/speckit-clarify` —
real fixes rather than tracked-only notes for 6 allocation/complexity-hygiene items). Four
clarifications were resolved: downgrade (not implement) the 2 validation keys with no
`ValidationOptions` field; implement (not merely track) the perf-hygiene items; add a new
fixed-identity AT acceptor mode additively (not replace the dynamic-template mode); and change
`reconnect_interval`'s bare struct default to 30 (a real code change, not documentation-only). This
plan's own Phase 0 research live-re-verified a risk-weighted sample of the highest-severity items
directly against current source (zero drift found from the audit document's own fourth-pass claims)
and resolved the one open technical unknown: `NEW-11`'s TLS fix adds `rustls-native-certs` as this
feature's sole new dependency.

## Technical Context

**Language/Version**: Rust, edition 2024, MSRV per workspace pin (`rust-toolchain.toml`, 1.96.0 —
bumped from edition 2021 in the commit immediately preceding this feature). `unsafe_code = "forbid"`
(workspace-wide) and per-crate `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used,
clippy::panic, clippy::indexing_slicing))]` remain unchanged (Principle I) — every fix touches code
already under that discipline (`truefix-session::state`, `truefix-transport::lib`,
`truefix-store::{mongo,file,mssql}`, `truefix-core::{tags,codec,framing}`, `truefix-dict::{model,
validate,parser,codegen,fix_repository,orchestra,fixt}`, `truefix-config::{keys,builder}`,
`truefix-log::{sql,mssql,mongo,redb}`, `truefix-at::{runner,scenarios}`, `truefix::lib`).

**Primary Dependencies**: One new dependency — `rustls-native-certs` (Apache-2.0 OR MIT, same
maintainer org as the already-present `rustls`), added to `truefix-transport` for `NEW-11`'s default
OS-native TLS trust-store fallback (research.md §R2). No other new external crates; every other FR
is implemented against already-present workspace dependencies (`tokio`, `mongodb` — session/
transaction API for `NEW-08`/`NEW-54`/`NEW-75` — `tiberius`, `thiserror`, `time`, `tracing`,
`ppp`, `quick-xml`).

**Storage**: `MongoStore` (`crates/truefix-store/src/mongo.rs`) gets its `set_seq` key-name bug
fixed (`NEW-54`), a transactional `save_and_advance_sender` override (`NEW-08`), and a unique
index/upsert on `sessions.session_id` (`NEW-75`) — all three gated behind the existing `mongodb`
Cargo feature. `FileStore`'s `BodyLog` gets a lock-ordering fix (`NEW-76`) and a single-handle-reuse
optimization for large resends (`NEW-41`); `RedbStore::reset()` avoids an intermediate key
collection (`NEW-42`). No other backend (SQL/Memory) is touched — `005.md` raised no findings
against them.

**Testing**: Table-driven unit tests for every codec/dictionary/config-resolution fix (the large
majority of this feature's 90 items). Two-process integration tests for session-state-machine and
transport-level changes requiring live acceptor/initiator pairs (gap-fill verification, teardown
reason routing, the pre-Logon DoS timeout, TLS handshake fallback, duplicate-connection-adjacent
items). Per Constitution Principle II, genuine protocol-behavior corrections additionally require an
AT scenario — `contracts/README.md`'s AT-impact table identifies roughly 20 items expected to
raise the current **424-scenario-run floor** (research.md §R3); the rest are session/store/
transport/dictionary/harness-*internal* fixes with no new AT scenario expected, per the same
per-item triage discipline features 004-007 established. Mongo/MSSQL-gated tests run under
`--features mongodb`/`--features mssql` (existing pattern).

**Target Platform**: Linux/macOS servers (tokio). No platform-specific code added. Windows/macOS/
Linux OS-native trust store lookup (`rustls-native-certs`) works identically across all three at the
API level; only manual verification on each OS is out of scope for automated CI (existing repo
practice — network/OS-integration edges are integration-tested on Linux CI, per prior features).

**Project Type**: Library/engine (existing cargo workspace: `truefix`, `truefix-at`,
`truefix-config`, `truefix-core`, `truefix-dict`, `truefix-log`, `truefix-session`, `truefix-store`,
`truefix-transport`). No new crates, no new binary targets.

**Performance Goals**: Correctness/completeness first, per Constitution Principle II's conflict
order — except User Story 3's allocation/complexity-hygiene items (`FR-083`, `NEW-35`-`38`), which
*are* performance-motivated per `/speckit-clarify`'s decision to implement them in this feature; no
specific new latency/throughput target is set for them beyond "reduce allocations/complexity with
unchanged behavior" (spec SC criteria don't set a numeric perf target — none was requested).

**Constraints**: No panic/unwrap/expect/unreachable on codec/session/I-O/timer paths (Principle I),
unchanged. `unsafe_code = forbid` remains workspace-wide. No reference source read during this
planning pass beyond `docs/todo/005.md`'s own prose citations of QuickFIX/J/Go class/method names
and documented behavior, plus this plan's own direct re-reads of **TrueFix's own current source**
(Principle III's established boundary — QuickFIX/J/Go are read only for documented behavior, never
copied). This feature discloses several additive-only public-API surface growths (full list in
research.md's "Summary of decisions requiring disclosure" and each contract file): a new internal
`DisconnectReason` enum (private), a new `TlsConfigError` variant (public, additive), a
`ResolvedSession.log_message_when_session_not_found` field (additive), `SessionConfig`'s
`reconnect_interval`/builder.rs's `LogoutTimeout` default-*value* changes (the latter changing real
`.cfg`-driven behavior — flagged prominently), a new AT harness fixed-identity acceptor mode
(additive), and each async log backend gaining a `shutdown`/flush method (additive). No existing
public signature narrows, removes, or changes its accepted-input semantics.

**Scale/Scope**: 3 user stories, 90 in-scope items / 83 functional requirements (`FR-001`-`FR-083`),
touching all 9 crates (the broadest crate footprint of any audit-remediation feature so far, since
`005.md` itself is a full 9-crate sweep), staged US1→US2→US3 matching the spec's own priority
ordering (mirroring 007's precedent of doing all three tiers in one pass).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design (below).*

| Principle | Check | Status |
|---|---|---|
| I. 生产就绪优先 (Production-Ready First) | Every User Story 1 item replaces a silent-data-loss (`NEW-54`), unauthenticated-DoS (`NEW-93`), or security-control-defeated (`NEW-11`, `NEW-90`) path with a typed error, a bounded resource, or a working control. `unwrap`/`expect`/`panic`/indexing-slicing denies stay in force — research.md's live-verified sample confirms every touched function already returns `Result`/`Vec<Action>`; the new `rustls-native-certs` fallback and Mongo transaction path both use the same `Result`-returning error patterns already established (`TlsConfigError`, `StoreError`). | **PASS** |
| II. 协议正确性优先 (Protocol Correctness First) — NON-NEGOTIABLE | `NEW-02`, `NEW-03`, `NEW-84`, `NEW-55`, `NEW-58`, `NEW-56`, `NEW-62`, `NEW-63`, `NEW-85`, `NEW-86`, `NEW-87`, `NEW-88`, and the US2 session/dictionary items are all genuine session-protocol-behavior corrections grounded in QuickFIX/J/Go's documented behavior (research.md §R1; each contract file's per-item AT-requirement notes). `contracts/README.md`'s AT-impact table is the Test Discipline candidate list, finalized per-item at `/speckit-tasks`. | **PASS** |
| III. License 与来源纪律 (License & Provenance) — NON-NEGOTIABLE | Every decision in research.md is derived from `docs/todo/005.md`'s prose description of the gap plus TrueFix's own current source (this plan's Phase 0 re-reads, R1). QuickFIX/J/Go citations throughout are class/method **names and documented behavior** only. The one new dependency, `rustls-native-certs`, is Apache-2.0 OR MIT-licensed (verified, research.md §R2) — compatible, no copyleft exposure. | **PASS** |
| IV. 数据字典双轨 (Dual-Track Data Dictionary) | `NEW-05`, `NEW-60`, `NEW-61`, `NEW-78`, `NEW-79`, `NEW-82` (codegen/converter tooling fixes) make codegen's output *match* what the runtime `DataDictionary` path already correctly does (e.g. `NEW-05`'s delimiter/member-exclusion bug — the shipped, hand-normalized `.fixdict` files are already correct; only the *converter* diverges) — closing existing dual-track divergences, not introducing new ones. `NEW-43`/`NEW-44` touch `parser.rs`, the runtime loader shared by both tracks (per the source document's own classification note) — fixed once, benefiting both tracks identically. | **PASS** |
| V. 测试纪律 (Test Discipline) | Table-driven + integration + (where applicable) AT-port discipline designed into every User Story's Independent Test (spec.md) and this plan's Testing section / each `contracts/*.md` file's per-item test-type column. `NEW-65`'s cyclic-group recursion guard specifically exercises Principle I's "never panics" requirement with a crafted-dictionary fixture, called out explicitly in `contracts/fixt-and-dictionary.md` so it isn't missed at `/speckit-tasks` time. | **PASS** |
| VI. Acceptor/Initiator 对等 (Parity) | `NEW-03` (acceptor `ResetOnLogon`), `NEW-62` (acceptor schedule-exit Logout), `NEW-93` (multi-session acceptor DoS), `NEW-71` (acceptor `HeartBtInt`), and `NEW-33` (`AcceptorBuilder::bind`) are acceptor-only by construction (the described gap only exists on the acceptor side — matching 005/006/007's own precedent for genuinely asymmetric gaps, e.g. only acceptors route inbound connections by SessionID or enforce a schedule against an already-connected peer). Every other session-protocol item (`NEW-02`, `NEW-56`, `NEW-84`, `NEW-85`, `NEW-86`, `NEW-87`, `NEW-88`, etc.) applies symmetrically and must be tested on both roles — spec's Edge Cases and each contract's test-type column require this. | **PASS** |
| VII. 基于清点的完整性 (Inventory-Based Completeness) | This entire feature is the inventory-based remediation of `docs/todo/005.md`'s own four-pass-verified findings — every FR traces to a specific `NEW-NN` citation with a verified file:line source (research.md, each contract file). The spec's Assumptions section discloses every item `005.md` raised that this feature does **not** close (4 refuted, 1 parity-note, 6 feature-gaps, plus the 2 out-of-scope 004.md/007-owned cross-references) with a stated reason each — no silent scope-narrowing. | **PASS** |

**Result**: No violations. No Complexity Tracking entries required (table intentionally empty).

## Project Structure

### Documentation (this feature)

```text
specs/009-audit-005-remediation/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── README.md                    # index + AT-scenario-impact summary
│   ├── session-protocol.md          # US1/US2 — truefix-session state machine
│   ├── store-and-transport.md       # US1/US2/US3 — truefix-store/log/transport
│   ├── fixt-and-dictionary.md       # US1/US2/US3 — truefix-core/dict/config
│   └── hardening-and-tooling.md     # US2/US3 — AT harness, perf hygiene, defaults
├── checklists/
│   └── requirements.md
└── tasks.md              # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
# Existing cargo workspace (001-007) — no new crates, no new binary targets.
crates/
├── truefix/                # Engine facade — US1 (NEW-96 Services wiring), US3 (NEW-48 Monitor,
│                            # NEW-91 log-writer shutdown, NEW-92 deterministic acceptor-group order,
│                            # NEW-27 async DNS)
├── truefix-core/           # wire codec, framing, tags — US1 (NEW-55 is_header FIXT tags), US2
│                            # (NEW-21 tag 0, NEW-22 group round-trip), US3 (NEW-04 MAX_BODY_LEN
│                            # consistency, NEW-80 render_members_ordered dedup, NEW-81 FieldMap::set
│                            # dedup, NEW-35–38 allocation/complexity hygiene)
├── truefix-session/         # session state machine — US1 (NEW-02 BeginSeqNo=0, NEW-03 acceptor
│                            # ResetOnLogon, NEW-84 gap-fill verification, NEW-56 teardown reason,
│                            # NEW-62 schedule-exit Logout, NEW-63 dict-invalid Logon, NEW-17 UDF
│                            # short-circuit), US2 (NEW-18, NEW-32, NEW-34, NEW-70, NEW-71, NEW-73,
│                            # NEW-85, NEW-86), US3 (NEW-39 SessionId::Display, NEW-87 Reject-before-
│                            # Logout, NEW-88 DefaultApplVerID validation, NEW-89 LogoutTimeout)
├── truefix-store/          # MessageStore backends — US1 (NEW-54 Mongo set_seq, NEW-08 Mongo
│                            # atomic save_and_advance_sender), US2 (NEW-23, NEW-75, NEW-76, NEW-77),
│                            # US3 (NEW-41, NEW-42)
├── truefix-log/            # Log backends — US1 (NEW-09 MssqlLog URL parsing, NEW-90 MSSQL
│                            # trust-cert opt-in), US2 (NEW-24 bounded channels), US3 (NEW-91
│                            # async-writer shutdown)
├── truefix-transport/      # session I/O, TLS, PROXY — US1 (NEW-93 pre-Logon DoS, NEW-11 TLS
│                            # default trust store, NEW-58 FIXT group-aware decode, NEW-12/13 PROXY
│                            # header capture, NEW-14 nonblocking scheduled connect), US2 (NEW-25,
│                            # NEW-26, NEW-33, NEW-74, NEW-94), US3 (NEW-49, NEW-95)
├── truefix-dict/           # runtime DataDictionary + codegen/converter tooling — US1 (NEW-06
│                            # FIX5.x ApplVerID dispatch, NEW-07 MultipleCharValue), US2 (NEW-19,
│                            # NEW-20, NEW-64–69), US3 (NEW-05, NEW-43–46, NEW-53, NEW-60, NEW-61,
│                            # NEW-78, NEW-79, NEW-82 — all gated behind the existing `dict-tooling`
│                            # feature except NEW-43/44/53, which are the runtime parser/model path)
├── truefix-config/         # .cfg → SessionConfig/ResolvedSession resolution — US1 (NEW-10 7
│                            # validation keys, NEW-96 LogMessageWhenSessionNotFound field), US2
│                            # (NEW-26 DNS-at-connect-time plumbing), US3 (NEW-46 circular vars,
│                            # NEW-47 line-number tracking, NEW-73/89 defaults)
└── truefix-at/             # acceptance-test harness — US3 (NEW-15/16 read_message outcomes,
                             # NEW-28 all-version validators, NEW-29/30/50/51 stricter assertions,
                             # NEW-52 per-scenario reporting, NEW-83 fixed-identity acceptor mode)

# Existing top-level docs (unaffected structurally)
docs/todo/005.md            # Source audit for this feature (read-only input)
```

**Structure Decision**: No new crates or directories. This feature is a remediation pass entirely
within the existing layered workspace established by 001-007 — the broadest crate footprint of any
audit-remediation feature so far (all 9 crates touched, since `005.md` is a full 9-crate sweep), but
every fix still lands in the crate that already owns the affected behavior; no fix requires
introducing a new cross-crate dependency edge. `truefix-dict`'s codegen/converter items remain gated
behind the existing `dict-tooling` feature (no change to that gating).

## Complexity Tracking

*No entries — Constitution Check above found no violations requiring justification.*
