# Contract: Session Protocol Correctness (US3, US4; FR-007–FR-011)

**This is the one theme in this feature that is genuinely protocol-behavioral** (Constitution
Principle II). Every behavior change here MUST land with a new AT scenario, not just a unit/
integration test — the AT suite is expected to grow past its 353/353 baseline, not merely stay
unmodified.

## Surface

```text
// truefix-session
pub enum SendOrigin { Live, Resend { seq: u64 } }   // NEW
pub enum Action {
    Send { message: Message, origin: SendOrigin },   // CHANGED shape (was Send(Message))
    Disconnect,
    ResetStore,
}
impl Session {
    pub fn gap_fill_after_veto(&mut self, seq: u64) -> Action;   // NEW
    pub fn reject_logon(&mut self, reject: &truefix_core::Reject) -> Vec<Action>;  // unchanged, new call sites
}
pub struct SessionConfig {
    // ...existing fields...
    pub requires_orig_sending_time_on_low_seq: bool,   // NEW (naming TBD in tasks — see research.md §6)
}

// truefix-transport
// perform_actions: CHANGED internal handling of a vetoed Action::Send { origin: Resend { seq }, .. }
```

## Behaviour

1. **Resend veto is already possible (correction, research.md §5)**: `Application::to_app` is already
   invoked for every resend-originated `Action::Send`. This feature does not add a new callback.
2. **Gap-fill substitution on veto (GAP-07/FR-007)**: when `to_app` returns `Err(DoNotSend)` for an
   `Action::Send { origin: Resend { seq }, .. }`, `perform_actions` — in addition to today's
   `discard_sent(seq)` — calls `session.gap_fill_after_veto(seq)` and immediately processes the
   resulting `Action::Send` (a `SequenceReset-GapFill`, admin-typed) through the normal write/log/
   persist path. A vetoed *live* send (`origin: Live`) is unaffected — today's discard-only behavior is
   exactly preserved.
3. **PossDup anti-replay (GAP-08/FR-008)**: `on_received`'s `Ordering::Less` + `poss_dup` branch, on
   detecting `OrigSendingTime` later than the message's own `SendingTime`, calls `self.reject_logon(...)`
   directly (no `Application` round-trip) — full session logout + disconnect, the same severity as
   duplicate-Logon handling (resolved via `/speckit-clarify`).
4. **`RequiresOrigSendingTime` on this path (FR-009)**: when `SessionConfig`'s new switch is set, a
   *missing* `OrigSendingTime` on this same low-seq/PossDup path is also a violation, triggering the
   same `reject_logon` path.
5. **Duplicate Logon rejection (GAP-18a/FR-010)**: `on_logon`, when already `LoggedOn`, calls
   `self.reject_logon(...)` instead of the current no-op — full logout + disconnect.
6. **Inbound chunk auto-continuation (GAP-09/FR-011)**: when `resend_request_chunk_size > 0` and
   TrueFix is the resend requester, reaching a chunk boundary below the originally-detected gap's full
   extent automatically emits the next chunk's `ResendRequest`, without waiting for an external one.

## No breaking changes

- `Action` is `truefix-session`-internal; its `Send` variant's shape change (tuple → struct, or an
  additive `Resend` variant — final choice per `/speckit-tasks`) has no external consumer to break,
  confirmed by grep (only `truefix-transport`'s `perform_actions` pattern-matches `Action` directly).
- `reject_logon`'s signature is unchanged — only new internal call sites.
- `SessionConfig`'s new field defaults to `false`/unset, preserving today's behavior for every `.cfg`
  not setting the new key.

## Acceptance (maps to spec US3/US4 scenarios)

- An application-vetoed resend message is replaced by a `SequenceReset-GapFill` on the wire, in every
  exercised gap-fill scenario; an unvetoed resend is unaffected (SC-005). ✔
- A PossDup message with a falsified earlier `OrigSendingTime` results in full logout + disconnect, in
  every exercised scenario (SC-006). ✔
- A duplicate Logon on an already-logged-on session is rejected (logout + disconnect), not silently
  accepted, in every exercised scenario (SC-007). ✔
- An inbound resend spanning multiple chunks completes without any manual/external re-request, in every
  exercised multi-chunk scenario (SC-008). ✔
- The AT conformance suite gains new passing scenarios for all of the above, and stays green throughout
  (SC-013). ✔

## Test hooks

- `truefix-session`: state-machine unit tests for `gap_fill_after_veto` (a vetoed resend produces
  exactly one `SequenceReset-GapFill` action, sequence numbers stay consistent afterward), the
  PossDup-anti-replay branch, the duplicate-Logon branch, and the inbound chunk-continuation branch
  (extends `state_machine.rs`/`recovery.rs`'s existing table-driven pattern).
- `truefix-transport`: two-process integration tests exercising each of the four behaviors end-to-end
  over a real connection (extends `recovery.rs`'s/`role_parity.rs`'s existing integration pattern —
  acceptor and initiator sides both covered per Constitution Principle VI).
- `truefix-at`: **new AT scenarios** for all four behaviors, added to the appropriate suite function in
  `crates/truefix-at/src/scenarios.rs` and exercised by `crates/truefix-at/src/runner.rs` — the
  regression-floor test in `crates/truefix-at/tests/coverage.rs`
  (`server_suite_scenario_run_count_does_not_regress`) is expected to require an updated floor value
  once these land (a deliberate, disclosed bump — not a silently-adjusted threshold).
