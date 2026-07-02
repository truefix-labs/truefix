# Contract: Field-Order Validation, Session Switches, Extra Validation Toggles (US3, US4, US8; FR-006–FR-008)

## Surface

```text
// truefix-dict::model
struct ValidationOptions {
    // ...existing 9 toggles...
    validate_fields_out_of_order: bool,   // NEW, default false (today's behavior)
    validate_checksum: bool,              // NEW
    validate_incoming_message: bool,      // NEW, default true (today's behavior = always validate)
    allow_pos_dup: bool,                  // NEW, default true (today's behavior)
    requires_orig_sending_time: bool,     // NEW, default false
}

// truefix-session::config
struct SessionConfig {
    // ...existing fields...
    send_redundant_resend_requests: bool,
    closed_resend_interval: Option<Duration>,
    reset_on_error: bool,
    disconnect_on_error: bool,
    disable_heart_beat_check: bool,
    reject_message_on_unhandled_exception: bool,
    logon_tag: Option<u32>,
    max_scheduled_write_requests: Option<usize>,
    continue_initialization_on_error: bool,
    log_message_when_session_not_found: bool,
    refresh_on_logon: bool,               // field already exists; now acted on
    force_resend_when_corrupted_store: bool, // detection already exists; now acted on
}
```

## Behaviour

1. `validate_fields_out_of_order = true` ⇒ `validate()` rejects header/body/trailer field ordering that
   violates the dictionary-declared order; default `false` preserves today's acceptance.
2. `validate_checksum = true` ⇒ an independent checksum check runs regardless of framing-level checks.
3. `validate_incoming_message = false` ⇒ dictionary validation is skipped entirely (session-level checks
   still apply).
4. `allow_pos_dup = false` ⇒ a `PossDupFlag=Y` message is rejected per policy.
5. `requires_orig_sending_time = true` ⇒ a PossDup message missing `OrigSendingTime` is rejected.
6. Each of the 12 session switches changes session behavior at its documented trigger point (error
   handling, resend redundancy/interval, heartbeat-check suppression, logon-tag validation, write-request
   throttling, init-error continuation, not-found logging, post-logon refresh from store, forced resend on
   detected store corruption) — see `docs/todo-gap-analysis.md` TODO-04 for the QuickFIX/J-parity
   behavior each name maps to.

## Acceptance (maps to spec US3/US4/US8 scenarios)

- AT scenarios `14g`, `15`, `2t` (field order) reach documented outcomes. ✔
- Each of the 12 switches has a dedicated behavioral test (SC-004). ✔
- Four extra validation toggles each independently gate their documented message class (SC-003). ✔

## Test hooks

Table-driven `ValidationOptions` unit tests (one toggle on/off pair per case); per-switch session
integration tests; `validateChecksum` AT special-category suite.
