# Phase 0 Research: TrueFix

This document resolves the technical-context decisions and the protocol-behavior questions that the
design depends on. Protocol decisions cite the FIX specification or reference-implementation **behavior**
(observed, not copied) per Constitution Principle II.

## R1. Async runtime & concurrency model

- **Decision**: tokio, async-first public API; one runtime drives all sessions. Each session owns an
  async task pair (read loop + write/timer loop) communicating via channels; the Application callback
  surface is `async`.
- **Rationale**: First-class acceptor support implies many concurrent sessions on one process; tokio's
  task model scales to large socket fan-out far better than thread-per-session. Confirmed by clarify
  (async-first) and FR-F7.
- **Alternatives**: thread-per-session (QuickFIX/J MINA-style) — simpler callbacks but poor scaling and
  un-idiomatic in Rust; `async-std`/`smol` — smaller ecosystems, weaker TLS/metrics integration.
- **"Single/multi-threaded model" parity**: QuickFIX/J's single- vs multi-threaded `SocketAcceptor`
  distinction is mapped to tokio runtime flavors (current-thread vs multi-thread) + per-session task
  isolation, preserving the *operational* semantics without copying the threading implementation.

## R2. Decimal & numeric precision

- **Decision**: `rust_decimal::Decimal` for Price/Qty/Amt/Percentage FIX types; integers for int-like
  tags; never f64 for monetary/price values.
- **Rationale**: FIX values are decimal strings; binary floats lose precision and break byte-exact
  round-trip (SC-002). rust_decimal preserves scale and round-trips text faithfully.
- **Alternatives**: fixed-point newtype (more work, marginal gain); `bigdecimal` (heavier, less ergonomic).
- **Round-trip note**: the codec preserves the original textual representation where FIX permits multiple
  encodings, to guarantee byte-identical re-encode; canonicalization happens only on engine-generated
  fields.

## R3. Timestamps (nanosecond, parse ≤ picosecond)

- **Decision**: `time::OffsetDateTime`/`PrimitiveDateTime` with a custom UTCTimestamp parser/formatter
  storing nanosecond precision; accept input with up to picosecond digits and truncate to ns on store.
- **Rationale**: Matches FR-B7 and the timestamps AT suite (QFJ873/QFJ921). `time` has explicit
  nanosecond support and no implicit tz surprises.
- **Alternatives**: `chrono` (viable; tz handling heavier), `jiff` (newer, smaller adoption). Either
  acceptable; `time` chosen for minimal surface and no-panic parsing APIs.
- **Behavior cite**: FIX UTCTimestamp format `YYYYMMDD-HH:MM:SS(.sss...)`; sub-second fractional digits
  vary by version/config (`TimeStampPrecision`).

## R4. Dual-track data dictionary (Constitution IV — must argue coexistence)

- **Decision**: One **normalized dictionary** (TrueFix's own format) is the single source of truth.
  `truefix-dict/build.rs` consumes it to **codegen** strongly-typed message/field structs; the same
  normalized data is also embedded/loaded at runtime to construct a **`DataDictionary`** validator.
- **How they coexist (not either/or)**:
  - *Codegen track* gives compile-time ergonomics & type safety for integrators who use typed messages
    (e.g. `NewOrderSingle` struct) — mirrors QuickFIX/J's generated `quickfix.fix44.*` classes.
  - *Runtime track* gives version-agnostic validation, custom/extended dictionaries loaded at runtime,
    and UDF support — mirrors QuickFIX/J's `DataDictionary`/`validate()`.
  - Both derive from the same source, so a generated struct and the runtime rules cannot disagree
    (a build check asserts codegen output and runtime model are generated from the same dictionary
    version/hash). This is exactly QuickFIX/J's actual arrangement (generated messages + runtime
    DataDictionary), satisfying Principle IV.
- **Source provenance**: dictionaries derived from FIX Trading Community **Orchestra/Repository**
  machine-readable specs, transformed into the normalized format. QuickFIX/J's `FIXxx.xml` files are
  **not** copied (Constitution III). research validates Orchestra coverage for 4.0–5.0SP2 + FIXT 1.1.
- **Alternatives**: runtime-only (loses type ergonomics, breaks parity intent); codegen-only (loses
  runtime custom-dict/UDF flexibility). Both rejected by Principle IV.

