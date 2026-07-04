# Contract: Session Protocol Correctness (US1, US2)

**Requirements**: FR-011–FR-024, FR-026–FR-027 (US1/US2 session items)
**Research**: research.md §R1.2–R1.5, R1.8, R1.10–R1.11, R1.21, §R2/R3

## `BeginSeqNo=0` treated as "from sequence 1" (FR-011)

**Contract**: An inbound `ResendRequest` with `BeginSeqNo=0` is answered as if `BeginSeqNo=1`, not
silently dropped.

**Protocol-behavioral**: yes — a genuine wire-handshake correction (a peer's valid, spec-legal
resend demand goes unanswered today). **AT scenario required** per Constitution Principle II.

## Acceptor `ResetOnLogon` (FR-012)

**Contract**: An acceptor configured with `ResetOnLogon=Y` resets both of its own sequence numbers
to 1 upon receiving any Logon, independent of that Logon's own `ResetSeqNumFlag`.

**Protocol-behavioral**: yes — QuickFIX/J's primary, documented use of this setting is exactly this
acceptor-side behavior. **AT scenario required** — distinct from feature 007's `BUG-28`
(protocol-level echo/verify handshake); this is the local-config-driven proactive reset.

## Gap-fill `SequenceReset` too-high/too-low verification (FR-013)

**Contract**: A gap-fill `SequenceReset` whose own `MsgSeqNum` is ahead of expectation is queued and
answered with a `ResendRequest` via the same dispatch every other too-high message type uses,
instead of applying its `NewSeqNo` jump unconditionally.

**Protocol-behavioral**: yes — directly affects recovery correctness and can cause silent,
permanent message loss if unfixed. **AT scenario required.**

## Teardown reset-reason scoping (FR-014)

**Contract**: `reset_on_logout` is honored only for a graceful Logout-driven teardown;
`reset_on_disconnect` only for a non-graceful one (TCP drop, schedule exit, heartbeat timeout). See
data-model.md's `DisconnectReason` shape.

**Protocol-behavioral**: yes, in the sense that it changes when a store reset is observable via
sequence-number behavior on reconnect — but the *mechanism* is session-internal. A
`truefix-session`/`truefix-transport` integration test exercising both teardown paths with each
flag set independently is the primary vehicle; confirm at `/speckit-tasks` whether a dedicated AT
scenario adds value beyond that.

## Schedule-exit Logout (FR-015)

**Contract**: A schedule-window exit that tears down a logged-on session sends a `Logout` before
disconnecting.

**Protocol-behavioral**: yes — wire-observable (the counterparty sees a graceful Logout instead of
an abrupt TCP drop). **AT scenario required** if the AT harness supports schedule-configured
scenarios (confirm at `/speckit-tasks`, per feature 007's own note on this same gap for its
`BUG-86`/`87` schedule-enforcement AT coverage).

## Dictionary-invalid Logon routing (FR-016)

**Contract**: A Logon failing data-dictionary validation is rejected via the same `Logout`+
disconnect path every other Logon-rejection reason uses, regardless of `disconnect_on_error`.

**Protocol-behavioral**: yes. **AT scenario required** — a dictionary-invalid Logon with
`disconnect_on_error=false` previously left the session stranded in `AwaitingLogon`, an
observable protocol-inconsistency.

## Narrower-impact session fixes (US2) — grouped, no dedicated AT scenario expected

Each of the following is a session-internal or narrow-condition fix; a targeted unit or integration
test is sufficient, matching the pattern established for equivalent 006/007 US2-tier items:

- **FR-017**: pre-logon admin messages (`Logout`/`Reject`/`SequenceReset`) with too-high seq
  rejected without a `ResendRequest`.
- **FR-018**: non-gap-fill `SequenceReset` with `NewSeqNo=0` rejected as a value error.
- **FR-019**: gap-fill `SequenceReset` missing `NewSeqNo` rejected as a required-field violation.
- **FR-020**: `reset_sequences()` clears `resend_target`/`resend_chunk_end`/
  `test_request_outstanding`.
- **FR-021**: too-high `Logout` processed immediately (not queued behind a `ResendRequest`) —
  wire-observable (avoids a session hang); consider for an AT scenario at `/speckit-tasks`.
- **FR-022**: a too-high `ResendRequest` already answered immediately is not reprocessed when
  `drain_queue` later reaches it.
- **FR-023**: a non-Logon message failing latency/CompID/PossDup checks receives a session-level
  `Reject` before the `Logout` — wire-observable (two messages instead of one); consider for an AT
  scenario at `/speckit-tasks`.
- **FR-024**: an inbound Logon's `DefaultApplVerID(1137)` is validated against the registered
  `ApplVerID` set before use.
- **FR-026/FR-027**: `SessionConfig::new()`'s bare-struct `reconnect_interval` default and
  `.cfg`-driven `LogoutTimeout` default corrected to match QuickFIX/J (30s / 2s) — pure default-value
  changes, config-level unit tests only, no AT scenario.
