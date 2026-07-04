# Quickstart: Validating Feature 007

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Runnable scenarios that prove each user story works end-to-end. Run from the repo root.

> **Lesson from features 003/005/006's own quickstarts** (re-applied here): `cargo test --
> <substring>` filters match against *test function names*, not file names — a filter that doesn't
> match any `#[test] fn` silently reports `0 passed; ... ok`, a false-green. Every command below uses
> `--test <file>` (runs the named integration-test binary in full) instead.
>
> **Test file names below are placeholders** — `/speckit-tasks` hasn't run yet, so these are
> plausible names following each crate's existing convention, not yet-created files. Update this
> file with the actual landed names once `/speckit-tasks`/`/speckit-implement` land them (006's own
> quickstart needed exactly this correction as its final Polish task, T092 — expect the same here).

## Prerequisites

- Rust toolchain per `rust-toolchain.toml` (1.96.0 pin).
- `cargo test --workspace --all-features` and `cargo clippy --workspace --all-targets --all-features
  -- -D warnings` green before validating any scenario below. Baseline at `/speckit-plan` time
  (2026-07-04): 118 test binaries / 578 tests passing (feature 006's closing state); AT suite
  405/405 scenario runs. **Updated at T118 (007 Polish closeout, same day)**: 174 test-result
  blocks / 716 tests passing; AT suite 424/424 scenario runs (the +19 runs and all new test files
  below are this feature's own additions — see T116's disclosure in `tasks.md`).
- No new external services or dependencies required (plan.md's Technical Context).

## US1 — Pre-production-blocking correctness, durability, and resource-safety fixes

```bash
cargo test -p truefix-store --test seqfile_migration      # new — legacy-to-two-file migration
cargo test -p truefix-store --test seqfile_crash_safety    # new — atomic-write crash injection
cargo test -p truefix-store --test sql_schema_migration_order  # new — BUG-26
cargo test -p truefix-store --features mssql --test mssql_commit_rollback  # new — BUG-41
cargo test -p truefix-session --test reset_seq_num_flag_handshake  # new — BUG-28
cargo test -p truefix-session --test resend_request_too_high  # new — BUG-29
cargo test -p truefix-transport --test duplicate_connection_refused  # new — BUG-32
cargo test -p truefix-transport --test disconnect_event  # new — BUG-33
cargo test -p truefix-transport --test callback_ordering  # new — BUG-34
cargo test -p truefix-transport --test scheduled_reconnect_after_drop  # new — BUG-25/94
cargo test -p truefix --test engine_shutdown_stops_all_initiators  # new — BUG-27
cargo test -p truefix-core --test framing_bounds  # extend (feature 006's file) — BUG-100
cargo test -p truefix-session --test schedule_enforcement  # new — BUG-86/87/88
cargo test -p truefix-session --test admin_dictionary_validation  # new — BUG-89
cargo test -p truefix-at --test coverage
cargo test -p truefix-at --test conformance
```
Expected: sequence-number crashes recover intact (or fail loudly, never silently defaulting); a
legacy-format store migrates transparently on open; a pre-existing-schema SQL database upgrades
cleanly; an MSSQL commit failure rolls back; the `ResetSeqNumFlag` handshake completes correctly on
both roles; a `ResendRequest`-vs-`ResendRequest` deadlock no longer hangs; a duplicate connection is
refused; an unexpected TCP drop honors `ResetOnDisconnect`; application callbacks never see a message
the session layer is about to reject; a dropped scheduled-initiator connection reconnects; `Engine::
shutdown()` leaves zero running tasks; `BodyLength=0` is rejected; an acceptor enforces its
configured schedule; admin messages are dictionary-validated. AT suite scenario-run count grows to
424 (see `docs/todo/004.md`'s per-item AT-requirement notes; the +19 runs vs. the 405 baseline are
disclosed in `tasks.md`'s T116 completion note).

## US2 — Real defects with narrower blast radius

```bash
cargo test -p truefix-session --test resend_requested_reset_on_reconnect  # new — BUG-35
cargo test -p truefix-session --test valid_logon_state  # new — BUG-42
cargo test -p truefix-session --test logon_edge_cases  # new — BUG-43/44/95
cargo test -p truefix-core --test data_field_verification  # new — BUG-38/49
cargo test -p truefix-transport --test reconnecting_socket_options  # new — BUG-39
cargo test -p truefix-transport --test acceptor_group_member_resolution  # new — BUG-61
cargo test -p truefix-session --test reject_field_correctness  # new — BUG-58/59
cargo test -p truefix-session --test poss_dup_equal_seq  # new — BUG-57
cargo test -p truefix-session --test reset_seq_num_flag_all_configs  # new — BUG-92/109
# BUG-94 (200ms tight-loop backoff) landed as an added test *within* US1's own
# scheduled_reconnect_after_drop.rs (see above), not a separate file — the placeholder name below
# never corresponded to a real file:
cargo test -p truefix-transport --test scheduled_reconnect_after_drop -- failed_connect_attempts_back_off
cargo test -p truefix-session --test sequence_reset_dequeue  # new — BUG-96
cargo test -p truefix-session --test stray_logon_rejected  # new — BUG-90
cargo test -p truefix-transport --test logout_callback_mid_handshake  # new — BUG-93
cargo test -p truefix-store --features mssql --test mssql_url_at_sign  # new — BUG-70
cargo test -p truefix-store --test file_store_open_memory_bound  # new — BUG-45/81
```
Expected: each targeted test demonstrates the specific narrow-condition previously-broken behavior
now works correctly (spec.md US2 acceptance scenarios 1-16).

## US3 — Low-priority hygiene and defensive hardening

```bash
cargo test -p truefix-core --test frame_checksum_overflow_hardening  # new — BUG-23/24
cargo test -p truefix-session --test heartbeat_timeout_multiplier  # new — BUG-36
cargo test -p truefix-dict --test group_recursive_validation  # new — BUG-54/55
cargo test -p truefix-dict --test char_field_version_leniency  # new — BUG-62
cargo test -p truefix-config --test precision_and_bool_parsing  # new — BUG-63/64
cargo test -p truefix-session --test resend_timestamp_precision  # new — BUG-66
cargo test -p truefix-session --test heartbeat_int_clamping  # new — BUG-67
cargo test -p truefix-session --test reset_on_error_low_seq  # new — BUG-68
cargo test -p truefix-session --test heartbeat_during_awaiting_logout  # new — BUG-69
cargo test -p truefix-session --test resend_request_infinity_version  # new — BUG-97
cargo test -p truefix-transport --test multi_endpoint_rotation_reset  # new — BUG-51
cargo test -p truefix-transport --test reconnect_handle_stop_mid_connection  # new — BUG-50
cargo test -p truefix-transport --test acceptor_first_message_must_be_logon  # new — BUG-52
cargo test -p truefix-transport --test admin_channel_backpressure  # new — BUG-53
cargo test -p truefix-core --test timestamp_leniency  # new — BUG-48/78
cargo test -p truefix-core --test checksum_position_verification  # new — BUG-46
cargo test -p truefix-core --test begin_string_format  # new — BUG-79
cargo test -p truefix-core --test decode_requires_msg_type  # new — BUG-80
cargo test -p truefix-dict --features dict-tooling --test codegen_hardening  # new — BUG-72/73/74
```
Expected: each targeted test exercises the specific low-probability edge case (spec.md US3
acceptance scenarios 1-20).

## Full regression pass

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p truefix-at --test coverage
cargo test -p truefix-at --test conformance
cargo deny check
```
Expected: zero regressions against the pre-feature baseline (405/405 AT scenario-runs at minimum, no
test-count decrease anywhere in the workspace — SC-005/SC-006).
