# Parity Traceability Matrix

Maps the spec parity baselines to where they are realized and verified (T096; parity.md
CHK033/CHK036). This is the human-readable companion to the machine-checked registries
(`truefix_config::APPENDIX_A_KEYS`, the AT scenario list).

## Appendix A — configuration keys → owning crate

Every key is enumerated with a stance in `crates/truefix-config/src/keys.rs`
(`APPENDIX_A_KEYS`), and `tests/key_coverage.rs` asserts none is silently unrecognized (SC-004).

| Appendix A group | Owning crate(s) | Notes |
|------------------|-----------------|-------|
| identity, session behavior, validation toggles | `truefix-session`, `truefix-dict` | session config + dictionary validation |
| scheduling (StartTime/EndTime/Weekdays/TimeZone/NonStop) | `truefix-session` (`schedule`) | `Schedule::is_in_session` |
| acceptor / dynamic, initiator, socket, SSL/TLS | `truefix-transport` | routing, dynamic sessions, allow-list, rustls, `SocketOptions` |
| proxy | — | documented unsupported (not implemented) |
| file store, SQL store | `truefix-store` | SQL behind the `sql` feature |
| file/screen/facade log | `truefix-log` | tracing facade; SLF4J-specific keys documented unsupported |
| Sleepycat/JE | — | documented unsupported (deferred v1) |

Stances: **Implemented** (honored), **Recognized** (parsed; behavior partial/pending), or
**Unsupported(reason)**. See the registry for per-key detail.

## Appendix B — AT scenarios → fixtures

The AT runner (`truefix-at`) drives a real acceptor as a black box. Authored scenarios live in
`crates/truefix-at/src/scenarios.rs`; the conformance test (`tests/conformance.rs`) runs the matrix
across the target versions and is the CI gate.

| Scenario (behaviour) | Versions | Status |
|----------------------|----------|--------|
| 1a ValidLogonWithCorrectMsgSeqNum | 4.2, 4.4 | ✅ |
| 1a ValidLogonMsgSeqNumTooHigh (→ ResendRequest) | 4.2, 4.4 | ✅ |
| 2b MsgSeqNumTooHigh (→ ResendRequest) | 4.2, 4.4 | ✅ |
| 2c MsgSeqNumTooLow (→ Logout + disconnect) | 4.2, 4.4 | ✅ |
| 4b ReceivedTestRequest (→ Heartbeat echo) | 4.2, 4.4 | ✅ |
| 13b UnsolicitedLogoutMessage (→ Logout + disconnect) | 4.2, 4.4 | ✅ |
| Remaining 67 server scenarios + special-category suites | all | ⏳ to port onto the runner |

## Codec reference vectors

`crates/truefix-core/tests/fixtures/reference/` holds QuickFIX/J-sourced wire vectors,
cross-validated in `tests/reference_vectors.rs` (byte-exact BodyLength/CheckSum).

## Feature 002 — dependency license audit (T003, Constitution III)

New/newly-exercised dependencies for the parity-completion work, verified Apache-2.0 OR
MIT-compatible (no copyleft introduced); enforced in CI by the `deny` job (`deny.toml`):

| Dependency | Use (002) | License |
|------------|-----------|---------|
| `rustls-pemfile` | load TLS key/cert/CA from `.cfg` paths (US7) | MIT OR Apache-2.0 |
| `sqlx-postgres` | SQL store/log Postgres backend (US12) | MIT OR Apache-2.0 |
| `sqlx-mysql` | SQL store/log MySQL backend (US12) | MIT OR Apache-2.0 |
| `metrics` | observability facade export (US9) | MIT |

All within the existing Apache-2.0 OR MIT release posture; `cargo deny check` gates regressions.

## Feature 002 — US6: all-message typed codegen + MessageCracker (Principle IV complete)

`build.rs` now generates, per bundled dictionary version (`fix40`..`fix50sp2`, `fixt11`), from the
same normalized source the runtime parses:

- **Typed message structs** (`truefix_dict::fix44::NewOrderSingle`, etc.) — thin wrappers over the
  generic `Message`, so encode/decode is byte-identical with the generic codec path by construction
  (no separate wire representation to keep in sync).
