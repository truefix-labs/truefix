# Feature Specification: OKX Parity Remediation

**Feature Branch**: `016-okx-parity-remediation`
**Created**: 2026-07-10
**Status**: Draft
**Input**: Correct the reviewed differences between the native client and the local `python-okx@fa8d738` capability baseline.

## Clarifications

### Session 2026-07-10

- Q: Should remediation cover only reviewed findings or the full Python baseline? → A: Complete and verify all 264 Python baseline REST operations.
- Q: How should Rust operations outside the Python baseline be handled? → A: Remove unsupported server endpoints; retain only convenience abstractions that introduce no server capability.
- Q: What bounded retry policy applies to safe reads? → A: At most one automatic retry for transient read failures only.
- Q: Which source resolves a baseline-versus-protocol conflict? → A: Python defines the capability set; official OKX documentation defines current protocol details.

## User Scenarios & Testing

### User Story 1 - Submit interoperable authenticated requests (Priority: P1)

SDK users can send private requests whose signed bytes and headers match the baseline contract.

**Independent Test**: Local fixtures verify a millisecond UTC timestamp, JSON content type, simulation flag for both environments, and byte-identical signing input.

**Acceptance Scenarios**:

1. **Given** an authenticated request, **When** it is sent, **Then** its timestamp always has exactly three fractional-second digits and its signature covers that exact value.
2. **Given** a JSON request, **When** it is sent in Demo or live mode, **Then** it includes JSON content type and the appropriate simulation declaration.

### User Story 2 - Use baseline-correct product operations (Priority: P1)

SDK users can invoke supported funding, loan, finance, broker, and trading operations without an invented or stale endpoint path.

**Independent Test**: A local contract fixture validates each corrected method/path/verb pairing against the source-baseline inventory.

**Acceptance Scenarios**:

1. **Given** a corrected operation, **When** it is constructed, **Then** its path and HTTP method match the approved baseline identity.
2. **Given** an operation absent from the baseline, **When** callers inspect the inventory, **Then** it is not represented as a supported baseline operation.

### User Story 3 - Trust coverage and retry behavior (Priority: P2)

SDK users can rely on documented read retry behavior and an auditable report of remaining parity gaps.

**Independent Test**: Transient read failures retry within the configured bound; writes never retry; inventory tests fail for missing endpoint metadata or fixture evidence.

## Edge Cases

- Empty query values are omitted consistently with the approved baseline.
- A connection loss after a write remains an unknown completion and is never automatically retried.
- Server permission, regional availability, and product retirement errors remain visible to callers.

## Requirements

### Functional Requirements

- **FR-001**: Private request timestamps MUST use UTC with exactly millisecond precision.
- **FR-002**: JSON requests MUST declare their media type; simulated-trading intent MUST be explicit for Demo and live environments.
- **FR-003**: Empty query values MUST be omitted from canonical query serialization.
- **FR-004**: Corrected operations MUST use the baseline-approved path and request method, including funding, fixed/VIP loan, flexible loan, staking, dual investment, broker rebate, and manual borrow/repay operations.
- **FR-005**: The operation inventory MUST distinguish approved baseline operations from unsupported or newly discovered operations and retain native entrypoint and fixture evidence.
- **FR-006**: Safe read requests MUST perform bounded transient retry; asset-changing requests MUST never be automatically replayed.
- **FR-006a**: A safe read MAY retry at most once after a timeout, connection interruption, rate limit, or server failure; writes MUST never be automatically replayed.
- **FR-007**: The remediation MUST add inventory-backed coverage for every one of the 264 Python-baseline REST operations across account, trade, market, public data, funding, subaccount, professional, strategy, and finance domains.
- **FR-008**: The remediation MUST remove any Rust endpoint not present in the Python baseline; convenience abstractions may remain only when they map exclusively to approved baseline operations.
- **FR-009**: Python defines the 264-operation capability set; current official OKX documentation MUST resolve any path, method, header, authentication, or response-semantics conflict.

### Key Entities

- **Baseline operation record**: Source identity, method, path, auth class, replay class, native entrypoint, and fixture evidence.
- **Canonical authenticated request**: Timestamp, headers, query, body, and exact signing bytes.
- **Parity finding**: Verified mismatch, its disposition, and regression evidence.

## Success Criteria

### Measurable Outcomes

- **SC-001**: 100% of reviewed critical authentication/header cases have deterministic fixture coverage.
- **SC-002**: 100% of corrected path/verb findings have a baseline inventory record and contract test.
- **SC-003**: Inventory validation reports zero unclassified operations across all 264 baseline REST operations.
- **SC-004**: Safe reads recover from one transient fixture failure without replaying any write command.

## Assumptions

- The local `python-okx@fa8d738` source is the capability baseline; it is used only to record behavior and endpoint facts.
- Official OKX documentation is the authority for current protocol details when it differs from the Python baseline.
- Operations that exist only in newer official documentation are tracked separately and are not treated as baseline parity until reviewed.
- Demo credentials are optional; live smoke tests remain opt-in.
