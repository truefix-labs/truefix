# Phase 1 Data Model: Third/Fourth-Pass Audit Remediation (docs/todo/005.md)

**Feature**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

This is a bug-fix remediation feature — no new domain entity is added. This translates the spec's
Key Entities plus research.md's decisions into grounded internal shapes for the structural changes
large enough to warrant one. The great majority of this feature's 90 FRs are same-shape logic fixes
(a changed condition, an added guard, a corrected field name) with no data-model impact; those are
covered directly in tasks.md, not repeated here. Every disclosed public-API surface growth from
research.md's summary is expanded below.

## Teardown reason (`crates/truefix-session/src/state.rs`) — `NEW-56`

```text
// BEFORE: enter_disconnected() has no way to know *why* it was called.
fn enter_disconnected(&mut self) -> Vec<Action> {
    if self.config.reset_on_disconnect || self.config.reset_on_logout {
        self.reset_sequences(true);
        ...
    }
}

// AFTER: reason threaded through every call site.
enum DisconnectReason {
    GracefulLogout,   // an inbound/outbound Logout completed the exchange
    Other,            // TCP drop, or any error-driven teardown
}

fn enter_disconnected(&mut self, reason: DisconnectReason) -> Vec<Action> {
    let should_reset = match reason {
        DisconnectReason::GracefulLogout => self.config.reset_on_logout,
        DisconnectReason::Other => self.config.reset_on_disconnect,
    };
    if should_reset {
        self.reset_sequences(true);
        ...
    }
}
```

- Internal-only (`DisconnectReason` is not `pub`) — no public API surface growth.
- Every call site that currently sets `self.state = SessionState::Disconnected` directly (the
  latency-failure, identity-failure, `validLogonState`-failure, `disconnect_on_error`, and
  dictionary-invalid-Logon branches research.md/`NEW-57`'s corrected framing identified as all
  funneling through one unconditional final `Event::Disconnected` dispatch) is unaffected in
  *timing* — they still just set state — but the eventual `enter_disconnected` call (from
  `run_connection`'s shutdown tail) must be told `DisconnectReason::Other` in all of those cases,
  and `DisconnectReason::GracefulLogout` only from the one path where a Logout exchange actually
  completed. The call site in `truefix-transport::run_connection` needs a way to know which reason
  applies — the simplest shape is for `Session` to track "did a graceful Logout exchange complete"
  as internal state (already implicitly known — `on_logout_msg`/the Logout-send path — just not
  currently surfaced) and have `run_connection` query it before calling `handle(Event::Disconnected)`.

## Mongo store (`crates/truefix-store/src/mongo.rs`) — `NEW-54`, `NEW-08`, `NEW-75`

```text
// NEW-54: fixed key — was `{ field: seq as i64 }` (literal "field"), now:
let update = match sender {
    true  => doc! { "$set": { "sender": seq as i64 } },
    false => doc! { "$set": { "target": seq as i64 } },
};

// NEW-08: new trait method override (mirrors SqlStore/MssqlStore/RedbStore/FileStore).
impl MessageStore for MongoStore {
    async fn save_and_advance_sender(&self, msg: &Message, seq: u64) -> Result<(), StoreError> {
        // start a Mongo session + transaction; insert into `messages`, $set `sender` in
        // `sessions` within the same transaction; commit (or abort on either failing).
    }
}

// NEW-75: ensure_session_row becomes an upsert (or the `sessions` collection gains a unique
// index on `session_id`, created alongside the existing `messages` index at connect time).
```

- No public trait shape changes — `save_and_advance_sender` already exists on the `MessageStore`
  trait (added by an earlier feature for the other backends); `MongoStore` is simply implementing
  the override every other backend already has.
- Requires `mongodb` crate's session/transaction API (already a workspace dependency at version
  `3`, which supports multi-document transactions against a replica-set/sharded deployment — a
  pre-existing operational requirement for Mongo transactions generally, not new to this feature).

## TLS trust store (`crates/truefix-transport/src/tls_config.rs`) — `NEW-11`, `NEW-95`

```text
// NEW-11: build_client_config's fallback changes from an empty store to the OS-native one.
fn build_client_config(spec: &TlsSpec) -> Result<ClientConfig, TlsConfigError> {
    let roots = match trust_store(spec)? {
        Some(store) => store,
        None => native_root_store()?,   // NEW: rustls-native-certs, replacing RootCertStore::empty()
    };
    ...
}

// NEW-95: load_root_store/load_root_store_bytes track per-cert failures instead of `let _ = ...;`.
fn load_root_store(path: &Path) -> Result<RootCertStore, TlsConfigError> {
    let mut roots = RootCertStore::empty();
    let mut failed = 0usize;
    for cert in load_certs(path)? {
        if roots.add(cert).is_err() {
            failed += 1;
        }
    }
    if failed > 0 {
        tracing::warn!(failed, "trust store: {failed} certificate(s) failed to load");
    }
    if roots.is_empty() {
        return Err(TlsConfigError::EmptyTrustStore { path: path.display().to_string() });
    }
    Ok(roots)
}
```

- New error variant `TlsConfigError::EmptyTrustStore` (or equivalent) — additive to an existing
  `pub enum`, same disclosure category as `NEW-56`'s internal enum but this one **is** public
  (`TlsConfigError` is re-exported). Downstream code matching this enum exhaustively (only within
  `truefix-transport`/`truefix` today, per research.md) needs the new arm.

