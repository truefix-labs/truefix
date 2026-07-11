# Tasks: OKX Client SDK

**Input**: Design documents in `/specs/015-okx-client-sdk/`

**Tests**: Required by FR-024, SC-001–SC-006, the quickstart, and the constitution. Write the listed tests first and confirm they fail before their implementation tasks.

**Organization**: Tasks are grouped by user story. The `python-okx@fa8d738` operation manifest is the completeness source of truth: every operation needs an intentional native entrypoint and test evidence.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the independent crate and controlled baseline.

- [X] T001 Create `crates/truefix-okx-client/Cargo.toml` and register `truefix-okx-client` plus approved shared dependencies in `Cargo.toml`.
- [X] T002 Create the crate facade, public re-exports, package documentation, and critical-path lint policy in `crates/truefix-okx-client/src/lib.rs`.
- [X] T003 [P] Create the source-baseline manifest schema and seed source domain/operation identities from `python-okx@fa8d738` in `crates/truefix-okx-client/src/inventory.rs`.
- [X] T004 [P] Record new dependency license/provenance evidence and the source baseline in `crates/truefix-okx-client/THIRD_PARTY.md`.
- [X] T005 [P] Add credential-free public-data and Demo-order example skeletons in `crates/truefix-okx-client/examples/public_market_data.rs` and `crates/truefix-okx-client/examples/demo_order_lifecycle.rs`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Implement shared safety, transport and domain primitives.

**⚠️ CRITICAL**: Complete this phase before user-story work.

- [X] T006 Define named `OkxError`, `OkxResult`, redacted context, exchange codes, partial failure, and unknown completion in `crates/truefix-okx-client/src/error.rs`.
- [X] T007 [P] Define exact decimal, identifier, instrument/product, time, pagination and envelope types in `crates/truefix-okx-client/src/types/common.rs` and `crates/truefix-okx-client/src/types/instrument.rs`.
- [X] T008 [P] Define immutable credentials, Demo/live/custom endpoint policy, typed live confirmation, timeout and proxy policy in `crates/truefix-okx-client/src/config.rs`.
- [X] T009 [P] Write configuration and redaction unit tests in `crates/truefix-okx-client/src/config.rs` and `crates/truefix-okx-client/src/error.rs`.
- [X] T010 Implement canonical URL/query/body construction, request metadata, and safe-read/write classification in `crates/truefix-okx-client/src/request.rs`.
- [X] T011 [P] Implement injected clock, REST/WS signing, and redacted authorization headers in `crates/truefix-okx-client/src/auth.rs`.
- [X] T012 [P] Write table-driven signer/request tests for encoded query, empty body, Demo header, time skew and redaction in `crates/truefix-okx-client/src/auth.rs` and `crates/truefix-okx-client/src/request.rs`.
- [X] T013 Implement response envelopes, pagination, per-item results, error-code and unknown-field decoding in `crates/truefix-okx-client/src/response.rs`.
- [X] T014 Implement shared scoped rate reservations, bounded read retry, and write-replay prohibition in `crates/truefix-okx-client/src/limiter.rs`.
- [X] T015 [P] Implement pooled HTTP execution, canonical sending, status mapping, proxy/timeout and redacted telemetry in `crates/truefix-okx-client/src/transport/http.rs`.
- [X] T016 Define `OkxClient`, its immutable shared core, and domain accessors in `crates/truefix-okx-client/src/client.rs` and `crates/truefix-okx-client/src/services/mod.rs`.
- [X] T017 Create local HTTP fixture helpers for method/path/query/signature/header/body validation in `crates/truefix-okx-client/tests/support/http.rs`.
- [X] T018 Create the manifest coverage harness in `crates/truefix-okx-client/tests/operation_inventory.rs`.

**Checkpoint**: Signed HTTP operations, safe configuration, typed errors and manifest evidence are ready for every story.

---

## Phase 3: User Story 1 - Type-safe OKX Trading (Priority: P1) 🎯 MVP

