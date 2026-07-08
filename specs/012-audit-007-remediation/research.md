# Phase 0 Research: Audit 007 Remediation

**Input**: `spec.md` (Requirements, Clarifications) + repository inspection of
`crates/truefix-log`, `crates/truefix-store`, `crates/truefix-config`, `crates/truefix-transport`,
`crates/truefix`.

No Technical Context fields were left as `NEEDS CLARIFICATION` — the language/toolchain, dependency
set, and testing approach are all fixed by the existing workspace, and the 10 clarification-session
answers in `spec.md` already resolved every scope-shaping product decision. This document instead
records the *technical* decisions needed to turn those answers into a design, each grounded in a
pattern already present in the codebase (per Constitution Principle III's provenance discipline:
reuse this repo's own established shape, not an external port).

## NEW-156 — Non-blocking `FileLog` writes

**Decision**: Rebuild `FileLog` on the same bounded-`mpsc`-channel + background-`tokio::spawn`-task
shape already used by `SqlLog`, `MssqlLog`, `MongoLog`, and `RedbLog` (all in
`crates/truefix-log/src/`), rather than wrapping each `write_line` call in a bare
`spawn_blocking` per call.

**Rationale**:
- This is the fix direction the audit itself calls preferable (`docs/todo/007.md` NEW-156, option
  (a)), "for consistency with the rotation/backpressure semantics of the other backends and for
  keeping `on_*`'s current fire-and-forget, infallible-to-the-caller contract."
- The pattern is already proven in this crate three times over (`sql.rs`, `mssql.rs`, `redb.rs`,
  `mongo.rs`), including the exact `Log::shutdown()` override contract (drop the sender, await the
  writer task's `JoinHandle`) that FR-002 needs, and an existing test,
  `crates/truefix-log/tests/async_writer_flush_on_shutdown.rs`, already exercises this shape against
  `RedbLog` — the same test structure applies directly to `FileLog`.
- `RedbLog` additionally demonstrates the sub-pattern needed here: `redb`'s API is blocking, so each
  DB operation inside the background task is itself wrapped in `tokio::task::spawn_blocking`
  (`crates/truefix-log/src/redb.rs:145-168`). `std::fs::File::write`/`writeln!` are exactly the same
  kind of blocking call, so `FileLog`'s background task should apply the same inner
  `spawn_blocking` wrap around each `RotatingFile::write_line`, not just move the whole loop onto
  one dedicated task (a single `tokio::spawn`ed async task still runs on a shared worker thread,
  and moving it there merely relocates the blocking call rather than removing it from a worker
  thread — `spawn_blocking` is what actually hops it to the blocking thread pool).

**Alternatives considered**:
- *Bare `spawn_blocking` per `on_*` call* (audit's option (b), `RedbLog`-style-lite): simpler, but
  loses the shared bounded-backpressure contract (FR-003) and the explicit `shutdown()`/drain point
  (FR-002) that the channel shape gives "for free" — would need to be re-invented per-call.
- *`tracing-appender`-backed sink*: rejected for `NEW-156` by the same clarification that rejected
  it for `NEW-157` (out of scope for this feature; see below).

## NEW-157 — Retention policy (generation-count + time-based)

**Decision**: Extend `RotatingFile` (`crates/truefix-log/src/file.rs`) with a `RetentionPolicy`
carrying an optional generation count and an optional roll interval, composable per the
clarification session. `RotatingFile::rotate()` is restructured so a crash at any point leaves
exactly one unambiguous active file (FR-011): write to a temp/staging name is not needed since
`std::fs::rename` is atomic on the same filesystem — the existing `rename`-then-`open_append`
sequence in `rotate()` already has this property *if* the reopen step cannot itself corrupt state;
the fix must ensure the in-memory `RotatingFile` (size counter, generation index) is only mutated
after the filesystem operations that could fail have already succeeded, and that on next `open()`
the constructor re-derives state (current generation, current interval) from what's on disk rather
than trusting stale in-memory assumptions.

**Rationale**:
- Directly what the clarification session resolved: extend `RotatingFile`/`FileLogOptions`
  directly (not a `tracing-appender` sink), composable generation-count + time-based, default
  unchanged when unset (FR-004), atomic-on-crash (FR-011), and roll-once-on-restart-if-stale
  (FR-013).
- The existing `rotate()` (`crates/truefix-log/src/file.rs:147-158`) already establishes the
  rename-then-reopen shape and a "best-effort, don't block writes" comment for the rename step;
  this feature generalizes it from a single fixed `.1` backup to `.1..N` shifting, and adds a
  parallel date-stamped naming path, without discarding the existing size-triggered
  `MaxFileLogSize` behavior (`NEW-108`), which stays the trigger condition — retention governs what
  happens *to old files* once a rotation is triggered (by size and/or the new time-based trigger),
  not whether one happens.

**Alternatives considered**:
- *`tracing_appender::rolling` as a new `LogConfig` variant*: rejected by clarification — would
  buy an already-async, battle-tested rolling policy "for free," but changes `truefix-log`'s
  architecture (introduces a `tracing`-facade path for file logs alongside the existing hand-rolled
  one) for a bigger scope than this feature's clarified boundary.
- *Fabricating placeholder files for missed intervals* (rejected by clarification, FR-013): would
  give a complete file-per-interval record but writes empty files for time the process was down,
  adding disk churn with no log content.

## NEW-158 — Custom `MessageStore`/`Log` injection via `Engine::start`

**Decision**: Two changes, matching the two-part clarification answer:

1. Add `StoreConfig::Custom(Arc<dyn MessageStore>)` to the existing `StoreConfig` enum
   (`crates/truefix-store/src/lib.rs:119`), handled by `build_store` (`lib.rs:188`) as a direct
   pass-through. `StoreConfig` is `#[derive(Debug, Clone)]` today; `Custom` requires `Arc` (not
   `Box`) specifically so `Clone` stays derivable without a manual impl, and a hand-written `Debug`
   arm (`Custom(_) => f.debug_tuple("Custom").finish()`) since `dyn MessageStore` isn't `Debug`.
   On the log side, there is no single top-level `LogConfig` field on `ResolvedSession` today —
   `crates/truefix-config/src/builder.rs`'s `ResolvedSession` instead carries three independent
   optional specs (`log: Option<LogSpec>` for File, `sql_log: Option<SqlLogSpec>`, `log_kind:
   Option<LogKind>` for Screen/Tracing/Composite), resolved with `log_kind` > `sql_log` > `log`
   precedence in `Engine::start` (`crates/truefix/src/lib.rs:486-495`). The equivalent for a custom
   log is a fourth optional slot (e.g. `ResolvedSession.custom_log: Option<Arc<dyn Log>>` populated
   only via programmatic construction, since a trait object cannot come from a `.cfg` text key) that
   Phase 1's data model names explicitly, taking precedence over all three existing log specs —
   mirroring `StoreConfig::Custom`'s role for the store side while fitting the log side's actual
   (already-tri-state, not single-enum) shape.
2. Add an explicit builder/override hook that layers a caller-supplied backend on top of the
   `.cfg`-resolved one, reusing `truefix-transport`'s existing `Services`/`with_session_store`/
   `with_session_services` mechanism (`crates/truefix-transport/src/lib.rs:1850-1921`) rather than
   duplicating it — e.g. an `Engine::start_with_overrides` entry point accepting a caller-supplied
   `Services` (or a per-`SessionId` map of them) that `Engine::start` already has the internal
   plumbing to apply, since every session-start branch already builds a `Services` value.

**Precedence** (FR-012): the builder-level override wins over the `.cfg`-driven `Custom(...)`
variant when both are present for the same slot, because it is applied later in the same
call — consistent with how `Services` already overrides `.cfg`-resolved backends today (the
existing group-acceptor code already layers `AcceptorBuilder::with_session_store`/
`with_session_services` on top of a group-wide `Services`, so "later application wins" is already
this codebase's convention, not a new one).

**Fail-fast validation** (FR-010): when `Custom(...)` (or its log-side equivalent) is set for a
session alongside built-in-only settings that don't apply to it (`MaxFileLogSize`/the new retention
fields, which only make sense for the built-in `FileLog`), resolution MUST reject the configuration
with a new `ConfigError` variant at the same point `resolve_store`/`resolve_log`
(`crates/truefix-config/src/builder.rs`) already validate other cross-key constraints — this is
all-or-nothing resolution, matching the crate's existing "first invalid/missing key aborts" doc
comment (`builder.rs:1-4`).

**Rationale**: Both `MessageStore` and `Log` are already `#[async_trait] pub trait ... : Send +
Sync` with no sealed supertrait, and `build_store`/`build_log` already return
`Box<dyn MessageStore>`/`Box<dyn Log>`, proving object-safety — the audit's own finding. No new
abstraction is needed, only a hole in the existing enum/spec surface plus a documented entry point.

**Alternatives considered**:
- *`Custom` variant only, no builder hook*: simpler, but a `.cfg` value can never itself carry a
  live trait object, so a `Custom(...)` variant would still need to be constructed
  programmatically and threaded in some way — the builder hook ends up necessary regardless, so
  implementing only the enum variant doesn't actually avoid needing a second mechanism.
- *Builder hook only, no `Custom` variant*: works, but doesn't give `SessionSettings`/`.cfg`-driven
  programmatic code any way to *express* "this session uses a custom backend" in the resolved
  config value itself (e.g. for introspection/logging of what backend a session resolved to) —
  clarification explicitly chose both.

## Structured observability for background write/flush failures (FR-009)

**Decision**: On a background writer's I/O failure (disk full, permission error, etc.), emit a
`tracing::error!` event (matching `FileLog`'s existing NEW-124 stderr-print convention, upgraded to
structured `tracing`) and increment a `metrics` counter, consistent with the crate's existing
`tracing`/`metrics` dependencies (already in `workspace.dependencies`) — no new dependency needed.

**Rationale**: Matches Constitution Principle I's "MUST provide structured observability" mandate,
and generalizes `FileLog`'s existing `eprintln!`-based failure visibility (`crates/truefix-log/src/
file.rs:161-173`, NEW-124) to the new async writer task, where a bare `eprintln!` inside a detached
`tokio::spawn`ed task is easy to lose in production log aggregation.

## Summary of design surface

| Finding | Crate(s) touched | New public surface |
|---|---|---|
| NEW-156 | `truefix-log` | `FileLog`'s internals become channel-backed; `Log::shutdown()` gains a real override (already the trait's designed extension point) |
| NEW-157 | `truefix-log` | `FileLogOptions` gains retention fields (generation count, roll interval) |
| NEW-158 | `truefix-store`, `truefix-config`, `truefix-transport`/`truefix` | `StoreConfig::Custom`, a log-side custom slot on `ResolvedSession`, a new `ConfigError` variant, an `Engine` override entry point |

No new external crate dependencies are required for any of the three findings.
