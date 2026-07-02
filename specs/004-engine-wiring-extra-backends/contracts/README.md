# Contracts Index — Feature 004

Each file documents a changed or new public-surface contract this feature introduces, grouped by theme.
Every addition is additive — see each file's "No breaking changes" note for how the one genuinely
tricky case (US1's handle-type mismatch) stays additive rather than changing an existing public type.

| File | Covers (User Stories) | Theme |
|------|------------------------|-------|
| [engine-cfg-wiring.md](./engine-cfg-wiring.md) | US1, US2, US3, US4 | Initiator failover, dictionary/validator, and SQL-backend selection reachable purely from `.cfg`; multi-session startup fault tolerance |
| [additional-backends.md](./additional-backends.md) | US5, US6 | `RedbStore`/`RedbLog` (embedded transactional store) and `MongoStore`/`MongoLog` |
