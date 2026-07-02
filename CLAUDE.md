<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/004-engine-wiring-extra-backends/plan.md`

Active feature: **004-engine-wiring-extra-backends** — close 6 remaining gaps from a 2026-07-02
code-vs-code comparison (`docs/todo-gap-analysis.md`'s GAP-01–GAP-06), none of them protocol-correctness
defects. Built on **001-fix-engine-parity** (layered cargo workspace), **002-qfj-parity-completion**
(config-driven start, session-owned resend infra, group parsing, typed callback outcomes, all-message
codegen, TLS-from-config, metrics, SQL PG/MySQL/SQLite), and **003-qfj-full-parity-closure** (full
QuickFIX/J parity: durable resend completion, validation toggles, remaining session switches,
dictionary components + runtime loading, field types, FIX Latest, extended app hooks, 353/353-scenario
AT suite across 9 versions, session benchmark, network hardening, `truefix-dict` CLI, inbound
backpressure, MSSQL backend). 004 adds: initiator failover wired into `Engine::start` (previously
unused `failover_addresses`, plus a new TLS-aware reconnecting connector), `.cfg`-only dictionary/
validator wiring (`UseDataDictionary`/`ValidationOptions` reaching `Services.validator` with zero Rust
code), `.cfg`-only SQL backend selection via `JdbcURL` (scheme-dispatched, not JDBC-driver-class-based),
`ContinueInitializationOnError` (multi-session startup fault tolerance), and two new optional storage
backends — `RedbStore`/`RedbLog` (embedded transactional KV via `redb`, replacing QuickFIX/J's obsolete
`SleepycatStore`) and `MongoStore`/`MongoLog` (matching QuickFIX/Go's MongoDB option, a deliberate
reversal of 003's own MongoDB deferral). Spec: `specs/004-engine-wiring-extra-backends/spec.md` (6 US,
10 FR, 0-question — no clarifications needed). Governing constitution: `.specify/memory/constitution.md`
(v2.0.0). One pass, internally staged W1–W6 with per-stage checkpoints. This feature touches no
session-state-machine/codec/protocol behavior — **no new AT scenarios**; the existing 353/353-scenario
suite staying green and unmodified is itself the release gate (FR-010). No breaking API changes (one
disclosed, grep-confirmed-non-breaking-in-practice addition of new members to `ResolvedSession`/
`ConfigError`). (001/002/003 spec/plan remain the baseline that 004's FR-IDs reference.)
<!-- SPECKIT END -->
