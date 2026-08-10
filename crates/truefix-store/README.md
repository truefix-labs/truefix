# truefix-store

Pluggable FIX sequence/message persistence.

## Independent use

Yes. Backends implement `MessageStore` and can be constructed outside the engine. The crate stores
FIX sequence state and wire messages; it is not a general-purpose application database layer.

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
