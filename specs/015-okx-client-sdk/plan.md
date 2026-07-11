# Implementation Plan: OKX Client SDK

**Branch**: `015-okx-client-sdk` | **Date**: 2026-07-10 | **Spec**: [spec.md](./spec.md)

## Summary

Add an independent `truefix-okx-client` crate that fully covers the native OKX V5 surface in
`python-okx@fa8d738`: 264 REST operations across 20 domains plus public/private/business
WebSocket. It is a Rust domain API, not a translation of Python classes. `truefix-gateway` later
composes only mappable orders, accounts, positions, executions and market data.

The design separates immutable client context, canonical signing/request bytes, HTTP/WebSocket
transport, scoped rate limits, domain services, rich types and an auditable operation manifest.
Demo is the default; live requires explicit typed confirmation. Only safe reads retry
automatically. Writes require reconciliation before any caller-authorized retry.

## Technical Context

**Language/Version**: Rust 2024; workspace MSRV Rust 1.96

**Primary Dependencies**: Tokio; Serde/serde_json; rust_decimal; time; tracing; metrics;
thiserror; reqwest 0.13 (rustls, HTTP/2); tokio-tungstenite 0.29 (rustls); hmac, sha2, base64,
URL/query serialization and futures-util. Every new dependency requires Apache-2.0 OR MIT review.

**Storage**: N/A; callers own credentials, order persistence and caches.

**Testing**: `cargo test`; table-driven unit tests; local HTTP/WebSocket fixtures; inventory,
decimal and environment-safety integration tests; opt-in Demo Trading smoke tests.

**Target Platform**: Tokio-supported desktop/server platforms; no WASM commitment.

**Project Type**: Workspace library crate plus examples.

**Performance Goals**: Reuse pooled HTTP connections; bounded real-time event streams; reconnect
without duplicate subscriptions; obey server rate and connection limits.

**Constraints**: Exact decimal preservation; no panic/unwrap/expect in non-test critical paths;
redacted diagnostics; immutable credential context; Demo default; explicit live confirmation; no
blind write replay.

**Scale/Scope**: 264 baseline REST operations plus public/private/business real-time sessions and
their order commands; 100% manifest → native entrypoint → test traceability.

## Constitution Check

**Pre-design gate: PASS.**

| Obligation | Evidence |
|---|---|
| Production readiness | Documented domain API, typed `OkxError`, redacted diagnostics, lifecycle/backpressure tests. |
| Protocol correctness | Canonical signing, environment, login/ack/heartbeat/recovery tests against official V5 behaviour. |
| License/provenance | `python-okx@fa8d738` is capability evidence only; independently write code, docs and fixtures. |
| Dual-track FIX dictionary | Not applicable: this SDK neither produces nor validates FIX. |
| Test discipline | Table-driven units and local fixture integration; no copied upstream tests. |
| Acceptor/initiator parity | Not applicable: this is an OKX client, not a FIX session endpoint. |
| Inventory completeness | Machine-readable manifest records each baseline operation, Rust entrypoint and test. |

**Post-design gate: PASS.** The independent native-client boundary, named errors, provenance
discipline and inventory proof need no exception.

## Project Structure

### Documentation

```text
specs/015-okx-client-sdk/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── sdk-api.md
│   └── operation-inventory.md
└── tasks.md                 # created by speckit-tasks
```

### Source Code

```text
crates/truefix-okx-client/
├── Cargo.toml
├── examples/
│   ├── public_market_data.rs
│   └── demo_order_lifecycle.rs
├── src/
│   ├── lib.rs               # facade and lint policy
│   ├── client.rs            # OkxClient and domain accessors
│   ├── config.rs            # immutable context and environment safety
│   ├── auth.rs              # canonical V5 signer and clock source
│   ├── error.rs             # OkxError / OkxResult
│   ├── request.rs           # canonical request and retry classification
│   ├── response.rs          # envelope, pagination, per-item failures
│   ├── limiter.rs           # shared REST/WS scoped reservations
│   ├── inventory.rs         # operation-manifest access
│   ├── transport/{http.rs,websocket.rs}
│   ├── services/{account.rs,trade.rs,market.rs,public_data.rs,funding.rs,subaccount.rs,finance.rs,strategy.rs,professional.rs}
│   ├── types/{common.rs,instrument.rs,order.rs,account.rs,websocket.rs}
│   └── ws/{session.rs,subscription.rs,event.rs,public.rs,private.rs,business.rs}
└── tests/{http_contract.rs,ws_session.rs,operation_inventory.rs,decimal_roundtrip.rs,environment_safety.rs}
```

**Structure Decision**: A single independent workspace library crate. Services model business
domains, not Python modules; transport and WS are shared infrastructure. A future gateway adapter
lives in `truefix-gateway` and composes this client.

## Complexity Tracking

No constitution violations or complexity exceptions.
