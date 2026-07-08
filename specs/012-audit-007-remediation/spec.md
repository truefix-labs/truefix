# Feature Specification: Audit 007 Remediation

**Feature Branch**: `012-audit-007-remediation`

**Created**: 2026-07-07

**Status**: Draft

**Input**: User description: "按照docs/todo/007.md的内容，确认合适后 改进"

## Clarifications

### Session 2026-07-07

- Q: For `NEW-157`, should retention be implemented by extending `RotatingFile` directly, by adding
  a `tracing-appender`-backed rolling sink, or both? → A: Extend `RotatingFile`/`FileLogOptions`
  directly with a generation-count and/or time-based rotation policy; a `tracing-appender`-backed
  sink is out of scope for this feature.
- Q: For `NEW-158`, should the fix be a `Custom(...)` config enum variant, an explicit
  builder/override hook, or both? → A: Implement both — a `Custom(Arc<dyn MessageStore>)`/
  `Custom(Arc<dyn Log>)` variant on `StoreConfig`/`LogConfig`, and an explicit builder/override hook
  (e.g. `SessionConfig::with_store`/`with_log` or an `Engine::start_with_services` overload).
- Q: How should a background write/flush failure (e.g. disk full) be handled? → A: Surface it via
  structured logging/metrics (e.g. a `tracing` error event plus a counter), while keeping
  `on_incoming`/`on_outgoing`/`on_event` fire-and-forget/infallible to the caller.
- Q: Should generation-count and time-based rotation be composable (both active at once) or must an
  operator pick exactly one policy? → A: Composable — an operator may configure generation-count
  retention, time-based rolling, or both together (e.g. roll daily and cap at N retained files).
- Q: When a custom store/log backend is configured alongside built-in-only settings (e.g.
  rotation/`MaxFileLogSize`) that don't apply to it, what should happen? → A: Fail fast at startup
  with a clear, typed error rather than silently ignoring or only warning.
- Q: What recovery guarantee is required if the process crashes mid-rotation? → A: Rotation MUST be
  structured so that on restart exactly one file is unambiguously "active" and no log content is
  lost, at any crash point — not best-effort/manual-recovery.
- Q: Should SC-001's "no measurable scheduling delay" be quantified for automated testing? → A: Yes
  — bound it by the session's configured heartbeat tolerance: under a simulated slow/blocked disk
  write, other scheduled session work (e.g. heartbeat timers) must still fire within that tolerance.
- Q: Should the new retention config default to today's existing single-backup-overwrite behavior
  when unset, or ship with a new non-zero default applied automatically? → A: Preserve the current
  default — when no retention policy is configured, behavior stays exactly as today; operators must
  opt in to multi-generation/time-based retention explicitly.
- Q: If a session configures both a `Custom(...)` config variant and a builder-level override
  (`with_store`/`with_log`/`start_with_services`) for the same store/log slot, which takes effect?
  → A: The builder override wins — it layers on top of the `.cfg`-resolved configuration, consistent
  with how `Services` already overrides `.cfg`-resolved backends today.
- Q: If a configured time-based roll interval elapses while the process is stopped, what happens to
  the active file on the next write after restart? → A: Roll once to the current interval on the
  first write after restart (no gap files fabricated for missed intervals; the active file is always
  named for the interval containing "now").

## User Scenarios & Testing *(mandatory)*

`docs/todo/007.md` is a scoped follow-up audit (not a full-codebase re-audit) covering three
findings identified while reviewing `truefix-log`'s `FileLog` and its rotation behavior, plus a
related extensibility question: `NEW-156` (`FileLog` performs blocking file I/O directly on the
tokio executor), `NEW-157` (`FileLog` has no multi-generation or time-based rotation policy), and
`NEW-158` (the `.cfg`-driven `Engine::start` path cannot accept a caller-supplied `MessageStore`/
`Log` backend, unlike the lower-level `truefix::transport::Services` path which already supports
this). This feature converts the confirmed, actionable items from that audit into fixable,
independently testable requirements, following the same "audit-remediation" pattern used for
`docs/todo/005.md`/`006.md` (features 009/010).

### User Story 1 - Non-blocking file log writes (Priority: P1)

An operator running TrueFix with file-based message/event logging needs `FileLog` writes to never
block the tokio worker thread that is processing other sessions' heartbeats, timers, and message
traffic, even when the underlying disk is slow or contended.

