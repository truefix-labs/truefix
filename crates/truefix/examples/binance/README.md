# `binance` example

A real, runnable FIX.4.4 client for [Binance's Spot FIX API](https://developers.binance.com/legacy-docs/binance-spot-api-docs/fix-api),
built entirely on `truefix`'s public API. It connects to any combination of Binance's three FIX
endpoints (order entry, drop copy, market data), signs each Logon with Binance's non-standard
Ed25519 `RawData` scheme, and gives you an interactive REPL to place/cancel/amend orders, submit
OCO/OTO/OTOCO order lists, subscribe to market data, and query instrument rules.

The supported message set is implemented in `messages.rs`; consult the example's `help` output and
source rather than assuming full coverage of every current Binance API operation. It is a reference
for wiring `truefix` up to a real venue: `.cfg`-driven
multi-session `Engine`-adjacent construction (via the lower-level `truefix::transport` API, since
each session needs a per-session Ed25519 identity that `Engine::start` alone doesn't model), a
custom `Application` that signs outbound Logons and pretty-prints inbound traffic, a real FIX data
dictionary (`dict/binance-{oe,md}.fixdict`) for structured repeating-group decoding, and a choice
of async log backends (`RedbLog` or `FileLog`, the latter exercising this repo's own
non-blocking file-write and rotation support).

## 1. Get Binance testnet credentials

1. Go to <https://testnet.binance.vision/> and log in with GitHub.
2. Generate an **Ed25519** API key pair (the site can generate the keypair for you, or you can
   generate your own with `openssl genpkey -algorithm ed25519 -out binance-testnet-key.pem` and
   upload the public key). Save the **private key PEM file** and the **API key** it gives you.

## 2. Set environment variables

```sh
export BINANCE_FIX_SENDER_COMP_ID=<your testnet account id, e.g. from the site>
export BINANCE_FIX_API_KEY=<your Ed25519 API key>
export BINANCE_FIX_PRIVATE_KEY=/path/to/binance-testnet-key.pem
```

(`config/binance-testnet.cfg` reads these via `${BINANCE_FIX_...}` interpolation — nothing secret
is ever committed to this repo.)

## 3. Run

From the repository root:

```sh
cargo run -p truefix --example binance -- crates/truefix/examples/binance/config/binance-testnet.cfg
```

This connects all three sessions (OrderEntry/MarketData/DropCopy) and drops you into a REPL; type
`help` for the full command list, e.g.:

```
order BTCUSDT BUY 0.001 limit 50000 GTC
mdreq BTCUSDT book 5
instruments BTCUSDT
limitquery
quit
```

To run only a subset of endpoints, copy `config/binance-testnet.cfg` and delete the `[SESSION]`
blocks you don't want.

## 4. Inspect the logs afterward

Each session logs its full wire traffic (readable on stdout as it runs, via `tracing`) plus an
on-disk audit trail. By default (`BinanceLogBackend=Redb`, the OrderEntry/DropCopy sessions above):

```sh
cargo run -p truefix --example binance -- --dump-log log/<SENDER_COMP_ID>-order-entry-log.redb
```

The MarketData session above uses `BinanceLogBackend=File` instead (see the `.cfg`'s comments) —
its `messages.log`/`event.log` are already plain text, so just `cat`/`tail -f`
`log/<SENDER_COMP_ID>-market-data-log/messages.log`.

## Production

Swap `SocketConnectHost` in the `.cfg` from `fix-{oe,md,dc}.testnet.binance.vision` to
`fix-{oe,md,dc}.binance.com` (production hosts are commented at the bottom of the `.cfg`) and use
a production API key/private key. **Test on testnet first.**
