# Contracts Index — Feature 008

Each file documents a changed or fixed public/internal-behavior contract this feature introduces,
grouped by theme (mirroring spec.md's user-story ordering). Every change is additive or
internal-behavior-only — no breaking public-API change anywhere in this feature. One disclosed new
external dependency (`rustls-native-certs`) spans `store-and-transport.md`'s TLS section; one
disclosed new internal (non-public) enum shape (`DisconnectReason`) spans `session-protocol.md`'s
teardown section — see data-model.md for both.

| File | Covers (User Stories) | Theme |
|------|------------------------|-------|
| [session-protocol.md](./session-protocol.md) | US1, US2 | `ResetOnLogon` (acceptor), gap-fill verification, teardown reset-reason scoping, schedule-exit Logout, dictionary-invalid-Logon routing, `BeginSeqNo=0`, Logout/ResendRequest too-high handling, plus narrower session fixes |
| [fixt-and-dictionary.md](./fixt-and-dictionary.md) | US1, US2, US3 | FIXT 1.1 header/group-decode, FIX 5.x `ApplVerID` dispatch, validation-key wiring, dictionary/codegen correctness (runtime + tooling tracks) |
| [store-and-transport.md](./store-and-transport.md) | US1, US2 | Mongo persistence/atomicity, MSSQL TLS opt-out, pre-Logon DoS timeout, PROXY header handling, TLS default trust store, scheduled-initiator connect-blocking |
| [hardening-and-tooling.md](./hardening-and-tooling.md) | US3 | Hygiene, dictionary-tooling correctness, AT-harness coverage improvements |

Each contract file's own note states whether the item is genuinely wire-protocol-behavioral (a
Constitution Principle II AT-scenario candidate) or session/store/tooling-internal (unit/integration
test sufficient) — `/speckit-tasks` finalizes the AT-scenario decision per-item, matching 006/007's
precedent.
