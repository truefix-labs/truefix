# Contracts Index — Feature 003

Each file documents a changed or new public-surface contract this feature introduces, grouped by theme
(mirroring feature 002's contract grouping). None of these are breaking changes — every addition here is
additive to the existing public API (new methods with no-op defaults, new optional config fields
defaulting to today's behavior, new constructors alongside existing ones).

| File | Covers (User Stories) | Theme |
|------|------------------------|-------|
| [correctness-foundation.md](./correctness-foundation.md) | US1, US2 | AT scenario coverage; session-owned durable resend |
| [validation-completeness.md](./validation-completeness.md) | US3, US4, US8 | Field-order validation; session config switches; extra validation toggles |
| [dictionary-model.md](./dictionary-model.md) | US5, US6, US7, US9 | Components; runtime dictionary loading; field types; FIX Latest |
| [application-hooks.md](./application-hooks.md) | US10 | Extended logon predicate, pre-reset hook, SessionStatus |
| [network-hardening.md](./network-hardening.md) | US12 | PROXY protocol (trusted-upstream), SOCKS4/5, HTTP CONNECT, inline PEM, cipher suites, sync writes |
| [tooling-and-ops.md](./tooling-and-ops.md) | US11, US13, US14 | Benchmark, dictionary CLI, inbound backpressure, MSSQL/Oracle backends |
