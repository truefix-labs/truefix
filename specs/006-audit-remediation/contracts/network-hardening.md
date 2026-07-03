# Contract: Network Hardening (US6)

**Covers**: FR-024 through FR-029. **Source**: `docs/todo/003.md` `BUG-13`, `BUG-19`, `GAP-53`,
`GAP-54`, `B14`, `B15`, `B30`. **Grounding**: research.md §R6.

**Files**: `crates/truefix-transport/src/framing.rs`, `crates/truefix-transport/src/lib.rs`,
`crates/truefix-transport/src/tls_config.rs`, `crates/truefix-transport/src/proxy.rs`,
`crates/truefix-core/src/framing.rs`.

## Behavioral contract changes

| # | Change | Contract |
|---|---|---|
| 1 | `frame_length` | Gains a maximum body-length/buffer-size bound (configurable or sane hardcoded default); a connection declaring a frame beyond the bound is closed instead of accumulating an unbounded buffer. New `DecodeError::BodyLengthTooLarge` variant. Applies uniformly pre- and post-Logon. |
| 2 | `connect_initiator_via_proxy`/`_tls` | Wrapped with the same `connect_timeout` behavior the direct/TLS initiator paths already have — a proxy that accepts the TCP connection but never completes its handshake no longer hangs indefinitely |
| 3 | `CipherSuites` filtering (`tls_config.rs`) | If zero configured names match any recognized cipher suite, returns a clear configuration-time error instead of silently building a zero-suite `CryptoProvider` |
| 4 | PROXY-protocol trusted-header peek (`proxy.rs`) | Bounded by a timeout — a connection that opens but never completes its header no longer hangs its task indefinitely |
| 5 | `classify_buffered`'s framing-error recovery | Discards only the malformed prefix, not the entire buffer — a legitimate trailing message is preserved |
| 6 | PROXY v2 header peek buffer sizing | Sized (or switched to a length-driven read) to avoid truncating large TLV sets, preventing PROXY-protocol bytes from being misread as FIX data. `truefix-core::framing`/`truefix-transport::framing` duplication (`B30`) folded into this same pass as a housekeeping side-effect. |

## No breaking changes

`DecodeError` gains one additive variant (confirm exhaustive-match sites at implementation time, per
data-model.md's note). `ConfigError`/TLS error types gain additive variant(s) for row 3. No existing
variant removed or renamed.

## Acceptor/Initiator parity

Rows 1, 5, 6: apply to any inbound connection (predominantly acceptor-side, since acceptors receive
untrusted inbound connections — but the framing/decode path is shared code, so an initiator receiving
a malformed response from a compromised/misbehaving counterparty benefits identically). Row 2:
initiator-only by construction (only initiators connect through a proxy). Row 3: both roles (TLS
config is symmetric). Row 4: acceptor-only by construction (PROXY protocol is inbound-only).
