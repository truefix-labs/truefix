# Contract: Transport And Engine Lifecycle

## Scope

Applies to `truefix-transport` and the `truefix` engine facade.

## Required Behaviors

- Accept loops must log/surface transient accept errors and continue when the listener remains
  usable (`NEW-115`).
- Engine shutdown must cancel or otherwise stop already-accepted session tasks, not only listener
  loops (`NEW-149`).
- Inbound application staging must remain bounded when configured channel capacity is finite
  (`NEW-150`).
- TLS handshakes must be covered by configured timeout behavior and release active-session state
  after timeout (`NEW-151`).
- Dynamic acceptor sessions must use a per-dynamic-identity store/services factory (`NEW-148`).
- Engine/session handles must expose session identity, addressability, state query, and skipped
  session reporting (`NEW-121`, `NEW-122`, `NEW-123`, `NEW-143`).
- Scheduled initiators must support TLS variants (`NEW-134`).
- Listener backlog must be configurable (`NEW-135`).
- Socket option failures must be observable (`NEW-132`).
- Administrative traffic priority must not starve ticks, controls, or application messages
  indefinitely (`NEW-153`).

## Acceptance Evidence

- Integration tests for shutdown with active accepted sessions.
- Bounded-memory/backpressure tests with a slow application receiver.
- Timeout tests for stalled TLS handshakes.
- API tests for session identity and state query.
- Config-to-bind tests for backlog and socket option diagnostics.

## Compatibility Notes

New handle/query APIs should be additive. Shutdown semantics may become stricter by ensuring no
accepted task survives engine shutdown.
