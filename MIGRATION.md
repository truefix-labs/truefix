# Migration Notes

Breaking-change and upgrade notes for TrueFix. Per Constitution Principle I, breaking changes to the
public API are recorded here with a semantic-version bump (FR-028).

## Unreleased — feature 002 (QuickFIX/J parity completion)

### Application callback signatures are now typed (breaking) — FR-016 / US5

The `Application` trait's fault-returning callbacks change from opaque `Result<(), String>` to typed
outcomes carrying a reason code and optional reference tag:

| Callback | Before | After |
|----------|--------|-------|
| `from_admin` | `Result<(), String>` | `Result<(), Reject>` (admin/logon refusal → Logout + disconnect) |
| `to_app`     | `Result<(), String>` | `Result<(), DoNotSend>` (suppress outbound) |
| `from_app`   | `Result<(), String>` | `Result<(), BusinessReject>` (→ 35=j) |

`Ok(())` is unchanged. An `Err("...")` becomes the matching typed variant:

- `Reject { reason, ref_tag, text }` — returned from `from_admin`. The engine sends a Logout
  carrying `text` (if any) and disconnects. `reason`/`ref_tag` are not currently placed on the
  Logout (Logout has no SessionRejectReason field); they are reserved for callers who want to
  record structured context.
- `DoNotSend` — returned from `to_app`. The engine does not write the message to the wire and does
  not persist it to durable storage; the sequence number it consumed is gap-filled (not replayed)
  on a subsequent ResendRequest.
- `BusinessReject { reason, ref_tag, text }` — returned from `from_app`. The engine emits a
  Business Message Reject (35=j) carrying `reason` (BusinessRejectReason, tag 380), `ref_tag`
  (RefTagID, tag 371) if given, and `text` (tag 58). The inbound message's sequence number still
  advances normally.

These types are exported from `truefix_core` (re-exported by the `truefix` facade). Implementors
update their callback bodies to return the typed variants. No back-compat string variants are
retained.

**Example migration**:

```rust
// Before
async fn from_app(&self, msg: &Message, _id: &SessionId) -> Result<(), String> {
    if !self.known_symbol(msg) {
        return Err("unknown symbol".to_owned());
    }
    Ok(())
}

// After
async fn from_app(&self, msg: &Message, _id: &SessionId) -> Result<(), BusinessReject> {
    if !self.known_symbol(msg) {
        return Err(BusinessReject { reason: 99, ref_tag: None, text: Some("unknown symbol".to_owned()) });
    }
    Ok(())
}
```

Status: fully implemented (stage G5 / US5). Version bump: see workspace `Cargo.toml`/crate manifests.
