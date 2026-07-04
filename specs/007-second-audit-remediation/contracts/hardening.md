# Contract: Hardening — Narrower-Impact and Low-Priority Items (US2, US3)

**Requirements**: FR-021–FR-022, FR-030–FR-050
**Research**: research.md §R2, §R3

Each item below is a targeted, independently-testable fix; grouped here by sub-theme rather than one
contract per FR (matching this feature's own spec.md grouping). None is expected to require a new AT
scenario — every one is either a unit-testable parsing/arithmetic fix, a config-mapping fix, or a
narrow-condition transport/store fix reachable by a two-process integration test at most.

## Transport/connection hardening (US2)

- **FR-021**: socket options applied identically across every initiator connection path (plain, TLS,
  failover, plain-reconnecting) — currently the plain-reconnecting path alone skips this.
- **FR-022**: acceptor-group per-session log/dictionary-validator/socket-options/TLS resolution
  (currently silently inherited from `members.first()`/the first TLS-configured member).
- **FR-030**: MSSQL URL credential parsing handles a password containing `@` (split on the *last*
  `@` before the host, or an equivalent unambiguous grammar — the exact approach is a
  `/speckit-tasks`-time implementation decision, not a contract-level one).
- **FR-042**: multi-endpoint rotation resets to the primary endpoint after any successful connect.
- **FR-043**: `ReconnectHandle::stop()` tears down a currently-active connection, not only future
  attempts.
- **FR-044**: acceptor refuses a connection whose first message isn't a Logon.
- **FR-045**: the transport's internal admin-message channel applies bounded backpressure.

## Store hardening (US2)

- **FR-031**: `FileStore`/`CachedFileStore` open-time memory is bounded independent of total message
  history size — the offset-index rebuild (and, for `CachedFileStore`, cache warming) must not read
  every stored message body into memory just to open.

## Codec/parsing hardening (US2, US3)

- **FR-020** (also listed in `session-protocol.md`): length-prefixed data-field verification.
- **FR-032**: frame-length/checksum arithmetic doesn't silently wrap (defense in depth — `FR-014`'s
  `MAX_BODY_LEN` bound already makes the practical overflow path unreachable, per research.md
  §R1.11's confirmation).
- **FR-046**: fractional-seconds digit-count rejection (0/3/6/9 only) + leap-second (`ss=60`)
  mapping.
- **FR-047**: checksum-position verification narrowing frame acceptance.
- **FR-048**: `BeginString` format verification (`FIX.\d\.\d`/`FIXT.\d\.\d` pattern, not just a
  leading `8=`).
- **FR-049**: rejection of a decoded message with no `MsgType`.

## Session/config parsing leniency (US2, US3)

- **FR-033**: `HeartBtIntTimeoutMultiplier` fractional-precision preservation + overflow-safe
  timeout arithmetic.
- **FR-034**: recursive group required-field/type validation.
- **FR-035**: `CHAR` multi-character acceptance on FIX 4.0/4.1.
- **FR-036**: case-insensitive `TimeStampPrecision`; strict Y/N rejection for `.cfg` boolean
  switches (including `SocketUseSSL`).
- **FR-037**: resent-message `SendingTime` uses configured timestamp precision.
- **FR-038**: `HeartBtInt` clamping instead of silent truncation.
- **FR-039**: `ResetOnError` honored on the low-sequence-without-PossDup rejection path.
- **FR-040**: heartbeats continue during `AwaitingLogout` before its own timeout.
- **FR-041**: `EndSeqNo=999999` treated as infinity on FIX ≤ 4.2 (in addition to `EndSeqNo=0`).

## Codegen hardening (US3, `truefix-dict`, `dict-tooling` feature only)

- **FR-050**: codegen produces compiling, behaviorally-consistent output for field types outside its
  explicit type table, Rust-keyword-colliding enum labels, and the `open` enum modifier — matching
  the runtime (non-codegen) dictionary path's existing correct handling of each case. No runtime
  `DataDictionary` behavior changes; this is purely a `codegen.rs` robustness fix (Constitution
  Principle IV note: not a new dual-track divergence, since the *runtime* path is already correct —
  this brings codegen's output up to parity with it).