**Goal**: Provide public market data, account inspection and a typed Demo Trading order lifecycle.

**Independent Test**: Local HTTP fixtures verify unsigned public calls, signed Demo calls, order lifecycle, account queries, rejection and partial success; Demo smoke is opt-in.

### Tests for User Story 1

- [X] T019 [P] [US1] Add public/auth/Demo/rejection/pagination HTTP contract tests in `crates/truefix-okx-client/tests/http_contract.rs`.
- [X] T020 [P] [US1] Add exact decimal request/response round-trip tests for spot, margin, swap and option samples in `crates/truefix-okx-client/tests/decimal_roundtrip.rs`.
- [X] T021 [P] [US1] Add Demo-default, live-confirmation and immutable-credential tests in `crates/truefix-okx-client/tests/environment_safety.rs`.

### Implementation for User Story 1

- [X] T022 [P] [US1] Define account balance, position, bill, leverage, margin, risk and fee models in `crates/truefix-okx-client/src/types/account.rs`.
- [X] T023 [P] [US1] Define ordinary/algorithmic order, fill, batch-result, client ID and completion-state models in `crates/truefix-okx-client/src/types/order.rs`.
- [X] T024 [P] [US1] Define ticker, book, candle, trade, mark/index and funding-rate models in `crates/truefix-okx-client/src/types/market.rs`.
- [X] T025 [US1] Implement Account balance, position, bill, configuration, leverage, margin, risk and fee operations in `crates/truefix-okx-client/src/services/account.rs`.
- [X] T026 [US1] Implement Trade single/batch place, cancel, amend, close, query, fills and histories in `crates/truefix-okx-client/src/services/trade.rs`.
- [X] T027 [US1] Implement MarketData ticker, books, candles, trades, index/mark, funding and history operations in `crates/truefix-okx-client/src/services/market.rs`.
- [X] T028 [US1] Link Account, Trade and MarketData records to methods and fixture IDs in `crates/truefix-okx-client/src/inventory.rs`.
- [X] T029 [US1] Complete safe typed examples in `crates/truefix-okx-client/examples/public_market_data.rs` and `crates/truefix-okx-client/examples/demo_order_lifecycle.rs`.
- [X] T030 [US1] Verify the Phase 3 fixture suite and opt-in Demo scenario from `specs/015-okx-client-sdk/quickstart.md`.

**Checkpoint**: Public data works without credentials and a typed Demo order lifecycle exposes account and result data.

---

## Phase 4: User Story 2 - Continuous Market and Account Events (Priority: P1)

**Goal**: Provide multiplexed public/private/business sessions with acknowledgement-gated authentication, liveness, reconnect/resubscribe and no write replay.

**Independent Test**: A local WebSocket fixture drives login, subscription confirmation, events, ping/pong, error, disconnect and reconnect; subscriptions receive only their own events and no write is resent.

### Tests for User Story 2

- [X] T031 [P] [US2] Create local WebSocket fixture helpers for login, acknowledgement, ping/pong, disconnect and scripted events in `crates/truefix-okx-client/tests/support/websocket.rs`.
- [X] T032 [P] [US2] Add public/private/business, correlation, deduplication, reconnect and no-write-replay tests in `crates/truefix-okx-client/tests/ws_session.rs`.

### Implementation for User Story 2

