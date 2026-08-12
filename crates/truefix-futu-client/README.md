# truefix-futu-client

A native asynchronous Rust client for Futu OpenD.

## Independent use

Yes. This is a standalone broker SDK and does not depend on the TrueFix FIX engine. It requires a
reachable OpenD process plus the user's market/trading permissions.

Add it with `truefix-futu-client = "0.1.10"`. Running it requires a reachable Futu OpenD instance;
credentials and connection settings must be provided by the application.

## Current scope

The crate provides protobuf framing/RPC correlation, connection and reconnect handling, quote and
trade requests, push-event decoding, typed gateway projections, and a runnable `futu_cli` example.
Availability of markets, accounts, and trading operations still depends on the connected OpenD
instance and the user's Futu permissions.

```sh
cargo run -p truefix-futu-client --example futu_cli
cargo test -p truefix-futu-client
```

See [Getting Started](../../docs/getting-started.md#2-futu-opend).
