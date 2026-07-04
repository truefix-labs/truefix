# Contracts Index — Feature 007

Each file documents a changed or fixed public/internal-behavior contract this feature introduces,
grouped by theme (mirroring spec.md's user-story ordering). Every change is additive or
internal-behavior-only — see each file's own note. Three disclosed public-API surface growths span
this feature (all additive, none breaking): `SessionHandle::abort`/`is_finished`, `impl Drop for
Engine`, and `Event::Disconnected` — see `engine-lifecycle.md` and `session-protocol.md`
respectively. `session-protocol.md` covers the items most likely to require new AT scenarios
(Constitution Principle II) — see each item's own note for whether it's session-*internal* (unit/
integration test sufficient) or genuinely wire-protocol-behavioral (AT scenario required).

| File | Covers (User Stories) | Theme |
|------|------------------------|-------|
| [store-durability.md](./store-durability.md) | US1 | Sequence-number store format/migration, SqlStore migration ordering, MssqlStore commit rollback |
| [session-protocol.md](./session-protocol.md) | US1, US2 | ResetSeqNumFlag handshake, ResendRequest deadlock, duplicate-connection refusal, Event::Disconnected, callback-ordering restructure, admin dictionary validation, schedule enforcement, plus narrower-impact session fixes |
| [engine-lifecycle.md](./engine-lifecycle.md) | US1 | Engine::shutdown/Drop stopping plain initiators, scheduled-initiator reconnect detection + backoff |
| [hardening.md](./hardening.md) | US2, US3 | Frame/checksum bounds, socket options, MSSQL URL parsing, timestamp/CHAR/enum parsing leniency, codegen robustness, and the rest of the narrower-impact and low-priority tiers |
