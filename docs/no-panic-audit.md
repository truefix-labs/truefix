# No-Panic Audit (critical paths)

> Scope note (2026-07-20): this is the FIX-engine critical-path policy, not a proof that tests,
> examples, build scripts, or every broker client contain no panic sites. Rerun clippy for the
> revision being released.

Constitution Principle I and SC-005 require that the codec, session state machine, I/O, and timer
paths contain no reachable `panic!`/`unwrap`/`expect`/`unreachable!`/panicking index, and that all
recoverable errors are typed. This documents how that is enforced and reviewed (T097).

## Enforced by lints

Every library crate's `lib.rs` carries, for non-test builds:

```rust
#![cfg_attr(not(test), deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
))]
```

and the workspace forbids `unsafe_code`. The validation command is
`cargo clippy --workspace --all-targets -- -D warnings`,
so any `unwrap`/`expect`/`panic`/slice-index introduced on a library path fails the build. Tests and
examples are exempt (they may `unwrap`).

## Typed errors

- `truefix-core`: `DecodeError`, `FieldError` (thiserror). Decoding never panics — fuzz-style tests
  feed every byte value and arbitrary fragments (`tests/garbled.rs`).
- `truefix-dict`: `ParseError`, `ValidationError`.
- `truefix-store`: `StoreError`. `truefix-log`: `LogError`. `truefix-config`: `ConfigError`.
- `truefix-transport` returns `io::Result` from connectors; the per-connection loop maps all errors
  to a clean teardown.

## Manual review notes

- Codec (`truefix-core/src/codec`, `framing.rs`): all slicing uses `.get(..)`; integer reads use
  `checked_*`; no indexing. Verified by review + the lint.
- Session engine (`truefix-session/src/state.rs`): sans-IO, returns `Vec<Action>`; uses
  `saturating_add`, `filter`, and `Option` combinators; no panics.
- Transport: the connection loop ignores best-effort errors (`let _ = ...`) and converts fatal ones
  to teardown; locks use non-panicking `.lock().ok()` patterns in the monitor.
- The latency check uses wall-clock time (`OffsetDateTime::now_utc`) — the one intentionally
  real-time path; it cannot panic.

## Result

The listed FIX critical paths are intended to remain panic-free under the enforced lints. This
document records the policy and review scope; a green clippy run on the target revision is required
before asserting that the mechanical gate passes.
