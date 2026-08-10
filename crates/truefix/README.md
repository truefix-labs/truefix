# truefix

Facade crate for the TrueFix FIX engine. It re-exports the message codec, session state machine,
transport, configuration, dictionary, store, log, and binary codec layers, and adds `.cfg`-driven
`Engine` startup/shutdown.

## Independent use

Yes. This is the recommended standalone dependency for a complete FIX initiator or acceptor. It
combines the lower-level workspace crates; broker-specific Futu/TWS/OKX/IG clients are separate SDKs
and are not re-exported by this facade.

## Installation

```toml
truefix = "0.1.6"
```

Use `Engine::start` for built-in services or `Engine::start_with_overrides` for per-session custom
`MessageStore`/`Log` implementations. Lower-level `Acceptor` and initiator functions remain
available through re-exports.

See [Getting Started](../../docs/getting-started.md) and the [workspace README](../../README.md).

```sh
cargo test -p truefix
```
