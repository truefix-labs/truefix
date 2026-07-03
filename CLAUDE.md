<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/006-audit-remediation/plan.md`

Active feature: **006-audit-remediation** — remediate every gap found by a 2026-07-02/03
pre-production audit against QuickFIX/J + QuickFIX/Go, recorded in `docs/todo/003.md` (five
independent deep-dive reviews — session/state-machine, transport+config, codec/dictionary, store/log,
Engine facade+AT harness — plus a supplementary Chinese-language pass `B1`-`B30`). Built on
**001-fix-engine-parity** (layered cargo workspace) through **005-engine-gap-remediation** (fixed
002.md's 4 bugs + 4 session-state-machine gaps + dictionary/field-type-completeness tail, 373/373 AT
scenario-runs). 006 fixes 6 fresh P0 bugs (`BUG-05`/`BUG-06` session anti-replay+desync holes,
`BUG-07` SubID/LocationID/Qualifier routing break, `BUG-08`/`BUG-09` silent store failures + spurious
mid-window resets, `BUG-11` orphaned-task leak), 6 P1 bugs (`BUG-10` JDBC grammar, `BUG-12`-`BUG-17`),
5 "closed-but-incomplete" re-openings from 005 (`GAP-26`/`GAP-27` dormant dictionary features,
`GAP-28`/`GAP-32` orphaned version-meta, `GAP-33`/`GAP-56` stale provenance, `GAP-39`/`GAP-49`
FileStore atomicity gap), 12 carried-forward feature-completeness gaps, 11 P2 minor bugs/tooling gaps,
2 AT-harness follow-ups, and 12 supplementary Chinese-pass findings. Spec:
`specs/006-audit-remediation/spec.md` (10 US, 46 FR, 5 clarifications resolved). Governing
constitution: `.specify/memory/constitution.md` (v2.0.0). One pass, internally staged R1-R10 (one
stage per user story) with per-stage checkpoints. **Like 005, this feature touches
session-state-machine/codec/protocol behavior** — the AT suite (373/373 at plan time, confirmed live)
is expected to *grow* via US1's 8 new protocol-correctness scenario families, not merely stay
unmodified; `BUG-17`'s regression-floor bump (353→373) happens first as its own trivial sub-task,
then again after US1 lands. Three scope-defining decisions were resolved via `/speckit-clarify`:
`jdbc:h2:` is removed from the recognized-scheme table rather than gaining a real H2 backend (no new
dependency); a `SessionQualifier`-distinguished session must bind its own distinct listener/template;
a grouped-acceptor's conflicting `ContinueInitializationOnError` values resolve by
strictest-member-wins. No new external dependencies except a possible `time-tz` addition for US8's
IANA-timezone gap (evaluated at implementation time per Principle III). One disclosed public-API
surface growth: `MssqlStore::parse_url` gains a second accepted grammar (real QuickFIX/J
semicolon-delimited MSSQL JDBC URLs, additive to the existing path-based form) — see plan.md's
Technical Context/Complexity Tracking for the full disclosure. (001-005 spec/plan remain the baseline
that 006's FR-IDs reference.)
<!-- SPECKIT END -->
