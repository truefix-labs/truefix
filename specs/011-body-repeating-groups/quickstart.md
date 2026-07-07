# Quickstart: Validating Body-Level Repeating Group Decoding & Vendor Dictionary Support

This is a validation guide, not an implementation guide — it documents runnable checks that prove
each user story works end-to-end once implemented. See [contracts/](./contracts/) for the detailed
behavior each check exercises.

## Prerequisites

- Rust workspace builds: `cargo build --workspace`.
- For vendor-XML checks (User Story 3), build with the tooling feature:
  `cargo build -p truefix-dict --features dict-tooling`.

## User Story 1 — Single-dictionary body group structuring

1. Run the extended body-group test additions in `truefix-transport` and `truefix-dict`:
   ```sh
   cargo test -p truefix-transport
   cargo test -p truefix-dict
   ```
2. Manually verify with a decoded fixture: construct a session with a `DataDictionary` that declares
   a body-level group (e.g. bundled `FIX44`'s `NoMDEntries`), feed it a raw multi-entry
   `MarketDataSnapshotFullRefresh`-style message through the receive path, and confirm:
   - `message.body.group(NO_MD_ENTRIES_TAG)` returns every entry.
   - A codegen-generated typed accessor for the same group returns every entry (not empty).
   - `message.body.fields()` still yields the flat, group-skipping view unchanged.
3. Confirm the equivalence bar: run the full pre-existing `truefix-dict` validation test suite and
   confirm 100% still pass (spec.md SC-002).

**Expected outcome**: SC-001 — zero entries silently dropped, across both the low-level group API
and codegen-generated accessors.

## User Story 2 — FIXT 1.1 dual-dictionary body group structuring

1. Run (after adding, per contracts/decode-and-restructure.md) the FIXT dual-dictionary integration
   tests:
   ```sh
   cargo test -p truefix-transport --test fixt_group_aware_decode
   cargo test -p truefix-session
   ```
2. Manually verify: configure a session with `FixtDictionaries` (a transport dictionary plus one
   application dictionary registered under a negotiated `ApplVerID`), as both an acceptor and an
   initiator (Constitution Principle VI). Send:
   - An application-layer message with a body-level group under the negotiated `ApplVerID` — confirm
     full entry visibility.
   - A session-layer message (e.g. carrying `NoHops`) — confirm unaffected, transport-dictionary
     structuring.
   - An application-layer message received before `ApplVerID` negotiation completes — confirm
     transport-scoped fallback, not a rejection or misattribution.

**Expected outcome**: SC-004 — correct dictionary selection per message, zero entries dropped,
matching the single-dictionary case's correctness.

## User Story 3 — Vendor XML dictionary ingestion

1. Build and test with `dict-tooling` enabled:
   ```sh
   cargo test -p truefix-dict --features dict-tooling
   ```
2. Convert a vendor-shaped fixture via the CLI:
   ```sh
   cargo run -p truefix-dict --features dict-tooling --bin truefix-dict -- \
     generate-dict --format vendor-xml --source <fixture.xml> --out /tmp/converted.fixdict
   cargo run -p truefix-dict --features dict-tooling --bin truefix-dict -- \
     validate --dict /tmp/converted.fixdict
   ```
3. Load the same vendor-shaped file directly (no offline conversion step) via
   `truefix_dict::load_from_file("<fixture.xml>")` and confirm it returns a working
   `DataDictionary` — validating a matching sample message and structuring its body groups exactly
   like the CLI-converted `.fixdict` would.
4. Confirm a malformed vendor XML fixture (missing version attributes, undefined component
   reference, broken XML syntax) produces a clear, typed error at both the CLI and `load_from_file`
   entry points — not a partial or silently-incorrect dictionary (SC-005).

**Expected outcome**: SC-003 — a vendor-published dictionary file loads and works directly, with zero
manual translation steps, when `dict-tooling` is enabled.

## Regression Check (all stories)

```sh
cargo test --workspace --features dict-tooling
```

All pre-existing tests (including AT scenarios) must continue to pass unchanged.

## Recorded Results (2026-07-07)

All three user stories were validated end-to-end against the real implementation, not just unit
tests:

- **User Story 1**: `crates/truefix-transport/tests/body_group_decode_production.rs` spins up a
  real `Acceptor`, sends a raw-encoded `NewOrderSingle` with a two-entry `NoPartyIDs`(453) body
  group (one entry carrying a nested `NoPartySubIDs`(802) group) over a live TCP connection, and
  confirms the captured, application-delivered message has both levels fully structured while
  `.fields()` stays flat. `crates/truefix-dict/tests/codegen.rs`'s new test confirms the
  codegen-generated `NewOrderSingle::no_party_ids()` accessor returns both entries for a *decoded*
  message (the actual defect the external proposal reported). SC-001 confirmed.
- **User Story 2**: `crates/truefix-transport/tests/fixt_application_group_restructure.rs` (5
  tests) confirms, over real acceptor *and* initiator sessions, that FIXT 1.1 application messages
  structure against the negotiated/explicit `ApplVerID`'s dictionary, session-layer messages stay
  on the transport dictionary, an unregistered per-message `ApplVerID` falls back to transport-scoped
  structuring without rejecting the connection, and multiple `ApplVerID`s in one connection resolve
  independently per message. SC-004 confirmed.
- **User Story 3**: `crates/truefix-dict/tests/vendor_xml_conversion.rs`,
  `load_from_file.rs`, and `cli.rs` confirm a Binance-shaped vendor XML fixture (fields, a
  body-level group, a component reference, a nested group, and a multi-character custom `MsgType`)
  converts and loads correctly both via the CLI and directly via `load_from_file`, and that
  malformed variants (missing version, undefined component reference, broken XML) each produce a
  typed `VendorXmlError`/`DictLoadError::VendorXml`, never a panic or partial dictionary. SC-003 and
  SC-005 confirmed.
- **Full regression**: `cargo test --workspace --features dict-tooling` — zero failures across
  every crate. `cargo test -p truefix-at` — **483/483 AT scenario runs pass**. `cargo fmt --all --
  --check` and `cargo clippy --workspace --all-targets --features dict-tooling` are both clean.

**One real regression was caught and fixed during implementation, not just theorized**: the first
full-suite run surfaced 3 AT scenario failures (`14i_RepeatingGroupCountNotEqual`,
`14j_OutOfOrderRepeatingGroupMembers`, `QFJ934_MissingDelimiterNestedRepeatingGroup`) from a
pre-existing gap in `validate_structured_group` (it never checked a structured group's declared
count or member order — see research.md R4) that this feature made reachable on the production path
for the first time. Fixed as part of T014; re-verified green.