## R5. Session state machine & protocol behavior anchors

- **Decision**: Model explicit states (Disconnected, LogonSent/LogonReceived (initiator/acceptor),
  LoggedOn, LogoutSent/LogoutReceived, Reconnecting) with transitions driven by inbound admin messages,
  timers, and operator actions. Sequence handling, resend, and gap-fill are part of this crate.
- **Behavior anchors (cited in code docs, observed not copied)**:
  - MsgSeqNum too high → accept logon, then issue ResendRequest, queue subsequent in-order (AT 1a, 2b).
  - MsgSeqNum too low w/o PossDup → log + disconnect (AT 2c).
  - ResendRequest reply: app messages resent PossDupFlag=Y + OrigSendingTime; admin messages collapse to
    SequenceReset-GapFill (AT 19a/19b, FR-D7/D9).
  - `ResendRequestChunkSize` chunks the resend range; 0 = single unbounded request (FR-D7).
  - `NextExpectedMsgSeqNum` (789) logon synchronization for FIX ≥ 4.4 (nextExpectedMsgSeqNum AT suite).
  - Heartbeat at HeartBtInt; TestRequest after ~1.2× idle; multipliers configurable (FR-D6).
- **Rationale**: These are the bulk of the AT gate; encoding them as explicit, table-tested transitions
  makes conformance auditable.

## R6. TLS

- **Decision**: rustls via tokio-rustls for both Initiator and Acceptor; PEM key/cert loading via
  rustls-pemfile; map Appendix A SSL keys (protocols, cipher suites, client auth, SNI, trust/key stores).
- **Rationale**: Pure-Rust, no OpenSSL build dependency, Apache/MIT-compatible, modern defaults.
- **Parity note**: QuickFIX/J Java keystore (JKS) keys are mapped to PEM/DER inputs; JKS-specific keys
  (`KeyStoreType=JKS`) documented with their TrueFix equivalent or unsupported-with-reason (FR-I2).

## R7. SQL store/log backend (JDBC-equivalent)

- **Decision**: sqlx async backend implementing `MessageStore` and `Log`; in v1 scope per clarification.
  Trait-first design means memory/file implementations land before SQL and SQL is additive.
- **Rationale**: sqlx is async-native (fits tokio), compile-time-checked queries optional, supports
  Postgres/MySQL/SQLite. Maps QuickFIX/J JDBC tables (messages, sessions, log tables) to schema.
- **Alternatives**: diesel (sync, blocking — fights async core); raw drivers (more code).

## R8. Config `.cfg` parser

- **Decision**: Hand-written parser for the QuickFIX `[DEFAULT]`/`[SESSION]` dialect with `${name}`
  interpolation and programmatic builder; serde used for typed extraction into settings structs.
- **Rationale**: The dialect is INI-like but not standard TOML/INI (sections repeat, defaults inherited);
  a small dedicated parser is clearer and avoids forcing a mismatched crate.

## R9. Observability (JMX-equivalent)

- **Decision**: `tracing` for structured events/message+event logs; `metrics` facade for session state,
  sequence numbers, connection health gauges/counters; an admin control surface (reset, force logout)
  exposed programmatically (and optionally over a small API later).
- **Rationale**: Idiomatic Rust observability; satisfies Principle I structured observability and SC-007;
  parity-by-capability with JMX (clarified as capability-parity, not literal JMX).

## R10. Dependency license audit (Constitution III)

- **Decision**: Add `cargo-deny` (licenses + bans + advisories) to CI; allowlist = Apache-2.0, MIT,
  BSD-2/3, Unicode-DFS, ISC, Zlib; deny copyleft (GPL/LGPL/MPL into source). Audit all of:
  tokio, rustls, thiserror, rust_decimal, time, serde, sqlx, tracing, metrics + transitive deps.
- **Rationale**: Clean Apache-2.0 OR MIT release requires every dependency be compatible; this is a
  Stage S0 gate. Findings recorded here as the audit runs.

## Open items → resolved

All Technical Context unknowns are resolved above. No remaining NEEDS CLARIFICATION. The earlier
spec-level clarifications (async-first, storage scope, AT version gate, perf non-gate, dictionary
provenance) are reflected in R1/R7/R5/R9-perf/R4 respectively.
