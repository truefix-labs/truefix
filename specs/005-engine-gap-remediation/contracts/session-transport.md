# Contract: Session Identity & Initiator Connection Robustness (US5, US6; FR-012–FR-016)

## Surface

```text
// truefix-session
impl SessionId {
    pub fn new(begin_string, sender_comp_id, target_comp_id) -> Self;   // unchanged
    pub fn new_full(                                                     // NEW
        begin_string: impl Into<String>,
        sender_comp_id: impl Into<String>,
        sender_sub_id: Option<String>,
        sender_location_id: Option<String>,
        target_comp_id: impl Into<String>,
        target_sub_id: Option<String>,
        target_location_id: Option<String>,
        session_qualifier: Option<String>,
    ) -> Self;
}
pub struct SessionConfig {
    // ...existing fields...
    pub sender_sub_id: Option<String>,          // NEW
    pub sender_location_id: Option<String>,     // NEW
    pub target_sub_id: Option<String>,          // NEW
    pub target_location_id: Option<String>,     // NEW
    pub session_qualifier: Option<String>,      // NEW
    pub reconnect_interval_steps: Vec<u32>,     // NEW
    pub local_bind_addr: Option<SocketAddr>,    // NEW
    pub connect_timeout: Option<Duration>,      // NEW
}

// truefix-transport — every connect_initiator* variant's internal connect step gains:
//   - local-bind-then-connect (when local_bind_addr is set)
//   - tokio::time::timeout wrapping (when connect_timeout is set)
//   - stepped backoff indexing (reconnect loops only, when reconnect_interval_steps is non-empty)
```

## Behaviour

1. **Session identity completeness (GAP-47/FR-012)**: `builder.rs::resolve_one` parses `SenderSubID`/
   `SenderLocationID`/`TargetSubID`/`TargetLocationID`/`SessionQualifier` into the five new
   `SessionConfig` fields; `SessionConfig::session_id()` constructs the resulting `SessionId` via
   `new_full` instead of `new`.
2. **Distinct-by-qualifier sessions (FR-013)**: two `.cfg` `[SESSION]` blocks with identical
   BeginString/SenderCompID/TargetCompID but different `SessionQualifier` produce two distinct
   `SessionId`s (already guaranteed by the existing 8-field `Hash`/`Eq` derive) and therefore start as
   two independent sessions, not a duplicate/conflicting configuration.
3. **Reconnect backoff array (GAP-14/FR-014)**: `ReconnectInterval` accepts a space/comma-separated
   list of integers (in addition to today's single-integer form). The reconnect loop indexes into the
   resulting `reconnect_interval_steps` by attempt number, clamped to the last element once exhausted.
   An empty list (the default, or a single-value `ReconnectInterval`) preserves today's fixed-interval
   behavior exactly.
4. **Local bind address (GAP-15/FR-015)**: `SocketLocalHost`+`SocketLocalPort` (both required together)
   populate `local_bind_addr`. Every initiator connect path binds the outbound socket to it (via
   `socket2`, the same crate already used for the existing socket-options feature set) before
   connecting.
5. **Connect timeout (GAP-16/FR-016)**: `SocketConnectTimeout` populates `connect_timeout`. Every
   initiator connect call is wrapped in `tokio::time::timeout` when set; unset preserves today's
   unbounded wait.

## No breaking changes

- `SessionId::new` is unchanged; `new_full` is a new, additional constructor.
- `SessionConfig`'s 8 new fields all default to their zero value, preserving today's behavior for
  every `.cfg` not setting the corresponding new key.
- No existing `connect_initiator*` function signature changes — the new behaviors are internal to each
  function's body, driven by the (already-passed-in) `SessionConfig`.

## Acceptance (maps to spec US5/US6 scenarios)

- A `.cfg` session with all five identity keys set carries them on its resolved `SessionId` (SC-009,
  first half). ✔
- Two session blocks differing only by `SessionQualifier` both start as distinct sessions (SC-009,
  second half). ✔
- A stepped reconnect interval reaches and sticks at its final value within the expected number of
  attempts (SC-010). ✔
- A short connect timeout against an unresponsive peer fails within the configured bound, never hanging
  indefinitely (SC-011). ✔

## Test hooks

- `truefix-config`: `.cfg` mapping tests for all 8 new keys (extends `key_coverage.rs`'s/
  `socket_and_failover_mapping.rs`'s existing pattern).
- `truefix`/`truefix-transport`: an integration test starting two `.cfg` sessions sharing
  BeginString/SenderCompID/TargetCompID but distinct `SessionQualifier` and asserting both are live,
  independently addressable sessions; a reconnect-backoff timing test (extends `reconnect.rs`'s
  existing pattern); a local-bind test asserting the outbound connection's source address; a
  connect-timeout test against a black-holed address (extends `sync_writes.rs`'s "stalled peer" test
  pattern, applied to the connect phase instead of the write phase).
