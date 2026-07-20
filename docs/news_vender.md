# Market data, trading, and news provider research

> Product research note. The original file mixed unverified third-party prices, availability, and
> implementation plans. Volatile vendor claims have been removed; this document now records only
> repository-verifiable status and decisions still required.

## Current repository status

| Provider | Crate | Transport scope |
| --- | --- | --- |
| Futu OpenD | `truefix-futu-client` | quote/trade RPC and push events through OpenD |
| Interactive Brokers | `truefix-twsapi-client` | TWS/IB Gateway requests and event stream |
| OKX | `truefix-okx-client` | V5 REST plus public/private/business WebSocket |
| IG | `truefix-ig-client` | REST; streaming is not implemented by the crate |

The Binance examples live under the `truefix` facade crate. There is no common provider trait,
provider registry, instrument master, news client, Tauri UI, or AI-agent implementation today.

## Proposed abstraction (not implemented)

A future provider registry may expose independent capabilities for historical market data,
real-time market data, trading/account state, news, and instrument/reference data. Each configured
client instance would have a stable user-defined name and an explicit state: unsupported,
supported but disabled, or enabled. One provider must not be assumed to implement every role.

## Decisions required before implementation

- Define shared instrument identifiers and venue-specific mapping rules.
- Define normalized candle, tick, order, execution, position, and account models without losing
  provider-native fields.
- Define credential storage, Demo/live confirmation, rate limits, reconnect, and entitlement state.
- Choose UI and persistence boundaries before adding a Tauri application.
- Re-evaluate every external SDK's current license, maintenance, and API coverage at implementation
  time; do not rely on old free-tier numbers.

See [Getting Started](getting-started.md) for the clients that exist now.
