# Contract: QuickFIX/J JDBC URL Grammar Compatibility (US4)

**Covers**: FR-018 through FR-020. **Source**: `docs/todo/003.md` `BUG-10` (extends `BUG-04`, 005),
`GAP-55`. **Grounding**: research.md §R4. **Scope resolved via `/speckit-clarify`**: no H2 backend.

**Files**: `crates/truefix-config/src/builder.rs`, `crates/truefix-store/src/mssql.rs`.

## Behavioral contract changes

| # | Change | Contract |
|---|---|---|
| 1 | `is_jdbc_sql_scheme` | `jdbc:h2:` arm **removed**. A `.cfg` with `JdbcURL=jdbc:h2:...` now fails with a clear, typed unsupported-backend error at config-resolution time — it MUST NOT silently open a SQLite file. No H2-compatible backend is implemented anywhere in this feature. |
| 2 | `MssqlStore::parse_url` | Gains a second accepted grammar: real QuickFIX/J semicolon-delimited `jdbc:sqlserver://host[:port][;databaseName=X;user=Y;password=Z]`, tried when the existing `user:password@host/database` form doesn't match (no `@` present). Both forms remain accepted — additive, not a replacement. **This is the feature's one disclosed public-API-surface growth.** |
| 3 | `splice_credentials` | `JdbcUser`/`JdbcPassword` are percent-encoded before being spliced into a JDBC-style URL's authority — a value containing `@`/`:`/`/` no longer corrupts the resulting URL |

## No breaking changes

`StoreConfig`, `ConfigError` shapes unchanged. `is_jdbc_sql_scheme` losing the `jdbc:h2:` arm is a
narrowing of *accepted input*, not a public-type change — any `.cfg` that previously silently
misrouted through this scheme was never actually working (it opened the wrong backend), so no
previously-*correct* configuration stops working.

## Acceptor/Initiator parity

Not applicable — store/config resolution has no acceptor/initiator distinction (a `JdbcURL` is
resolved identically regardless of the session's `ConnectionType`).
