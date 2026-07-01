# Requirements Quality Checklist: QuickFIX/J Parity Completion

**Purpose**: Validate the quality (completeness, clarity, consistency, measurability, coverage) of the
002 requirements before `/speckit-tasks` and `/speckit-implement`. These are "unit tests for the spec"
— they test what is *written*, not the implementation.
**Created**: 2026-06-30
**Feature**: [spec.md](../spec.md) · **Depth**: release gate · **Audience**: author + reviewer

## Requirement Completeness

- [ ] CHK001 - Are persistence requirements complete for every message class in a resend range (admin GapFill vs application PossDup)? [Completeness, Spec §FR-001/§FR-002, §US2]
- [ ] CHK002 - Is the `PersistMessages=N` behaviour fully specified (gap-fill instead of body replay)? [Completeness, Spec §FR-003]
- [ ] CHK003 - Are group-validation requirements specified for each toggle the spec names (count, delimiter, member ordering)? [Completeness, Spec §FR-005]
- [ ] CHK004 - Are inbound-integrity requirements enumerated for every check listed (BeginString, BodyLength, CheckSum, CompID, SendingTime, first-three-field order, field order, repeated tag)? [Completeness, Spec §FR-007]
- [ ] CHK005 - Are the typed callback outcomes defined for all three callbacks together with the engine effect of each? [Completeness, Spec §FR-016, §US5]
- [ ] CHK006 - Are the generated codegen artifacts enumerated for "all messages" (message/group/component structs, field accessors, value enums, MessageCracker)? [Completeness, Spec §FR-020/§FR-022]
- [ ] CHK007 - Are TLS-from-config requirements complete (key, cert, CA, min version, SNI, client-auth)? [Completeness, Spec §FR-017]
- [ ] CHK008 - Are schedule requirements complete for daily windows, weekly StartDay/EndDay windows, and the non-stop case? [Completeness, Spec §FR-018]
- [ ] CHK009 - Are the exported metric signals fully enumerated (which gauges and counters)? [Completeness, Spec §FR-023]
- [ ] CHK010 - Are storage/logging requirements specified per backend (Postgres/MySQL/SQLite), for the cached store (cache + fsync), and for each log switch? [Completeness, Spec §FR-024/§FR-025/§FR-026]
- [ ] CHK011 - Is the Appendix A stance-update obligation stated for every key this feature implements? [Completeness, Spec §FR-027]
- [x] CHK012 - Does the spec define a migration-documentation deliverable for the breaking callback change? [Resolved by §FR-028]

## Requirement Clarity

- [ ] CHK013 - Is "source of truth for resends" defined precisely enough to be testable (resend identical before and after restart)? [Clarity, Spec §FR-002]
- [ ] CHK014 - Is "byte-identical" encode/decode for typed vs generic messages anchored to an objective comparison? [Clarity, Spec §FR-021]
- [ ] CHK015 - Are the supported timestamp-precision levels and the default enumerated rather than described vaguely? [Clarity, Spec §FR-009]
- [ ] CHK016 - Are the "correctness-affecting switches" named explicitly (PersistMessages, HeartBeatTimeoutMultiplier, TestRequestDelayMultiplier)? [Clarity, Spec §FR-010]
- [ ] CHK017 - Is the reverse-routing field set and the empty-routing-tags rule specified, not just "reverse the routing fields"? [Clarity, Spec §FR-011]
- [ ] CHK018 - Is the config-mapping error given a defined shape (key + session + kind) rather than "human-readable error"? [Clarity, Spec §FR-015]
- [ ] CHK019 - Is the set of SQL databases pinned to a concrete list rather than "common databases"? [Clarity, Spec §FR-024, §Clarifications]

## Requirement Consistency

- [ ] CHK020 - Do the FRs, SCs, and Assumptions use one consistent set of targeted FIX versions? [Consistency, Spec §Assumptions, §SC-003/§SC-004]
- [ ] CHK021 - Is the "no partial start" guarantee stated consistently between FR-015 and the Edge Cases? [Consistency, Spec §FR-015, §Edge Cases]
- [ ] CHK022 - Is the breaking-change framing for callbacks consistent across US5, FR-016, and the Clarifications? [Consistency, Spec §FR-016, §Clarifications]
- [ ] CHK023 - Is the metrics-facade-only decision consistent between FR-023 and the Assumptions (no bundled exporter)? [Consistency, Spec §FR-023, §Assumptions]
- [ ] CHK024 - Are group-validation toggle names consistent between FR-005 and the Dependencies section? [Consistency, Spec §FR-005, §Dependencies]

