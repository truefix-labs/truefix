# Implementation Plan: OKX Parity Remediation

**Branch**: `feature/twsapi-client-python-port` | **Date**: 2026-07-10 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

Make `truefix-okx-client` protocol-correct against all 264 operations in the local Python baseline. Python defines capability scope; current official OKX documentation resolves protocol details. Correct signing/header/query behavior, replace invalid endpoint metadata, remove unsupported server operations, and prove every baseline record through fixture evidence.

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: Rust 2024, MSRV 1.96

**Primary Dependencies**: Tokio, reqwest, tokio-tungstenite, Serde, rust_decimal, time, tracing

**Storage**: N/A

**Testing**: cargo test; local HTTP/WebSocket contract fixtures; table-driven inventory tests

**Target Platform**: Tokio-supported desktop/server platforms

**Project Type**: Workspace library crate

**Performance Goals**: Reuse HTTP connections; bounded one-retry read recovery; no duplicate writes

**Constraints**: Exact milliseconds for signed timestamps; JSON content type; explicit live/Demo intent; no write replay; no production panic paths

**Scale/Scope**: 264 baseline REST operations plus existing WS behavior

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

PASS: inventory-backed parity, independent implementation/provenance, typed errors, test-first fixture coverage, redacted diagnostics. FIX-specific dictionary and acceptor/initiator gates do not apply to this external SDK.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
crates/truefix-okx-client/
├── src/{auth,request,limiter,inventory}.rs
├── src/services/
├── src/types/
└── tests/{http_contract,operation_inventory}.rs
```

**Structure Decision**: Extend the existing standalone client crate; endpoint facts live in the inventory and domain services, while shared signing/retry behavior remains centralized.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
