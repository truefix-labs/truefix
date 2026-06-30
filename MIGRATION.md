# Migration Notes

Breaking-change and upgrade notes for TrueFix. Per Constitution Principle I, breaking changes to the
public API are recorded here with a semantic-version bump (FR-028).

## Unreleased — feature 002 (QuickFIX/J parity completion)

### Application callback signatures are now typed (breaking) — FR-016 / US5

The `Application` trait's fault-returning callbacks change from opaque `Result<(), String>` to typed
outcomes carrying a reason code and optional reference tag:

| Callback | Before | After |
|----------|--------|-------|
| `from_admin` | `Result<(), String>` | `Result<(), Reject>` (logon/admin refusal → 35=3) |
| `to_app`     | `Result<(), String>` | `Result<(), DoNotSend>` (suppress outbound) |
| `from_app`   | `Result<(), String>` | `Result<(), BusinessReject>` (→ 35=j) |

`Ok(())` is unchanged. An `Err("...")` becomes the matching typed variant:

- `Reject { reason, ref_tag, text }` — `reason` is the numeric SessionRejectReason (tag 373).
- `DoNotSend` — marker; the engine omits the message and does not store it as sent.
- `BusinessReject { reason, ref_tag, text }` — `reason` is the numeric BusinessRejectReason (tag 380).

These types are exported from `truefix_core` (re-exported by the `truefix` facade). Implementors update
their callback bodies to return the typed variants. No back-compat string variants are retained.

> Status: types landed (Setup/Foundational, stage G-foundational); trait signatures and engine effects
> are wired in stage G5 (US5). This section is finalised in T080 alongside the version bump.
