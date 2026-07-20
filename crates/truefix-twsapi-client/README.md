# truefix-twsapi-client

A native asynchronous Rust client for Interactive Brokers TWS and IB Gateway.

Add it with `truefix-twsapi-client = "0.1.4"`. It requires a reachable TWS or IB Gateway instance.

## Current scope

The crate implements the start-API handshake, server-version feature gates, length-prefixed field
encoding/decoding, request IDs and event pumping. Public request types cover market data, accounts,
orders, executions, contracts, scanners, news, fundamentals, and historical data. Actual access is
controlled by the connected TWS/Gateway version, account permissions, and market-data entitlements.

```sh
cargo run -p truefix-twsapi-client --example twsapi
cargo test -p truefix-twsapi-client
```

See [Getting Started](../../docs/getting-started.md#3-ib-twsgateway).