## Validation key registry (`crates/truefix-config/src/keys.rs`) — `NEW-10`

```text
// BEFORE
k("ValidateSequenceNumbers", "session", Impl),
k("RejectInvalidMessage", "session", Impl),

// AFTER — status downgraded, no field/struct change (per Clarifications: downgrade only)
k("ValidateSequenceNumbers", "session", Recognized),
k("RejectInvalidMessage", "session", Recognized),
```

- The other 5 keys in `NEW-10` (`ValidateFieldsHaveValues`, `ValidateUnorderedGroupFields`,
  `ValidateUserDefinedFields`, `AllowUnknownMsgFields`, `FirstFieldInGroupIsDelimiter`) map directly
  onto existing `ValidationOptions` fields already present on the struct — `resolve_validator` gains
  5 new `bool_key(...)` calls, no new fields.

## `ResolvedSession` (`crates/truefix-config/src/builder.rs`) — `NEW-96`

```text
// AFTER — one new field, parsed like every other bool_key in this struct.
pub struct ResolvedSession {
    ...
    pub log_message_when_session_not_found: bool,  // NEW
}
```

- Threaded into every `Services { ... }` construction site in `crates/truefix/src/lib.rs`
  (`Services` already has the matching field; it was simply never populated from config).

## `SessionConfig` defaults (`crates/truefix-session/src/config.rs`) — `NEW-73`, `NEW-89`

```text
// NEW-73: bare-struct default only (every .cfg-driven session already gets 30 via builder.rs).
reconnect_interval: u32 = 5 → 30

// NEW-89: builder.rs's .cfg-parsing default (this one DOES reach real deployments).
u32_key(map, "LogoutTimeout", &session, 10) → u32_key(map, "LogoutTimeout", &session, 2)
```

- Both are default-*value* changes, not shape changes. Disclosed in research.md's summary; no
  further modeling needed.

## AT harness fixed-identity acceptor mode (`crates/truefix-at/src/runner.rs`) — `NEW-83`/`FR-062`

```text
// NEW, additive alongside the existing dynamic-template start_acceptor(...):
pub fn start_fixed_identity_acceptor(
    sender_comp_id: &str,
    target_comp_id: &str,
    begin_string: &str,
    ...
) -> AcceptorHandle {
    // Unlike the dynamic template (which adopts whatever identity the first Logon claims), this
    // rejects a Logon whose SenderCompID/TargetCompID/BeginString don't match the fixed identity
    // configured here — enabling the `1c_InvalidSenderCompID`/`1c_InvalidTargetCompID`/
    // wrong-BeginString-on-Logon scenario category.
}
```

- Additive — existing `start_acceptor` and every scenario using it are untouched (per
  Clarifications).

## Async log writer lifecycle (`crates/truefix-log/src/{sql,mssql,mongo,redb}.rs`) — `NEW-91`, `NEW-24`

```text
// BEFORE: fire-and-forget, no handle retained, unbounded channel.
pub fn connect_with_config(...) -> impl Log {
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move { while let Some(entry) = rx.recv().await { ... } });
    LogHandle { tx }
}

// AFTER: bounded channel (NEW-24) + retained JoinHandle + explicit flush/shutdown (NEW-91).
pub fn connect_with_config(...) -> impl Log {
    let (tx, mut rx) = mpsc::channel(BOUND);   // bounded — backpressure on a slow/failed DB
    let task = tokio::spawn(async move { while let Some(entry) = rx.recv().await { ... } });
    LogHandle { tx, task: Some(task) }
}

impl LogHandle {
    pub async fn shutdown(self) {
        drop(self.tx);                          // closes the channel — writer drains and exits
        if let Some(task) = self.task {
            let _ = task.await;                  // await drain completion
        }
    }
}
```

- **Disclosed public-API surface growth**: each async log backend's handle type gains a `shutdown`
  (or equivalent `flush`) async method; `Engine::shutdown()` (already async) calls it for every
  configured log backend before returning, alongside its existing session-task teardown from
  feature 007.
- Channel bound (`BOUND`) is an internal constant, not user-configurable in this feature (no FR
  requests a config key for it).

## `Engine::start` acceptor-group ordering (`crates/truefix/src/lib.rs`) — `NEW-92`

```text
// BEFORE: HashMap<SocketAddr, Vec<ResolvedSession>> iterated in random order.
for (addr, members) in acceptor_groups { ... }

// AFTER: sorted before iterating — no type change, just an added sort at the call site.
let mut groups: Vec<_> = acceptor_groups.into_iter().collect();
groups.sort_by_key(|(addr, _)| *addr);
for (addr, members) in groups { ... }
```

- No structural change — `acceptor_groups` itself can remain a `HashMap` (it's still convenient for
  construction); only the iteration order at startup is made deterministic.

## MSSQL trust-certificate opt-in (`crates/truefix-store/src/mssql.rs`, `crates/truefix-log/src/mssql.rs`) — `NEW-90`

```text
// NEW config surface — mirrors the JDBC-URL TrustServerCertificate convention.
struct MssqlConnectOptions {
    ...
    trust_server_certificate: bool,  // NEW, default true (preserves today's trust_cert() behavior)
}

// call site:
if !opts.trust_server_certificate {
    // real validation: do not call config.trust_cert()
} else {
    config.trust_cert();
}
```

- Additive field with a backward-compatible default (`true` = today's behavior unchanged) — no
  existing deployment's behavior changes unless it opts in.
