<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/008-third-audit-remediation/plan.md`

Active feature: **008-third-audit-remediation** — remediate every confirmed defect found by a
2026-07-04 third-pass audit, recorded in `docs/todo/005.md` (four successive independent
full-codebase reviews of all 9 crates, each cross-referenced against QuickFIX/J + QuickFIX/Go,
with each later pass re-verifying and in several cases refuting/downgrading/folding together
findings from the passes before it). Numbering: `NEW-01` onward (independent series, not continuing
`BUG`/`GAP`). Built on **001-fix-engine-parity** through **007-second-audit-remediation** (007 fixes
`docs/todo/004.md`'s `BUG-23`-`BUG-111`, in progress on its own branch). 008 fixes 21
protocol-correctness/data-loss/security defects (US1: a total-functional-failure Mongo
sequence-persistence bug, an unauthenticated multi-session-acceptor pre-Logon DoS vector,
`BeginSeqNo=0` handling, acceptor `ResetOnLogon`, gap-fill `SequenceReset` verification bypass, FIXT
1.1 header/group-decode gaps, teardown reset-reason conflation, and more), 30 narrower-blast-radius
defects (US2), and 32 hardening/dictionary-tooling/AT-harness-coverage items (US3) — 83 FRs total
(`FR-001`-`FR-083`). Spec: `specs/008-third-audit-remediation/spec.md`. Governing constitution:
`.specify/memory/constitution.md` (v2.0.0). One pass, staged US1→US2→US3 (P1→P2→P3). Four
scope-defining decisions were resolved via `/speckit-clarify`: all three priority tiers are
addressed in this one pass (mirroring 007's precedent); the TLS default-trust-store fix
(`NEW-11`/`FR-049`) may add one narrowly-scoped new dependency (`rustls-native-certs`); the Mongo
sequence-store fix (`NEW-54`/`FR-001`) is forward-only (no migration of pre-existing corrupted
rows); and the multi-session-acceptor pre-Logon DoS timeout (`NEW-93`/`FR-044`) reuses the existing
`logon_timeout` setting rather than a new config key. All 21 User Story 1 items were independently
re-verified by reading current source during Phase 0 research (not merely trusted from `005.md`'s
own already-four-pass-verified text) — see research.md; every one confirmed exactly as described,
no further correction needed. (001-007 spec/plan remain the baseline that 008's FR-IDs reference;
`docs/todo/003.md`/`004.md`'s own findings are out of scope here — closed by 006/in progress on 007
respectively.)
<!-- SPECKIT END -->