- **Named field accessors** (`.symbol()`/`.set_symbol()`) typed per the field's dictionary type
  (`i64`/`rust_decimal::Decimal`/`char`/`bool`/`time::OffsetDateTime`/`&str`).
- **Field-value enums** (`Side::Buy`, `OrdType::Market`, ...) for fields carrying labeled enum
  values (the normalized `.fixdict` format gained an optional `Value=Label` enum-token suffix,
  authored from the public FIX specification's standard enum tables — protocol facts, not copied
  source, per Constitution Principle III).
- **Typed repeating-group entry structs** (`NoPartyIDsEntry`, with nested `NoPartySubIDsEntry`),
  generated recursively from the dictionary's `group` directives.
- **A `crack_<version>` dispatcher** + `<Version>MessageHandler` trait (e.g. `crack_fix44`,
  `FIX44MessageHandler`) routing an inbound message to its typed handler method by
  `(BeginString, MsgType)` — completing `truefix_core::MessageCracker`'s contract (FR-020/022).

This completes the previously partially-met **Constitution Principle IV (dual-track data
dictionary)**: build-time codegen now produces genuinely strongly-typed messages, not only MsgType
constants, alongside the unchanged runtime `DataDictionary` validator — both still provably from one
source (`dual_track.rs`).

A real bug was caught while wiring the UTCTIMESTAMP field type: `time::OffsetDateTime`'s own
`Display` output is not the FIX wire format. Fixed via a dedicated `Field::utc_timestamp`
constructor (millisecond precision) used by the generated setters instead of `.to_string()`.

## Feature 002 — US10: full socket options + multi-endpoint failover (FR-019)

`truefix_transport::SocketOptions` now covers the full Appendix A socket-tuning surface —
`SocketKeepAlive`/`SocketReuseAddress`/`SocketLinger`/`SocketOobInline`/
`SocketReceiveBufferSize`/`SocketSendBufferSize`/`SocketTrafficClass` alongside the pre-existing
`SocketTcpNoDelay` — applied via `socket2::SockRef` best-effort against a live connection, and
`SO_REUSEADDR` applied at bind time (`bind_listener_with_options`) since `tokio::net::TcpListener`
cannot express it directly. `connect_initiator_reconnecting_multi` rotates round-robin through a
`Vec<SocketAddr>` on every (re)connect attempt, so a dead primary endpoint fails over to a
configured backup instead of retrying the same dead address forever.

`truefix-config` gained a data-only `SocketOptionsSpec` (mirroring `SocketOptions`'s fields, kept
in `truefix-config` rather than `truefix-transport` since `SocketOptions::apply()` is an inherent
impl that must live alongside its `socket2`-consuming type) plus `failover_addresses: Vec<SocketAddr>`
on `ResolvedSession`, parsed from numbered `SocketConnectHost<N>`/`SocketConnectPort<N>` keys
(N=1,2,... contiguous from 1; a gap stops enumeration; a host without its matching port, or vice
versa, is a typed `ConfigError`). `Engine::start` converts `SocketOptionsSpec` into
`transport::SocketOptions` and threads it through `Services` for both acceptor and initiator
sessions, so socket options are fully config-driven end-to-end.

**Scope boundary (documented, not a regression)**: `Engine::start`'s initiator path still uses the
one-shot `connect_initiator_with`/`connect_initiator_tls` (returning `SessionHandle`, which supports
`.logout()`/`.send()`) rather than `connect_initiator_reconnecting_multi` (returning the more
limited `ReconnectHandle`, supporting only `.stop()`/`.join()`); unifying them would require
extending `ReconnectHandle` to proxy send/logout to whichever endpoint is currently active, which is
a real API design task of its own. `failover_addresses` is therefore fully parsed and tested at the
config layer, and the rotation mechanism is fully implemented and tested at the transport layer
(`crates/truefix-transport/tests/failover.rs`), but `Engine::start` does not yet wire the two
together — no different from `Engine::start` not doing single-endpoint auto-reconnect today either.
