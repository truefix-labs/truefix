# truefix-transport

Tokio-based FIX initiator and acceptor runtime.

## Current scope

- TCP and rustls TLS, reconnecting and scheduled initiators;
- single-, multi-, and dynamic-session acceptors with session-ID routing;
- SOCKS4/SOCKS5/HTTP CONNECT proxies and configurable socket options;
- bounded inbound staging, persistent store/log services, dictionaries, monitoring, and shutdown;
- optional binary codec attachment through `truefix-binary`.

Most applications should depend on the [`truefix`](https://crates.io/crates/truefix) facade crate.
See [Getting Started](../../docs/getting-started.md) for TLS and engine examples.

```sh
cargo test -p truefix-transport
```
