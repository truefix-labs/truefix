# Data Model: OKX Client SDK

## Client and common responses

| Entity | Fields | Rules |
|---|---|---|
| `ClientConfig` | environment, endpoint policy, credential reference, timeout, proxy, retry/limiter policy | Immutable; Demo default; live needs confirmation. |
| `Credentials` | key, secret, passphrase | One context/client; redacted in diagnostics. |
| `CanonicalRequest` | method, path, encoded query, exact body, metadata | The same bytes are signed and sent. |
| `ResponseEnvelope<T>` | OKX code/message, data, request ID | Non-success maps to a typed exchange error. |
| `Page<T>` | items, before/after cursors | Preserve server cursor semantics. |
| `BatchResult<T>` | per-item successes/failures | Partial success remains visible per item. |

## Trading, account and real-time entities

| Entity | Fields / states | Rules |
|---|---|---|
| `Instrument` | ID, type/family, tick/lot rules, contract value | Requests must be compatible with product/trade mode. |
| Decimal amount/price | exact decimal value | Never use binary float; exact textual round trip. |
| `OrderRequest` / `Order` / `Fill` | client ID, exchange ID, state, values, timestamps | One order has zero or more fills; state-changing retry is unsafe until reconciled. |
| `Position` / `Balance` | instrument/currency, quantity, availability, margin/risk | Preserve native fields beyond gateway projection. |
| `AlgorithmicOrder` | algorithm IDs, trigger/execution parameters, state | Separate lifecycle with cross references to orders. |
| `RealtimeSession` | Disconnected, Connecting, Authenticating, Resubscribing, Active, Backoff | Private/business cannot subscribe before successful login acknowledgement. |
| `SubscriptionKey` / `Subscription` | endpoint, canonical args, correlation ID, event receiver | Desired and active sets are distinct; route only matching events. |
| `CompletionState` | confirmed, rejected, unknown | Connection loss before write acknowledgement is unknown, never auto-replayed. |

## Domain services

| Service | Scope |
|---|---|
| Account | balances, positions, bills, configuration, leverage, margin, loans, risk and fees |
| Trade | lifecycle, fills/history, algorithms, conversion and repayment |
| Market / Public Data | tickers, books, candles, trades, indexes, instruments, funding and platform data |
| Funding / Subaccount | deposits, withdrawals, transfers, valuation, bills and account hierarchy |
| Finance | savings, staking, flexible loans and dual investment |
| Strategy | grid, recurring buy and copy trading |
| Professional | block/spread trading, trading data, convert, broker rebate and status |
