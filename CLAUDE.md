<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:
`specs/002-qfj-parity-completion/plan.md`

Active feature: **002-qfj-parity-completion** — close all remaining QuickFIX/J-parity gaps from
`docs/todo-gap-analysis.md` except QuickFIX/Go-only extras. Built on the completed **001-fix-engine-parity**
foundation (layered cargo workspace truefix-core/dict/session/store/log/transport/config + truefix facade
+ truefix-at; async tokio, rustls, thiserror, rust_decimal, dual-track dictionary). 002 adds: session-owned
persistent resend, dictionary-driven group parsing+validation, `.cfg`→engine mapping & one-shot start,
inbound integrity/reject-layers/reverse-route, typed callback outcomes (breaking change), all-message typed
codegen + MessageCracker (completes Principle IV), TLS/mTLS-from-config, schedule weekly windows, metrics
export (completes Principle I), SQL PG/MySQL/SQLite. Spec: `specs/002-qfj-parity-completion/spec.md`
(12 US, 27 FR). Governing constitution: `.specify/memory/constitution.md` (v2.0.0). One pass, internally
staged G1–G10 with per-stage checkpoints; AT suite is the release gate. (001 spec/plan remain the baseline
that 002's FR-IDs reference.)
<!-- SPECKIT END -->
