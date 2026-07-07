# Contract: Configuration, Defaults, And Diagnostics

## Scope

Applies to `truefix-config`, plus downstream consumers in session, transport, log, and engine
construction.

## Required Behaviors

- Heartbeat timeout, test-request delay, field-order validation, PossDup handling, and event
  timestamp defaults must align with clarified parity behavior (`NEW-102`, `NEW-103`, `NEW-104`,
  `NEW-111`, `NEW-125`).
- Boolean values and TLS protocol values must reject unrecognized input instead of silently falling
  back (`NEW-112`, `NEW-113`).
- Duplicate `[SESSION]` identities must be detected (`NEW-129`).
- Initiator missing-port errors must name `SocketConnectPort` precisely (`NEW-128`).
- Config values containing `#` must not be unexpectedly truncated outside documented comment
  positions (`NEW-130`).
- `SocketOptions` application failures must be visible (`NEW-132`).
- TLS scheduled initiator and backlog settings must be configurable where the runtime behavior is
  implemented (`NEW-134`, `NEW-135`).

## Acceptance Evidence

- Parser tests for strict booleans/TLS protocol values.
- Resolved-settings tests for default parity changes.
- Duplicate session config fixtures.
- Config-to-runtime tests for backlog, TLS scheduled initiator, and socket option diagnostics.

## Compatibility Notes

Default changes are intentional parity changes and must be called out in docs/release notes. Explicit
operator overrides take precedence over changed defaults.