**Why this priority**: This is the only finding in this audit with a direct, if usually latent,
production tail-latency/fairness risk — a blocking `write(2)` syscall on a shared runtime thread
can stall unrelated sessions. It is also the foundation the rotation work in Story 2 should be
built on, so it is fixed first.

**Independent Test**: Under simulated slow-disk conditions (e.g. a write that is artificially
delayed), verify that `FileLog` writes execute off the async runtime thread and that other
concurrent session tasks (heartbeat timers, message dispatch) continue to make progress without
being stalled by the slow write.

**Acceptance Scenarios**:

1. **Given** a session configured with `FileLog` for messages and/or events, **When**
   `on_incoming`/`on_outgoing`/`on_event` is invoked from an async session task, **Then** the
   underlying file write executes without blocking the calling task's executor thread for the
   duration of the disk operation.
2. **Given** `FileLog` is shutting down (engine shutdown or session teardown), **When** the shutdown
   path runs, **Then** all previously queued/buffered log lines are flushed to disk before shutdown
   is reported complete — no silent loss of already-accepted log entries.
3. **Given** log writes arrive faster than the disk can absorb them, **When** this sustained
   backlog occurs, **Then** the system applies bounded buffering/backpressure consistent with the
   other log backends (`SqlLog`, `MssqlLog`, `MongoLog`, `RedbLog`) rather than growing memory
   without bound.
4. **Given** both acceptor and initiator sessions configured with `FileLog`, **When** either role
   writes log entries under load, **Then** both roles get the same non-blocking behavior — this is
   not an acceptor-only or initiator-only fix.

---

### User Story 2 - Configurable file log/store rotation retention (Priority: P2)

