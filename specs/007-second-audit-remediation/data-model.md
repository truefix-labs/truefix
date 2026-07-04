# Phase 1 Data Model: Second-Pass Audit Remediation (docs/todo/004.md)

**Feature**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

This is a bug-fix remediation feature, not a data-model-introducing one — no new domain entity is
added. This translates the spec's two Key Entities plus research.md's concrete decisions into
grounded internal shapes. Every change here is additive or internal-behavior-only; the three
disclosed public-API surface growths (all additive, no existing signature narrows or removes) are
called out explicitly.

## Sequence-number store record (`crates/truefix-store/src/file.rs`)

```text
// BEFORE: one combined file, two text lines.
struct SeqFile {
    path: PathBuf,           // one path: "<store-dir>/seqnums"
    state: Mutex<(u64, u64)>,
    sync: bool,
}

// AFTER: two independent files, each holding one value.
struct SeqFile {
    sender_path: PathBuf,    // "<store-dir>/senderseqnums"
    target_path: PathBuf,    // "<store-dir>/targetseqnums"
    state: Mutex<(u64, u64)>,
    sync: bool,
}
```

- `write()`'s single combined-file truncate-then-write is replaced by two independent per-file
  atomic writes (write-to-temp-path, then rename over the real path) — `set_sender`/`set_target`
  each touch only their own file, not both, avoiding an unnecessary rewrite of the value that didn't
  change.
- `reset()` deletes and recreates both files wholesale (not merely truncates), matching QuickFIX/J's
  `closeAndDeleteFiles()`-then-reinitialize semantics (research.md §R1.1).
- `load_seqnums`-equivalent logic gains a distinct "file present but fails to parse" error path
  (previously indistinguishable from "file absent") — surfaces as a typed `StoreError` at
  `SeqFile::open` time.
- **New**: on `SeqFile::open`, if neither `sender_path` nor `target_path` exists but the legacy
  combined path (`<store-dir>/seqnums`) does, read and split it into `(sender_path, target_path)`
  before proceeding with the normal open logic — the migration, run once, is then indistinguishable
  from a normal two-file open for every subsequent operation. The legacy file is left in place
  (not deleted), so a downgrade to an older TrueFix build still finds it.
- No public API changes — `SeqFile` is a private implementation detail of `FileStore`/
  `CachedFileStore`; `MessageStore`'s own trait surface (`next_sender_seq`/`next_target_seq`/
  `set_next_sender_seq`/`set_next_target_seq`/`reset`) is unchanged.

## Session connection lifecycle event (`crates/truefix-session/src/state.rs`)

```text
// BEFORE
pub enum Event {
    Connected,
    Received(Message),
    Tick,
    StartLogout,
    Garbled,
}

// AFTER (additive — one new variant)
pub enum Event {
    Connected,
    Received(Message),
    Tick,
    StartLogout,
    Garbled,
    Disconnected,   // NEW: an unexpected TCP-level disconnect, distinct from a Logout-driven one
}
```

- **Disclosed public-API surface growth**: `Event` is `pub`, so this is visible to any external
  consumer matching on it exhaustively — but `Event` is constructed and matched only within
  `truefix-session` (the state machine's own `handle()` dispatch) and `truefix-transport` (feeding
  events in), both first-party crates this same feature already touches; no downstream crate breaks.
- `Session::handle(Event::Disconnected)` routes to the existing `enter_disconnected()` (unchanged
  itself), returning whatever `Action`s that produces (primarily `Action::ResetStore` when
  `reset_on_disconnect`/`reset_on_logout` is configured) through the same action-return contract
  every other `Event` variant already uses.
- `crates/truefix-transport/src/lib.rs`'s `run_connection` dispatches `Event::Disconnected` in its
  shutdown tail, before the `Session` value is dropped — previously this path called no
  `session.handle(...)` at all.

## `SessionHandle` (`crates/truefix-transport/src/lib.rs`)

```text
// BEFORE
impl SessionHandle {
    pub async fn logout(&self);
    pub async fn send(&self, message: Message);
    pub async fn join(self);  // consuming
}

// AFTER (additive — two new methods, both non-consuming and synchronous)
impl SessionHandle {
    pub async fn logout(&self);
    pub async fn send(&self, message: Message);
    pub async fn join(self);
    pub fn abort(&self);              // NEW: synchronous, non-graceful task abort
    pub fn is_finished(&self) -> bool; // NEW: synchronous, non-consuming liveness check
}
```

- **Disclosed public-API surface growth**: two additive methods, delegating directly to the
  already-private `task: JoinHandle<()>` field's own existing synchronous `abort()`/`is_finished()`
  (no new capability at the `tokio` level — this is purely exposing what `JoinHandle` already offers
  through `SessionHandle`'s public surface).
- `abort()` is used by `Engine::shutdown()` (research.md §R1.6) to stop plain (non-failover)
  initiators from a synchronous context.
- `is_finished()` is used by `run_scheduled_initiator` (research.md §R1.7) to detect a dropped
  connection and clear its `current: Option<SessionHandle>` slot, re-enabling reconnect.

## `Engine` (`crates/truefix/src/lib.rs`)

```text
// BEFORE: no Drop impl — dropping an Engine value silently detaches every spawned task.

// AFTER (additive — new trait impl, no field/method changes to the struct itself)
impl Drop for Engine {
    fn drop(&mut self) {
        // Best-effort, synchronous-only sweep: abort every acceptor, failover initiator, and
        // (new) plain initiator. Cannot perform a graceful async Logout from Drop — this is a
        // safety net against silent task leaks, not a substitute for calling `shutdown()`.
    }
}
```

- **Disclosed behavior growth**: previously-undefined "what happens if you drop an `Engine` without
  calling `shutdown()`" now has defined, non-leaking (if non-graceful) behavior. No existing public
  method's signature changes.
- `Engine::shutdown()` itself also grows to abort plain initiators (via `SessionHandle::abort()`
  above), closing the "cannot stop from the public sync API" half of `BUG-27`; `Drop` closes the
  "never called `shutdown()` at all" half.

## Registry active-connection tracking (`crates/truefix-transport/src/lib.rs`)

```text
// NEW internal state, not exposed publicly — added to the acceptor's shared Registry (or an
// equivalent shared structure alongside it).
active_connections: Arc<Mutex<HashSet<SessionId>>>,
```

- Checked-and-inserted in `route_and_run` immediately after a connection's routing `SessionId` is
  resolved (before `run_connection` is spawned); removed when that connection's task ends (a natural
  cleanup point already exists in `run_connection`'s own shutdown tail, alongside the new
  `Event::Disconnected` dispatch above).
- A second connection presenting an already-tracked `SessionId` is refused before its Logon is
  processed at all (research.md §R1.8).
- Internal only — no public type changes.
