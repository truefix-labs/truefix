# Phase 1 Data Model: Third-Pass Audit Remediation (docs/todo/005.md)

**Feature**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

This is a bug-fix remediation feature, not a data-model-introducing one — no new domain entity is
added. This translates the spec's Key Entities plus research.md's concrete decisions into grounded
internal shapes. Every change here is additive or internal-behavior-only; the one disclosed public
surface addition (a new external dependency, not a new public type) is called out explicitly.

## `MongoStore` sequence-number field write (`crates/truefix-store/src/mongo.rs`)

```text
// BEFORE — the `field` identifier is stringified by the doc! macro, not evaluated:
let field = if sender { "sender" } else { "target" };
let update = doc! { "$set": { field: seq as i64 } };   // always writes key "field"

// AFTER — the variable's value is used as the key:
let field = if sender { "sender" } else { "target" };
let update = doc! { "$set": { field.to_string(): seq as i64 } };
// (or: let mut set = Document::new(); set.insert(field, seq as i64);
//      let update = doc! { "$set": set };)
```

- No schema/shape change to the `sessions` collection document itself — `sender`/`target` were
  always the intended fields (`ensure_session_row` already seeds them at 1, and `reset()` in the
  same file already writes them correctly); this fix makes `set_seq` finally agree with the schema
  every other method already assumes.
- Per Clarifications (Session 2026-07-04): forward-fix only. A pre-existing row's stray literal
  `"field"` key (if any prior buggy write created one) is left untouched — no migration step reads
  or removes it. New code must not fail if that stray key happens to be present on an old row.
- No public API change — `MongoStore`'s `MessageStore` trait implementation is unchanged in
  signature.

## `MongoStore::save_and_advance_sender` (`crates/truefix-store/src/mongo.rs`)

```text
// BEFORE: no override — falls back to the MessageStore trait's default two-call sequence
// (save() then set_next_sender_seq()), each its own independent MongoDB operation.

// AFTER: a dedicated override using a MongoDB multi-document transaction, mirroring SqlStore's
// existing single-transaction pattern.
async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
    let mut session = self.client.start_session().await...;
    session.start_transaction().await...;
    // messages.update_one(...) [save] + sessions.update_one(...) [sender-seq advance],
    // both within `session`, then session.commit_transaction().
}
```

- No new public method — this overrides an existing default-provided trait method, closing the
  same crash-safety window `FileStore`/`CachedFileStore`/`SqlStore`/`MssqlStore`'s own overrides
  already close (established pattern, feature 007's `FR-018`/GAP-39).

## Session teardown reason (`crates/truefix-session/src/state.rs`)

```text
// NEW — internal only, not exported from the crate's public surface.
enum DisconnectReason {
    Logout,             // graceful, peer- or self-initiated Logout exchange completed
    TcpDrop,             // Event::Disconnected — an unexpected transport-level disconnect
    ScheduleExit,        // on_tick's schedule-window-exit branch
    HeartbeatTimeout,    // on_tick's missed-heartbeat branch
    LocalReset,          // an explicit local reset()/reset_on_error() call, not a teardown at all
}

// BEFORE
fn enter_disconnected(&mut self) -> Option<Action> {
    self.state = SessionState::Disconnected;
    if self.config.reset_on_disconnect || self.config.reset_on_logout {
        ...
    }
}

// AFTER
fn enter_disconnected(&mut self, reason: DisconnectReason) -> Option<Action> {
    self.state = SessionState::Disconnected;
    let should_reset = match reason {
        DisconnectReason::Logout => self.config.reset_on_logout,
        DisconnectReason::TcpDrop | DisconnectReason::ScheduleExit | DisconnectReason::HeartbeatTimeout
            => self.config.reset_on_disconnect,
        DisconnectReason::LocalReset => false, // reset() has its own separate, unconditional path
    };
    if should_reset { ... }
}
```

- Every existing call site of `enter_disconnected()` (the `Event::Disconnected` handler, `on_tick`'s
  schedule-exit and heartbeat-timeout branches) is updated to pass the appropriate variant. The
  graceful-Logout teardown path (currently building a `Logout` reply and setting
  `self.state = SessionState::Disconnected` directly in a few places, per `NEW-63`'s fix and the
  existing `on_logout_msg`) is routed through this same function with `DisconnectReason::Logout`,
  so `NEW-56`'s fix and `NEW-63`'s fix land together consistently.
- Exact variant set/name is this feature's own internal design choice (research.md's "Summary of
  decisions requiring disclosure" #3) — `/speckit-tasks` may adjust naming, but the *behavior*
  (each reason maps to exactly one of `reset_on_logout`/`reset_on_disconnect`/neither) is the
  FR-014 contract this shape must preserve.
- Not `pub` — `Event` (the public enum already carrying `Disconnected` since feature 007) is
  unaffected; this is purely an internal parameter to a private method.

## `ValidationOptions` (`crates/truefix-dict/src/model.rs` or wherever the type is defined)

```text
// BEFORE — 5 of 12 Impl-registered validation keys have a corresponding field, wired in builder.rs;
// 5 more have a field but are never read from builder.rs; 2 have no field at all.

// AFTER — the 5 currently-unwired-but-fielded keys become wired in builder.rs::resolve_validator
// (no new fields — validate_fields_have_values, validate_unordered_group_fields,
// validate_user_defined_fields, allow_unknown_msg_fields, first_field_in_group_is_delimiter already
// exist on ValidationOptions per NEW-10's own table). ValidateSequenceNumbers/RejectInvalidMessage
// are downgraded to Recognized in keys.rs unless /speckit-tasks finds a real field/effect
// warranted for either.
```

- No shape change to `ValidationOptions` itself is required for the 5 keys (fields already exist,
  confirmed in research.md §R1.16) — only `builder.rs`'s `resolve_validator` gains 5 new
  `bool_key(...)` reads. `keys.rs`'s classification for the other 2 changes from `Impl` to
  `Recognized` (a registry-metadata change, not a code-shape change).

## Config-key registry entry: `LogMessageWhenSessionNotFound` (`crates/truefix-config`)

```text
// NEW field on ResolvedSession (or acceptor-level equivalent config struct):
pub log_message_when_session_not_found: bool,

// Parsed in builder.rs::resolve_one via the existing bool_key(...) helper, defaulting to false
// (matching today's always-false behavior for any .cfg that doesn't set it).
```

- Threaded into every `Services { ... }` construction site in `crates/truefix/src/lib.rs` that
  currently uses `..Services::default()` — `Services::log_message_when_session_not_found` (already
  a real, consumed field per research.md's R2/R3 spot-check) receives the resolved value instead of
  always defaulting to `false`.

## Dependency addition: `rustls-native-certs`

```text
# Cargo.toml [workspace.dependencies] — new entry
rustls-native-certs = "..."   # exact version pinned at /speckit-tasks time
```

- Used only in `crates/truefix-transport/src/tls_config.rs`'s `build_client_config`, replacing the
  `RootCertStore::empty` fallback with the platform-native trust anchors this crate loads. No other
  crate depends on it. Disclosed in plan.md's Technical Context and research.md's "Summary of
  decisions requiring disclosure".
