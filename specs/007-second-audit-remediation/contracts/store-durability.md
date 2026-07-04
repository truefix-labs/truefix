# Contract: Store Durability (US1)

**Requirements**: FR-001, FR-001a, FR-002, FR-003, FR-004
**Research**: research.md §R1.1, §R1.4, §R1.5

## Sequence-number store: format change + crash-safety + migration

**Contract**: `FileStore`/`CachedFileStore`'s sequence-number persistence:
1. Uses two independent files, `senderseqnums` and `targetseqnums`, instead of one combined
   `seqnums` file.
2. Each file's writes are atomic (no window in which a crash leaves it empty) — implemented via
   write-to-temp-then-rename, not truncate-then-write-in-place.
3. `reset()` deletes and recreates both files wholesale.
4. A file present but failing to parse surfaces `StoreError` at open time — never silently
   defaults to sequence 1.
5. **Migration**: if neither new file exists but a legacy combined `seqnums` file does, its content
   is split into the new two-file layout on open, before any other store operation proceeds. The
   legacy file is left in place afterward (not deleted).

**Test obligation**: beyond the standard crash-injection unit tests (interrupt each write point,
confirm recovery), a **dedicated migration test** is required — construct a store directory
containing only a legacy-format `seqnums` file, open it, and confirm both new files now exist with
the correct split values and the legacy file is still present unchanged.

**Backward compatibility**: an operator can downgrade to a pre-007 TrueFix binary after this
feature's migration has run, provided they haven't deleted the (deliberately-retained) legacy file
— the old binary only ever reads the legacy file, which migration never touches.

## `SqlStore::ensure_schema` migration ordering

**Contract**: `add_creation_time_column_if_missing()` (or equivalent) runs before any statement that
references the `creation_time` column, on every code path — including opening against a database
created before that column existed. Applies identically to SQLite/Postgres/MySQL (one shared code
path).

## `MssqlStore::save_and_advance_sender` commit-failure handling

**Contract**: if the transaction's final `COMMIT` fails, a `ROLLBACK` is issued before returning the
error — matching the rollback behavior already used when an earlier statement in the same
transaction fails. No connection is left holding an open, uncommitted transaction after this
function returns (success or failure).
