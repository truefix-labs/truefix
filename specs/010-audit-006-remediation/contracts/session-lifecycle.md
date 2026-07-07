# Contract: Session Lifecycle And Protocol Behavior

## Scope

Applies to `truefix-session`, `truefix-at`, and any engine/transport wiring required to expose the
behavior to live acceptor/initiator sessions.

## Required Behaviors

- Logout requested before logon completion must not be silently dropped (`NEW-97`).
- Logon rejection must consume sequence state consistently when the incoming Logon sequence is at
  or above expected (`NEW-98`).
- `SequenceReset` must be dictionary validated, sequence checked, queue safe, and metadata complete
  across gap-fill and non-gap-fill variants (`NEW-99`, `NEW-119`, `NEW-140`).
- Error-teardown diagnostics must not persist one-shot Logout messages for later resend
  (`NEW-100`).
- Queued validation failure with disconnect-on-error must send Logout before disconnect (`NEW-120`).
- Schedule-driven logout must wait for peer Logout or timeout before closing (`NEW-139`).
- `PossResend(97)` must be observable by applications (`NEW-106`).
- Cancel-on-disconnect must provide an application/session mechanism for cancel requests without
  inventing application order state inside the session (`NEW-105`).
- Acceptor logon state must transition after sequence handling completes (`NEW-142`).

## Acceptance Evidence

- Unit tests for state-machine action lists.
- Two-process integration tests for acceptor and initiator role coverage.
- AT scenario additions for externally visible FIX behavior.

## Shared Test Fixture Notes

- State-machine unit tests should assert ordered `Action` output and final `SessionState` for both
  acceptor and initiator variants when the finding is role-sensitive.
- Live-session tests should use paired local sessions and record the peer-visible FIX messages,
  especially Logout, Reject, `SequenceReset`, and `PossResend` paths.
- AT scenarios should be reserved for behavior visible on the FIX wire; pure internal bookkeeping
  should stay in crate-level unit or integration tests.

## Compatibility Notes

Any new application callback or handle API must be additive and documented. If cancel-on-disconnect
requires application-supplied open-order data, the contract must make absence of that data explicit
and observable.
