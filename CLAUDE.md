<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/009-audit-005-remediation/plan.md`

Active feature: **009-audit-005-remediation** — remediate every confirmed defect found by a
2026-07-04 third/fourth-pass audit, recorded in `docs/todo/005.md` (four successive independent
full-codebase reviews of all 9 crates, each cross-referenced against QuickFIX/J + QuickFIX/Go,
with each later pass re-verifying and in several cases refuting/downgrading/folding together
findings from the passes before it). Numbering: `NEW-01` onward (independent series, not continuing
`BUG`/`GAP`). Built on **001-fix-engine-parity** through **007-second-audit-remediation** (007 fixes
`docs/todo/004.md`'s `BUG-23`-`BUG-111`, in progress on its own branch). Note: an earlier feature
directory, `specs/008-third-audit-remediation/`, covered this same source document but its files
were lost (deleted from disk, uncommitted) before being merged; 009 is an independently re-derived
replacement, not a continuation — its FR numbering and clarification answers are its own (though
they landed on largely the same structure, since both read the same source audit). 009 fixes 21
protocol-correctness/data-loss/security defects (US1: a total-functional-failure Mongo
sequence-persistence bug, an unauthenticated multi-session-acceptor pre-Logon DoS vector,
`BeginSeqNo=0` handling, acceptor `ResetOnLogon`, gap-fill `SequenceReset` verification bypass, FIXT
1.1 header/group-decode gaps, teardown reset-reason conflation, and more), 30 narrower-blast-radius
defects (US2), and 32 hardening/dictionary-tooling/AT-harness-coverage items (US3, including 6
allocation/complexity-hygiene items implemented as real fixes rather than tracked-only, per
clarification) — 90 in-scope items / 83 FRs total (`FR-001`-`FR-083`). Spec:
`specs/009-audit-005-remediation/spec.md`. Governing constitution:
`.specify/memory/constitution.md` (v2.0.0). One pass, staged US1→US2→US3 (P1→P2→P3). Four
scope-defining decisions were resolved via `/speckit-clarify`: downgrade (not implement) the 2
validation keys with no `ValidationOptions` field (`NEW-10`/`FR-014`); implement (not merely track)
the perf-hygiene items (`FR-082`/`FR-083`); add a new fixed-identity AT acceptor mode additively,
not replace the dynamic-template mode (`NEW-83`/`FR-062`); and change `reconnect_interval`'s bare
struct default to 30, a real code change (`NEW-73`/`FR-042`). A representative, risk-weighted sample
of User Story 1 items (the highest-severity, spanning session/transport/store/config/log) was
independently re-verified by reading current source during Phase 0 research (not merely trusted
from `005.md`'s own already-four-pass-verified text) — see research.md §R1; every one confirmed
exactly as described, zero drift found. The one new dependency this feature adds is
`rustls-native-certs` (`NEW-11`'s TLS default trust-store fix, research.md §R2). (001-007 spec/plan
remain the baseline that 009's FR-IDs reference; `docs/todo/003.md`/`004.md`'s own findings are out
of scope here — closed by 006/in progress on 007 respectively.)
<!-- SPECKIT END -->
