# Contracts Index: Third/Fourth-Pass Audit Remediation

Each file below groups this feature's 90 FRs by the subsystem they touch, states the observable
behavior contract for each, and flags which items are genuine protocol-behavior corrections
requiring a new/extended AT scenario per Constitution Principle II (vs. session-internal,
tooling-only, or harness-internal items that need a unit/integration test but not necessarily an AT
scenario). Cross-reference `data-model.md` for the structural shapes several of these contracts
depend on, and `research.md` for the source citations behind each.

- **[session-protocol.md](./session-protocol.md)** — `truefix-session`'s state machine: sequence
  handling, teardown-reason routing, Logon/Logout/ResendRequest/SequenceReset ordering, dictionary
  validation timing. FR-001 through FR-018 (US1), plus the US2 session-layer items.
- **[store-and-transport.md](./store-and-transport.md)** — `truefix-store`, `truefix-log`,
  `truefix-transport`: Mongo/File/SQL/MSSQL/Redb backends, TLS, PROXY protocol, DNS resolution,
  scheduled initiators, acceptor binding/multi-session routing.
- **[fixt-and-dictionary.md](./fixt-and-dictionary.md)** — `truefix-core`, `truefix-dict`,
  `truefix-config`: FIXT 1.1 header/group decoding, field-type/group validation, config-key wiring,
  codegen/converter tooling correctness.
- **[hardening-and-tooling.md](./hardening-and-tooling.md)** — `truefix-at` harness improvements,
  performance/allocation hygiene, diagnostic-quality fixes, config-default corrections.

## AT-scenario impact summary

Per Constitution Principle II, a genuine wire-protocol-behavior correction requires an AT scenario
(new or extended) as its highest-priority acceptance evidence, on top of unit/integration tests. The
current regression floor is **424 scenario runs** (research.md §R3). Items below are the ones this
plan expects to raise that floor (confirmed per-item at `/speckit-tasks` time, not finalized here):

| Contract file | Items likely requiring new/extended AT coverage |
|---|---|
| session-protocol.md | `NEW-02`, `NEW-03`, `NEW-84`, `NEW-56`, `NEW-62`, `NEW-63`, `NEW-85`, `NEW-86`, `NEW-18`, `NEW-34`, `NEW-70`, `NEW-71` |
| store-and-transport.md | `NEW-93` (DoS timeout, integration test — AT harness has no concurrent-connection-flood primitive; confirm at tasks time) |
| fixt-and-dictionary.md | `NEW-55`, `NEW-58`, `NEW-17`, `NEW-19`, `NEW-21`, `NEW-69` |
| hardening-and-tooling.md | `NEW-83` (new fixed-identity scenarios), `NEW-29`, `NEW-30`, `NEW-51` (existing-scenario assertion strengthening, not new runs) |

Everything else is a session-internal, store/transport-internal, dictionary/codegen-internal, or
harness-internal fix verified by table-driven unit tests and/or two-process integration tests,
without necessarily adding a new AT scenario — same triage discipline features 006/007 applied.
