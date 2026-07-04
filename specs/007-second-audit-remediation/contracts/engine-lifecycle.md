# Contract: Engine Lifecycle (US1)

**Requirements**: FR-013, FR-012, FR-026, FR-014
**Research**: research.md §R1.6, §R1.7, §R1.11

## `Engine::shutdown()` / `Drop` stopping plain initiators (FR-013)

**Contract**: `Engine::shutdown()` stops every session task it started, including plain
(non-failover) initiators — via the new `SessionHandle::abort()` (data-model.md), synchronous and
non-graceful (no Logout is sent; this is a hard stop, not a courtesy disconnect — callers wanting a
graceful stop for plain initiators must call `.logout().await` on the handles from
`Engine::initiators()` themselves, as `shutdown()`'s doc comment already directs today for the
pre-fix gap). `impl Drop for Engine` performs the same abort-everything sweep as a safety net for
callers that never call `shutdown()` explicitly.

**Test obligation**: an integration test starting an `Engine` with a mix of acceptor + plain
initiator + failover initiator, calling `shutdown()`, and confirming zero running tasks remain
(matching spec SC-004 exactly) — extending the existing `continue_on_error.rs`-style pattern
(feature 006) that already exercises `Engine::start`'s partial-failure cleanup.

## Scheduled-initiator reconnect after a drop + retry backoff (FR-012, FR-026)

**Contract**: `run_scheduled_initiator`'s reconnect loop detects a dropped connection (via the new
`SessionHandle::is_finished()`, data-model.md) and clears its `current` slot so the existing
`was_in_session && current.is_none()` condition can re-fire. Failed connection attempts back off
across a small number of retries (matching the existing `ReconnectInterval`-step pattern already
used elsewhere in this crate) rather than polling at a fixed 200ms interval indefinitely — the
schedule-boundary check itself keeps its own fast, responsive cadence; only the failed-connect-retry
cadence backs off.

**Test obligation**: a `truefix-transport` integration test that establishes a scheduled-initiator
connection, force-drops it, and confirms reconnection happens on a subsequent loop iteration
(previously this would hang forever) — extending `scheduled.rs` (feature 006).

## `BodyLength=0` rejection (FR-014)

**Contract**: `frame_length` rejects a declared `BodyLength=0` as malformed, consistent with both
QuickFIX/J and QuickFIX/Go.

**Test obligation**: a `truefix-core` unit test (`framing_bounds.rs`-adjacent, feature 006's
existing file for frame-size-related tests) plus a `truefix-transport` live-connection test
confirming a `BodyLength=0` message closes the connection rather than being processed.
