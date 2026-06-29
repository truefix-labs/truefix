# Contract: truefix-store & truefix-log

Covers FR-G1/G2, FR-H1/H2, SC-006.

## MessageStore (trait)
- **Operations**: get/set next sender seq; get/set next target seq; save sent message for a seq;
  get sent messages in a range (for resend); reset; creation time.
- **Implementations (v1)**: `MemoryStore`, `FileStore`, `CachedFileStore`, `SqlStore` (sqlx), `NoopStore`,
  via `MessageStoreFactory`. `SleepycatStore` deferred-with-reason (FR-G1, Appendix A keys documented).
- **Guarantee**: persisted state is sufficient to satisfy a ResendRequest for messages sent before a
  process restart (SC-006); `ForceResendWhenCorruptedStore` recovery path defined.

## Log (trait)
- **Operations**: `on_incoming(msg)`, `on_outgoing(msg)`, `on_event(text)`, `on_error_event(text)` —
  message vs event streams separated (FR-H2).
- **Implementations (v1)**: `ScreenLog`, `FileLog`, `TracingLog` (tracing facade), `CompositeLog`
  (fan-out), via `LogFactory`. SQL-backed log (sqlx) in v1 scope (JDBC log tables equivalent).

## Acceptance hooks
- Restart-survivable resend integration test (file/cached/SQL stores).
- Composite log fan-out + stream-separation test.
