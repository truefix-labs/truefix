# Contract: Config-Key Stance Registry Accuracy (US8; FR-020–FR-021)

## Surface

```text
// truefix-config::keys — stance changes only, no schema change to KeyInfo/Stance
k("SocketAcceptProtocol", "acceptor", Unsup("..."))    // CHANGED: was Rec
k("SocketConnectProtocol", "initiator", Unsup("..."))  // CHANGED: was Rec
k("JdbcDataSourceName", "sql", Unsup("..."))            // CHANGED: was Rec
k("JdbcConnectionTestQuery", "sql", Unsup("..."))       // CHANGED: was Rec
k("JdbcUser", "sql", Impl)                               // CHANGED: was Rec (US2/FR-004 lands first)
k("JdbcPassword", "sql", Impl)                           // CHANGED: was Rec
k("JdbcMaxActiveConnection", "sql", Impl)                // CHANGED: was Rec
// ...+ 5 sibling pool keys, + JdbcStoreMessagesTableName/SessionsTableName/SessionIdDefaultPropertyValue

// truefix-store — StoreConfig::Sql/Mssql field growth (see data-model.md)
```

## Behaviour

1. **Doc-accuracy downgrades (FR-020)**: `SocketAcceptProtocol`/`SocketConnectProtocol` (VM_PIPE-only
   alternative) and `JdbcDataSourceName`/`JdbcConnectionTestQuery` (JNDI lookup / no `.cfg`-expressible
   equivalent) move to `Unsupported` with the specific reason already documented in
   `docs/engine-comparison-gaps.md`'s "Documentation-accuracy notes."
2. **JDBC pool/table-name wiring (FR-021)**: `StoreConfig::Sql`/`Mssql` grow optional
   `sessions_table`/`messages_table`/`session_id`/`pool` fields (data-model.md), defaulted to
   `SqlStoreConfig`'s/`MssqlStoreConfig`'s own existing defaults. `jdbc_store_config`/`resolve_store`
   parse `JdbcMaxActiveConnection` and its 5 pool-tuning siblings plus `JdbcStoreMessagesTableName`/
   `JdbcStoreSessionsTableName`/`JdbcSessionIdDefaultPropertyValue` into these fields when present.

## Dependency

This contract depends on `correctness-bugs.md`'s `JdbcURL` scheme-recognition landing first (there is
no `.cfg`-driven SQL store selection for the pool/table-name keys to attach to otherwise) — reflected
as plan.md's G8-depends-on-G2 staging note.

## No breaking changes

- Stance changes are metadata-only (the `KeyInfo`/`Stance` registry has no runtime effect on `.cfg`
  parsing itself — a key changing stance doesn't change whether a `.cfg` file using it still parses).
- `StoreConfig::Sql`/`Mssql`'s new fields are all `Option`-wrapped, defaulting to `None` — zero behavior
  change for any `.cfg` not setting the new keys.

## Acceptance (maps to spec US8 scenarios)

- The four identified keys read `Unsupported` with a documented reason each. ✔
- A `.cfg` session with the JDBC pool-tuning and table-name keys set alongside a `jdbc:`-style
  `JdbcURL` uses those settings in the resulting store. ✔

## Test hooks

- `truefix-config`: `key_coverage.rs`'s existing `every_key_has_a_known_stance` test continues to pass
  post-change (no key becomes unregistered); new `.cfg`-mapping tests for the 9 newly-`Impl` JDBC
  pool/table-name keys, extending `store_and_log_mapping.rs`'s existing `JdbcURL` test block.
