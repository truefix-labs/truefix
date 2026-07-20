# truefix-config

QuickFIX-style `[DEFAULT]`/`[SESSION]` configuration parsing and resolution for TrueFix.

## Current scope

- default inheritance, per-session overrides, comments, and `${name}` interpolation;
- initiator/acceptor endpoints, schedules and time zones, FIX/FIXT dictionaries;
- store/log selection, TLS, proxy, socket, reconnect, timeout, and validation settings;
- typed errors for malformed, ambiguous, duplicate, or unsupported configuration.

The `sql` and `mssql` features enable resolution of their corresponding store backends. Parsing a
file does not start networking; the `truefix` facade maps resolved sessions into engine services.

Most applications use this through the `truefix` facade. See the
[configuration guide](https://github.com/truefix-labs/truefix/blob/main/docs/getting-started.md)
for examples and prerequisites.

```sh
cargo test -p truefix-config
```
