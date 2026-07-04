<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/007-second-audit-remediation/plan.md`

Active feature: **007-second-audit-remediation** — remediate every confirmed defect found by a
2026-07-03 second-pass audit, recorded in `docs/todo/004.md` (a pure per-crate code review plus a
systematic QuickFIX/J + QuickFIX/Go source comparison, itself carrying two internal
self-verification passes — a per-item "Verification Pass" and a four-agent "二次交叉验证"
cross-check against the current working tree, which found 3 items already fixed as a side effect of
006). Numbering continues `BUG-23` onward from `002.md`/`003.md`'s `BUG-05`-`BUG-22`. Built on
**001-fix-engine-parity** through **006-audit-remediation** (closed `003.md`'s full audit, 578 tests
passing, 405/405 AT scenario-runs — both merged to `main`). 007 fixes 13 must-fix-before-shipping
items (US1: `ResetSeqNumFlag` handshake, `ResendRequest` deadlock, duplicate-connection protection,
`Engine::shutdown()`/`Drop` resource leaks, scheduled-initiator reconnect, sequence-number store
crash-safety, SQL/MSSQL store fixes, `BodyLength=0` rejection, acceptor schedule enforcement, admin
dictionary validation, callback-ordering restructure), 16 narrower-blast-radius defects (US2), and
20 low-priority hardening/hygiene items (US3) — 51 FRs total (`FR-001`-`FR-050` + `FR-001a`). Spec:
`specs/007-second-audit-remediation/spec.md`. Governing constitution:
`.specify/memory/constitution.md` (v2.0.0). One pass, staged R1→R2→R3 (one stage per user story,
P1→P2→P3). Three scope-defining decisions were resolved via `/speckit-clarify`: the sequence-number
store adopts QuickFIX/J-Go's split two-file layout (`senderseqnums`/`targetseqnums`) rather than a
minimal atomic-single-file fix; all three priority tiers are addressed in this one pass rather than
split across features; and existing single-file deployments auto-migrate transparently on first open
after upgrading (a disclosed, additive on-disk format change). No new external dependencies. Three
disclosed additive public-API surface growths: `SessionHandle::abort`/`is_finished` (sync,
non-consuming), `impl Drop for Engine`, and a new `Event::Disconnected` variant — see plan.md's
Technical Context and research.md's "Summary of decisions requiring disclosure" for the full
rationale on each. Every P0/P1-tier item in this plan was independently re-verified by reading
current source during Phase 0 research (not merely trusted from `004.md`'s own text) — see
research.md. (001-006 spec/plan remain the baseline that 007's FR-IDs reference; `docs/todo/003.md`'s
findings are fully closed by 006 and out of scope here.)
<!-- SPECKIT END -->
