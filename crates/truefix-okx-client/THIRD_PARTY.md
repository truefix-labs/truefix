# Third-party and provenance record

This crate is independently implemented. `thrdpty/clientapi/python-okx` at commit
`fa8d738249286b9b7ff8fed678218701f87bbb86` is used only as a capability inventory and external
behaviour reference; no source, comments, or tests are copied.

| Dependency | License | Purpose |
|---|---|---|
| reqwest | MIT OR Apache-2.0 | Async HTTP client, HTTP/2, proxy and rustls TLS |
| tokio-tungstenite | MIT | Tokio WebSocket transport |
| hmac, sha2, base64 | MIT OR Apache-2.0 | OKX V5 HMAC-SHA256/Base64 signing |
| futures-util, url | MIT OR Apache-2.0 | Stream and URL support |

The final dependency lockfile review is required by task T067 before release.
