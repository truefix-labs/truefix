# Contract: Feature-Completeness Gaps & AT Harness Coverage (US8, US9)

**Covers**: FR-035 through FR-043. **Source**: `docs/todo/003.md` `GAP-11`, `GAP-12`, `GAP-18c`,
`GAP-19`, `GAP-21`, `GAP-10`, `GAP-44`, `BUG-17`, MinQty follow-up. **Grounding**: research.md §R8,
§R9.

## US8 — Carried-forward gaps

| # | Change | File(s) | Contract |
|---|---|---|---|
| 1 | Recurring sequence reset | `truefix-session/src/schedule_reset.rs` | New recurring-time transition alongside existing `Enter`/`Exit` |
| 2 | Multi-tag `LogonTag` | `truefix-session/src/config.rs` | `Option<(u32, String)>` → `Vec<(u32, String)>`; `.cfg` parses `LogonTag`, `LogonTag1`, `LogonTag2`, … |
| 3 | FIXT `ApplVerID` negotiation | `truefix-session/src/state.rs` (`on_logon`), `truefix-config/src/builder.rs` (`resolve_validator`) | Inbound tag 1137 auto-extracted; real `FixtDictionaries` construction (transport + per-ApplVerID app dictionaries) instead of dictionary aliasing |
| 4 | Wildcard dynamic-session templates | `truefix-transport` (`AcceptorBuilder`) | Wildcard SubID/LocationID matching added to existing BeginString/SenderCompID/TargetCompID substitution |
| 5 | `.cfg`-selectable log backends | `truefix-config` (`resolve_log`) | `ScreenLog`/`TracingLog`/`CompositeLog` selectable alongside existing File/SQL |
| 6 | IANA timezone names | `truefix-config` (`TimeZone` parsing) | Accepts IANA names in addition to numeric offsets — may add `time-tz` dependency (evaluate at implementation time per Principle III) |
| 7 | `${var}` env-var fallback | `truefix-config/src/lib.rs` | Falls back to `std::env::var` when a name isn't in the settings map |

**No breaking changes**: item 2's `Vec` change is the one field-type change in this group — confirm
`LogonTag`'s field is not part of any exhaustively-matched public struct-literal pattern outside this
crate before implementing (internal `SessionConfig` field, additive in effect since `Vec` with one
element behaves identically to the old `Option` for existing `.cfg` files).

## US9 — AT harness coverage

| # | Change | File(s) | Contract |
|---|---|---|---|
| 1 | Regression floor | `truefix-at/tests/coverage.rs:41` | `>= 353` → `>= 373` immediately (this stage's first sub-task), then bumped again after US1 lands its 8 new scenario families |
| 2 | `MinQty` scenario | `truefix-at/src/scenarios.rs` | New scenario exercising tag 110, wired into `server_suite()` across `SUITE_VERSIONS` |

## Acceptor/Initiator parity

US8 items 1, 2, 3, 6, 7: role-agnostic (session/config-layer behavior). Item 4: acceptor-only by
construction (dynamic-session templates only apply to acceptors). Item 5: role-agnostic (log-backend
selection applies to any session). US9: AT scenarios are written server-side (acceptor-facing) per
the existing `server_suite()` convention, consistent with 001-005's AT harness design.
