# Contract: Session Protocol Correctness (US1, US2)

**Requirements**: FR-005–FR-012 (US1), FR-017–FR-020, FR-023–FR-025, FR-028–FR-029 (US2)
**Research**: research.md §R1.2–R1.3, §R1.8–R1.13, §R2

## `ResetSeqNumFlag` handshake (FR-005, FR-006)

**Contract**: An acceptor's Logon response echoes `ResetSeqNumFlag=Y` if and only if the inbound
Logon it's responding to carried `ResetSeqNumFlag=Y` — not driven by static `.cfg` configuration for
the *response* case (the initiator's own first outbound Logon is unaffected, still driven by
`ResetOnLogon`/`ResetOnLogout`/`ResetOnDisconnect`, see `hardening.md`'s `FR-025`). An initiator
that sent `ResetSeqNumFlag=Y` verifies the response either echoes it or is inferable as acknowledged
(response `MsgSeqNum == 1`); failing both is a handshake failure (Logout + disconnect, same
severity class as `BUG-05`'s low-seq-Logon rejection, feature 006).

**Protocol-behavioral**: yes — genuine wire-handshake correction. **AT scenario required** per
Constitution Principle II (both directions: acceptor not echoing when it shouldn't, and initiator
rejecting a non-echoing response).

## `ResendRequest` deadlock avoidance (FR-007)

**Contract**: A `ResendRequest` whose range extends beyond the highest sequence sent is answered
immediately (via the existing `build_resend()` path), not deferred to the out-of-order queue.

**Protocol-behavioral**: yes. **AT scenario required** — the deadlock scenario itself (two sessions
each holding an outstanding `ResendRequest` toward the other) needs a live two-connection or
AT-runner scenario to prove it no longer hangs.

## Duplicate/competing connection refusal (FR-008)

**Contract**: A second inbound connection presenting a `SessionId` that already has an active
connection is refused before its Logon is processed.

**Protocol-behavioral**: yes, but acceptor-transport-layer, not session-state-machine — a live
two-connection integration test in `truefix-transport`, not necessarily a new AT scenario (the AT
runner drives one connection per scenario today; adding this may require the same multi-connection
`Step` primitive gap `docs/todo/003.md`'s AT-harness follow-ups already noted as blocked — confirm
feasibility at `/speckit-tasks` time, defer to a plain integration test if the harness limitation
still applies).

## `ResetOnDisconnect` on unexpected TCP drop (FR-009)

**Contract**: An unexpected TCP disconnection dispatches `Event::Disconnected` into the session
state machine (see data-model.md), triggering the same `enter_disconnected()` logic
(`ResetOnDisconnect`/`ResetOnLogout` honoring) a graceful Logout-driven disconnect already receives.

**Protocol-behavioral**: session-internal (the wire behavior — what happens after reconnecting with
a reset session — is already covered by existing `ResetOnLogon`-adjacent AT coverage). A
`truefix-transport` integration test (force-drop a live connection, confirm store reset behavior)
is the primary test vehicle.

## Callback-ordering restructure (FR-010)

**Contract**: `app.from_admin`/`app.from_app` are invoked only after the session layer's own
sequence/identity/latency/PossDup/dictionary checks for that message have passed.

**Protocol-behavioral**: no (application-facing contract, not wire-protocol) — a `truefix-session`/
`truefix-transport` integration test with a test `Application` that records callback-invocation
order relative to session-layer rejects is sufficient; no AT scenario needed.

## Admin-message dictionary validation (FR-011)

**Contract**: Dictionary validation applies to admin-typed messages when a dictionary is configured,
using the plain session-level `Reject` path (not `BusinessMessageReject`, which stays
application-message-specific).

**Protocol-behavioral**: yes. **AT scenario required** — a malformed admin message (e.g. a Logon
with a dictionary-invalid field) previously passed through unvalidated.

## Acceptor schedule enforcement (FR-015, FR-016 — see `engine-lifecycle.md` for the related
scheduled-initiator reconnect fix)

**Contract**: An acceptor session with a configured schedule rejects a Logon arriving outside its
window, and disconnects an already-logged-on session that crosses out of its window while
connected — matching the initiator-side enforcement `run_scheduled_initiator` already has.

**Protocol-behavioral**: yes. **AT scenario required** — this is the one User Story 1 item requiring
a schedule-aware AT harness fixture (a scenario that runs with a configured out-of-window schedule);
confirm AT runner support for schedule-configured scenarios at `/speckit-tasks` time, since prior
schedule testing (005/006) has been transport-integration-test-only, not AT-scenario-based.

## Narrower-impact session fixes (US2) — grouped, no dedicated AT scenario expected

Each of the following is a session-internal or narrow-condition fix; a targeted unit or integration
test is sufficient, matching the pattern established for equivalent 005/006 US2-tier items — none
are expected to require a new AT scenario, but confirm per-item at `/speckit-tasks` if a citation
turns out to be wire-observable in a way this note didn't anticipate:

- **FR-017**: `resend_requested`/out-of-order queue reset on new connection.
- **FR-018**: `validLogonState`-equivalent rejection of non-Logon messages before login completes.
- **FR-019**: Logon rejection for over-high `NextExpectedMsgSeqNum`, missing `MsgSeqNum`, negative
  `HeartBtInt`.
- **FR-020**: length-prefixed data-field tag verification + non-numeric length rejection.
- **FR-023**: `RefMsgType` on `Reject`; version-filtered `SessionRejectReason`.
- **FR-024**: PossDup anti-replay check extended to equal-sequence messages.
- **FR-025**: initiator first-connection `ResetSeqNumFlag` driven by any of the three reset configs.
- **FR-028**: stray-Logon rejection in `AwaitingLogout`/`Disconnected` states.
- **FR-029**: logout callback fires on a mid-handshake disconnect.
