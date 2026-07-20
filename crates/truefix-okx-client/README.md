# truefix-okx-client

Native asynchronous Rust client for OKX V5 REST and WebSocket APIs.

Add it with `truefix-okx-client = "0.1.4"`. Provide API credentials through your application's secret
provider; the client never needs credentials embedded in source code.

## Current scope

- typed REST services for account, trade, market, public data, funding, subaccounts, strategies,
  finance, RFQ/spread, broker, convert, and trading-data operations;
- generated metadata for 264 REST capabilities extracted from the pinned local
  `python-okx@fa8d738` baseline;
- public, private, and business WebSocket sessions, login/subscription state, routing, heartbeat,
  reconnect state, trade commands, and command expiry;
- exact-millisecond signing, measured server-time offset, explicit Demo/live intent, typed exchange
  errors, response metadata, and decimal-preserving wire models.

The 264-row inventory proves count, classification, path metadata, and a discoverable Rust entrypoint.
It does **not** mean that every operation has a dedicated live-exchange or per-operation HTTP fixture;
local contract tests cover shared protocol behavior and representative corrected endpoints.

## Safety and verification

All write operations are non-replaying. Demo is the default; credentials belong in a secret
provider, never this repository.

```sh
python3 crates/truefix-okx-client/scripts/generate_operation_inventory.py
cargo test -p truefix-okx-client
cargo clippy -p truefix-okx-client --all-targets -- -D warnings
```

See [Getting Started](../../docs/getting-started.md#5-okx-exchange-cli).
