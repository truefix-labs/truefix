# Contracts: QuickFIX/J Parity Completion

Behaviour contracts for the public surfaces this feature changes or adds. These are black-box contracts
(inputs → observable outputs/effects), not implementation. They extend — and where noted, supersede —
the feature-001 contracts.

| Contract | Covers (US / FR) | Status vs 001 |
|----------|------------------|---------------|
| [config-mapping.md](./config-mapping.md) | `.cfg` → engine, typed errors, one-shot start (US1; FR-013/014/015) | New |
| [application-api.md](./application-api.md) | Typed callback outcomes Reject/DoNotSend/BusinessReject (US5; FR-016) | **Supersedes** 001 application-api |
| [typed-codegen.md](./typed-codegen.md) | All-message typed structs + MessageCracker (US6; FR-020/021/022) | Extends 001 dictionary |
| [group-parsing-validation.md](./group-parsing-validation.md) | Dictionary-driven group decode + validation (US3; FR-004/005) | Extends 001 core-codec/dictionary |
| [validation-integrity.md](./validation-integrity.md) | Integrity checks, two reject layers, garbled, reverse route, precision, switches (US4/US11; FR-006–011) | Extends 001 session |
| [operational.md](./operational.md) | TLS/mTLS, socket/failover, schedule+weekly, metrics, storage/log (US7–US12; FR-017–019, 023–026) | Extends 001 transport/store-log |

**Verification**: every correctness contract is gated by an AT scenario or session-level test (FR-012);
config/usability contracts by two-process integration tests. The full workspace gate (fmt / clippy
`-D warnings` / tests / AT) stays green at each stage exit (SC-013).
