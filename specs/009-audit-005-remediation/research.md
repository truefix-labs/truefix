# Phase 0 Research: Third/Fourth-Pass Audit Remediation

**Input**: `specs/009-audit-005-remediation/spec.md` (90 in-scope FRs, `docs/todo/005.md` as the
source audit).

## Method

`docs/todo/005.md` is itself the product of four independent full-codebase review passes, each
citing concrete `file:line` locations and cross-referencing QuickFIX/J (`quickfixj-core`) /
QuickFIX/Go source, and each later pass re-verifying the passes before it (refuting 4 items,
downgrading/narrowing 3, folding 1 into another). Rather than re-deriving all 90 items from
scratch, this research phase:

1. **Live-verified a representative, risk-weighted sample** directly against current source —
   every User Story 1 (P1) item that a spot-check could reach cheaply, plus a few illustrative
   User Story 2/3 items — to confirm the audit document's citations still hold against the
   *current* working tree (not just trust the document's own re-verification).
2. **Resolved the one open technical unknown**: which crate/dependency approach `NEW-11`'s default
   TLS trust store fix should take.
3. Defers exhaustive per-item file:line re-confirmation for the remaining ~70 items to
   `/speckit-tasks`, where each task will re-cite its own source location at the point of
   implementation — mirroring feature 007's precedent (its plan.md §Testing did the same:
   representative spot-verification at planning time, full per-item confirmation at task time).

## R1: Representative live verification (User Story 1 / P1 tier)

All of the following were re-read against the current working tree (not merely trusted from
`docs/todo/005.md`'s own text) during this research pass:

- **`NEW-54`** (Mongo `set_seq` literal-key bug): confirmed at
  `crates/truefix-store/src/mongo.rs:139-148` — `doc! { "$set": { field: seq as i64 } }` where
  `field` is a `&str` local; `bson::doc!`'s bare-identifier key syntax stringifies the identifier
  itself (`"field"`), not the variable's value. Exactly as described.
- **`NEW-93`** (multi-session acceptor pre-Logon DoS): confirmed at
  `crates/truefix-transport/src/lib.rs:1759-1798` (`route_and_run`) — the pre-Logon read loop has
  no `tokio::time::timeout` wrapper and `buf: Vec<u8>` has no size cap; `frame_length` returns
  `Ok(None)` (loop continues) for as long as no SOH byte has arrived. Exactly as described.
- **`NEW-55`** (`is_header` missing FIXT 1.1 transport tags): confirmed at
  `crates/truefix-core/src/tags.rs:27-65` — the `matches!` list includes `627`-`630` (`NoHops`,
  added by feature 006/GAP-26) but has no `1128`, `1129`, `1130`, `1156`, `1351`-`1355`. Exactly as
  described.
- **`NEW-10`** (7 unwired validation keys): confirmed at
  `crates/truefix-config/src/builder.rs:609-639` (`resolve_validator`) — only
  `validate_fields_out_of_order`, `validate_checksum`, `validate_incoming_message`,
  `allow_pos_dup`, `requires_orig_sending_time` are set from `.cfg`; the rest come from
  `ValidationOptions::default()`. Exactly as described.
- **`NEW-90`** (MSSQL `trust_cert()` unconditional): confirmed at
  `crates/truefix-store/src/mssql.rs:102,205` and `crates/truefix-log/src/mssql.rs:61` — all three
  call sites call `config.trust_cert()` unconditionally with no surrounding config check. Exactly
  as described.

**Decision**: Treat `docs/todo/005.md`'s remaining P1/P2/P3 citations as reliable for task-writing
purposes — the sample above (spanning session/transport/store/config/log, the crates with the
highest concentration of P1 items) confirms the document's fourth-pass re-verification claim holds
against current source with zero drift found. Each remaining item's task will still re-confirm its
own specific `file:line` at implementation time (standard practice in this repository, not skipped
here).

## R2: `NEW-11` default TLS trust store — dependency choice

**Decision**: Add `rustls-native-certs` as a new production dependency of `truefix-transport`
(where `tls_config.rs::build_client_config` lives), used only as the fallback when no explicit
`SocketTrustStore`/`SocketTrustStoreBytes` is configured.

**Rationale**:
- QuickFIX/J's own fallback (an unconfigured JVM `SSLContext`) uses the JRE's OS-integrated default
  trust store — `rustls-native-certs` (loads the OS's native certificate store: macOS Keychain,
  Windows cert store, `/etc/ssl` on Linux) is the closer behavioral match, vs. `webpki-roots`
  (a bundled, Mozilla-curated CA list baked into the binary at compile time).
- Constitution Principle I ("依赖纪律") favors minimal, actively-maintained dependencies;
  `rustls-native-certs` is maintained by the same org as `rustls` itself (already a workspace
  dependency), reducing supply-chain surface vs. adding an unrelated maintainer.
- License: `rustls-native-certs` is Apache-2.0 OR MIT (same dual-license as `rustls`), compatible
  with Constitution Principle III's OR Apache-2.0/MIT-compatibility requirement (verified against
  its published `Cargo.toml` license field).

