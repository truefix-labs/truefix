# Contract: Typed Application Callback Outcomes (US5; FR-016)

**Supersedes** the feature-001 `application-api` callback signatures. **Breaking change** (pre-1.0);
ships with `MIGRATION.md` + a minor-version bump (Principle I).

## Surface (replaces `Result<(), String>`)

```text
trait Application {
    async fn from_admin(&self, msg, id) -> Result<(), Reject>;        // logon/admin refusal
    async fn to_app(&self, msg, id)    -> Result<(), DoNotSend>;      // suppress outbound
    async fn from_app(&self, msg, id)  -> Result<(), BusinessReject>; // emit 35=j
    // on_create/on_logon/on_logout/to_admin unchanged
}
```

- `Reject{ reason: SessionRejectReason, ref_tag: Option<u32>, text: Option<String> }`
- `DoNotSend` (marker)
- `BusinessReject{ reason: BusinessRejectReason, ref_tag: Option<u32>, text: Option<String> }`

## Behaviour

1. `from_admin` returns `Err(Reject)` for a Logon ⇒ engine refuses the session (no logon response /
   disconnect), consistent with rejected-logon handling.
2. `to_app` returns `Err(DoNotSend)` ⇒ the outbound message is **not sent and not stored as sent** (it
   does not consume an outbound sequence number).
3. `from_app` returns `Err(BusinessReject)` ⇒ engine emits a Business Message Reject (35=j) carrying the
   supplied reason code and reference tag; the inbound sequence still advances.
4. `Ok(())` ⇒ normal processing.

## Acceptance (maps to spec US5 scenarios)

- Logon-validation `Reject` ⇒ session refused. ✔
- Outbound `DoNotSend` ⇒ message suppressed, seq not consumed. ✔
- Inbound `BusinessReject{reason, ref_tag}` ⇒ 35=j with that reason+tag. ✔

## Migration

`Result<(), String>` → typed results; `Ok(())` unchanged; `Err("...")` becomes the appropriate typed
variant. Documented in `MIGRATION.md` and the facade docs.

## Test hooks

`truefix-at`/session tests with an `Application` returning each variant; assert the engine's emitted
messages/effects.