An operator who needs bounded, auditable log retention (e.g. "keep 30 daily files" or "keep the
last N size-based generations") wants this natively from TrueFix's file-based logging, instead of
relying on external tooling that doesn't understand `FileLog`'s long-lived file handle.

**Why this priority**: Today, rotation (from the earlier `MaxFileLogSize` work) keeps exactly one
backup and overwrites it on every rotation, so operators cannot retain history across more than one
rotation cycle. This is a real but lower-urgency gap than Story 1's latency risk, and its
implementation should build on Story 1's non-blocking write path rather than being migrated onto it
later.

**Independent Test**: Configure a retention policy (generation count and/or time-based roll
interval) and verify, across multiple simulated rotation events, that old generations are retained
up to the configured limit and pruned beyond it, and/or that new dated files begin at the
configured interval boundary.

**Acceptance Scenarios**:

1. **Given** a configured retention generation count `N`, **When** the active log file rotates more
   than `N` times, **Then** at most `N` prior generations are retained on disk and generations
   beyond that limit are removed.
2. **Given** a configured time-based roll interval, **When** the interval elapses while the process
   is running, **Then** a new log file for the new interval begins automatically and prior files
   are preserved per the retention setting.
3. **Given** a rotation event occurs (either policy), **When** it completes, **Then** no log content
   already accepted before the rotation is lost or corrupted, and new writes land in the new active
   file.
4. **Given** both acceptor and initiator sessions using file-based logs, **When** either role's log
   rotates, **Then** the retention policy applies identically to both roles.

---

### User Story 3 - Discoverable custom store/log injection via `.cfg`-driven `Engine::start` (Priority: P3)

A developer wiring TrueFix through `SessionSettings`/`.cfg` config and `Engine::start` — the path
shown in the README/quickstart — wants a supported way to plug in a custom `MessageStore` and/or
`Log` implementation (e.g. a proprietary audit sink, or a backend not on the built-in list) without
having to discover, by reading source code, that dropping down to the lower-level
`truefix::transport::{Acceptor, Services}` API is necessary.

**Why this priority**: This is an ergonomics/discoverability gap, not a functional blocker — the
`truefix::transport::Services` escape hatch already works today. It is independent of Stories 1-2
(it concerns the construction API, not `FileLog` internals), and is the lowest urgency of the three
findings.

**Independent Test**: Configure `Engine::start` (or its documented builder/override path) with a
custom `MessageStore` and/or `Log` implementation and verify session persistence/logging uses the
custom implementation instead of a built-in backend, for both acceptor and initiator roles.

**Acceptance Scenarios**:

1. **Given** a caller-supplied custom `MessageStore` implementation, **When** an engine/session is
   started through the `.cfg`-driven path with that backend specified, **Then** session persistence
   uses the custom implementation instead of any built-in store.
2. **Given** a caller-supplied custom `Log` implementation, **When** an engine/session is started
   through the `.cfg`-driven path with that backend specified, **Then** message/event logging uses
   the custom implementation instead of any built-in log.
3. **Given** a developer reading only the README/quickstart, **When** they look for how to supply a
   custom store/log backend, **Then** the documentation describes a supported path (whichever
   mechanism is implemented) without requiring them to read source code first.
4. **Given** both acceptor and initiator sessions, **When** either role is configured with a custom
   store/log through the `.cfg`-driven path, **Then** both roles honor the custom backend
   identically.

---

### Edge Cases

- If the disk fills up (or another I/O error occurs) while a queued/background-written log line is
  being flushed, the failure MUST be surfaced via structured logging/metrics rather than silently
  dropped, while the triggering `on_incoming`/`on_outgoing`/`on_event` call itself remains
  fire-and-forget/infallible to its caller.
- If a configured time-based rotation boundary elapses while the process is not running (e.g.
  restarted after two days with a daily policy), the system MUST roll once to the current interval
  on the first write after restart — it MUST NOT fabricate placeholder files for intervals that
  elapsed while stopped, and MUST NOT continue writing to a file whose name no longer matches the
  current interval.
- When a custom `MessageStore`/`Log` is configured alongside built-in-only settings that assume a
  built-in backend (e.g. `MaxFileLogSize`/rotation settings, which only apply to the built-in
  `FileLog`), startup MUST fail fast with a clear, typed error rather than silently ignoring the
  unsupported combination or only warning.
- If the process crashes mid-rotation (e.g. after renaming the old file but before reopening the
  new active file), on restart exactly one file MUST be unambiguously resumed as "active" and no
  previously-accepted log content is lost — this is a hard guarantee, not best-effort.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST perform `FileLog`'s `on_incoming`/`on_outgoing`/`on_event` file writes
  without blocking the calling async runtime thread for the duration of the underlying disk I/O
  (`NEW-156`).
- **FR-002**: System MUST provide an explicit flush/drain point (e.g. on engine or session
  shutdown) that guarantees all previously accepted log lines are persisted to disk before shutdown
  is reported complete (`NEW-156`).
- **FR-003**: System MUST apply bounded buffering/backpressure to `FileLog` writes consistent with
  the existing bounded-channel-backed log backends (`SqlLog`, `MssqlLog`, `MongoLog`, `RedbLog`)
  rather than allowing unbounded queuing under sustained write pressure (`NEW-156`).
- **FR-004**: System MUST support a configurable retention policy for file-based logs beyond the
  current single-backup-overwrite behavior, implemented by extending `RotatingFile`/
  `FileLogOptions` with a generation-count policy (`<name>.1` .. `<name>.N`, shifting on rotate
  instead of overwriting) and a time-based (date-stamped) rolling policy that an operator MAY
  configure independently or together (e.g. daily rolling capped at N retained files) (`NEW-157`).
  When no retention policy is configured, behavior MUST remain exactly the current single-backup-
  overwrite default — this is an opt-in capability, not a change to default behavior.
- **FR-005**: System MUST NOT lose or corrupt already-accepted log content across a rotation event,
  under whichever retention mechanism is implemented (`NEW-157`).
- **FR-006**: System MUST provide a supported, documented mechanism for a caller using the
  `.cfg`/`SessionSettings`-driven `Engine::start` path to supply a custom `MessageStore` and/or
  `Log` implementation, implemented as both: (a) a `Custom(...)`-style variant added to the
  `StoreConfig`/`LogConfig` enums so `.cfg` and programmatic construction can both express it, and
  (b) an explicit builder/override hook (e.g. `SessionConfig::with_store`/`with_log`, or an
  `Engine::start_with_services` overload) that layers a caller-supplied backend on top of the
  `.cfg`-resolved one (`NEW-158`).
- **FR-007**: System documentation (README/quickstart) MUST describe how to supply a custom
  store/log backend through the `.cfg`-driven entry point, regardless of which mechanism from
  FR-006 is implemented (`NEW-158`).
- **FR-008**: All of the above (non-blocking writes, rotation retention, custom backend injection)
  MUST work identically for acceptor and initiator sessions — none of these findings are role-
  specific.
- **FR-009**: System MUST surface background write/flush failures (e.g. disk full, I/O errors)
  through structured logging/metrics, while keeping `on_incoming`/`on_outgoing`/`on_event` itself
  fire-and-forget/infallible to the caller (`NEW-156`).
- **FR-010**: System MUST reject, at startup, a configuration that combines a custom (`Custom(...)`)
  store/log backend with built-in-only settings that don't apply to it (e.g. rotation/
  `MaxFileLogSize`), returning a clear, typed error rather than silently ignoring the unsupported
  combination (`NEW-158`).
