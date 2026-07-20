# truefix-store

Pluggable FIX sequence/message persistence.

## Backends and features

| Backend | Cargo feature |
| --- | --- |
| memory, file, cached file, noop | default |
| SQLite/PostgreSQL/MySQL through `sqlx` | `sql` |
| Microsoft SQL Server through `tiberius` | `mssql` |
| embedded Redb | `redb` |
| MongoDB | `mongodb` |

All backends implement `MessageStore`; `StoreConfig::Custom` accepts an application-owned
implementation. External database tests require their respective services and configuration.

```sh
cargo test -p truefix-store
cargo test -p truefix-store --features redb
```