**Alternatives considered**:
- `webpki-roots` — already present as a **dev-dependency only** of `truefix` (used to mint test
  certs, confirmed via `grep`: no production use anywhere in `truefix-transport`/`truefix`). Rejected
  for the production fallback because a bundled CA list can drift out of date without a crate
  update, whereas the native store stays current with the OS.
- No fallback (status quo) — rejected; this is precisely `NEW-11`'s defect.

## R3: Other technical facts confirmed during this pass

- **Workspace edition**: `edition = "2024"`, `rust-version = "1.96"` (per `Cargo.toml`/
  `rust-toolchain.toml` — the workspace was bumped from edition 2021 to 2024 in the commit
  immediately preceding this feature's spec, per `git log`). This plan targets that same
  toolchain; no edition-related work is in scope.
- **AT suite regression floor**: `server_suite()` currently produces **424 scenario runs**
  (enforced by `crates/truefix-at/tests/coverage.rs::server_suite_scenario_run_count_does_not_regress`,
  established at feature 007's Polish closeout). All 4 conformance tests
  (`server_acceptance_suite_passes`, `timestamps_suite_passes`, `validate_checksum_suite_passes`,
  `resynch_suite_passes`) currently pass. This is this feature's regression floor — `FR-065`
  (`NEW-52`, splitting `server_acceptance_suite_passes` into per-scenario results) changes *how*
  failures are reported, not the run count itself, so 424 remains the numeric floor post-fix (or
  higher, if `FR-059`/`FR-062`/`FR-064` add new scenario runs, which they will).
- **MSSQL/Mongo backends are feature-gated**: `mssql`/`mongodb`/`sql` are all optional Cargo
  features in `truefix-store`/`truefix-log`/`truefix-config`/`truefix`. Fixes to those backends
  (`NEW-08`, `NEW-09`, `NEW-54`, `NEW-75`, `NEW-90`) only compile/test under the matching feature
  flag — task-writing must include `--features mongodb`/`--features mssql` in the relevant test
  commands, matching the pattern already established in features 004/006/007.
- **`Monitor` (NEW-48)** is a real struct at `crates/truefix-transport/src/lib.rs:247` with a
  `MonitorEntry` — confirmed reachable and not requiring a new type, just an entry-removal call on
  disconnect.
- **`render_members_ordered` (NEW-38/NEW-80)** at `crates/truefix-core/src/codec/encode.rs:71-89`
  confirmed: the outer loop over `order` has no dedup, and the whole function is O(members ×
  order) with no early-exit — both `NEW-38`'s complexity observation and `NEW-80`'s duplicate-emit
  defect live in the same function, so both FRs (`FR-047`, `FR-083`) should be implemented together
  as one task to avoid touching the function twice.

## Summary of decisions requiring disclosure (Constitution / public-API impact)

- **New dependency**: `rustls-native-certs` added to `truefix-transport` (R2 above) — the sole new
  external dependency this feature introduces, matching the spec's Assumptions section.
- **New public API surface**:
  - `truefix-config`'s key registry: 2 keys (`ValidateSequenceNumbers`, `RejectInvalidMessage`)
    change status from `Impl` to `Recognized` (`FR-014`) — a behavior-disclosure change (these keys
    were never actually enforced; the registry now accurately says so), not a breaking API change.
  - `truefix-session::config::SessionConfig`'s `reconnect_interval` default value changes from `5`
    to `30` (`FR-042`) — a default-value change affecting only programmatic callers that bypass
    `truefix-config` (every `.cfg`-driven session already gets 30 today).
  - `truefix-session::config::SessionConfig`'s `logout_timeout` default: **not** changed by this
    plan — re-reading the spec, `NEW-89`/`FR-075` targets `builder.rs`'s `.cfg`-parsing default
    (`u32_key(map, "LogoutTimeout", &session, 10)`), which *is* in scope, changing the real
    `.cfg`-driven default from 10 to 2 seconds to match QuickFIX/J. This is a behavior change for
    every `.cfg`-driven deployment that doesn't set `LogoutTimeout` explicitly — disclosed here as
    the one FR in this feature whose default-value change reaches real `.cfg`-driven deployments
    (unlike `NEW-73`/`reconnect_interval`, which only reaches programmatic callers).
  - `truefix-at`'s harness gains a new fixed-identity acceptor mode (`FR-062`) as an addition
    alongside the existing dynamic-template mode (per Clarifications) — additive, no existing
    scenario setup changes.
  - `truefix-log`/`truefix-store` MSSQL config gains a new `trust_server_certificate`-style opt-in
    (`FR-020`) — additive, default behavior (`trust_cert()`) unchanged for existing deployments.
- **No other breaking changes**: every other FR is an internal correctness fix (session state
  machine, codec, dictionary/validation logic, store/log backends, transport framing, AT harness
  internals) with no public-API shape change.
