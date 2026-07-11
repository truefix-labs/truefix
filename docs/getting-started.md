# TrueFix Getting Started

This guide covers the current workspace entry points: the FIX engine, the Futu OpenD client,
the Interactive Brokers TWS/Gateway client, and the OKX and Binance exchange CLIs. The broker
and exchange clients are independent crates. They do not start OpenD or TWS/Gateway for you, and
they do not bypass broker account, market-data, or trading permissions.

## 1. Prerequisites

Install or prepare:

- the Rust toolchain specified by `rust-toolchain.toml`;
- a running Futu OpenD or IB TWS/Gateway instance;
- the required account permissions for quotes, historical data, and trading.

Check the workspace and build the client crates:

```bash
cargo check --workspace
cargo build -p truefix-futu-client -p truefix-twsapi-client -p truefix-okx-client
```

Run all examples from the repository root.

## 2. Futu OpenD

### 2.1 Connect

OpenD listens on `127.0.0.1:11111` by default. Start the interactive CLI with:

```bash
FUTU_HOST=127.0.0.1 \
FUTU_PORT=11111 \
FUTU_CLIENT_ID=1 \
FUTU_CLIENT_VER=300 \
cargo run -p truefix-futu-client --example futu_cli
```

The CLI opens an interactive prompt:

```text
futu>
```

Enter `help` to print the complete command list.

### 2.2 Quotes and live push updates

The CLI accepts market aliases such as `us`, `hk`, `sh`/`cnsh`, `sz`/`cnsz`, `fx`, and `cc`.
The symbol and market are separate arguments. For `SH.600389`, use `600389 sh`.

Query a basic quote:

```text
quote 600389 sh
```

Subscribe to basic quote, ticker, order book, and real-time data:

```text
watch 600389 sh all
```

Subscribe to one stream:

```text
watch AAPL us basic
watch AAPL us ticker
watch AAPL us book
watch AAPL us rt
```

Inspect subscriptions:

```text
sub-info current
```

Stop subscriptions:

```text
unsub AAPL us basic
unsub-all
```

### 2.3 K-line data

The `kline` command automatically subscribes to the corresponding K-line subtype before calling
OpenD `GetKL`, which is required by OpenD:

```text
kline 600389 sh day none 100
```

The argument order is:

```text
kline <symbol> <market> <kl_type> <rehab_type> <req_num>
```

Supported periods include `1m`, `5m`, `15m`, `30m`, `60m`, `day`, `week`, `month`, `quarter`, and
`year`. Supported adjustment modes are `none`, `forward`/`qfq`, and `backward`/`hfq`.

### 2.4 Accounts and trading

Query accounts first:

```text
accounts
```

Then query positions, orders, and fills:

```text
positions
orders
fills
```

The CLI can select an account from the `accounts` result. It can also use environment variables:

```bash
FUTU_ACC_ID=<account_id> \
FUTU_TRD_ENV=0 \
FUTU_TRD_MARKET=1 \
cargo run -p truefix-futu-client --example futu_cli
```

Before placing an order, verify the account, trading environment, market, OpenD unlock state, and
permissions. Example:

```text
place-order 600389 buy 100 10.50 cnsh
```

Use a simulated account first. Live orders are subject to OpenD unlock state, account permissions,
market hours, and broker risk controls.

### 2.5 Futu environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `FUTU_HOST` | `127.0.0.1` | OpenD host |
| `FUTU_PORT` | `11111` | OpenD port |
| `FUTU_CLIENT_ID` | `1` | Client identifier |
| `FUTU_CLIENT_VER` | `300` | InitConnect client version |
| `FUTU_REQUEST_TIMEOUT_MS` | `10000` | Request timeout |
| `FUTU_AUTO_RECONNECT` | `true` | Enable automatic reconnect |
| `FUTU_RECONNECT_INTERVAL_MS` | `6000` | Reconnect interval |
| `FUTU_MARKET` | `us` | Default quote market |
| `FUTU_TRD_SEC_MARKET` | `us` | Default trading security market |
| `FUTU_ACC_ID` | unset | Default trading account |
| `FUTU_TRD_ENV` | simulated | Trading environment |
| `FUTU_PACKET_ENC_ALGO` | `0` | OpenD packet encryption algorithm |
| `FUTU_INIT_RSA_KEY` | unset | RSA initialization key path |

OpenD errors `10167` and `10168` usually indicate missing real-time or delayed-market-data
permissions. If OpenD says that `KL_*` must be subscribed before requesting K-lines, use the current
CLI and run the `kline` command again.

## 3. IB TWS/Gateway