- [X] T033 [P] [US2] Implement WebSocket connect/send/receive/close with endpoint selection and redacted diagnostics in `crates/truefix-okx-client/src/transport/websocket.rs`.
- [X] T034 [P] [US2] Define real-time login/ack/error/event/channel/command models in `crates/truefix-okx-client/src/types/websocket.rs`.
- [X] T035 [US2] Implement desired/active subscription keys, bounded IDs, correlation, routing, cancellation and deduplication in `crates/truefix-okx-client/src/ws/subscription.rs` and `crates/truefix-okx-client/src/ws/event.rs`.
- [X] T036 [US2] Implement lifecycle, login gate, ping/pong deadline, backoff and replay of desired subscriptions in `crates/truefix-okx-client/src/ws/session.rs`.
- [X] T037 [P] [US2] Implement public and private session entrypoints in `crates/truefix-okx-client/src/ws/public.rs` and `crates/truefix-okx-client/src/ws/private.rs`.
- [X] T038 [US2] Implement business session, optional login, mass-cancel limit class and upgrade recovery in `crates/truefix-okx-client/src/ws/business.rs`.
- [X] T039 [US2] Implement real-time order, batch-order, cancel, batch-cancel, amend, batch-amend and mass-cancel with unknown completion in `crates/truefix-okx-client/src/ws/private.rs` and `crates/truefix-okx-client/src/ws/business.rs`.
- [X] T040 [US2] Link all real-time commands and session operations to fixture evidence in `crates/truefix-okx-client/src/inventory.rs`.
- [X] T041 [US2] Verify the real-time quickstart scenario in `specs/015-okx-client-sdk/quickstart.md`.

**Checkpoint**: Real-time sessions multiplex, recover read subscriptions and never replay asset-changing commands.

---

## Phase 5: User Story 3 - Complete OKX V5 Business Surface (Priority: P2)

**Goal**: Implement every remaining source-baseline REST operation as a typed native domain API.

**Independent Test**: The manifest test has a record, native entrypoint and request/response fixture for all 264 REST operations; safe operations use Demo/read-only validation where available.

### Tests for User Story 3

- [X] T042 [P] [US3] Extend per-domain request/error fixture assertions and manifest coverage in `crates/truefix-okx-client/tests/operation_inventory.rs` and `crates/truefix-okx-client/tests/http_contract.rs`.

### Implementation for User Story 3

- [X] T043 [P] [US3] Implement PublicData instruments, delivery/exercise, interest, tiers, options, announcements and platform operations in `crates/truefix-okx-client/src/services/public_data.rs`.
- [X] T044 [P] [US3] Complete Account borrow/repay, fixed/VIP loans, Greeks, trading config and risk history operations in `crates/truefix-okx-client/src/services/account.rs`.
- [X] T045 [P] [US3] Complete Trade algorithmic orders, easy conversion, one-click repayment and histories in `crates/truefix-okx-client/src/services/trade.rs`.
- [X] T046 [P] [US3] Implement Funding deposit/withdrawal, transfer, lightning, dust, valuation and bills in `crates/truefix-okx-client/src/services/funding.rs`.
- [X] T047 [P] [US3] Implement SubAccount balance, bills, transfers, key/permission and loan operations in `crates/truefix-okx-client/src/services/subaccount.rs`.
- [X] T048 [P] [US3] Implement Grid and recurring-buy operations in `crates/truefix-okx-client/src/services/strategy.rs`.
- [X] T049 [US3] Implement CopyTrading operations in `crates/truefix-okx-client/src/services/strategy.rs`.
- [X] T050 [P] [US3] Implement Savings, ETH/SOL/DeFi staking, FlexibleLoan and DualInvest operations in `crates/truefix-okx-client/src/services/finance.rs`.
- [X] T051 [P] [US3] Implement BlockTrading RFQ, quote, trade and MMP operations in `crates/truefix-okx-client/src/services/professional.rs`.
- [X] T052 [US3] Implement SpreadTrading and TradingData operations in `crates/truefix-okx-client/src/services/professional.rs`.
- [X] T053 [US3] Implement Convert, FDBroker and Status operations in `crates/truefix-okx-client/src/services/professional.rs`.
- [X] T054 [US3] Add Phase 5 models, endpoint metadata, native entrypoints and fixture references in `crates/truefix-okx-client/src/types/` and `crates/truefix-okx-client/src/inventory.rs`.
- [X] T055 [US3] Make the inventory test fail for any missing baseline operation, auth class, safety class, native method or fixture evidence in `crates/truefix-okx-client/tests/operation_inventory.rs`.
- [X] T056 [US3] Run the long-tail fixture suite and document Demo/read-only limitations in `specs/015-okx-client-sdk/quickstart.md`.

