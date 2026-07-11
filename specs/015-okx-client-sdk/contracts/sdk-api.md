# SDK Public Contract

`OkxClient` is constructed from immutable `ClientConfig`; construction does not make a network
request. Demo Trading is the default. Production needs an explicit typed confirmation; custom
endpoints are separately explicit. A different credential context needs a different client.

| Contract | Responsibility |
|---|---|
| `client.account()` | account, positions, risk, bills, leverage and loans |
| `client.trade()` | orders, fills, algorithmic orders, conversion and repayment |
| `client.market()` / `client.public_data()` | market data, instruments and public platform data |
| `client.funding()` / `client.subaccounts()` | assets, transfers and hierarchy |
| `client.finance()` / `client.strategy()` / `client.professional()` | long-tail product domains |
| `client.ws()` | public, private and business real-time sessions |

Every operation has a typed request/result, documented auth/rate/pagination/retry semantics and
native OKX data preservation; arbitrary paths or mutable parameter maps are not the primary API.

All failures use `OkxResult<T>` and a named non-exhaustive `OkxError` for configuration, live
confirmation, signing/clock skew, transport/timeout, rate limit, authentication/permission,
exchange rejection, decoding, partial failure, connection loss and unknown completion. Errors keep
codes/request IDs but redact secrets.

Safe reads may retry bounded transient failures. Writes are never blindly replayed. A caller may
authorize retry only with a stable client identity and status reconciliation proving safety.

Real-time subscription handles expose a bounded event receiver and cancellation. Private/business
connections await authentication acknowledgement; reconnect replays desired subscriptions after
acknowledgement gates and never replays trade commands.

## Gateway composition boundary

`truefix-okx-client` is usable without `truefix-gateway` and does not depend on it. A consuming
application may adapt the optional projection types for common orders, balances, positions, fills,
and tickers into its gateway model. A projection is deliberately lossy only in scope, never by
mutation: retain the source native OKX value next to the projected value when the application may
need exchange-specific fields.

The SDK does not project option Greeks, portfolio-margin and liquidation details, algorithm/grid
strategy state, funding/earn records, RFQ/spread/broker data, or other product-specific records.
Those remain native SDK data and must be handled by an OKX-aware caller. This boundary prevents a
cross-venue abstraction from silently changing or discarding OKX semantics.