### 3.1 Connect

Typical default ports are:

- TWS live: `7496`;
- TWS paper: `7497`;
- IB Gateway live: `4001`;
- IB Gateway paper: `4002`.

Use the port configured in TWS/Gateway and enable API socket clients.

Start the interactive CLI:

```bash
TWS_HOST=127.0.0.1 \
TWS_PORT=7497 \
TWS_CLIENT_ID=1002 \
cargo run -p truefix-twsapi-client --example twsapi
```

Inside the client, use:

```text
help
market-data
positions
historical-data
```

The same CLI supports one-shot commands:

```bash
TWS_HOST=127.0.0.1 \
TWS_PORT=7497 \
TWS_CLIENT_ID=1002 \
TWS_SYMBOL=AAPL \
TWS_EXCHANGE=SMART \
TWS_CURRENCY=USD \
cargo run -p truefix-twsapi-client --example twsapi -- market-data
```

The `--` separator passes the command to the example instead of Cargo.

### 3.2 Common operations

The interactive CLI includes market data, orders, accounts, positions, historical data, contract
queries, scanners, real-time bars, tick-by-tick data, option calculations, PnL, news, and WSH
operations. Common commands include:

```text
market-data
market-depth
place-order
open-orders
cancel-order
positions
account-summary
historical-data
historical-ticks
contract-details
real-time-bars
tick-by-tick
```

Requests read contract and request parameters from `TWS_*` environment variables:

| Variable | Default | Purpose |
|----------|---------|---------|
| `TWS_HOST` | `127.0.0.1` | TWS/Gateway host |
| `TWS_PORT` | `7497` | TWS/Gateway port |
| `TWS_CLIENT_ID` | `1002` | API client ID |
| `TWS_SYMBOL` | `AAPL` | Contract symbol |
| `TWS_SEC_TYPE` | `STK` | Security type |
| `TWS_EXCHANGE` | `SMART` | Exchange or routing destination |
| `TWS_CURRENCY` | `USD` | Currency |
| `TWS_REQ_ID` | `9001` | Request ID |
| `TWS_MARKET_DATA_TYPE` | `1` | `1` real-time, `3` delayed |
| `TWS_ACCOUNT` | unset | Account code |
| `TWS_MODEL_CODE` | unset | Model portfolio code |
| `TWS_WAIT_SECS` | command-specific | Event wait timeout |

### 3.3 Market-data permissions and disconnects

Status messages such as `2104`, `2106`, and `2158` describe the state of a TWS data farm; they are
not necessarily request failures. Errors `10167` and `10168` indicate missing real-time market-data
permissions or delayed-data availability. To request delayed data:

```bash
TWS_MARKET_DATA_TYPE=3
```

Do not terminate the process immediately after sending a request. TWS responses are asynchronous;
the CLI reads events for `TWS_WAIT_SECS` and then cancels continuous requests where appropriate.

## 4. Binance exchange CLI

The Binance CLI covers the official connector's Spot, Margin Trading, Options, and Convert REST
modules. It also supports Spot WebSocket API calls and raw Spot, Margin, and Options stream names.

Start the interactive prompt:

    cargo run -p truefix --example binance_cli

The CLI reads the API key and Ed25519 PEM path only when a signed request is made. Its defaults
point to the local account directory; override them when needed:

| Variable | Default | Purpose |
|----------|---------|---------|
| BINANCE_API_KEY_FILE | .../truefix-account/binance/readme | File containing the API key |
| BINANCE_PRIVATE_KEY_PATH | .../truefix-account/binance/Ed25519/test-prv-key.pem | Ed25519 private-key PEM |

Use catalog to see the module groups. Public requests must include --public; signed requests
are the default. Every REST write requires --confirm-write.

The same commands can run without entering the prompt. Put the command after Cargo's -- separator:

    cargo run -p truefix --example binance_cli -- get spot /api/v3/exchangeInfo '{"symbol":"BTCUSDT"}' --public

Common REST examples:

    get spot /api/v3/exchangeInfo {"symbol":"BTCUSDT"} --public
    get spot /api/v3/account
    get margin /sapi/v1/margin/account
    get options /eapi/v1/exchangeInfo --public
    get convert /sapi/v1/convert/exchangeInfo

WebSocket examples:

    stream spot btcusdt@trade
    stream margin btcusdt@bookTicker
    stream options btcusdt@trade
    ws time --public
    ws account.status

Press Ctrl-C to disconnect an active stream. In the interactive prompt, JSON with whitespace may
be quoted with single quotes. Check endpoint-specific parameters and permissions in Binance
documentation before sending a request.

