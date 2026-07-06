# Quickstart: Validating Feature 009

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Runnable scenarios that prove each user story works end-to-end. Run from the repo root.

> **Test file names below are placeholders** — `/speckit-tasks` hasn't run yet. These are plausible
> names following each crate's existing convention, not yet-created files; update this guide with
> the actual landed names once `/speckit-tasks`/`/speckit-implement` land them (standard practice in
> this repository — 006/007's quickstarts needed the same correction as a Polish task).
>
> `cargo test -- <substring>` filters match *test function names*, not file names — a filter that
> matches nothing silently reports `0 passed; ... ok`, a false-green. Use `--test <file>` (runs the
> named integration-test binary in full) as shown below.

## Prerequisites

- Rust toolchain per `rust-toolchain.toml` (1.96.0 pin), edition 2024 (workspace default).
- `cargo test --workspace --all-features` and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` green before validating any
  scenario below. AT suite baseline at `/speckit-plan` time (2026-07-05): 424/424 scenario runs
  (research.md §R3), 4/4 conformance tests passing.
- `mongodb` and `mssql` Cargo features are required for the Mongo/MSSQL-specific scenarios below
  (`cargo test -p truefix-store --features mongodb ...`, etc.) — no live MongoDB/MSSQL server is
  required for the config-plumbing-only tests; the transactional/durability tests do need one (a
  local `mongod --replSet` for Mongo transaction support, or an equivalent CI service container),
  matching the pattern already established in features 004/006/007.
- The new `rustls-native-certs` dependency requires no additional setup — it reads whatever
  certificate store the OS already has.

## User Story 1 — Protocol-correctness, data-loss, and security defects

```bash
cargo test -p truefix-store --features mongodb --test mongo_seq_persistence     # new — NEW-54
cargo test -p truefix-transport --test multisession_prelogon_dos_timeout        # new — NEW-93
cargo test -p truefix-session --test resend_begin_seq_zero                      # new — NEW-02
cargo test -p truefix-session --test acceptor_reset_on_logon                    # new — NEW-03
cargo test -p truefix-session --test sequence_reset_gapfill_verification        # new — NEW-84
cargo test -p truefix-core --test fixt_header_tags                             # new — NEW-55
cargo test -p truefix-transport --test fixt_group_aware_decode                  # new — NEW-58
cargo test -p truefix-session --test teardown_reset_reason                     # new — NEW-56
cargo test -p truefix-store --features mongodb --test mongo_atomic_save_advance # new — NEW-08
cargo test -p truefix-transport --test tls_native_trust_store_fallback          # new — NEW-11
cargo test -p truefix-session --test schedule_exit_sends_logout                # new — NEW-62
cargo test -p truefix-session --test dict_invalid_logon_rejection              # new — NEW-63
cargo test -p truefix-log --features mssql --test mssql_url_parsing_parity      # new — NEW-09
cargo test -p truefix-config --test validation_key_wiring                      # new — NEW-10
cargo test -p truefix-transport --test proxy_header_capture                    # new — NEW-12/13
cargo test -p truefix-transport --test scheduled_initiator_connect_nonblocking  # new — NEW-14
cargo test -p truefix-dict --test udf_validation_short_circuit                 # new — NEW-17
cargo test -p truefix-dict --test fix50_applverid_dispatch                    # new — NEW-06
cargo test -p truefix-dict --test multiple_char_value_per_token               # new — NEW-07
cargo test -p truefix-store --features mssql --test mssql_trust_cert_opt_in    # new — NEW-90
cargo test -p truefix-transport --test log_message_when_session_not_found      # new — NEW-96
cargo test -p truefix-at --test coverage
cargo test -p truefix-at --test conformance
```

Expected: a Mongo-backed session's sequence numbers survive a restart; a multi-session acceptor
connection sending nothing is disconnected within `logon_timeout` with a bounded receive buffer; a
`ResendRequest` with `BeginSeqNo=0` is answered; an acceptor with `ResetOnLogon=Y` resets on any
Logon; a gap-fill `SequenceReset` with a too-high/low `MsgSeqNum` is queued/rejected rather than
applied unconditionally; FIXT 1.1 transport-header tags decode into the message header and
header/trailer groups decode structurally; `reset_on_logout`/`reset_on_disconnect` no longer leak
into each other's teardown scenario; Mongo `save_and_advance_sender` is atomic; TLS without an
explicit trust store succeeds against a publicly-trusted-CA cert; a schedule-exit sends a Logout
before disconnecting; a dictionary-invalid Logon always gets Logout+disconnect; MSSQL log URL
parsing matches the store's; the 5 real validation keys are enforced from `.cfg`; PROXY v2 headers
up to 64 KiB and slow-arriving headers are captured correctly; a scheduled initiator's stop
flag/schedule boundary is observed even mid-connect; UDF validation fully skips when disabled; FIX
5.0/SP1/SP2 dispatch correctly by `ApplVerID`; `MultipleCharValue` validates per-token; MSSQL gains
an opt-in for real cert validation; `LogMessageWhenSessionNotFound` has an observable effect. AT
suite scenario-run count grows above 424 (exact new count confirmed at `/speckit-tasks`/Polish
closeout, per `contracts/README.md`'s AT-impact table).

## User Story 2 — Narrower-blast-radius defects

```bash
cargo test -p truefix-session --test pre_logon_admin_no_resend        # new — NEW-18
cargo test -p truefix-dict --test header_group_validation             # new — NEW-19
cargo test -p truefix-dict --test month_year_local_mkt_date_format     # new — NEW-20
cargo test -p truefix-core --test tag_zero_rejected                   # new — NEW-21
cargo test -p truefix-core --test group_count_round_trip               # new — NEW-22
cargo test -p truefix-store --test cached_file_store_read_through      # new — NEW-23
cargo test -p truefix-log --test bounded_async_channel                 # new — NEW-24
cargo test -p truefix-transport --test cipher_suite_typo_errors        # new — NEW-25
cargo test -p truefix-config --test dns_resolved_at_connect_time        # new — NEW-26
cargo test -p truefix --test engine_start_async_dns                    # new — NEW-27
cargo test -p truefix-session --test reset_sequences_clears_resend_state # new — NEW-32
cargo test -p truefix-transport --test socket_reuse_address_honored     # new — NEW-33
cargo test -p truefix-session --test sequence_reset_new_seq_zero        # new — NEW-34
cargo test -p truefix-dict --test empty_applverid_fallback              # new — NEW-64
cargo test -p truefix-dict --test cyclic_group_recursion_bound          # new — NEW-65
cargo test -p truefix-dict --test day_of_month_and_negative_ranges      # new — NEW-66/67
cargo test -p truefix-dict --test group_count_type_check                # new — NEW-68
cargo test -p truefix-dict --test empty_value_inside_group              # new — NEW-69
cargo test -p truefix-session --test gapfill_missing_new_seq_no          # new — NEW-70
cargo test -p truefix-session --test acceptor_heartbtint_omitted_no_dict # new — NEW-71
cargo test -p truefix-session --test reconnect_interval_default          # new — NEW-73
cargo test -p truefix-transport --test frame_resync_preserves_valid_frame # new — NEW-74
cargo test -p truefix-store --features mongodb --test mongo_session_unique_index # new — NEW-75
cargo test -p truefix-store --test bodylog_reset_lock_order              # new — NEW-76
cargo test -p truefix-store --test creation_time_parse_error             # new — NEW-77
cargo test -p truefix-core --test render_members_ordered_no_dup          # new — NEW-80
cargo test -p truefix-core --test fieldmap_set_removes_duplicates        # new — NEW-81
cargo test -p truefix-session --test too_high_logout_immediate           # new — NEW-85
cargo test -p truefix-session --test too_high_resend_request_not_replayed # new — NEW-86
cargo test -p truefix-transport --test proxy_unknown_address_consumes_bytes # new — NEW-94
cargo test -p truefix-at --test coverage
cargo test -p truefix-at --test conformance
```

Expected: each item's specific previously-broken edge case now behaves correctly, independently of
User Story 1's fixes; AT suite scenario-run count does not regress below the post-US1 count.

## User Story 3 — Hardening, tooling, and AT-harness coverage

```bash
cargo test -p truefix-dict --test fix_repository_group_converter_fix    # new — NEW-05
cargo test -p truefix-dict --test orchestra_map_type_parity              # new — NEW-60
cargo test -p truefix-dict --test build_rs_rerun_if_changed              # new/manual — NEW-61
cargo test -p truefix-dict --test codegen_parse_dict_errors              # new — NEW-78
cargo test -p truefix-dict --test codegen_enum_dedup                     # new — NEW-79
cargo test -p truefix-dict --test generate_code_hash_consistency          # new — NEW-82
cargo test -p truefix-at --test read_message_three_outcomes              # new — NEW-15/16
cargo test -p truefix-at --test all_versions_have_validators             # new — NEW-28
cargo test -p truefix-at --test latency_scenarios_assert_reject_reason    # new — NEW-29
cargo test -p truefix-at --test buffer_empty_at_scenario_end              # new — NEW-30
cargo test -p truefix-at --test fixed_identity_acceptor_scenarios         # new — NEW-83
cargo test -p truefix-at --test check_match_flags_extra_fields           # new — NEW-50
cargo test -p truefix-at --test outbound_seqnum_ordering_asserted        # new — NEW-51
cargo test -p truefix-at --test conformance                              # extended — NEW-52 (per-scenario reporting)
cargo test -p truefix-dict --test duplicate_dict_definitions_error       # new — NEW-43
cargo test -p truefix-dict --test strip_comment_preserves_hash_in_string # new — NEW-44
cargo test -p truefix-dict --test fix5_subversion_parsing                # new — NEW-45
cargo test -p truefix-config --test circular_var_reference_errors        # new — NEW-46
cargo test -p truefix --test monitor_removes_entry_on_disconnect         # new — NEW-48
cargo test -p truefix-transport --test http_connect_status_validation    # new — NEW-49
cargo test -p truefix-dict --test extend_detects_name_collision          # new — NEW-53
cargo test -p truefix-session --test reject_before_logout                # new — NEW-87
cargo test -p truefix-session --test defaultapplverid_validated          # new — NEW-88
cargo test -p truefix-session --test logout_timeout_default              # new — NEW-89
cargo test -p truefix-log --test async_writer_flush_on_shutdown          # new — NEW-91
cargo test -p truefix --test acceptor_group_start_order_deterministic    # new — NEW-92
cargo test -p truefix-transport --test tls_trust_store_load_diagnostics  # new — NEW-95
cargo test -p truefix-core --test decode_max_body_len_consistency        # new — NEW-04
cargo test -p truefix-session --test sessionid_display_includes_subid    # new — NEW-39
cargo test -p truefix-config --test unresolved_variable_line_number      # new — NEW-47
cargo test -p truefix-store --test bodylog_read_single_handle_per_range  # new — NEW-41
cargo test -p truefix-store --test redb_reset_no_intermediate_collect    # new — NEW-42
cargo test -p truefix-core --test decode_encode_allocation_hygiene       # new — NEW-35/36/37/38
```

Expected: dictionary-regeneration tooling matches the shipped hand-normalized `.fixdict` files and
the runtime parser's error handling; the AT harness gains a fixed-identity acceptor mode and
stricter shared assertions without any existing scenario's setup changing; config defaults for
`reconnect_interval`/`LogoutTimeout` match QuickFIX/J (the latter changing real `.cfg`-driven
behavior — see `contracts/hardening-and-tooling.md`); codec allocation/complexity hygiene items are
fixed with byte-for-byte unchanged wire behavior on the existing codec test suite.

## Final regression check

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p truefix-at --features mongodb,mssql --test conformance
cargo test -p truefix-at --features mongodb,mssql --test coverage
```

Expected: zero fmt/clippy diffs; full workspace test suite green; AT suite at or above the 424-run
floor with all 4 conformance tests passing and (per `FR-065`/`NEW-52`) individually-reported
per-scenario results within `server_acceptance_suite_passes`.
