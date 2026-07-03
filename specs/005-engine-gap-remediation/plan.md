# Implementation Plan: Engine Gap Remediation

**Branch**: `main` (feature tracked by directory, not a dedicated branch — mirroring 004's convention;
no `before_plan`/branch-creation hook is registered) | **Date**: 2026-07-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/005-engine-gap-remediation/spec.md`

## Summary

A 2026-07-02 code-vs-code audit (`docs/engine-comparison-gaps.md`), including a full sweep of every
`Recognized`-stance `.cfg` key, found 4 real bugs (a config-parsing correctness bug, a wire-decode
correctness edge case, a config-key registry that overstates coverage, and a `JdbcURL` scheme mismatch
that defeats QuickFIX/J config-compatibility), 4 session-state-machine protocol-correctness gaps (no
resend veto/gap-fill substitution, no PossDup anti-replay check, no inbound chunked-resend
continuation, no duplicate-Logon rejection), and a long tail of feature-completeness gaps across the
session/transport/store-log/dictionary-codec layers. Unlike feature 004, **this feature does touch
session-state-machine and codec/protocol behavior** — the AT conformance suite is expected to gain new
passing scenarios, not merely stay unmodified. Phase 0 research grounded every decision by reading the
actual current code rather than trusting the source document's framing verbatim, and surfaced one
correction worth flagging up front: the widely-cited "no resend veto" gap (`GAP-07`) turns out to
already have its veto point (`Application::to_app` is already invoked for resent messages) — the real,
narrower gap is that a veto during a resend doesn't yet substitute a compensating `GapFill`, which is
what this feature actually implements (research.md §5). Two scope-defining decisions were made via
interactive `/speckit-clarify`/`/speckit-specify` Q&A rather than assumed: `BUG-03`'s acceptor-key
misrepresentation gets a real `AcceptorBuilder` wiring fix (not a stance-registry-only downgrade), and
the large codec/dictionary-model completeness cluster (`GAP-22` through `GAP-33`) is included in this
same round rather than deferred to a future feature. The work spans 9 user stories / 31 functional
requirements and is delivered **in one pass, internally staged (G1–G9)** with a verifiable checkpoint
per stage, mirroring 001–004's model.

## Technical Context

**Language/Version**: Rust, edition 2021, MSRV per workspace pin. `#![forbid(unsafe_code)]` and
per-crate `deny(clippy::unwrap_used/expect_used/panic/indexing_slicing)` on non-test code unchanged
(Principle I).

**Primary Dependencies**: No new external crates (research.md §18) — every FR is implemented against
already-present workspace dependencies (`tokio`, `tokio-rustls`/`rustls`, `thiserror`, `sqlx`,
`tiberius`, `redb`, `mongodb`, `socket2`, `time`, `tracing`). `socket2` (already a direct dependency of
`truefix-transport`) gains one new use (local-bind support, US6/FR-015) alongside its existing
socket-options usage.

**Storage**: All five existing `MessageStore` backends (Memory/File/CachedFile/SQL(PG/MySQL/SQLite)/
MSSQL/Redb/Mongo) gain a defaulted `creation_time()` trait method (US7/FR-017) and, where applicable
(SQL-family, Redb), an atomic `save_and_advance_sender` path (US7/FR-018). All four structured `Log`
backends (SQL/MSSQL/Redb/Mongo) gain a widened schema (timestamp + session-identity column, US7/FR-019).
No new backend, no trait-contract-breaking change (all additions are defaulted/additive).

**Testing**: Table-driven unit tests for the codec/config-parsing bug fixes (US1); two-process
integration tests for every session-state-machine change (US3's resend-veto-with-gap-fill, PossDup
anti-replay, duplicate-Logon rejection — each MUST land as both a `truefix-session` state-machine test
and, per Constitution Principle II, a corresponding **new AT scenario** since these are genuine
protocol-behavior additions); `.cfg`-mapping tests for every new/corrected config key (US2, US5, US6,
US8); full `MessageStore`/`Log` conformance-suite-pattern extensions for US7's persistence hardening;
dictionary/codec unit + property tests for US9's model additions, plus a coverage-diff test against
QuickFIX/J's bundled dictionaries for FR-031 (research.md §17h). **AT suite**: expected to grow past
today's 353/353 baseline (confirmed still green and unmodified as of plan time) — the growth itself,
not mere non-regression, is part of this feature's Definition of Done (SC-013).

