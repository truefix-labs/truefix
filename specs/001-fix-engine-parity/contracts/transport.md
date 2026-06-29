# Contract: truefix-transport (Initiator & Acceptor)

Covers FR-F1…F7. Acceptor is a first-class peer of Initiator (Constitution VI).

## Initiator
- Connect to configured target(s); manage session lifecycle; reconnect on disconnect per
  `ReconnectInterval` (FR-F4); local bind, connect timeout, socket options (FR-F5); TLS via rustls (FR-F6).

## Acceptor
- Listen on `SocketAcceptPort`/address; route inbound connections to sessions by SessionID; serve
  multiple static sessions concurrently (FR-F2).
- **Dynamic sessions**: accept peers not pre-configured via `AcceptorTemplate`/`DynamicSession`; optional
  `AllowedRemoteAddresses` allow-listing; refuse disallowed remotes (FR-F3).
- TLS via rustls including client-auth/SNI keys (FR-F6); socket options (FR-F5).

## Concurrency
- Async-first (tokio): each session = read loop + write/timer loop tasks. Single- vs multi-threaded
  operation maps to tokio runtime flavor (research R1); public API is async (FR-F7).

## Acceptance hooks
- Two-process integration: independent initiator & acceptor; multi-session; one dynamic session (US4).
- Reconnect test; disallowed-remote refusal test; TLS handshake both roles.
