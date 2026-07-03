# Contract: Store/Log Persistence & Durability (US3)

**Covers**: FR-013 through FR-017. **Source**: `docs/todo/003.md` `BUG-08`, `BUG-09`, `GAP-48`,
`GAP-39`/`GAP-49`, `BUG-15`, `B17`, `BUG-14`. **Grounding**: research.md §R3.

**Files**: `crates/truefix-transport/src/lib.rs`, `crates/truefix-store/src/file.rs`,
`crates/truefix-store/src/mssql.rs`.

## Behavioral contract changes

| # | Change | Contract |
|---|---|---|
| 1 | Store-operation failures (`crates/truefix-transport/src/lib.rs`, 7 call sites) | Every `Err` from `save_and_advance_sender`/`set_next_sender_seq`/`set_next_target_seq`/`reset`/`get` is routed through `services.log.on_event(...)` — no silent swallow remains |
| 2 | `run_scheduled_initiator`'s `was_in_session` seed | Consults `store.creation_time()` against the schedule window before defaulting to `false` — a restart inside an active window no longer triggers a spurious full reset |
| 3 | `FileStore`/`CachedFileStore::save_and_advance_sender` | New override providing the same atomicity guarantee SQL/MSSQL/Redb already have (established 005) — no longer falls through to the two-independent-writes trait default |
| 4 | `BodyLog::reset()` | Gains a conditional `sync_data()` call (gated on `self.sync`), matching `append()`'s existing discipline |
| 5 | `FileStore`/`CachedFileStore::reset()` internal ordering | `seq.reset()` now runs **before** `body.reset()` (reversed) — a crash mid-reset leaves `seqnums` at post-reset `(1,1)` with `body` still holding harmless unindexed pre-reset data, never the reverse (which was unrecoverable) |
| 6 | `MssqlStore::connect_with_config` | Validates `sessions_table`/`messages_table` identifiers (ported from `sql.rs`) before any query — clean `StoreError::Backend` on an invalid name instead of a raw driver error |

## No breaking changes

`MessageStore` trait signature is entirely unchanged — every change here is either a new trait-method
**override** on two backends that previously used the trait default, or an internal reordering/added
validation inside existing method bodies. No new `StoreError`/`ConfigError` variant is strictly
required (existing `StoreError::Backend` shape covers row 6).

## Acceptor/Initiator parity

All 6 changes are role-agnostic — `MessageStore`/store-layer behavior has no acceptor/initiator
distinction; both roles' sessions use the same store backends and transport-layer call sites.