**Target Platform**: Linux/macOS servers (tokio). No platform-specific code added.

**Project Type**: Library/engine (existing cargo workspace). No new crates, no new binary targets.

**Performance Goals**: Correctness/completeness first, per Constitution Principle II's conflict-order
(protocol correctness > production readiness > feature count > performance). No specific new
performance target is introduced; the reconnect-backoff array (US6/FR-014) and connect-timeout
(US6/FR-016) are themselves reliability/operability features, not throughput/latency targets.

**Constraints**: No panic/unwrap/expect/unreachable on codec/session/I-O/timer paths (Principle I),
unchanged. `unsafe_code = forbid` remains workspace-wide. No reference source/data copied — the
QuickFIX/J dictionary-coverage diff (US9/FR-031, research.md §17h) reads `thrdpty/quickfixj`'s bundled
XML **for field/message enumeration facts only** (names, tags, types, associations), never copying XML
markup or comments verbatim, per Principle III's established boundary for this kind of read (same
boundary already applied to AT-scenario porting). This feature introduces one disclosed, additive
public-API surface growth beyond what a strict "zero new members" reading of prior features' precedent
would allow: `SessionConfig` gains 5 new identity fields (US5) + up to 3 new connection-tuning fields
(US6) + 1 new field (US9's `field_order`/`version_meta`, on `MessageDef`/`DataDictionary` respectively,
not `SessionConfig`); `Action` gains a new field on its `Send` variant (US3, research.md §5); `keys.rs`
stance changes run in both directions (some `Recognized`→`Implemented`, some `Recognized`→
`Unsupported`, and `BUG-03`'s three keys stay `Implemented` but the underlying behavior catches up to
match). Every one of these is additive/non-breaking in the same sense 004's `ResolvedSession`/
`ConfigError` growth was (grep-confirmed no exhaustive external match exists on any of these types) —
see Complexity Tracking.

**Scale/Scope**: 31 FRs across 9 user stories; largest single component is US9's dictionary/codec
cluster (10 FRs, research.md §17a–§17h, including a per-FIX-version content-coverage diff against
QuickFIX/J — the single largest content (not mechanism) task in this feature). Up to 13 config keys
change stance in `keys.rs` (4 `Recognized`→`Implemented` bug/gap fixes' keys, 4 `Recognized`→
`Unsupported` doc-accuracy corrections, up to 9 JDBC pool/table-name keys `Recognized`→`Implemented`
per US8/FR-021 — some overlap with the US2 JDBC-URL keys already counted).

