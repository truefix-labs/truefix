# Contract: Store And Log Durability

## Scope

Applies to `truefix-store`, `truefix-log`, and any engine shutdown hooks that flush async log
writers.

## Required Behaviors

- Mongo/SQL/MSSQL reset operations must commit messages and sequence changes atomically or leave
  prior state intact (`NEW-116`).
- File-store save-and-advance must not expose advanced sequence without recoverable message data
  (`NEW-117`).
- NoopStore must track sequence numbers in memory for the session lifetime (`NEW-118`).
- File-store reset and corrupt-tail recovery must not replay stale data or hide later valid appends
  after restart (`NEW-136`, `NEW-152`).
- File-store in-memory sequence state must change only after persistence succeeds (`NEW-154`).
- Duplicate sequence detection must be exposed through a separate capability without changing the
  core save contract (`NEW-137`).
- File logs must surface write failures and timestamp events consistently (`NEW-124`, `NEW-125`).
- Generic log config must thread backend options (`NEW-126`).
- File logs/stores must provide configurable size-limit or rotation behavior while preserving store
  resend/recovery semantics (`NEW-108`).

## Acceptance Evidence

- Unit tests with injected I/O failures for file sequence persistence.
- Restart/reload tests for corrupt-tail and reset behavior.
- Backend-gated transaction tests for Mongo/SQL/MSSQL where available.
- Log write-failure and timestamp formatting tests.
- File growth policy tests for logs and store bodies.

## Backend Feature-Gate Notes

- File, memory, noop, and redb-style tests should run in default local validation where possible.
- Mongo, SQL, and MSSQL transaction tests may require feature flags and external services; keep them
  clearly named and ignored or gated when the service is unavailable.
- Backend-gated tests still need deterministic unit coverage for shared trait semantics such as
  duplicate detection and runtime/durable sequence consistency.

## Compatibility Notes

The duplicate-detection addition is separate from `save()` to avoid breaking existing
implementations. Store file growth policy must not silently discard data required for valid resend.