- **FR-011**: System MUST structure rotation so that a crash at any point during the rotation
  sequence leaves exactly one file unambiguously resumable as "active" on restart, with no loss of
  previously-accepted log content (`NEW-157`).
- **FR-012**: When a session is configured with both a `Custom(...)` config variant and a
  builder-level override for the same store or log slot, the builder-level override MUST take
  effect, consistent with how `Services` already layers on top of `.cfg`-resolved configuration
  (`NEW-158`).
- **FR-013**: When a configured time-based roll interval elapses while the process is stopped, the
  system MUST roll once to the current interval on the first write after restart, without
  fabricating placeholder files for intervals that elapsed while stopped and without continuing to
  write to a file whose name no longer matches the current interval (`NEW-157`).

### Key Entities

- **`FileLog` / `RotatingFile`**: The existing hand-rolled file-based log writer and its rotation
  helper; the direct subject of `NEW-156` and `NEW-157`.
- **`StoreConfig` / `LogConfig` (and `LogKind`)**: The closed configuration enums resolved by
  `Engine::start` into built-in store/log backends; the subject of `NEW-158`'s extensibility gap.
- **`Services`**: The lower-level `truefix-transport` bag of optional `Arc<dyn MessageStore>`/
  `Arc<dyn Log>` overrides that already supports custom backends today, referenced as the existing
  pattern to reuse or surface for `NEW-158`.
- **Retention policy**: The new configurable state (generation count and/or roll interval) governing
  how many rotated log files are kept and how new ones are named, introduced for `NEW-157`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Under a simulated slow/blocked disk write, concurrently scheduled session work (e.g.
  heartbeat timers) continues to fire within the session's configured heartbeat tolerance —
  scheduling delay attributable to file log writes MUST NOT cause a heartbeat/timer miss, verified
  by an automated test.
- **SC-002**: In automated tests, zero log lines are lost across a normal shutdown/drain cycle,
  including when writes are in flight at the moment shutdown is requested.
- **SC-003**: An operator can configure retention such that file-based log storage does not grow
  unbounded over a multi-cycle run, verified by an automated test that drives multiple rotation
  events and asserts the on-disk file count/age stays within the configured policy.
- **SC-004**: A developer following only the README/quickstart can wire a custom store and/or log
  backend through the `.cfg`-driven entry point on the first attempt, without reading source code,
  verified by a documented example that is exercised by an automated test.
- **SC-005**: 100% of the accepted findings in this feature (`NEW-156`, `NEW-157`, `NEW-158`) have
  at least one automated test that fails before the corresponding fix and passes after it.

## Assumptions

- All three findings from `docs/todo/007.md` (`NEW-156`, `NEW-157`, `NEW-158`) are in scope for this
  single feature, consistent with how prior audit-remediation features (009/010) bundled all
  confirmed findings from their source todo document rather than splitting by finding size.
- `ScreenLog` and `TracingLog` are explicitly out of scope for the non-blocking-write fix (Story 1):
  the audit assesses them as lower risk (stdout/stderr and the `tracing` facade are typically
  non-blocking or buffered), and only `FileLog` does a real per-line `write(2)` syscall.
- No new external crate is assumed to be required; if the chosen rotation approach (FR-004) needs a
  helper dependency, its license MUST be verified compatible with Apache-2.0 OR MIT before adoption,
  per existing project dependency discipline.
- "Custom" store/log backends for `NEW-158` are still expected to implement the existing public
  `MessageStore`/`Log` traits — this feature is about the construction/wiring path, not about
  changing the trait contracts themselves.