## 5. OKX exchange CLI

The OKX CLI exposes the complete 264-operation audited REST surface. It accepts an audited
domain/operation pair, rather than arbitrary URLs, and derives authentication and replay policy
from the operation manifest.

Start it from the repository root:

    cargo run -p truefix-okx-client --example okx_cli

Use catalog [domain] to list accepted operations. Get and post validate the verb against the
audited method. The default environment is Demo. Every POST requires a JSON body and
--confirm-write.

The CLI also accepts one-shot commands after Cargo's -- separator:

    cargo run -p truefix-okx-client --example okx_cli -- get market_data get_ticker '{"instId":"BTC-USDT"}'

Common REST examples:

    get market_data get_ticker {"instId":"BTC-USDT"}
    get public_data get_instruments {"instType":"SPOT"}
    get account get_account_balance
    post trade place_order {} {"instId":"BTC-USDT","tdMode":"cash","side":"buy","ordType":"limit","sz":"0.001","px":"1"} --confirm-write

Public operations need no credentials. To enable private operations, set OKX_PASSPHRASE; API key
and secret are read from the configured credential file unless overridden:

| Variable | Default | Purpose |
|----------|---------|---------|
| OKX_CREDENTIAL_FILE | .../truefix-account/okx/okx | Credential file with API key and secret |
| OKX_API_KEY | unset | Overrides the file API key |
| OKX_SECRET | unset | Overrides the file secret |
| OKX_PASSPHRASE | unset | Enables private authentication |
| OKX_ENV | demo | Set to live only for production |
| OKX_CONFIRM_LIVE | unset | Must be 1 when OKX_ENV=live |

Use Demo first. Live mode requires both OKX_ENV=live and OKX_CONFIRM_LIVE=1, in addition to
valid credentials.

## 6. Direct Rust API usage

Use the clients as workspace dependencies:

```toml
[dependencies]
truefix-futu-client = { path = "../truefix/crates/truefix-futu-client" }
truefix-twsapi-client = { path = "../truefix/crates/truefix-twsapi-client" }
truefix-okx-client = { path = "../truefix/crates/truefix-okx-client" }
```

For Futu, call `FutuClient::connect`, then use `client.quote()` and `client.trade()`. Consume
asynchronous push events through `client.subscribe_push()`.

For TWS, call `TwsApiClient::connect(ClientConfig)`, send requests through concrete `req_*` methods
or `send_request`, and consume callbacks through `read_event` as `Event` values.

## 7. Verification and troubleshooting

Recommended checks before submitting changes:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test -p truefix-futu-client --test mock_opend -- --test-threads=1
cargo test -p truefix-twsapi-client
cargo test -p truefix-okx-client
cargo test -p truefix --example binance_cli
cargo clippy -p truefix-futu-client -p truefix-twsapi-client --all-targets -- -D warnings
cargo clippy -p truefix-okx-client -- -D warnings
cargo clippy -p truefix --example binance_cli -- -D warnings
```

Common failures:

1. `Connection refused`: verify that OpenD or TWS/Gateway is running, the port is correct, and API
   socket access is enabled.
2. `UnexpectedEof` or `Connection reset`: verify the client version, port type, client ID ownership,
   and server logs.
3. `actor shut down`: inspect the protobuf or socket error immediately before it. Futu pushes must
   be decoded from `Response.s2c`, and quote subscriptions must be registered on the active
   connection.
4. No quote updates: verify market-data permissions, market hours, symbol format, and market value.
5. Order failure: query accounts first and verify the trading environment, account permissions,
   unlock state, and trading market.
6. Binance missing API key or private key: public REST and WebSocket calls need --public. For
   signed calls, set BINANCE_API_KEY_FILE and BINANCE_PRIVATE_KEY_PATH to readable files. Do not
   place key material in shell history or this repository.
7. Binance write rejected before sending: add --confirm-write only after checking the endpoint,
   request JSON, account permissions, and selected module. This guard is intentional.
8. OKX operation is unknown: run catalog or catalog <domain> and use the exact audited
   domain/operation pair. The CLI deliberately rejects arbitrary REST paths.
9. OKX private request has missing credentials: set OKX_PASSPHRASE and either provide
   OKX_API_KEY and OKX_SECRET or configure OKX_CREDENTIAL_FILE.
10. OKX live mode is rejected: the default is Demo. Set both OKX_ENV=live and
    OKX_CONFIRM_LIVE=1 only after reviewing the command and credentials.

The root README describes the FIX engine and workspace architecture. This document describes the
current broker-client workflows.
