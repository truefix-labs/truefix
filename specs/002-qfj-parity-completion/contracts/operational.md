# Contract: Operational Completeness — TLS, Socket/Failover, Schedule, Metrics, Storage/Log (US7–US12)

## TLS from config + mTLS (US7; FR-017)

- `ServerConfig`/`ClientConfig` built from `.cfg` key/cert/CA paths (rustls-pemfile), with min TLS
  version, SNI/server-name, and `NeedClientAuth`.
- `NeedClientAuth=Y` ⇒ server verifies a client cert against configured CA roots; missing/invalid ⇒
  handshake refused. A configured min version ⇒ lower-version peer refused.
- **Acceptance**: mTLS session from `.cfg` alone; un-credentialed client refused (SC-008).

## Socket options + multi-endpoint failover (US10; FR-019)

- All configured socket options applied (keep-alive, buffer sizes, reuse-address, linger, etc.).
- `SocketConnectHost<N>`/`SocketConnectPort<N>` form an ordered endpoint set; on reconnect the initiator
  rotates to the next; exhaustion wraps per the reconnect policy without busy-loop/panic.
- **Acceptance**: options applied; initiator fails over to a backup when the primary is unreachable
  (SC-011).

## Schedule reset + weekly windows (US8; FR-018)

- Crossing out of the window ⇒ disconnect → reset sequence numbers → clear store; crossing into the
  window (initiator) ⇒ reconnect → reset. `NonStopSession=Y` ⇒ no scheduled reset.
- Weekly `StartDay`/`EndDay` windows supported in addition to daily times; `is_in_session` correct across
  day boundaries.
- **Acceptance**: boundary reset + reconnect verified; weekly windows correct; non-stop unaffected
  (SC-009).

## Metrics export (US9; FR-023)

- Emitted via the `metrics` facade (no bundled exporter): gauges `truefix_session_state`,
  `truefix_next_sender_seqnum`, `truefix_next_target_seqnum`; counters `truefix_messages_sent_total`,
  `truefix_messages_received_total`, `truefix_reconnects_total`; labelled by SessionID.
- **Acceptance**: signals reflect a logon→traffic→reconnect cycle (SC-010).

## Storage / logging completeness (US12; FR-024/025/026)

- SQL store/log implement + test **PostgreSQL, MySQL, SQLite** (sqlx, `sql` feature); configurable table
  names + pool settings. External-DB tests gate on availability; SQLite always runs.
- `CachedFileStore` maintains an in-memory cache (configurable max) with a `FileStoreSync` fsync toggle;
  caches/flushes per config and survives restart.
- Log implementations honour output switches (heartbeat filtering, millisecond inclusion,
  event/incoming/outgoing visibility, session-ID prefixing).
- **Acceptance**: SQL suite green across the three DBs; cached store caches/flushes + survives restart;
  log switches change output (SC-012).

## Appendix A stance updates (FR-027)

Keys implemented above move `Recognized → Implemented` in `APPENDIX_A_KEYS`; `key_coverage.rs` keeps
SC-004/SC-014 accurate.
