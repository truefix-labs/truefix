# Contract: AT Harness Coverage, Performance Hygiene, and Config Defaults

**Requirements**: FR-058–FR-065 (US3, AT harness); FR-042, FR-047, FR-048 (US2); FR-073–FR-075,
FR-080, FR-081, FR-083 (US3, protocol/hygiene/defaults)
**Research**: research.md §R3 | **Data model**: `data-model.md` (fixed-identity acceptor mode,
`SessionConfig` defaults)

## AT harness internals (US3)

| FR | `NEW-ID` | Contract | Test type |
|---|---|---|---|
| FR-058 | 15, 16 | `read_message` distinguishes timeout / decode-failure / clean disconnect as three outcomes | Unit (harness-internal) — removes false-positive risk in existing `ExpectDisconnect` scenarios |
| FR-059 | 28 | `start_acceptor` provides a validator/dictionary for all 9 FIX versions, not just 4.2/4.4 | Extends existing field-validation scenarios to run against all 9 versions — **raises the 424-run floor** |
| FR-060 | 29 | Latency/bad-timestamp scenarios assert `Text(58)`/reject-reason, not just "a Logout occurred" | Strengthens existing scenario assertions — no new runs |
| FR-061 | 30 | `run_scenario` asserts the read buffer is empty after all steps | Harness-internal, applies to every existing scenario — no new runs, but tightens a shared assertion |
| FR-062 | 83 | New fixed-identity acceptor mode (see data-model.md), used only by the new identity-rejection scenarios | New scenarios (`1c_InvalidSenderCompID`/`1c_InvalidTargetCompID`/wrong-BeginString) — **raises the 424-run floor** |
| FR-063 | 50 | `check_match` flags unexpected/extra fields, not only missing expected ones | Harness-internal — no new runs, tightens a shared assertion |
| FR-064 | 51 | AT scenarios verify outbound `MsgSeqNum(34)` ordering where not already checked | Strengthens existing scenarios — no new runs |
| FR-065 | 52 | `server_acceptance_suite_passes` reports per-scenario pass/fail, not all-or-nothing | Harness-internal test-reporting change, no behavioral change to scenarios themselves |

All of the above are `truefix-at`-internal (harness robustness), not new production-code protocol
behavior — except `FR-059`/`FR-062`, which extend the harness's *coverage* of existing production
behavior (dictionary validation for 7 more FIX versions; Logon-time identity rejection) rather than
correcting a defect in the harness's assertions. Confirm exact new-scenario counts for `FR-059`/
`FR-062` at `/speckit-tasks` time.

## Genuinely protocol-behavioral P3 items (US3 by severity, but still need AT coverage)

## Session-level `Reject` before `Logout` (FR-073, `NEW-87`)

**Contract**: A non-Logon message triggering a latency, CompID-mismatch, or
PossDup-falsification disconnect sends a session-level `Reject` (correct `SessionRejectReason`)
before the `Logout`, matching QuickFIX/J's two-message sequence.

**Protocol-behavioral**: yes (wire-observable message sequence change). **AT scenario required** —
each of the three trigger conditions needs its expected-message-sequence assertion updated/extended.

## `DefaultApplVerID` validation (FR-074, `NEW-88`)

**Contract**: Under FIXT 1.1, an inbound Logon's `DefaultApplVerID(1137)` is validated against the
transport dictionary's known `ApplVerID` values before being accepted; a mismatch rejects the Logon
via the existing `reject_logon` path.

**Protocol-behavioral**: yes. **AT scenario required** — a FIXT 1.1 Logon with an invalid/typo'd
`DefaultApplVerID` must now be rejected.

## Config defaults (FR-042, FR-075; see data-model.md)

| FR | `NEW-ID` | Contract | Test type |
|---|---|---|---|
| FR-042 | 73 | `SessionConfig::new()`'s bare `reconnect_interval` default changes from 5 to 30 | Unit (default-value assertion) — no `.cfg`-driven deployment's behavior changes (already 30 via `builder.rs`) |
| FR-075 | 89 | `builder.rs`'s `.cfg`-parsing default for `LogoutTimeout` changes from 10 to 2 | Unit (default-value assertion) — **this one changes real `.cfg`-driven deployments** that don't set `LogoutTimeout` explicitly; flag prominently in the task/PR description per research.md's disclosure |

## Codec allocation/complexity hygiene (FR-047, FR-048, FR-080, FR-081, FR-083 — implemented in this feature per Clarifications)

| FR | `NEW-ID` | Contract | Test type |
|---|---|---|---|
| FR-047 | 80 | `render_members_ordered` emits a member at most once even if its tag repeats in `field_order` | Unit — fix alongside FR-083's `NEW-38` complexity fix in the same function (research.md §R3) |
| FR-048 | 81 | `FieldMap::set` removes stale duplicate entries for the tag it updates | Unit |
| FR-080 | 39 | `SessionId::Display` includes `SubID`/`LocationID` when present | Unit (log-string assertion) |
| FR-081 | 47 | `UnresolvedVariable` config errors report the original source line, not always `line: 0` | Unit |
| FR-083 | 35, 36, 37, 38 | Manual byte-scanning, decode/encode allocation counts, and `render_members_ordered`'s complexity are each reduced; decode/encode/framing behavior (accepted/rejected inputs, wire output) is unchanged | Unit — existing codec test suites must continue to pass unmodified (behavior-preserving refactor), plus new tests asserting the specific inefficiency is gone where practical (e.g. allocation counting, `#[bench]`/criterion micro-benchmark, or at minimum a code-level assertion the new code path is used) |

None of this group is a wire-protocol-behavior correction — no new AT scenario expected; existing
codec/session/config unit and integration tests are the primary regression net, extended with new
cases per FR above.
