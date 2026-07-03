# Contract: Store/Log Persistence Hardening (US7; FR-017–FR-019)

## Surface

```text
// truefix-store
pub trait MessageStore: Send + Sync {
    // ...existing methods unchanged...
    fn creation_time(&self) -> Option<time::OffsetDateTime> { None }   // NEW, defaulted
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        // NEW, defaulted (sequential save-then-set, same as today's two-call pattern)
    }
}
// SqlStore, MssqlStore, RedbStore override save_and_advance_sender with a single transaction.
// Every backend (File/CachedFile/Sql/Mssql/Redb/Mongo) overrides creation_time() to persist/return
// a real value instead of the trait default.

// truefix-log — schema-only changes, no trait/type signature change:
//   SQL/MSSQL: ensure_table gains `logged_at TIMESTAMP` + `session_id TEXT` columns
//   RedbLog: table value type widens (text-only -> (timestamp, session_id, text))
//   MongoLog: document gains "logged_at"/"session_id" fields
```

## Behaviour

1. **Creation-time persistence (GAP-38/FR-017)**: every backend persists a creation timestamp at first
   `connect`, returned by `creation_time()`. `reset()` updates it to "now" for every backend that has
   one, matching QuickFIX/J/Go's own "creation time updates on reset" semantics.
2. **Atomic save + sequence increment (GAP-39/FR-018)**: `SqlStore`/`MssqlStore`/`RedbStore` override
   `save_and_advance_sender` to commit the message body and the sequence-number increment in one
   transaction — no crash window where one is durable without the other. `MemoryStore`/`NoopStore`/
   `FileStore`/`CachedFileStore` use the trait-level default (already effectively atomic or not
   applicable).
3. **Structured log schema widening (GAP-41/FR-019)**: every structured log backend's persisted row
   carries a timestamp and the owning session's identity, queryable directly from that one row without
   external correlation.

## No breaking changes

- Both new `MessageStore` trait methods are defaulted — any external implementor of this trait
  continues to compile unmodified (matches the existing `was_corrupted()` precedent exactly).
- The log schema widening is additive columns/fields; no existing column/field is removed, and no
  existing `Log`/reader code that only reads `text` needs to change.

## Acceptance (maps to spec US7 scenarios)

- A newly created store's creation timestamp is queryable and updates on reset. ✔
- A SQL-family store's outbound-message save and sequence-number increment are committed atomically. ✔
- Every structured log entry carries a timestamp and session-identity column, queryable without
  external correlation (SC-012). ✔

## Test hooks

- `truefix-store`: `creation_time()`/`save_and_advance_sender()` conformance-pattern tests, added to
  every backend's existing test file (`redb_backend.rs`, `mongo_backend.rs`, `sql_backends.rs`,
  `mssql_backend.rs`) — mirroring how each backend's tests already share one contract-testing pattern.
- `truefix-log`: schema-widening tests asserting `logged_at`/`session_id` are populated correctly,
  added to each backend's existing test file (`sql_log.rs`, `mssql_log.rs`, `redb_log.rs`,
  `mongo_log.rs`).
