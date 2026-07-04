# Contract: Store and Transport Correctness/Security (US1, US2)

**Requirements**: FR-001–FR-010 (store/log), FR-044–FR-058 (transport/networking)
**Research**: research.md §R1.1, R1.4, R1.14–R1.15, R1.17–R1.20, §R2/R3

## `MongoStore` sequence persistence (FR-001)

**Contract**: `set_next_sender_seq`/`set_next_target_seq` write to the actual `sender`/`target`
document fields; `next_sender_seq`/`next_target_seq` reflect the new value immediately and after a
process restart.

**Test vehicle**: `truefix-store` integration test (behind the `mongo` feature) — write, drop the
`MongoStore` handle, reconnect, read back. Not wire-protocol-behavioral (store-internal), no AT
scenario.

## `MongoStore` transactional `save_and_advance_sender` (FR-002)

**Contract**: save + sender-seq-advance happen atomically within one MongoDB transaction.

**Test vehicle**: integration test simulating a mid-operation failure (or at minimum verifying both
writes commit/rollback together under the MongoDB driver's transaction API) — mirrors `SqlStore`'s
existing equivalent test.

## `MongoStore` session-row uniqueness (FR-003)

**Contract**: concurrent/duplicate connects for the same `session_id` cannot create duplicate
`sessions` rows.

**Test vehicle**: integration test issuing concurrent `ensure_session_row` calls for one
`session_id`, asserting exactly one row exists afterward.

## Other store/log robustness (US2/US3) — grouped, unit/integration tests

- **FR-004**: `BodyLog::reset` acquires the index lock before truncating, matching `append`'s
  ordering.
- **FR-005**: `FileStore`'s creation-time reader returns a typed error for unparseable content
  instead of defaulting to the Unix epoch.
- **FR-006**: `CachedFileStore::get` populates its cache on a read-through miss.
- **FR-007**: async SQL/MSSQL/Mongo/Redb log backends' write queues are bounded (backpressure or a
  capacity cap), not unbounded channels.
- **FR-008**: `Engine::shutdown()`/`Drop` ensure queued log-backend entries are flushed (or exposed
  as an awaitable drain) before completing — requires retaining/joining each log backend's
  background-writer `JoinHandle`, a `truefix`/`truefix-log` integration test asserting no entries
  are lost across a shutdown with a still-full queue.

None of the above are wire-protocol-behavioral — all are store/log-internal durability/robustness
fixes, verified by targeted unit/integration tests.

## MSSQL log-URL parity (FR-009)

**Contract**: `MssqlLog::parse_url` supports the semicolon connection-string form and
percent-decoded credentials, matching `MssqlStore::parse_url`.

**Test vehicle**: unit tests mirroring the store's existing `parse_semicolon_form`/`percent_decode`
test cases, ported to the log crate.

## MSSQL TLS certificate-validation opt-in (FR-010)

**Contract**: an operator can enable real TLS server-certificate validation for MSSQL store/log
connections via config (defaulting to today's `trust_cert()` behavior for backward compatibility).

**Test vehicle**: config-parsing unit test confirming the new field/URL-property is read and
threaded into `tiberius::Config`; a live-connection test against a cert-validating MSSQL instance is
out of scope for CI (no such instance available) — covered by a unit-level assertion that
`trust_cert()` is *not* called when the option is set to require validation.

## Multi-session acceptor pre-Logon DoS timeout/cap (FR-044)

**Contract**: `route_and_run`'s pre-Logon read loop is bounded by the existing `logon_timeout`
setting and a capped buffer size.

**Test vehicle**: `truefix-transport` integration test — open a connection to a multi-session
`AcceptorBuilder`-served port, send nothing (or non-SOH garbage), assert the connection is closed
within `logon_timeout` and the task/socket do not leak. Not wire-protocol-behavioral in the
AT-scenario sense (a resource-bound/DoS-hardening property), no AT scenario expected.

## PROXY header buffer growth and timeout responsiveness (FR-045, FR-046)

**Contract**: PROXY v2 headers larger than the current fixed peek buffer are handled (buffer
grows/reads incrementally) instead of dropping bytes; the peek loop retries until the outer 5s
timeout fires, not a fixed iteration count.

**Test vehicle**: `truefix-transport` unit/integration tests feeding an oversized v2 header and a
slow-arriving (multi-chunk, delayed) header respectively, both from a trusted proxy IP.

## PROXY `Unknown`/`Unspecified`/Unix header byte consumption (FR-047)

**Contract**: a fully-parsed, spec-legal PROXY header with an unmapped address type still has its
byte length consumed from the stream.

**Test vehicle**: unit test feeding a `PROXY UNKNOWN` v1 header (or a v2 header with
`Unspecified`/`Unix` address) followed by a real FIX message, asserting the FIX message decodes
cleanly (i.e. the header bytes were consumed, not left for the FIX decoder to choke on).

## Scheduled-initiator connect non-blocking (FR-048)

**Contract**: a hanging connect attempt does not block schedule-boundary/stop-flag detection in
`run_scheduled_initiator`.

**Test vehicle**: `truefix-transport` test using a black-holed address (or `tokio::time::pause` +
a never-resolving future, mirroring the file's existing `connect_timeout_tests` pattern) with a
short schedule window, asserting the schedule-exit/stop is observed promptly despite the hung
connect.

## TLS default trust store (FR-049)

**Contract**: TLS client configuration with no explicit trust store falls back to a platform-native
trust store (via `rustls-native-certs`) instead of an empty one.

**Test vehicle**: unit test confirming `build_client_config` with no `SocketTrustStore` configured
produces a non-empty `RootCertStore` (exact cert count is platform-dependent — assert non-empty and
no error, not a specific count).

## Other transport/networking hardening (US2) — grouped, unit/integration tests

- **FR-050**: TLS trust-store loading surfaces failed-certificate counts (log warning or typed
  error on an empty resulting store).
- **FR-051**: `CipherSuites` reports an error for any unrecognized name, even alongside valid ones.
- **FR-052**: reconnecting-initiator address resolution occurs at connect time, not config-load
  time.
- **FR-053**: `Engine::start`'s DNS resolution uses `tokio::net::lookup_host` (async), not blocking
  `std::net::ToSocketAddrs`.
- **FR-054**: `AcceptorBuilder::bind` honors `SocketReuseAddress` instead of hardcoding
  `SO_REUSEADDR=true`.
- **FR-055**: frame-resync does not discard a well-formed frame following a non-SOH-terminated
  garbage prefix.
- **FR-056**: HTTP CONNECT status-token validated as numeric, not just `starts_with('2')`.
- **FR-057**: acceptor `Monitor` map removes an entry on disconnect.
- **FR-058**: `LogMessageWhenSessionNotFound` parsed and threaded into `Services`.

None of the above are wire-protocol-behavioral in the AT-scenario sense — all are
transport/config-internal robustness or resource-bound fixes, verified by targeted unit/integration
tests.
