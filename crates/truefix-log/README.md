# truefix-log

Pluggable FIX message/event logging.

## Backends and features

| Backend | Cargo feature |
| --- | --- |
| screen, file, tracing, composite | default |
| SQLite/PostgreSQL/MySQL through `sqlx` | `sql` |
| Microsoft SQL Server through `tiberius` | `mssql` |
| embedded Redb | `redb` |
| MongoDB | `mongodb` |

`FileLog` uses a bounded background writer, supports size/time rotation and retention, and drains
queued entries through `Log::shutdown`. Database backends also require explicit shutdown to flush
their queues. External database tests require their respective services and configuration.

Most applications use this through `truefix`; see the
[workspace guide](../../docs/getting-started.md) for configuration details.

```sh
cargo test -p truefix-log
cargo test -p truefix-log --features redb
```
