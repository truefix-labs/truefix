# Quickstart: Validating Feature 008

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Runnable scenarios that prove each user story works end-to-end. Run from the repo root.

> **Lesson from features 003/005/006/007's own quickstarts** (re-applied here): `cargo test --
> <substring>` filters match against *test function names*, not file names — a filter that doesn't
> match any `#[test] fn` silently reports `0 passed; ... ok`, a false-green. Every command below uses
> `--test <file>` (runs the named integration-test binary in full) instead.
>
> **Test file names below are placeholders** — `/speckit-tasks` hasn't run yet, so these are
> plausible names following each crate's existing convention, not yet-created files. Update this
> file with the actual landed names once `/speckit-tasks`/`/speckit-implement` land them (006/007's
> own quickstarts each needed exactly this correction as a Polish task).

## Prerequisites

- Rust toolchain per `rust-toolchain.toml` (1.96.0 pin).
- This feature branches from `007-second-audit-remediation`'s own state. Before starting
  implementation, run `cargo test --workspace --all-features` and record the passing test/AT count
  as this feature's own regression floor (feature 007's own quickstart, once its Polish phase closes,
  documents its closing baseline) — this feature MUST NOT decrease that count.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` MUST stay green throughout.
- One new dependency to add before implementation starts: `rustls-native-certs` (per Clarifications,
  Session 2026-07-04, closing `FR-049`) — run `cargo deny check` after adding it to confirm license
  compatibility (MIT/Apache-2.0 dual, matching TrueFix's own release license).

## US1 — Protocol-correctness, data-loss, and security defects

```bash
cargo test -p truefix-store --features mongo --test mongo_seq_field_write   # new — NEW-54
cargo test -p truefix-store --features mongo --test mongo_save_and_advance_sender  # new — NEW-08
cargo test -p truefix-session --test resend_request_begin_seq_zero   # new — NEW-02
cargo test -p truefix-session --test acceptor_reset_on_logon          # new — NEW-03
cargo test -p truefix-transport --test multi_session_pre_logon_timeout # new — NEW-93
cargo test -p truefix-session --test sequence_reset_gap_fill_verification  # new — NEW-84
cargo test -p truefix-core --test fixt_header_tags                    # new — NEW-55
cargo test -p truefix-transport --test classify_buffered_fixt_groups  # new — NEW-58
cargo test -p truefix-session --test teardown_reset_reason_scoping    # new — NEW-56
cargo test -p truefix-dict --features dict-tooling --test codegen_begin_string_stamping  # new — NEW-59
cargo test -p truefix-session --test schedule_exit_sends_logout       # new — NEW-62
cargo test -p truefix-session --test dictionary_invalid_logon_routing # new — NEW-63
cargo test -p truefix-dict --features dict-tooling --test crack_fix50_appl_ver_id  # new — NEW-06
cargo test -p truefix-dict --test multiple_char_value_allows          # new — NEW-07
cargo test -p truefix-log --features mssql --test mssql_url_semicolon_form  # new — NEW-09
cargo test -p truefix-config --test validation_keys_wired             # new — NEW-10
cargo test -p truefix-transport --test tls_default_trust_store        # new — NEW-11
cargo test -p truefix-transport --test proxy_v2_oversized_header      # new — NEW-12
cargo test -p truefix-transport --test proxy_slow_arrival_timeout     # new — NEW-13
cargo test -p truefix-transport --test scheduled_initiator_connect_nonblocking  # new — NEW-14
cargo test -p truefix-dict --test udf_validation_full_skip            # new — NEW-17
cargo test -p truefix-at --test coverage
cargo test -p truefix-at --test conformance
```
Expected: a `MongoStore`-backed session's sequence numbers persist correctly and atomically;
`BeginSeqNo=0` resend requests are answered; an acceptor with `ResetOnLogon=Y` resets on any Logon;
a silent/slow multi-session-acceptor connection is disconnected within `logon_timeout`; a gap-fill
`SequenceReset` with a too-high seq is queued, not applied blindly; FIXT 1.1 header tags route
correctly and FIXT dual-dictionary sessions get group-aware decode; session teardown honors
`reset_on_logout`/`reset_on_disconnect` independently; a schedule-exit sends a Logout before
disconnecting; a dictionary-invalid Logon is always rejected via Logout+disconnect; FIX 5.x messages
dispatch to the correct sub-version's typed struct; `MultipleCharValue` validates per-token; MSSQL
log URLs parse the semicolon form; the 7 validation config keys have real effects (or are correctly
downgraded); TLS works with no explicit trust store configured; oversized/slow-arriving PROXY headers
are handled; a hanging scheduled-initiator connect doesn't block stop/schedule detection; UDF
validation is fully skipped when disabled. AT suite scenario-run count grows from this feature's own
established baseline (see Prerequisites) as new scenarios land per each contract file's AT-scenario
determination.

## US2 — Confirmed defects with narrower blast radius

```bash
cargo test -p truefix-session --test pre_logon_admin_message_too_high_seq  # new — NEW-18
cargo test -p truefix-dict --test header_group_validation                 # new — NEW-19
cargo test -p truefix-dict --test month_year_local_mkt_date_format         # new — NEW-20
cargo test -p truefix-core --test tag_zero_rejected                       # new — NEW-21
cargo test -p truefix-core --test group_count_fidelity                    # new — NEW-22
cargo test -p truefix-log --test bounded_async_channels                   # new — NEW-24
cargo test -p truefix-transport --test cipher_suite_typo_detection        # new — NEW-25
cargo test -p truefix-transport --test dns_resolution_at_connect_time     # new — NEW-26
cargo test -p truefix --test async_dns_resolution                        # new — NEW-27
cargo test -p truefix-session --test reset_sequences_clears_resend_state  # new — NEW-32
cargo test -p truefix-transport --test acceptor_bind_reuse_address        # new — NEW-33
cargo test -p truefix-session --test sequence_reset_new_seq_no_zero       # new — NEW-34
cargo test -p truefix-dict --test application_for_empty_appl_ver_id       # new — NEW-64
cargo test -p truefix-dict --test cyclic_group_guard                      # new — NEW-65
cargo test -p truefix-dict --test day_of_month_range                      # new — NEW-66
cargo test -p truefix-dict --test length_seqnum_numingroup_negative       # new — NEW-67
cargo test -p truefix-dict --test group_count_type_check                  # new — NEW-68
cargo test -p truefix-dict --test empty_group_member_value_rejected       # new — NEW-69
cargo test -p truefix-session --test gap_fill_missing_new_seq_no          # new — NEW-70
cargo test -p truefix-transport --test frame_resync_preserves_valid_frame # new — NEW-74
cargo test -p truefix-store --features mongo --test mongo_session_row_uniqueness  # new — NEW-75
cargo test -p truefix-store --test body_log_reset_lock_ordering          # new — NEW-76
cargo test -p truefix-store --test creation_time_corrupt_file_errors     # new — NEW-77
cargo test -p truefix-core --test render_members_ordered_no_duplicate    # new — NEW-80
cargo test -p truefix-core --test field_map_set_no_stale_duplicate       # new — NEW-81
cargo test -p truefix-session --test logout_too_high_seq_processed_immediately  # new — NEW-85
cargo test -p truefix-session --test resend_request_not_double_answered  # new — NEW-86
cargo test -p truefix-store --features mssql --test mssql_trust_cert_opt_out  # new — NEW-90
cargo test -p truefix-transport --test proxy_unknown_header_bytes_consumed  # new — NEW-94
cargo test -p truefix-config --test log_message_when_session_not_found  # new — NEW-96
```
Expected: each targeted test demonstrates the specific narrow-condition previously-broken behavior
now works correctly (spec.md US2 acceptance scenarios 1-30).

## US3 — Hardening, tooling correctness, and test-coverage gaps

```bash
cargo test -p truefix-core --test decode_max_body_len_consistency        # new — NEW-04
cargo test -p truefix-dict --features dict-tooling --test fix_repository_group_delimiter  # new — NEW-05
cargo test -p truefix-store --test cached_file_store_read_through        # new — NEW-23
cargo test -p truefix-at --test coverage -- fix40_field_validation        # extend — NEW-28
cargo test -p truefix-at --test scenarios -- check_latency_logout_reason  # extend — NEW-29
cargo test -p truefix-at --test runner -- buffer_empty_at_scenario_end    # new — NEW-30
cargo test -p truefix-core --test session_id_display_sub_id              # new — NEW-39
cargo test -p truefix-dict --test parser_duplicate_detection             # new — NEW-43
cargo test -p truefix-dict --test strip_comment_hash_in_value            # new — NEW-44
cargo test -p truefix-dict --test parse_fix_begin_string_sub_versions    # new — NEW-45
cargo test -p truefix-config --test circular_variable_reference          # new — NEW-46
cargo test -p truefix-config --test unresolved_variable_line_number      # new — NEW-47
cargo test -p truefix-transport --test monitor_cleanup_on_disconnect     # new — NEW-48
cargo test -p truefix-transport --test http_connect_status_numeric       # new — NEW-49
cargo test -p truefix-at --test runner -- check_match_rejects_extra_fields  # new — NEW-50
cargo test -p truefix-at --test scenarios -- outbound_seq_num_ordering   # new — NEW-51
cargo test -p truefix-at --test conformance -- per_scenario_reporting    # new — NEW-52
cargo test -p truefix-dict --test extend_collision_detection            # new — NEW-53
cargo test -p truefix-dict --features dict-tooling --test orchestra_map_type_full  # new — NEW-60
cargo test -p truefix-dict --features dict-tooling --test codegen_rerun_if_changed # new — NEW-61
cargo test -p truefix-session --test heart_bt_int_absent_no_dictionary   # new — NEW-71
cargo test -p truefix-session --test reconnect_interval_bare_default     # new — NEW-73
cargo test -p truefix-dict --features dict-tooling --test parse_dict_error_parity  # new — NEW-78
cargo test -p truefix-dict --features dict-tooling --test codegen_enum_dedup       # new — NEW-79
cargo test -p truefix --features dict-tooling --test generate_code_hash_consistency  # new — NEW-82
cargo test -p truefix-at --test fixed_identity_acceptor                  # new — NEW-83
cargo test -p truefix-session --test reject_before_logout_on_failure_checks  # new — NEW-87
cargo test -p truefix-session --test default_appl_ver_id_validated       # new — NEW-88
cargo test -p truefix-session --test logout_timeout_default              # new — NEW-89
cargo test -p truefix --test log_writer_flush_on_shutdown                # new — NEW-91
cargo test -p truefix --test acceptor_group_startup_order_deterministic  # new — NEW-92
cargo test -p truefix-transport --test tls_trust_store_malformed_cert_diagnostic  # new — NEW-95
```
Expected: each targeted test exercises the specific low-priority hardening/tooling/harness-coverage
item (spec.md US3 acceptance scenarios 1-32). No shipped `.fixdict` file changes as a result of any
tooling test above (SC-005).

## Full regression pass

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p truefix-at --test coverage
cargo test -p truefix-at --test conformance
cargo deny check
```
Expected: zero regressions against this feature's own established baseline (Prerequisites), no
test-count decrease anywhere in the workspace, and no shipped `.fixdict` diff (SC-001-SC-006).