**Reference-only architecture inputs**: `thrdpty/quickfixj` (Java) — read for resend-veto placement
(`Session.java`'s `resendApproved`), PossDup/duplicate-Logon severity (`validatePossDup`,
`AcceptorIoHandler`), `JdbcURL`/`JdbcUser`/`JdbcPassword` combination logic (`JdbcUtil.java`),
`ReconnectInterval` array semantics, and — largest single read — the bundled per-version XML
dictionaries themselves for US9/FR-031's coverage diff. `thrdpty/quickfix` (Go) — read for the PossDup/
resend behavioral cross-check already captured in `docs/engine-comparison-gaps.md`'s citations. All
reads are design/behavior/data-fact only, per Principle III — no source or markup copied.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Gate | Status |
|-----------|------|--------|
| I. Production-Ready First | Every trait/struct growth is additive/defaulted (`MessageStore::creation_time()` defaults to `None`, matching the existing `was_corrupted()` precedent); no new panic surface — `AmbiguousAcceptorTemplate`/credential-splicing failures are typed `ConfigError` variants, never a panic or silent fallback (research.md §3/§4); `SendOrigin`'s gap-fill-on-veto path (§5) is a bounded one-level recursion (GapFills are admin-typed, terminates immediately), not unbounded. | ✅ PASS |
| II. Protocol Correctness First (NON-NEG) | **This is the first feature since 004 to touch session-state-machine/codec behavior.** Every protocol-behavior change (US3's three FRs, US4) is grounded in QuickFIX/J and/or QuickFIX/Go's actual cited behavior (research.md §5–§8), and — per Testing above — MUST land with a new AT scenario, not just a unit/integration test, satisfying Principle II's "AT pass = highest-priority acceptance evidence" requirement. `/speckit-clarify`'s PossDup-severity answer explicitly chose the QuickFIX/J-matching behavior (full logout+disconnect) over a lighter-weight alternative specifically to keep this feature spec-grounded rather than convenience-driven. | ✅ PASS |
| III. License & Provenance (NON-NEG) | No new dependency (research.md §18). The one qualitatively new kind of reference-reading this feature introduces — diffing QuickFIX/J's bundled XML dictionaries field-by-field for US9/FR-031 (research.md §17h) — is scoped explicitly to enumeration facts (names/tags/types/associations), never markup/comments/structure, matching the same boundary already established and precedented for AT-scenario porting (Principle III's own stated exception). | ✅ PASS |
| IV. Dual-Track Data Dictionary | **Directly implicated by US9** — every dictionary-model addition (FieldType variants, open enums, per-group child dictionaries, custom field order, version metadata) MUST land in both the runtime `DataDictionary` path and the `codegen.rs` dual-track path, keeping the shared content-hash provenance intact (research.md §17e explicitly calls this out for field ordering; the same discipline applies to every other §17 sub-decision, verified per-task by the existing dual-track hash-equality test pattern). | ✅ PASS (conditioned on every US9 task updating both tracks, verified by existing hash tests) |
| V. Test Discipline | Table-driven tests for bug fixes (US1); two-process integration tests + **new AT scenarios** for every session-state-machine change (US3, US4) — this is the first feature required to grow the AT corpus rather than merely not regress it; `.cfg`-mapping tests for every new/corrected key; conformance-suite-pattern tests for US7's store/log hardening; dictionary/codec unit tests plus a coverage-diff test for US9. | ✅ PASS |
| VI. Acceptor/Initiator Parity | US2/`BUG-03` is acceptor-specific by nature (multi-session routing/dynamic templates/allow-lists are acceptor-side concepts, matching QuickFIX/J's own architecture — no initiator parallel exists to require, same reasoning already applied to PROXY-protocol-is-acceptor-side in 003's plan). US6 (reconnect backoff, local bind, connect timeout) is initiator-specific by the same logic in reverse. US3/US4/US5/US7/US9 apply uniformly to acceptor and initiator sessions — verified per contract file's test-hook list. | ✅ PASS |
| VII. Inventory-Based Completeness | Every `GAP-##`/`BUG-##` in this feature's scope traces to exactly one FR (spec.md's parenthetical citations); every stance-registry change updates `keys.rs` with a documented reason (US8); no gap is claimed closed without a test citation in its contract file; the out-of-scope items (spec.md's Assumptions) are each traced back to the source document's own recommendation, not silently dropped. | ✅ PASS |

**Conflict-order reminder**: License → Protocol correctness → Production readiness → feature count. This
feature's central tension is size vs. correctness rigor — resolved by staging (G1–G9 below) so each
stage is independently gate-checked (AT suite green + growing where applicable) rather than deferring
all verification to the end of a 31-FR feature.

**Initial gate result**: PASS. No dependency audit needed (§18). One Complexity Tracking entry (the
multi-field additive growth across `SessionConfig`/`Action`/`MessageStore`/`keys.rs`, all confirmed
non-breaking in practice per the pattern established below).

**Post-design re-check** (after Phase 1's `data-model.md`/`contracts/`/`quickstart.md`): PASS,
unchanged. Every type-shape decision in `data-model.md` and every contract in `contracts/` stayed
within what research.md already scoped — no new principle tension surfaced during design (in
particular, every US9 contract explicitly carries its dual-track/Principle IV obligation forward into
its own "Test hooks" section, and `session-protocol.md` explicitly carries its AT-growth/Principle II
obligation forward into its own header). No new Complexity Tracking entries beyond the one already
recorded.

## Project Structure

### Documentation (this feature)

```text
specs/005-engine-gap-remediation/
├── plan.md              # This file
├── spec.md              # Feature spec (9 US, 31 FR, 3 clarifications resolved)
├── research.md          # Phase 0 output — 18 grounded decisions incl. one source-document correction
├── data-model.md         # Phase 1 output — concrete shapes for every spec Key Entity + research findings
├── quickstart.md         # Phase 1 output — per-user-story validation commands
├── contracts/            # Phase 1 output (grouped by layer, mirroring the source document's own grouping)
│   ├── README.md
│   ├── correctness-bugs.md        # US1, US2 (BUG-01–04)
│   ├── session-protocol.md        # US3, US4 (GAP-07/08/09/18a)
│   ├── session-transport.md       # US5, US6 (GAP-47, GAP-14/15/16)
│   ├── store-log-hardening.md     # US7 (GAP-38/39/41)
│   ├── stance-registry.md         # US8 (doc-accuracy + JDBC pool/table wiring)
│   └── dictionary-model.md        # US9 (GAP-22–33)
└── checklists/
    └── requirements.md   # spec quality (from /speckit-specify, re-validated by /speckit-clarify)
```

### Source Code (repository root) — files touched/added per stage

No new crates, no new binary targets. Changes land in the existing layered workspace, dependency
direction unchanged (strictly downward):

```text
crates/
├── truefix-config/         # G1: strip_comment fix; G2: is_sql_scheme/is_mssql_scheme jdbc: recognition
│   │                        # + credential splicing, resolve_one parses 5 new SessionId keys (G5) + 3
│   │                        # new SessionConfig keys (G6) + AcceptorTemplate/DynamicSession/
│   │                        # AllowedRemoteAddresses (G2) + JDBC pool/table-name keys (G8)
│   └── src/lib.rs (strip_comment), src/builder.rs, src/keys.rs (stance updates throughout)
├── truefix-core/            # G1: data_field_for_length 93→89; G9: FieldType-adjacent codec/encode.rs
│   │                        # field-order support
│   └── src/tags.rs, src/codec/decode.rs, src/codec/encode.rs, src/field_map.rs (group mutation API)
├── truefix-session/         # G3: resend veto's Action::Send origin field + gap_fill_after_veto; G4:
│   │                        # PossDup anti-replay + duplicate-Logon (both via reject_logon reuse); G4:
│   │                        # inbound chunk auto-continuation; G5: SessionId new_full + SessionConfig
│   │                        # identity fields; G6: SessionConfig connection-tuning fields
│   └── src/state.rs, src/session_id.rs, src/config.rs, src/application.rs (SendOrigin doc)
├── truefix-transport/       # G2: AcceptorBuilder wiring from Engine::start (moved logic, see
│   │                        # crates/truefix below); G3: perform_actions gap-fill-on-veto handling; G6:
│   │                        # reconnect backoff array, local-bind, connect-timeout on every initiator path
│   └── src/lib.rs
├── truefix/                 # G2: Engine::start's acceptor branch restructured for port-grouping +
│   │                        # AcceptorBuilder construction
│   └── src/lib.rs
├── truefix-store/           # G7: creation_time() default method + per-backend overrides;
│   │                        # save_and_advance_sender (SQL/MSSQL/Redb overrides); StoreConfig::Sql/Mssql
│   │                        # field growth (G8)
│   └── src/lib.rs, src/sql.rs, src/redb.rs, src/mongo.rs, src/file.rs, src/memory.rs
├── truefix-log/              # G7: schema widening (timestamp + session_id) across all 4 structured
│   │                          # backends
│   └── src/sql.rs, src/redb.rs, src/mongo.rs
└── truefix-dict/             # G9: FieldType variants, open-enum sentinel, per-group child dictionaries,
    │                          # group CRUD (field_map.rs is truefix-core, not here), header/trailer group
    │                          # decode support (decode.rs is truefix-core), custom field order,
    │                          # version metadata + match validation, value-label lookup, bundled
    │                          # dictionary content expansion (dict-src/normalized/*.fixdict)
    └── src/model.rs, src/parser.rs, src/validate.rs, src/codegen.rs, dict-src/normalized/*.fixdict
docs/
├── engine-comparison-gaps.md  # GAP/BUG items checked off as each stage closes
├── parity-matrix.md           # stance updates (US8)
└── todo-gap-analysis.md       # cross-reference note updated once 005 lands
```

**Structure Decision**: Reuse the existing layered workspace unchanged; zero new crates, zero new
binary targets. The one structurally significant change is `Engine::start`'s acceptor branch
(`crates/truefix/src/lib.rs`), which grows from "one `Acceptor::bind_with` call per session" to "group
by bind address, then either the existing single-session path or a new `AcceptorBuilder`-based
multi-session path" (research.md §4) — the largest control-flow change in this feature outside the
dictionary-model cluster.

### Internal Delivery Stages (single release, dependency-ordered, each with a checkpoint)

> One-pass gap closure, **internally staged**. `/speckit-tasks` expands each stage into tasks; these
> are NOT separate releases. Ordered by spec priority (P1 → P2 → P3) with dependency notes where a
> later stage builds on an earlier one's output.

| Stage | Scope (US / FR) | Checkpoint (verifiable exit) |
|-------|-----------------|------------------------------|
| **G1** | Correctness bugs: `#` truncation, Signature/93 mapping (US1; FR-001/002) | A `.cfg` `Password` value containing `#` round-trips exactly; a `Signature`/`SignatureLength` field pair with an embedded SOH byte decodes byte-identically (SC-001/002). |
| **G2** | `.cfg` keys that misrepresent behavior: `AcceptorBuilder` wiring + port-grouping, `JdbcURL` scheme recognition + credential splicing (US2; FR-003/004/005/006/006a) | A `.cfg`-only multi-session acceptor with `AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` set actually governs connection acceptance/template resolution (SC-004/022); an unmodified QuickFIX/J-style `JdbcURL=jdbc:...` + separate user/password starts a working SQL session (SC-003). |
| **G3** | Resend veto + gap-fill substitution (US3 part 1; FR-007) | An application-vetoed resend message is replaced by a `SequenceReset-GapFill` on the wire, in every exercised gap-fill scenario (SC-005); **new AT scenario** added. |
| **G4** | PossDup anti-replay + duplicate-Logon rejection + inbound chunk auto-continuation (US3 part 2 + US4; FR-008/009/010/011) | A PossDup message with a falsified `OrigSendingTime` and a duplicate Logon on an already-logged-on session both trigger logout+disconnect, in every exercised scenario (SC-006/007); a multi-chunk inbound resend completes with no manual re-request (SC-008); **new AT scenarios** added for all three. |
| **G5** | Session identity completeness: sub-ID/location-ID/qualifier (US5; FR-012/013) | Two `.cfg` sessions sharing BeginString/SenderCompID/TargetCompID but distinct `SessionQualifier` both start as distinct sessions (SC-009). |
| **G6** | Initiator connection robustness: reconnect backoff array, local bind, connect timeout (US6; FR-014/015/016) | A stepped reconnect interval reaches and sticks at its final value (SC-010); a short connect-timeout against an unresponsive peer fails within bound, never hangs (SC-011); local-bind address is honored. |
| **G7** | Store/log persistence hardening: creation time, atomic save+increment, log schema widening (US7; FR-017/018/019) | Every structured log entry carries a timestamp + session-identity column (SC-012); a store's creation time is queryable and updates on reset; SQL-family save+increment is atomic. |
| **G8** | Stance-registry accuracy sweep + JDBC pool/table-name wiring (US8; FR-020/021) — depends on G2 (JDBC URL recognition must land first for the pool/table-name keys to have somewhere to attach) | Four inapplicable keys read `Unsupported` with a documented reason; JDBC pool-tuning/table-name keys set alongside a `jdbc:`-style `JdbcURL` actually apply to the resulting store. |
| **G9** | Dictionary and codec model completeness (US9; FR-022–031) — independent of G1–G8, can proceed in parallel; FR-031's per-version content diff is itself independently parallelizable across the 8 XML-backed versions (research.md §17h) | All 11 field types validate per-format (SC-014); open enums, per-group child dictionaries, group CRUD, header/trailer groups, custom field order, version-match, and value-label lookup each pass their dedicated acceptance scenario (SC-015–021); bundled dictionary coverage matches QuickFIX/J's own per version (SC-023). |

**Cross-cutting**: FR-009's PossDup-severity and FR-010's duplicate-Logon-severity share one
implementation primitive (`reject_logon` reuse, research.md §6/§7) — G4 delivers both together.
SC-013 (AT suite grows, stays green) is a per-stage exit criterion for every G that touches protocol
behavior (G3, G4), verified by running `cargo test -p truefix-at --test conformance` at the end of each
such stage and confirming the scenario-run count is unchanged-or-growing from this plan's baseline
(353/353, confirmed green at plan time) — never shrinking, never a previously-passing scenario turning
red.

## Complexity Tracking

> One documented, non-blocking note (not a Constitution violation), extending 004's own precedent for
> this same kind of disclosure: this feature grows several existing public types with new fields/
> variants rather than introducing new types for each concern. Every growth below is additive (no
> existing field/variant removed or retyped) and confirmed non-breaking in practice by the same
> grep-for-exhaustive-external-match check 004 already established as this project's verification
> method for this class of change.

| Growth | Why Needed | Simpler Alternative Rejected Because |
|--------|------------|---------------------------------------|
| `SessionConfig` gains 5 identity fields (US5) + up to 3 connection-tuning fields (US6) | `SessionId`/reconnect/local-bind/connect-timeout all need `.cfg`-sourced values threaded through the one struct `builder.rs` already populates and `Session`/transport already consume | A new wrapper struct for "extended" config — rejected: splits one session's configuration across two types for no behavioral gain, and every consumer of `SessionConfig` today would need to also thread the wrapper through, a larger blast radius than field addition |
| `Action::Send` gains a `SendOrigin` field (US3) | The gap-fill-on-veto fix (research.md §5) needs to distinguish a resend-originated send from a live one at the point `to_app`'s veto is handled, and `Action` is the only channel between `Session` (sans-IO) and the transport layer that executes the veto-check | A new `Action::Resend(Message, u64)` variant instead of widening `Send` — considered equally valid; deferred to `/speckit-tasks` as an implementation-detail choice, not a design one, since both are additive to the enum |
| `MessageStore` gains `creation_time()` (defaulted) and `save_and_advance_sender()` (defaulted) (US7) | Matches the existing `was_corrupted()` defaulted-method precedent exactly — optional-capability growth on a trait with external implementors | A required (non-defaulted) method — rejected: breaking for any external `MessageStore` implementor, unlike a defaulted addition |
| `keys.rs` stance directions diverge (some up, some down) within one feature | Genuinely different findings require genuinely different corrections — some keys were understated (`Recognized`→`Implemented`), some overstated (`Recognized`→`Unsupported`, doc-accuracy notes) | Picking one direction only and treating the other as a separate feature — rejected: both directions were found by the same audit pass and fixing only half would leave the registry still inaccurate in the other direction, undermining Principle VII |
