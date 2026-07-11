# Phase 0 Research: OKX Client SDK

## Decisions

### Native domain-oriented SDK

- **Decision**: Add `truefix-okx-client` as an independent workspace crate with `OkxClient`
  domain accessors and dedicated public/private/business real-time sessions.
- **Rationale**: The roadmap separates direct broker clients from `TradingGateway`; the source has
  264 operations in 20 business domains. Domain services keep native OKX meaning without Python
  class or parameter-dictionary translation.
- **Alternatives considered**: A gateway-only adapter loses OKX-only products; one source-file-
  shaped API preserves upstream organisation instead of a stable Rust API.

### Auditable capability baseline

- **Decision**: Maintain a machine-readable manifest for every REST operation and real-time
  trading command from `thrdpty/clientapi/python-okx@fa8d738`, recording source domain, native
  entrypoint, transport, authentication, retry safety and test evidence.
- **Rationale**: Source evidence is 262 V5 endpoint constants and 264 domain methods: Account
  52, Trade 28, PublicData 21, MarketData/Grid 19 each, Funding 18, BlockTrading 17 and remaining
  account, finance and strategy domains. A manifest makes full migration measurable.
- **Alternatives considered**: Counting generated methods or copying names cannot prove omitted
  functionality and violates provenance discipline.

### Canonical request serialization and signing

- **Decision**: One canonical request produces the encoded query, exact JSON body and signed path.
  Private REST signs `timestamp + uppercase method + request path including query + body` with
  HMAC-SHA256/Base64. The clock is injectable and time-skew is typed.
- **Rationale**: Signature validation is byte-sensitive. One serializer eliminates signing/sending
  drift and improves on the source client's manual query construction.
- **Alternatives considered**: Separate sign/send serializers can change ordering or bytes;
  copying upstream helpers is prohibited.

### Immutable identity and safe environment selection

- **Decision**: `ClientConfig` owns immutable credentials and endpoint policy. Demo defaults;
  production requires distinct explicit confirmation. Separate clients are required for different
  accounts, subaccounts or permissions.
- **Rationale**: Prevents identity leakage in concurrent HTTP and real-time work. Demo WebSocket
  endpoints differ from live endpoints, so a REST header alone is insufficient.
- **Alternatives considered**: Mutable per-request credentials risk cross-account use; live by
  default contradicts the clarified safety requirement.

### Shared scoped limiter and read-only retries

- **Decision**: REST and real-time operations reserve from a shared limiter keyed by documented
  user/subaccount/instrument scope and operation class. Bounded jittered retries are only for safe
  reads; writes need stable client identity plus reconciliation and explicit caller authorization.
- **Rationale**: OKX shares trade limits across REST/WS and applies endpoint-specific scopes.
  This avoids duplicated asset changes while retaining read resiliency.
- **Alternatives considered**: A global bucket wastes capacity; per-transport buckets can jointly
  exceed limits; retrying all writes risks duplicate orders.

### Acknowledgement-gated real-time supervisor

- **Decision**: Use `Disconnected → Connecting → Authenticating → Resubscribing → Active →
  Backoff`; persist desired subscriptions separately from active socket subscriptions; await
  login/subscribe acknowledgements; enforce ping/pong; replay subscriptions but never writes.
- **Rationale**: Official connection and request limits require liveness and controlled recovery.
  The Python baseline overwrites one callback and lacks reliable reconnect/resubscribe.
- **Alternatives considered**: A single callback cannot safely multiplex; replaying unacknowledged
  trade commands can duplicate execution.

### Minimal native transport stack

- **Decision**: Use async `reqwest` with rustls/HTTP2 and `tokio-tungstenite` with rustls, plus
  minimal crypto/encoding crates after license review.
- **Rationale**: They fit the existing Tokio/rustls workspace. `reqwest` provides pooled async
  HTTP/proxy/HTTP2 and is MIT OR Apache-2.0; `tokio-tungstenite` is Tokio-native and MIT.
- **Alternatives considered**: A hand-built HTTP/WebSocket stack increases maintenance and security
  surface.

## Sources

- Local source inventory: `thrdpty/clientapi/python-okx` commit
  `fa8d738249286b9b7ff8fed678218701f87bbb86`.
- [OKX V5 API documentation](https://www.okx.com/docs-v5/en/) — signing, Demo, rate limits and
  WebSocket behaviour.
- [reqwest documentation](https://docs.rs/reqwest/latest/reqwest/) — reusable async HTTP client,
  proxy/HTTP2 support and license.
- [tokio-tungstenite documentation](https://docs.rs/tokio-tungstenite/latest/tokio_tungstenite/) —
  Tokio WebSocket support and license.
