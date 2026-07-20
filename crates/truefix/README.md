# truefix

Facade crate for the TrueFix FIX engine. It re-exports the message codec, session state machine,
transport, configuration, dictionary, store, log, and binary codec layers, and adds `.cfg`-driven
`Engine` startup/shutdown.

## Installation

```toml
truefix = "0.1.4"
```

Use `Engine::start` for built-in services or `Engine::start_with_overrides` for per-session custom
`MessageStore`/`Log` implementations. Lower-level `Acceptor` and initiator functions remain
available through re-exports.

See [Getting Started](../../docs/getting-started.md) and the [workspace README](../../README.md).

```sh
cargo test -p truefix
```