**Checkpoint**: The complete source baseline is typed, tested and auditable; server permission/region restrictions are typed errors.

---

## Phase 6: User Story 4 - Gateway Composition Without Semantic Loss (Priority: P3)

**Goal**: Provide a narrow projection boundary for a future gateway without losing native data.

**Independent Test**: Mapping fixtures prove common order/account/position/execution/market projections are consistent while original native fields remain accessible.

### Tests for User Story 4

- [X] T057 [P] [US4] Add native-to-gateway projection and native-field-preservation fixtures in `crates/truefix-okx-client/tests/gateway_projection.rs`.

### Implementation for User Story 4

- [X] T058 [US4] Define optional projection traits/types without a gateway dependency in `crates/truefix-okx-client/src/types/gateway.rs` and `crates/truefix-okx-client/src/types/mod.rs`.
- [X] T059 [US4] Implement projections while retaining native extension data in `crates/truefix-okx-client/src/types/order.rs`, `crates/truefix-okx-client/src/types/account.rs`, and `crates/truefix-okx-client/src/types/market.rs`.
- [X] T060 [US4] Document composition and non-projectable product boundaries in `crates/truefix-okx-client/src/lib.rs` and `specs/015-okx-client-sdk/contracts/sdk-api.md`.
- [X] T061 [US4] Verify no `truefix-gateway` dependency and run projection fixtures in `crates/truefix-okx-client/Cargo.toml` and `crates/truefix-okx-client/tests/gateway_projection.rs`.

**Checkpoint**: A future adapter can consume common data while direct callers retain the complete native SDK.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T062 Add rustdoc for all public types, traits, services and operations in `crates/truefix-okx-client/src/`.
- [X] T063 Add connection/request/retry/rate-limit/subscription telemetry and redaction assertions in `crates/truefix-okx-client/src/` and `crates/truefix-okx-client/tests/telemetry.rs`.
- [X] T064 [P] Add manifest maintenance and upstream-drift instructions in `crates/truefix-okx-client/README.md`.
- [X] T065 Run `cargo fmt --check`, `cargo clippy -p truefix-okx-client -- -D warnings`, and `cargo test -p truefix-okx-client` from the repository root.
- [X] T066 Run every scenario in `specs/015-okx-client-sdk/quickstart.md`, running Demo smoke only when credentials are supplied.
- [X] T067 Audit `crates/truefix-okx-client/` for copied Python source/tests, panic-prone paths, secret exposure, missing inventory records and license evidence.

## Dependencies & Execution Order

- **Phase 1**: No dependencies.
- **Phase 2**: Depends on Phase 1 and blocks all stories.
- **US1 / US2 (P1)**: Start after Phase 2; US1 is the recommended MVP.
- **US3 (P2)**: Starts after Phase 2 and follows US1 service conventions; its manifest test independently proves completeness.
- **US4 (P3)**: Depends on US1 core models and remains independent from `truefix-gateway`.
- **Polish**: Follows all desired stories.

## Parallel Opportunities

- T003–T005; T007–T009; T011–T012; T015; and T017–T018 can proceed in parallel after stated prerequisites.
- In US1, T019–T021 and T022–T024 are parallel; separate service files may proceed concurrently after shared types land.
- In US2, T031–T034 and T037 are parallel once their interfaces stabilize.
- In US3, T043–T053 are deliberately split by service file; coordinate shared model additions through T054.

## Implementation Strategy

### MVP First

1. Complete Phases 1–2.
2. Complete US1 through T030.
3. Validate its fixture suite and use only supplied Demo credentials for the smoke path.
4. Demonstrate public market data and a typed Demo order lifecycle.

### Incremental Delivery

1. Add US2 for safe real-time observation and recovery.
2. Add US3 by service domain; the manifest makes omissions failing work.
3. Add US4 as a projection boundary, not a replacement for native semantics.
4. Pass Phase 7 before declaring baseline parity.
