# TrueFix Contracts

These documents define the **behavior contracts** TrueFix exposes — the stable, documented public
surface per crate (Constitution Principle I). They are conceptual API/behavior contracts, not full
signatures; concrete Rust APIs are implemented in `/speckit-implement` and verified by the tests and the
AT suite. Each contract ties back to spec requirements (FR-*) and the parity baselines (Appendix A/B).

| Contract | Crate(s) | Covers |
|----------|----------|--------|
| [core-codec.md](./core-codec.md) | truefix-core | Message model, codec, BodyLength/CheckSum, round-trip, typed errors |
| [dictionary.md](./dictionary.md) | truefix-dict | Normalized dict, runtime DataDictionary, codegen, validation, FIXT split |
| [session.md](./session.md) | truefix-session | State machine, sequence/resend/gap-fill, admin messages, scheduling |
| [store-log.md](./store-log.md) | truefix-store, truefix-log | MessageStore + Log traits and implementations |
| [transport.md](./transport.md) | truefix-transport | Initiator/Acceptor, multi/dynamic sessions, reconnect, TLS, sockets |
| [config-keys.md](./config-keys.md) | truefix-config | SessionSettings, .cfg parsing, ${} interpolation, Appendix A key contract |
| [application-api.md](./application-api.md) | truefix | Application trait, MessageCracker, facade |
| [at-runner.md](./at-runner.md) | truefix-at | AT scenario format, pass criteria, version gate |

**Contract stability rule**: once published, breaking changes to any contract MUST follow semantic
versioning with migration notes (Principle I). Async-first: all integrator callbacks are `async` (FR-F7).
