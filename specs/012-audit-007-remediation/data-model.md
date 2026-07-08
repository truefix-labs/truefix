# Phase 1 Data Model: Audit 007 Remediation

This feature has no persisted business data model (it changes logging/storage plumbing, not
domain data). "Entities" here are the Rust types/fields introduced or extended, derived from
`spec.md`'s Key Entities section and `research.md`'s design decisions.

## `RetentionPolicy` (new, `truefix-log`)

Governs `NEW-157`. Added to `FileLogOptions` (`crates/truefix-log/src/file.rs`) as a new field,
composable per the clarification session (FR-004).

| Field | Type | Notes |
|---|---|---|
| `generations` | `Option<u32>` | `<name>.1`..`<name>.N`; `None` (default) keeps today's single-backup-overwrite behavior — unset MUST NOT change existing behavior (FR-004). |
| `roll_interval` | `Option<RollInterval>` | Time-based rolling (e.g. daily); `None` (default) disables it. Composable with `generations`. |

`RollInterval` is an enum (`Daily`, `Hourly`, or similar — exact granularity is an implementation
choice within FR-004's "date-stamped file naming" requirement; not user-facing beyond the
config surface).

**Validation rules**:
- `generations = Some(0)` is invalid (a policy that retains zero backups is indistinguishable from
  "no policy," so reject it explicitly rather than silently behaving like `None`) — surfaced as a
  `LogError`/`ConfigError` at construction/resolution time, not a panic (Constitution Principle I).
- Both fields may be `Some` simultaneously (composable, FR-004).

**Lifecycle**: rotation is triggered by *either* the existing size threshold (`max_size_bytes`,
`NEW-108`, unchanged) or a `roll_interval` boundary; `generations` governs pruning of files already
rotated, independent of what triggered the rotation.

**Recovery invariant** (FR-011, FR-013): on `RotatingFile::open()`, state (current generation
index, current interval) MUST be re-derived from what's on disk, not assumed from an in-memory
default — this is what makes crash-mid-rotation recovery and stale-interval catch-on-restart both
well-defined without a separate recovery log.

## `FileLogWriter` (new internal type, `truefix-log`)

The async background-writer counterpart to `FileLog`, mirroring `RedbLog`'s internal shape
(`tx: Mutex<Option<mpsc::Sender<Entry>>>`, `task: Mutex<Option<JoinHandle<()>>>`). Not part of the
public API — `FileLog`'s existing public constructors (`open`, `open_with_options`) and the `Log`
trait impl are unchanged in signature; only the internals move from synchronous inline writes to
channel-plus-background-task (`NEW-156`).

| Field | Type | Notes |
|---|---|---|
| `tx` | `Mutex<Option<mpsc::Sender<Entry>>>` | Bounded (capacity matching the crate's existing `ASYNC_LOG_CHANNEL_CAPACITY` convention); `None` after `shutdown()`. |
| `task` | `Mutex<Option<JoinHandle<()>>>` | Awaited and cleared by `shutdown()`. |

`Entry` (message direction + text, or event text) mirrors `SqlLog`/`RedbLog`'s existing `Entry`
enum shape.

## `StoreConfig::Custom` (extends existing enum, `truefix-store`)

| Variant | Payload | Notes |
|---|---|---|
| `Custom` | `Arc<dyn MessageStore>` | `Arc`, not `Box`, so `StoreConfig` (currently `#[derive(Debug, Clone)]`) keeps deriving `Clone`; `Debug` needs a manual/partial impl since `dyn MessageStore` isn't `Debug`. |

`build_store` passes this variant through as-is (no construction needed — it's already built).

## Log-side custom slot (extends `ResolvedSession`, `truefix-config`)

`ResolvedSession` (`crates/truefix-config/src/builder.rs`) gains a new optional field carrying a
caller-supplied `Log`, populated only via programmatic construction (never from `.cfg` text, since
a trait object cannot be expressed as a config value):

| Field | Type | Notes |
|---|---|---|
| `custom_log` | `Option<Arc<dyn Log>>` | Takes precedence over `log_kind`/`sql_log`/`log` in `Engine::start`'s existing resolution order (FR-006, FR-012). |

## Builder-level override (extends `Engine`, `truefix`)

A new `Engine` entry point (naming TBD in tasks/implementation, e.g.
`Engine::start_with_overrides`) accepting a caller-supplied override — conceptually a
`Services`-shaped value (reusing `truefix_transport::Services`, which already carries `store:
Option<Arc<dyn MessageStore>>` / `log: Option<Arc<dyn Log>>`) applied per session or globally,
layered on top of whatever `.cfg` resolution (including any `Custom(...)`/`custom_log` value)
already produced.

**Precedence rule** (FR-012): builder-level override > `Custom(...)`/`custom_log` > `.cfg`-driven
built-in backend selection.

## `ConfigError` new variant (`truefix-config`)

A new variant (e.g. `ConfigError::CustomBackendWithBuiltinOnlySetting { session, setting }`) raised
by `resolve_store`/`resolve_log` when a session combines `Custom(...)`/`custom_log` with a
built-in-only setting (`MaxFileLogSize`, the new retention fields) that has no effect on a custom
backend (FR-010) — participates in the crate's existing all-or-nothing resolution (one bad key
aborts the whole resolution, per `builder.rs`'s existing doc comment).

## Relationships

```text
FileLogOptions ──has──> RetentionPolicy
FileLog ──owns──> FileLogWriter ──sends `Entry` to──> RotatingFile (background task)
StoreConfig::Custom ──wraps──> Arc<dyn MessageStore>
ResolvedSession.custom_log ──wraps──> Arc<dyn Log>
Engine::start_with_overrides(..., override: Services) ──layers over──> ResolvedSession-resolved backend
resolve_store/resolve_log ──validates──> Custom(...) XOR built-in-only settings, else ConfigError
```
