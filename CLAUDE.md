<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/003-qfj-full-parity-closure/plan.md`

Active feature: **003-qfj-full-parity-closure** — close all 14 remaining QuickFIX/J-parity gaps from a
2026-07-01 audit of `docs/todo-gap-analysis.md` (post-002), covering all three priority tiers (P0+P1+P2)
plus two suitable QuickFIX/J-only extras. Built on **001-fix-engine-parity** (layered cargo workspace) and
**002-qfj-parity-completion** (config-driven start, session-owned resend infra, group parsing, typed
callback outcomes, all-message codegen, TLS-from-config, metrics, SQL PG/MySQL/SQLite). 003 adds:
session-owned durable resend completion, field-order + extra validation toggles, 12 remaining session
config switches, dictionary `component` model, runtime dictionary loading/extend, Data/UtcDateOnly/
UtcTimeOnly field types, FIX Latest support (via FIX Orchestra), extended application hooks
(logon predicate docs, pre-reset hook, `SessionStatus`), AT scenario coverage to 73 scenarios + 3
special suites across all versions, session benchmark, network hardening (PROXY protocol w/
trusted-upstream, SOCKS4/5+HTTP CONNECT, inline PEM, cipher suites, sync writes), a `truefix-dict` CLI,
inbound backpressure (admin/application dual-channel), and MSSQL (+ deferred-pending-license-review
Oracle) SQL backends. Spec: `specs/003-qfj-full-parity-closure/spec.md` (14 US, 21 FR, 5-question
Clarifications session). Governing constitution: `.specify/memory/constitution.md` (v2.0.0). One pass,
internally staged G1–G14 (AT coverage split across G1a/G1b) with per-stage checkpoints; AT suite is the
release gate. Unlike 002, this feature introduces **no breaking API changes**. (001/002 spec/plan remain
the baseline that 003's FR-IDs reference.)
<!-- SPECKIT END -->
