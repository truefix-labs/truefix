# Quickstart Validation: OKX Client SDK

## Prerequisites

- Rust 1.96 and the workspace toolchain.
- Optional OKX Demo Trading credentials with only the permissions required by the scenario. Keep
  them in a secure local secret source, never source control.

## Local validation

```sh
cargo test -p truefix-okx-client --test http_contract
cargo test -p truefix-okx-client --test ws_session
cargo test -p truefix-okx-client --test operation_inventory
cargo test -p truefix-okx-client --test decimal_roundtrip
cargo test -p truefix-okx-client --test environment_safety
```

Expected: fixtures validate signing/sending identity, redaction, Demo handling, login/subscription
acknowledgements, ping/pong, bounded reconnect/resubscribe and no write replay. Inventory tests
prove complete baseline coverage under [operation-inventory.md](./contracts/operation-inventory.md);
decimal/environment tests reject precision loss, live without confirmation and identity switching.

## Demo smoke validation

After securely configuring Demo credentials, run:

```sh
cargo run -p truefix-okx-client --example public_market_data
cargo run -p truefix-okx-client --example demo_order_lifecycle
```

Expected: public data works without credentials; the Demo example completes the configured order
lifecycle and private subscription without duplicated writes. Live credentials are rejected unless
the [SDK contract](./contracts/sdk-api.md) confirmation is supplied.

## Long-tail limitations

Long-tail fixture validation is local and credential-free. Server-side permission, regional-product
and Demo availability restrictions remain typed exchange responses. Do not enable a write example
without the explicit Demo environment variables; all write paths are non-replaying.
