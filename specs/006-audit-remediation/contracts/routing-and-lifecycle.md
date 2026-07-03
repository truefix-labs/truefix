# Contract: Multi-Session Routing & Engine Lifecycle (US2, US5)

**Covers**: FR-010 through FR-012 (US2), FR-021 through FR-023 (US5). **Source**: `docs/todo/003.md`
`BUG-07`, `GAP-20`, `B11`, `BUG-11`, `BUG-16`, `BUG-21`. **Grounding**: research.md §R2, §R5.

## US2 — Routing (`crates/truefix-transport/src/lib.rs`, `crates/truefix/src/lib.rs`)

| # | Change | Contract |
|---|---|---|
| 1 | `route_and_run`'s session lookup key | Extracts tags 50/142/57/143 from the inbound Logon and builds the key via `SessionId::new_full` (8 fields) instead of `SessionId::new` (3 fields, 5 hardcoded `None`) |
| 2 | `SessionQualifier`-distinguished sessions in one acceptor group | Config-resolve-time rejection with a clear `ConfigError` when two group members would produce an identical wire-extractable key (no live-routing ambiguity is silently tolerated) — resolved via `/speckit-clarify` |
| 3 | `AcceptorBuilder::bind` | Honors `SocketReuseAddress` (threaded from the caller's socket options before bind, not defaulted) — brings it to parity with `Acceptor::bind_with` |

**No breaking changes**: `SessionId::new_full` already exists (feature 002). No new public type.

## US5 — Engine lifecycle (`crates/truefix/src/lib.rs`)

| # | Change | Contract |
|---|---|---|
| 1 | `Engine::start` partial-failure return path | Before returning `Err`, aborts every already-started acceptor `JoinHandle` and stops every already-started `SessionHandle`/`ReconnectHandle` — no orphaned running session survives a failed `start()` call. `Result<Self, EngineError>` signature unchanged. |
| 2 | Grouped-acceptor `ContinueInitializationOnError` | `members.iter().all(...)` (strictest-wins: any `N` makes the group fail-fast) replaces `members.first()` — resolved via `/speckit-clarify` |
| 3 | `Engine::shutdown()` doc comment | Explicitly discloses that plain (non-failover) `self.initiators` are left running (sync/async mismatch — `SessionHandle::logout()` is async) and that callers needing to stop them must call `.logout()` on each handle themselves. **No behavior change.** |

**No breaking changes**: no new public method, no signature change on `Engine::start`/`shutdown`.

## Acceptor/Initiator parity

US2 is acceptor-only by construction (only acceptors route inbound connections by SessionID) —
correctly asymmetric, consistent with 005's precedent for acceptor-specific routing work. US5 applies
to `Engine::start`, which starts both acceptors and initiators — the cleanup-before-return fix (row 1
above) covers both session classes uniformly.
