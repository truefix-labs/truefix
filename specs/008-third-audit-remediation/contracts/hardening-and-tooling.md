# Contract: Hardening and AT-Harness Coverage (US3)

**Requirements**: FR-059–FR-063 (encode/decode hygiene), FR-076–FR-083 (AT harness + diagnostics)
**Research**: research.md §R1.9 (cross-referenced from US1), §R2/R3

## Codegen `BeginString`/`MsgType` stamping (FR-059)

**Contract**: a codegen-generated message constructed via `new()` and encoded without an explicit
`BeginString` set does not silently emit a malformed `8=<SOH>` wire message.

**Test vehicle**: codegen fixture test — construct a generated struct via `new()`, encode it, assert
`BeginString`/`MsgType` are both present and correctly stamped on the wire.

## Encode/decode data-structure hygiene (FR-060–FR-063)

- **FR-060**: `render_members_ordered` doesn't double-emit a member whose tag repeats in
  `field_order`.
- **FR-061**: `FieldMap::set` doesn't leave a stale duplicate field for re-emission on encode.
- **FR-062**: `Message::decode` enforces `MAX_BODY_LEN` directly, matching `frame_length`.
- **FR-063**: `SessionId::Display` renders `SubID`/`LocationID`.

**Test vehicle**: table-driven unit tests in `truefix-core`, no AT scenario (internal
encode/decode/logging correctness, not a protocol-behavior change observable by a counterparty
beyond what the corrected wire output already fixes).

## AT harness coverage (FR-076–FR-082)

**Contract group**: these items improve the acceptance-test harness's own ability to detect defects
— they are test-infrastructure changes, not production-behavior changes. Constitution Principle V
(Test Discipline) directly motivates this group; Constitution Principle II's AT-gate benefits from
every one of these once landed (more scenarios can now exercise more of the already-shipped
behavior).

- **FR-076**: `start_acceptor` loads a dictionary validator for all 9 FIX versions, not just
  4.2/4.4.
- **FR-077**: `CheckLatency`-family scenarios assert on the resulting Logout's reason text.
- **FR-078**: `run_scenario` asserts the read buffer is empty at completion.
- **FR-079**: `check_match` rejects extra/erroneous fields on the actual message.
- **FR-080**: AT scenarios verify outbound `MsgSeqNum(34)` ordering.
- **FR-081**: `server_acceptance_suite_passes` reports per-scenario pass/fail.
- **FR-082**: a fixed-identity (non-dynamic-template) acceptor mode is available for Logon-time
  identity-validation scenarios (QuickFIX/J's `1c` category).

**Test vehicle**: each is validated by the AT harness's own test suite (`truefix-at`'s `coverage`/
`conformance` tests) gaining new assertions/scenarios — self-verifying by construction (a harness
fix is proven by the scenario it now correctly catches or newly represents).

## Operational hygiene/diagnostics (FR-083)

**Contract**: `Engine::start`'s acceptor-group startup iteration order is stable/deterministic
across process runs (e.g. sorted by socket address).

**Test vehicle**: unit test constructing a multi-group config, running `Engine::start`'s grouping
logic multiple times (or across repeated test-process invocations), asserting identical iteration
order each time.
