# Contract: Extended Logon Predicate, Pre-Reset Hook, SessionStatus (US10; FR-013)

**Additive only** — no existing `Application` trait method signature changes; one new no-op-default
method is added, and the existing `Reject` outcome type gains one new optional field.

## Surface

```text
trait Application {
    // ...existing methods (on_create/on_logon/on_logout/to_admin/from_admin/to_app/from_app) unchanged...
    async fn on_before_reset(&self, session_id: &SessionId) {}   // NEW, no-op default
}

struct Reject {
    reason: SessionRejectReason,
    ref_tag: Option<u32>,
    text: Option<String>,
    session_status: Option<u16>,   // NEW, optional — FIX SessionStatus (tag 573)
}
```

## Behaviour

1. The "extended logon predicate" is the existing `from_admin(&Message, &SessionId) -> Result<(),
   Reject>` hook — any refusal condition an integrator implements there (beyond the engine's own CompID/
   dictionary checks) already refuses the logon today; this feature documents that convention and ensures
   a `Reject` returned from `from_admin` for a Logon message can carry `session_status`.
2. When `Reject.session_status` is `Some(v)`, the engine's outbound Logout/Reject for the refused logon
   carries `SessionStatus(573) = v`; when `None`, behavior is unchanged from today (no tag 573 emitted).
3. `on_before_reset` is invoked at the top of `Session::reset()` (both logon-time and scheduled resets),
   before any in-memory or store state is cleared.

## Acceptance (maps to spec US10 scenarios)

- A logon predicate refusing a specific CompID with a chosen `SessionStatus` produces an outbound
  Logout/Reject carrying that value (SC-009). ✔
- `on_before_reset` fires before every reset, observably (test double records invocation + timing
  relative to state clearing). ✔

## Test hooks

`Application` test-double implementing `on_before_reset` to assert ordering; session integration test
asserting the emitted `SessionStatus` value on a predicate-refused logon.
