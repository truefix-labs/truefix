# Contract: Store, Log, and Transport Backends

**Requirements**: FR-001, FR-004, FR-006, FR-009, FR-010, FR-013, FR-015, FR-020, FR-021 (US1);
FR-027–FR-031, FR-033, FR-044–FR-046, FR-051 (US2); FR-070, FR-076–FR-078, FR-082 (US3)
**Research**: research.md §R1, §R2, §R3 | **Data model**: `data-model.md` (Mongo store, TLS trust
store, MSSQL opt-in, async log lifecycle, acceptor-group ordering)

## Mongo sequence persistence (FR-001, `NEW-54`)

**Contract**: `set_next_sender_seq`/`set_next_target_seq` persist to the actual `sender`/`target`
document field; a value set is the value read back after a restart.

**Protocol-behavioral**: no (backend-internal durability bug, not wire behavior) — but is the single
highest-severity item in this feature (total functional failure of the Mongo backend). Table-driven
unit test against a real (or `mongodb`-crate in-process test) Mongo instance is the primary vehicle,
gated behind the `mongodb` Cargo feature.

## Multi-session acceptor pre-Logon DoS (FR-006, `NEW-93`)

**Contract**: `route_and_run`'s pre-Logon read loop is bounded by a timeout (reusing
`logon_timeout`) and never grows `buf` past a fixed bound before a complete Logon frame is observed.

**Protocol-behavioral**: yes (a real, unauthenticated denial-of-service vector) — but AT harness has
no concurrent-connection-flood primitive today; primary test vehicle is a `truefix-transport`
integration test (open N connections, send nothing/trickle non-SOH bytes, assert bounded time +
memory). Confirm at `/speckit-tasks` whether an AT scenario is feasible or this stays
integration-test-only.

## Mongo atomic `save_and_advance_sender` (FR-004, `NEW-08`)

**Contract**: `MongoStore::save_and_advance_sender` performs the message save and sequence advance
within one Mongo transaction — matching `SqlStore`/`MssqlStore`/`RedbStore`/`FileStore`'s existing
atomicity guarantee.

**Protocol-behavioral**: no — backend durability parity. Unit test with an injected mid-transaction
failure (or equivalent) proving no partial-write state, gated behind `mongodb` feature.

## TLS default trust store (FR-010, `NEW-11`; see data-model.md's `native_root_store`)

**Contract**: A TLS client with no `SocketTrustStore`/`SocketTrustStoreBytes` configured falls back
to the OS-native trust store (`rustls-native-certs`) instead of an empty `RootCertStore`; an
explicit trust store, when configured, still takes precedence.

**Protocol-behavioral**: no (transport-security configuration, not wire protocol) — integration test
connecting to a server with a publicly-trusted-CA-signed cert (or a local CA injected into the OS
store in a controlled test environment) proving the handshake now succeeds without explicit
configuration.

## MSSQL log URL parsing parity (FR-013, `NEW-09`)

**Contract**: `truefix-log::mssql::parse_url` supports the semicolon-form connection string and
percent-decoded credentials, matching `truefix-store::mssql::parse_url`.

**Protocol-behavioral**: no. Table-driven unit test mirroring the store crate's existing
`parse_url`/`parse_semicolon_form` test table.

## PROXY protocol header capture (FR-015, `NEW-12`, `NEW-13`)

**Contract**: `peek_proxy_header` captures PROXY v2 headers up to the spec's 64 KiB maximum and
continues reading until the full header is available or the outer 5s timeout fires, rather than
giving up after a fixed 4096-byte buffer / 10-iteration cap.

**Protocol-behavioral**: no (proxy-protocol parsing, not FIX wire protocol). Unit/integration test
feeding a header that arrives across multiple slow segments and one exceeding 4096 bytes.

## MSSQL trust-certificate opt-in (FR-020, `NEW-90`; see data-model.md)

**Contract**: MSSQL store/log connections gain a `trust_server_certificate`-style option, defaulting
to today's `trust_cert()` (unvalidated) behavior; disabling it enables real TLS certificate-chain
validation.

**Protocol-behavioral**: no. Config-resolution unit test (option parses and threads through) plus a
connection-level integration test if a test MSSQL instance with a real cert is available (otherwise
unit-test the config plumbing only, per `docs/todo/005.md`'s own note that no MSSQL reference
implementation exists to compare against).

## `LogMessageWhenSessionNotFound` wiring (FR-021, `NEW-96`; see data-model.md's `ResolvedSession`)

**Contract**: The `.cfg` value is parsed into `ResolvedSession` and threaded into every `Services`
construction site — an operator setting it to a non-default value sees an observable diagnostic
effect.

**Protocol-behavioral**: no. Config-resolution unit test plus a `truefix-transport` integration test
confirming the log message actually appears/is suppressed per the setting.

## Narrower-blast-radius store/transport items (US2/US3)

| FR | `NEW-ID` | Contract | Test type |
|---|---|---|---|
| FR-027 | 23 | `CachedFileStore::get` inserts a cache-miss result into the cache (read-through) | Unit |
| FR-028 | 24 | Async log backends use a bounded channel with backpressure, not unbounded | Unit/integration (slow-consumer simulation) |
| FR-029 | 25 | `CipherSuites` parsing errors if *any* listed name fails to match, not only if all fail | Unit |
| FR-030 | 26 | Initiator connection targets are DNS-resolved at connect time, not only at config-load | Integration (mock DNS or resolver injection) |
| FR-031 | 27 | `Engine::start` hostname resolution uses non-blocking async DNS | Integration |
| FR-033 | 33 | `AcceptorBuilder::bind` honors configured `SocketReuseAddress=N` | Integration (bind behavior) |
| FR-044 | 75 | Mongo `sessions` collection prevents duplicate rows per `session_id` under concurrent connects | Integration, gated behind `mongodb` |
| FR-045 | 76 | `BodyLog::reset` acquires the index lock before truncating | Unit (lock-ordering assertion) |
| FR-046 | 77 | Unparseable (but present) creation-time file surfaces a typed error | Unit |
| FR-051 | 94 | A fully-parsed PROXY header with `Unknown`/`Unspecified`/Unix address still consumes its byte length | Unit |
| FR-070 | 48 | `Monitor` removes a connection's entry on disconnect | Unit/integration |
| FR-076 | 91 | `Engine::shutdown()`/`Drop` flush/await outstanding async log-writer entries (see data-model.md) | Integration |
| FR-077 | 92 | `Engine::start` processes acceptor groups in a stable, sorted order | Unit (deterministic-order assertion across repeated runs) |
| FR-078 | 95 | TLS trust-store loading tracks/surfaces per-cert load failures | Unit (malformed-cert-in-store fixture) |
| FR-082 | 41, 42 | `BodyLog::read` reuses a file handle across a resend range; `redb` `reset()` avoids collect-then-remove | Unit (allocation/syscall-count assertion where feasible, else behavior-preserving refactor + existing tests) |

None of the items in this table are wire-protocol-behavior corrections (backend durability,
resource-hygiene, and diagnostic-quality fixes) — no new AT scenario is expected for this group;
existing unit/integration test patterns from features 004-007 apply directly.