## Acceptance Criteria Quality (Measurability)

- [ ] CHK025 - Are the P1 success criteria objectively measurable (config-start, restart-resend, group-parsing)? [Measurability, Spec §SC-001/§SC-002/§SC-003]
- [ ] CHK026 - Can "100% of the targeted AT scenarios" be evaluated, i.e. is the targeted scenario set named concretely? [Measurability, Spec §SC-003/§SC-004]
- [ ] CHK027 - Is "satisfies Principle IV" tied to verifiable artifacts (dual-track hash + round-trip equality)? [Measurability, Spec §SC-007]
- [ ] CHK028 - Is "exported metrics reflect reality" expressed as checkable signal changes over a defined cycle? [Measurability, Spec §SC-010]

## Scenario & Recovery Coverage

- [ ] CHK029 - Are exception flows defined for config errors (missing key, invalid value, unusable resource)? [Coverage/Exception, Spec §FR-015, §Edge Cases]
- [ ] CHK030 - Are recovery flows defined for process restart followed by a post-restart ResendRequest? [Coverage/Recovery, Spec §US2, §FR-001]
- [ ] CHK031 - Are recovery flows defined for schedule-boundary reset and reconnect (and the non-stop exception)? [Coverage/Recovery, Spec §FR-018]
- [ ] CHK032 - Are exhaustion/exception flows defined when all configured failover endpoints are unreachable? [Coverage/Exception, Spec §Edge Cases, §FR-019]
- [ ] CHK033 - Are alternate flows defined for validation toggles in both states (e.g. ValidateUnorderedGroupFields on vs off)? [Coverage, Spec §FR-005]

## Edge Case Coverage

- [ ] CHK034 - Are nested-group and zero-count group edge cases specified? [Edge Case, Spec §Edge Cases, §FR-004]
- [ ] CHK035 - Is the mixed persisted/never-sent resend-range edge case specified? [Edge Case, Spec §Edge Cases]
- [ ] CHK036 - Is the unsupported combination (a group-dependent validation toggle without group parsing active) required to be surfaced rather than silently passed? [Edge Case, Spec §Edge Cases, §Dependencies]

## Non-Functional & Constitution Alignment

- [ ] CHK037 - Are typed-error / no-panic requirements stated for the new config-mapping and callback error paths? [NFR, Constitution I, Spec §FR-015/§FR-016]
- [ ] CHK038 - Is AT/session-test gating stated as a requirement for every correctness behaviour (not just assumed)? [Coverage, Constitution II, Spec §FR-012]
- [ ] CHK039 - Are license-provenance requirements stated for the newly-exercised dependencies and the codegen data source? [NFR, Constitution III, Spec §Assumptions]
- [ ] CHK040 - Do the requirements assert that completing codegen (FR-020/022) and metrics (FR-023) closes the partially-met Principles IV and I? [Traceability, Constitution IV/I, Spec §Context]

## Dependencies, Assumptions & Boundaries

- [ ] CHK041 - Are the inter-requirement dependencies documented (FR-004→FR-005, FR-020→FR-022, FR-013→FR-017/018/019)? [Dependency, Spec §Dependencies]
- [ ] CHK042 - Is the CI/DB-availability assumption for multi-backend SQL testing documented? [Assumption, Spec §Assumptions]
- [ ] CHK043 - Is the assumption that bundled dictionary subsets bound codegen coverage stated? [Assumption, Spec §Assumptions]
- [ ] CHK044 - Is the boundary against QuickFIX/Go-only extras unambiguous, with each excluded item named? [Clarity, Spec §Out of Scope]

## Notes

- Items marked `[Gap]` (e.g. CHK012) flag content that may need to be added to the spec before
  implementation; the rest validate existing requirement quality.
- Traceability: every item references a spec section or carries a `[Gap]`/`[Assumption]`/`[Dependency]`
  marker (≥80% target met).
- This checklist complements — does not duplicate — `requirements.md` (spec-quality gate from
  `/speckit-specify`) and the forthcoming `/speckit-analyze` coverage pass.
