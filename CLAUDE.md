<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/005-engine-gap-remediation/plan.md`

Active feature: **005-engine-gap-remediation** — remediate every gap found by a 2026-07-02 full-code
audit against QuickFIX/J + QuickFIX/Go, recorded in `docs/todo/002.md` (which superseded
two untracked scratch analyses and a full sweep of every `Recognized`-stance `.cfg` key). Built on
**001-fix-engine-parity** (layered cargo workspace), **002-qfj-parity-completion** (config-driven
start, session-owned resend infra, group parsing, typed callback outcomes, all-message codegen,
TLS-from-config, metrics, SQL PG/MySQL/SQLite), **003-qfj-full-parity-closure** (full QuickFIX/J
parity: durable resend completion, validation toggles, remaining session switches, dictionary
components + runtime loading, field types, FIX Latest, extended app hooks, 353/353-scenario AT suite
across 9 versions, session benchmark, network hardening, `truefix-dict` CLI, inbound backpressure,
MSSQL backend), and **004-engine-wiring-extra-backends** (initiator failover wired into
`Engine::start`, `.cfg`-only dictionary/validator + SQL-backend-selection wiring,
`ContinueInitializationOnError`, `RedbStore`/`RedbLog`, `MongoStore`/`MongoLog`). 005 fixes 4 real bugs
(`#`-in-config-value truncation, a `Signature`/`SignatureLength` decode mapping, an acceptor-key
registry that overstated `.cfg` coverage now backed by real `AcceptorBuilder` wiring, and a `JdbcURL`
scheme mismatch that defeated QuickFIX/J config drop-in-compatibility), closes 4
session-state-machine protocol-correctness gaps (resend veto + `GapFill` substitution — note: the veto
point, `Application::to_app`, already existed; only the gap-fill substitution was missing — PossDup
anti-replay, duplicate-Logon rejection, inbound chunked-resend auto-continuation), and completes a long
tail of session/transport/store-log/dictionary-codec feature-completeness gaps — the largest being full
parity with QuickFIX/J's bundled dictionary field/message coverage across all 9 targeted versions. Spec:
`specs/005-engine-gap-remediation/spec.md` (9 US, 31 FR, 3 clarifications resolved). Governing
constitution: `.specify/memory/constitution.md` (v2.0.0). One pass, internally staged G1–G9 with
per-stage checkpoints. **Unlike 004, this feature touches session-state-machine/codec/protocol
behavior** — the 353/353-scenario AT suite is expected to *grow*, not merely stay unmodified, and its
regression-floor test's threshold is expected to require one deliberate, disclosed bump. No new
external dependencies. Several existing public types grow additive fields/variants (`SessionConfig`,
`Action`, `MessageStore`, `StoreConfig::Sql`/`Mssql`, `FieldType`/`FieldDef`/`GroupDef`/`MessageDef`/
`DataDictionary`) — see plan.md's Complexity Tracking for the full disclosure. (001/002/003/004
spec/plan remain the baseline that 005's FR-IDs reference.)
<!-- SPECKIT END -->
