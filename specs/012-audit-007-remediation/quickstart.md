# Quickstart: Validating Audit 007 Remediation

Prerequisites: repository checked out on branch `012-audit-007-remediation` (or with these changes
applied), Rust toolchain matching workspace `rust-version` (`1.96`), `cargo` on `PATH`.

## 1. Non-blocking `FileLog` writes (`NEW-156`)

```sh
cargo test -p truefix-log --test async_writer_flush_on_shutdown
```

Expected: this existing test currently exercises `RedbLog`'s channel+background-writer shutdown
contract; after this feature, an equivalent `FileLog` case in the same (or a sibling) test file
passes, proving `FileLog::shutdown()` flushes every queued entry before returning.

A new test asserting non-blocking behavior under a simulated slow disk (SC-001: other scheduled
session work still fires within the session's configured heartbeat tolerance) should be runnable
as:

```sh
cargo test -p truefix-log --test file_log_non_blocking
```

## 2. Retention policy (`NEW-157`)

```sh
cargo test -p truefix-log --test file_log_retention
```

Expected: a new test drives multiple rotation events (both a configured generation count and a
configured roll interval) and asserts:
- at most `N` prior generations remain on disk (FR-004),
- no content is lost across rotation (FR-005),
- a simulated crash mid-rotation still leaves exactly one unambiguous active file on next `open()`
  (FR-011),
- a roll interval that elapsed while "stopped" (simulated by manipulating the file's mtime/a fake
  clock) causes exactly one roll on the next write, with no fabricated gap files (FR-013).

Manual smoke check:

```sh
cargo run --example file_log_rotation -- --generations 3 --daily
```

(or the nearest equivalent example/binary added during implementation) should produce
`messages.log`, `messages.log.1` .. `messages.log.3` after forcing several rotations, with older
generations pruned.

## 3. Custom `MessageStore`/`Log` via `Engine::start` (`NEW-158`)

```sh
cargo test -p truefix --test config_start
cargo test -p truefix --test custom_backend_injection
```

Expected: a new test constructs a custom in-process `MessageStore`/`Log` implementation, starts an
`Engine` through the `.cfg`-driven path with `StoreConfig::Custom(...)`/the log-side equivalent
and/or the new builder-level override entry point, and asserts:
- session persistence/logging observably goes through the custom implementation (FR-006),
- when both the `Custom(...)` config and a builder override are set for the same slot, the builder
  override is the one actually used (FR-012),
- a session combining `Custom(...)` with a built-in-only setting (e.g. `MaxFileLogSize`) fails
  `Engine::start` with a clear, typed `ConfigError` rather than starting (FR-010),
- the same assertions hold for both an acceptor session and an initiator session (FR-008).

## 4. Documentation (FR-007)

```sh
grep -n "Custom" README.md
```

Expected: the README/quickstart documents how to supply a custom store/log backend through the
`.cfg`-driven entry point, per FR-007/SC-004.

## 5. Full regression

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

All of the above MUST pass before this feature is considered complete (Constitution's
Implementation/Review quality gate).
