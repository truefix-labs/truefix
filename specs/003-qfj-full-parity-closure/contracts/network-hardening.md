# Contract: PROXY Protocol, SOCKS/HTTP-CONNECT Proxy, Inline PEM, Cipher Suites, Sync Writes (US12; FR-015–FR-017)

## Surface

```text
// truefix-config (new recognised → implemented keys)
UseTCPProxy: bool,
TrustedProxyAddresses: Vec<IpAddr>,        // NEW — bounds PROXY-header trust (Clarifications)
ProxyType: Enum(Socks4, Socks5, HttpConnect),
ProxyHost/ProxyPort: String/u16,
ProxyUser/ProxyPassword: Option<String>,   // SOCKS5 only
SocketPrivateKeyBytes/SocketCertificateBytes/SocketCABytes: Option<Vec<u8>>,  // alongside existing *Path keys
CipherSuites: Vec<String>,                 // moves Recognized -> implemented
SocketSynchronousWrites: bool,
SocketSynchronousWriteTimeout: Duration,
```

## Behaviour

1. **PROXY protocol**: the acceptor checks the physical peer's source IP against
   `TrustedProxyAddresses` **first**. Only if it matches does the engine parse a PROXY (v1/v2) header
   from the connection and substitute the declared original client IP for IP-allow-list/logging
   purposes; from a non-trusted source, the header is not parsed/trusted (rejected or ignored per
   config) — the physical source IP is used instead.
2. **Forward proxy**: an initiator with `ProxyType` set connects to `ProxyHost:ProxyPort` and performs the
   corresponding handshake (SOCKS4 connect; SOCKS5 connect, with `ProxyUser`/`ProxyPassword` if set; HTTP
   `CONNECT`) before starting the FIX session on the resulting stream.
3. **Inline PEM bytes**: `SocketPrivateKeyBytes`/`CertificateBytes`/`CABytes`, when set, are parsed
   in-memory (same `rustls-pki-types::pem` parser used for file paths) instead of reading a file; file-
   path keys remain fully functional and unchanged.
4. **Cipher suites**: `CipherSuites`, when set, restricts the rustls `ServerConfig`/`ClientConfig` to only
   the listed suites; unset preserves today's rustls-default behavior.
5. **Synchronous writes**: `SocketSynchronousWrites=Y` makes outbound writes block for completion; a
   write exceeding `SocketSynchronousWriteTimeout` surfaces a typed timeout error rather than blocking
   indefinitely.

## Acceptance (maps to spec US12 scenarios)

- PROXY header trusted only from a configured trusted upstream; untrusted source's header is not
  trusted (SC-011, Edge Cases). ✔
- SOCKS4/SOCKS5(+auth)/HTTP CONNECT initiator connections succeed through the configured proxy. ✔
- TLS establishes from inline PEM bytes with no file path involved. ✔
- A restricted cipher-suite list is honored in the handshake. ✔
- A stalled synchronous write times out with a typed error. ✔

## Test hooks

Integration tests: acceptor behind a fake PROXY-protocol-emitting TCP shim (trusted and untrusted source
cases); initiator through a local SOCKS4/SOCKS5/HTTP-CONNECT test proxy; TLS handshake from in-memory PEM
bytes; handshake with a deliberately restricted cipher suite; a write-timeout test using a stalled peer.
