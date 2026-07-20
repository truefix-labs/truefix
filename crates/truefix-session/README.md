# truefix-session

Sans-I/O FIX session state machine.

## Current scope

It owns logon/logout, sequence numbers, resend and gap-fill decisions, heartbeat/test-request
timers, schedules/resets, validation decisions, and application callbacks. Inputs are `Event`s and
outputs are `Action`s; socket I/O and persistence are supplied by other crates.

Most applications should depend on the [`truefix`](https://crates.io/crates/truefix) facade crate.
See [Getting Started](../../docs/getting-started.md) for engine examples.

```sh
cargo test -p truefix-session
```
