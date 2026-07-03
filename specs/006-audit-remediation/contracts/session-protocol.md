# Contract: Session Protocol Correctness (US1)

**Covers**: FR-001 through FR-009. **Source**: `docs/todo/003.md` `BUG-05`, `BUG-06`, `BUG-22`, `B1`,
`B3`, `B4`, `B5`, `B6`, `B7`. **Grounding**: research.md §R1.

**File**: `crates/truefix-session/src/state.rs` (all changes in this file).

## Behavioral contract changes

| # | Trigger | Before (current, wrong) | After (this feature) |
|---|---|---|---|
| 1 | Inbound Logon, `MsgSeqNum < next_in_seq`, no PossDup | Accepted: state → `LoggedOn`, Logon reply sent, `next_in_seq` unchanged | Rejected: `reject_logon` (Logout + disconnect), no state transition, `next_in_seq` unchanged |
| 2 | Plain `SequenceReset`, `NewSeqNo < next_in_seq` | Accepted unconditionally: `next_in_seq = NewSeqNo` (rewinds) | Rejected: session Reject (`ValueIsIncorrect`, ref tag 36), `next_in_seq` unchanged |
| 3 | Plain `SequenceReset`, tag 36 absent | Silently skips adjustment, still drains queue | Rejected: required-tag-missing, ref tag 36, before draining |
| 4 | `ResendRequest`, `BeginSeqNo`/`EndSeqNo` tag absent | Silent no response | Required-tag-missing response (ref tag 7 or 16) |
| 4b | `ResendRequest`, both tags present, `begin > end` | Silent no response | **Unchanged** — kept as-is (spec Edge Cases) |
| 5 | Initiator `ResetOnLogon`, reconnect | Partial reset (`full = !logon_sent`, usually `false`) → outbound/store not reset → gap → resend stale Logon → `reject_logon` loop | Full, consistent reset in both directions on the connection's first reset-carrying Logon, regardless of send order |
| 6 | Gap-filled message drained from queue | `process_in_order` only — no `validate_app` | `validate_app` runs first, same reject/disconnect-or-continue semantics as the in-order path |
| 7 | New connection established | `ticks_since_recv`/`test_request_outstanding`/`resend_target`/`resend_chunk_end` carry over from prior connection | All reset in `on_connected` |
| 8 | Inbound message fails dictionary validation, `disconnect_on_error=true` | Reject sent, `Action::Disconnect` (no Logout) | Logout sent, then `Action::Disconnect` (matches identity/latency paths) |
| 9 | Inbound `SendingTime` unparseable | `latency_ok` returns `true` (bypasses check) | `latency_ok` returns `false` (fails check → existing Logout+disconnect path) |

## AT suite obligation (Constitution Principle II)

Each row above that changes counterparty-observable wire behavior (all except row 4b, which is
explicitly unchanged) MUST land a corresponding AT scenario in
`crates/truefix-at/src/scenarios.rs`, wired into `server_suite()` across `SUITE_VERSIONS` — 8 new
scenario families × 9 versions = up to 72 new scenario-runs (exact count depends on whether every
scenario is meaningful across all 9 versions; some protocol behaviors may be version-invariant and
share one scenario). `crates/truefix-at/tests/coverage.rs`'s regression floor is bumped to reflect
the new total as the final sub-task of this stage (after `BUG-17`'s initial 353→373 bump, itself done
in `completeness-and-harness.md`'s stage).

## No breaking changes

No public type's shape changes. `Session`'s internal state gains one new private field
(`connection_reset_done: bool` — see data-model.md); `Action`, `SessionConfig`, and all public error
types are untouched by this contract.

## Acceptor/Initiator parity (Constitution Principle VI)

- Rows 1, 2, 3, 4, 4b, 6, 8, 9: symmetric — apply identically regardless of role, since `on_logon`/
  `on_sequence_reset`/`on_resend_request`/`drain_queue`/`validate_app`/`latency_ok` are role-agnostic
  functions on `Session`.
- Row 5 (`ResetOnLogon`): initiator-specific by construction (only initiators send the connection's
  first Logon), but its fix must be verified by a two-process test where an **acceptor** performs its
  own expected full reset and the initiator's corrected reset now converges with it (not verified in
  isolation).
- Row 7: symmetric.
