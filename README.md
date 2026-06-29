# TrueFix

> A production-grade FIX protocol engine in Rust, at feature parity with QuickFIX/J — first-class
> acceptor/sell-side support, a dual-track data dictionary, and FIX conformance (acceptance tests) as the
> release gate.

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)
![Status](https://img.shields.io/badge/status-pre--implementation%20(design%20complete)-orange.svg)
![Rust](https://img.shields.io/badge/rust-edition%202021-informational.svg)

**Status**: 🚧 Pre-implementation. The specification, plan, and task breakdown are complete and committed
as the project baseline; engine code has not started. See [`specs/001-fix-engine-parity/`](specs/001-fix-engine-parity/).

## Why TrueFix

Existing Rust FIX libraries are either buy-side only or pre-1.0 and not production-trusted. TrueFix aims
to be the first Rust FIX engine that is:

- **Production-ready** — stable, documented public API; no `panic`/`unwrap`/`expect` on critical paths;
  typed, recoverable errors; structured observability (session state, sequence numbers, connection health).
- **Acceptor-first** — the sell-side/acceptor is a first-class citizen, on equal footing with the
  initiator (multi-session, dynamic sessions).
- **Conformance-gated** — protocol correctness is verified by a ported FIX **Acceptance Test (AT)** suite,
  which is the hard release gate.

The north-star goal is **full feature parity with QuickFIX/J**, delivered in one pass.

## Supported FIX versions (target)

FIX 4.0 / 4.1 / 4.2 / 4.3 / 4.4 / 5.0 / 5.0SP1 / 5.0SP2 and **FIXT 1.1** (with separate transport and
application dictionaries; `DefaultApplVerID` resolution).

## Architecture

Async-first engine on **tokio** (TLS via **rustls**), organized as a layered cargo workspace:

| Crate | Responsibility |
|-------|----------------|
| `truefix-core` | Field / FieldMap / Group / Message, field types, SOH codec, BodyLength/CheckSum, typed errors |
| `truefix-dict` | Data dictionary: **build-time codegen of typed messages + runtime `DataDictionary` validation** (dual-track, one source) |
| `truefix-session` | Session state machine, sequence management, admin messages, scheduling, resend/gap-fill, `NextExpectedMsgSeqNum`, chunked resend |
| `truefix-store` | `MessageStore` trait + Memory / File / CachedFile / SQL (sqlx) / Noop |
| `truefix-log` | `Log` trait + Screen / File / Tracing / Composite (message vs event streams) |
| `truefix-transport` | Initiator + Acceptor, multi/dynamic sessions, reconnect, socket options, rustls TLS |
| `truefix-config` | `SessionSettings`, `.cfg` parsing with `${name}` interpolation, programmatic config |
| `truefix` | Facade: re-exports + `Application` trait + `MessageCracker` |
| `truefix-at` | Ported Acceptance Test suite (release gate) |
| `examples/` | `executor`, `banzai`, `multi_acceptor`, `ordermatch` |

### Dual-track data dictionary

Both tracks derive from **one normalized dictionary** (built from the FIX Trading Community's
Orchestra/Repository specs): build-time codegen gives compile-time-typed messages, while the runtime
`DataDictionary` gives version-agnostic validation, custom dictionaries, and user-defined fields — they
cannot diverge because they share a single source.

## Roadmap

Delivered in one pass to full parity, but internally **dependency-staged** with a verifiable checkpoint
per stage (see [`plan.md`](specs/001-fix-engine-parity/plan.md)):

```
S0 Workspace & CI ─▶ S1 Core codec ─▶ S2 Minimal session ─▶ S3 Recovery/resend ─▶ S4 Data dictionary
   ─▶ S5 Store/log ─▶ S6 Multi/dynamic/schedule/reconnect ─▶ S7 All versions + codegen
   ─▶ S8 TLS/auth/monitoring ─▶ S9 Acceptance Test suite (release gate)
```

## Testing & conformance

- Table-driven unit tests for the codec and session state machine.
- Two-process integration tests (real initiator ↔ acceptor) for handshake, heartbeat, resend, etc.
- The **AT suite** (`truefix-at`) ports the QuickFIX/QuickFIX-J acceptance scenarios as black-box
  behavior contracts; **all targeted FIX versions must pass to release**.
- CI runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny`, and the AT matrix.

## Governance

Development follows a written [constitution](.specify/memory/constitution.md) (v2.0.0). Core principles:
production-readiness, protocol correctness first, **License & provenance discipline**, dual-track
dictionary, test discipline, acceptor/initiator parity, and inventory-based completeness. On conflict:
License (absolute) → protocol correctness → production-readiness → feature count.

## License & provenance

Licensed under either of **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE)) or
**MIT license** ([LICENSE-MIT](LICENSE-MIT)) at your option.

TrueFix is implemented from the FIX protocol specification itself. The QuickFIX (Go) and QuickFIX/J
(Java) projects were read **only** to understand architecture and protocol behavior; **no source code or
private data files were copied or translated**. Data dictionaries are derived from the FIX Trading
Community's official machine-readable specifications. This keeps TrueFix cleanly dual-licensable under
Apache-2.0 OR MIT.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work
by you shall be dual licensed as above, without any additional terms or conditions.
