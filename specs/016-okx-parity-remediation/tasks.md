# Tasks: OKX Parity Remediation

**Input**: Design documents in `/specs/016-okx-parity-remediation/`

**Tests**: Required by the specification and constitution. Write contract/inventory tests before their corresponding corrections.

## Phase 1: Setup

- [X] T001 Create the 264-row baseline operation manifest schema and source extractor in `crates/truefix-okx-client/src/inventory.rs`.
- [X] T002 [P] Record reviewed parity findings and protocol authority in `crates/truefix-okx-client/README.md`.

## Phase 2: Foundational Protocol Correctness

- [X] T003 Add failing timestamp/header/query/retry contract cases in `crates/truefix-okx-client/tests/http_contract.rs`.
- [X] T004 [P] Add 264-operation completeness assertions in `crates/truefix-okx-client/tests/operation_inventory.rs`.
- [X] T005 Implement fixed-millisecond signing timestamps in `crates/truefix-okx-client/src/auth.rs`.
- [X] T006 Implement JSON content type, explicit Demo/live simulation headers, and transient status mapping in `crates/truefix-okx-client/src/transport/http.rs`.
- [X] T007 Implement empty-query filtering and canonical serialization in `crates/truefix-okx-client/src/request.rs`.
- [X] T008 Implement one bounded safe-read retry and limiter throttling in `crates/truefix-okx-client/src/client.rs` and `crates/truefix-okx-client/src/limiter.rs`.

## Phase 3: User Story 1 - Interoperable Authentication (P1)

**Goal**: Signed requests match the approved protocol contract.

**Independent Test**: Local HTTP fixtures verify millisecond timestamp, headers, query omission, one read retry, and no write replay.

- [X] T009 [US1] Extend table-driven signing coverage in `crates/truefix-okx-client/src/auth.rs`.
- [X] T010 [US1] Extend request/transport fixture assertions in `crates/truefix-okx-client/tests/http_contract.rs`.
- [X] T011 [US1] Document authentication and retry semantics in `specs/016-okx-parity-remediation/contracts/parity-inventory.md`.

## Phase 4: User Story 2 - Correct Baseline Operations (P1)

**Goal**: Every baseline operation maps to a current, approved method and path.

**Independent Test**: Inventory plus per-domain fixtures reject incorrect verbs, paths, and Rust-only server endpoints.

- [X] T012 [P] [US2] Correct account/fixed/VIP loan/manual borrow paths and add missing account operations in `crates/truefix-okx-client/src/services/account.rs`.
- [X] T013 [P] [US2] Correct funding currencies/lightning/dust paths and methods in `crates/truefix-okx-client/src/services/funding.rs`.
- [X] T014 [P] [US2] Correct flexible loan, staking, dual-investment, and savings operations in `crates/truefix-okx-client/src/services/finance.rs`.
- [X] T015 [P] [US2] Complete missing market/public-data operations in `crates/truefix-okx-client/src/services/market.rs` and `crates/truefix-okx-client/src/services/public_data.rs`.
- [X] T016 [P] [US2] Complete trade, subaccount, strategy, and copy-trading baseline operations in `crates/truefix-okx-client/src/services/trade.rs`, `crates/truefix-okx-client/src/services/subaccount.rs`, and `crates/truefix-okx-client/src/services/strategy.rs`.
- [X] T017 [P] [US2] Correct broker, RFQ, spread, trading-data, and convert operations in `crates/truefix-okx-client/src/services/professional.rs`.
- [X] T018 [US2] Remove or recast non-baseline server endpoints as pure convenience methods in `crates/truefix-okx-client/src/services/`.
- [X] T019 [US2] Add per-domain path/verb fixtures in `crates/truefix-okx-client/tests/http_contract.rs`.

## Phase 5: User Story 3 - Auditable Complete Coverage (P2)

**Goal**: Users can trust the exact parity status and retry guarantees.

**Independent Test**: The inventory fails for any absent, duplicate, unsupported, or unevidenced baseline record.

- [X] T020 [US3] Populate all 264 records with auth, replay, entrypoint, and fixture evidence in `crates/truefix-okx-client/src/inventory.rs`.
- [X] T021 [US3] Add duplicate/unsupported/unclassified inventory failure tests in `crates/truefix-okx-client/tests/operation_inventory.rs`.
- [X] T022 [US3] Add read-retry/write-no-replay and 429 throttle fixtures in `crates/truefix-okx-client/tests/http_contract.rs`.
- [X] T023 [US3] Update parity validation scenarios in `specs/016-okx-parity-remediation/quickstart.md`.

## Phase 6: Polish & Validation

- [X] T024 Add rustdoc for corrected public operations in `crates/truefix-okx-client/src/`.
- [X] T025 Audit production paths for copied source, secret exposure, and panic-prone code in `crates/truefix-okx-client/`.
- [X] T026 Run `cargo fmt --check`, `cargo clippy -p truefix-okx-client -- -D warnings`, and `cargo test -p truefix-okx-client` from the repository root.

## Dependencies & Execution Order

- Phase 2 blocks all stories.
- US1 can complete after T003–T008.
- US2 domain tasks T012–T017 can run in parallel after T004–T008; T018–T019 follow them.
- US3 depends on corrected domain methods and fixtures.

## Parallel Opportunities

- T002, T004, and T012–T017 use separate files and can run in parallel.
- T009 and T010 can run together after foundational protocol tasks.

## Implementation Strategy

1. Establish exact signing, header, query, retry, and inventory behavior.
2. Correct endpoint domains in parallel, with fixture evidence before each correction.
3. Populate and enforce all 264 inventory records.
4. Run the complete validation suite and provenance audit.
