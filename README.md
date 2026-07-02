# TrueFix

> A production-grade FIX protocol engine in Rust, at feature parity with QuickFIX/J — first-class
> acceptor/sell-side support, a dual-track data dictionary, and FIX conformance (acceptance tests) as the
> release gate.

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)
![Status](https://img.shields.io/badge/status-implemented-brightgreen.svg)
![Rust](https://img.shields.io/badge/rust-edition%202021-informational.svg)

**Status**: QuickFIX/J parity is implemented across two specs:
[`specs/001-fix-engine-parity/`](specs/001-fix-engine-parity/) (foundation: codec, session state
machine, recovery/resend, data dictionary, storage/logging, multi/dynamic sessions, all FIX
versions + codegen, TLS/auth/monitoring, the AT suite) and
[`specs/002-qfj-parity-completion/`](specs/002-qfj-parity-completion/) (remaining gaps: session-owned
persistent resend, dictionary-driven group validation, `.cfg`-driven one-shot engine start, inbound
integrity/reject-layers/reverse-route, typed callback outcomes, all-message typed codegen +
`MessageCracker`, TLS/mTLS-from-config, weekly schedule windows, metrics export, multi-backend SQL
storage/logging). See [`MIGRATION.md`](MIGRATION.md) if upgrading past the typed-callback breaking
change.

## Why TrueFix

Existing Rust FIX libraries are either buy-side only or pre-1.0 and not production-trusted. TrueFix aims
to be the first Rust FIX engine that is:

- **Production-ready** — stable, documented public API; no `panic`/`unwrap`/`expect` on critical paths;
  typed, recoverable errors; structured observability (session state, sequence numbers, connection health,
  `metrics`-facade export).
- **Acceptor-first** — the sell-side/acceptor is a first-class citizen, on equal footing with the
  initiator (multi-session, dynamic sessions).
- **Conformance-gated** — protocol correctness is verified by a ported FIX **Acceptance Test (AT)** suite
  (56 scenario classes / 81 scenario runs across FIX.4.2/4.4), which is the hard release gate.

## Quick start

Start an entire engine — acceptor and/or initiator sessions, TLS, socket options, store, schedule —
from a `.cfg` file alone; only the `Application` callbacks are code:

```rust
use std::sync::Arc;
use truefix::config::SessionSettings;
use truefix::{Application, Engine, SessionId};

struct MyApp;

#[async_trait::async_trait]
impl Application for MyApp {
    async fn on_logon(&self, id: &SessionId) {
        println!("logged on: {id}");
    }
}

#[tokio::main]
async fn main() {
    let cfg = std::fs::read_to_string("session.cfg").unwrap();
    let settings = SessionSettings::parse(&cfg).unwrap();
    let engine = Engine::start(&settings, Arc::new(MyApp)).await.unwrap();
    tokio::signal::ctrl_c().await.ok();
    engine.shutdown();
}
```

A `.cfg` with `SocketUseSSL=Y`/`SocketKeyStore=...` builds mTLS from PEM files (no Java keystores);
`SocketConnectHost1`/`SocketConnectPort1` (etc.) add initiator failover endpoints; `FileLogPath` builds
a working session log — no code beyond `Application` required for any of it (FR-013/014/017/019/026).

The `Application` trait's callbacks return typed outcomes rather than a bare `Result<(), String>`
(see [`MIGRATION.md`](MIGRATION.md) for the breaking-change rationale and upgrade path):

```rust
use truefix::Message;
use truefix_core::{BusinessReject, DoNotSend, Reject};

#[async_trait::async_trait]
impl Application for MyApp {
    async fn from_admin(&self, _msg: &Message, _id: &SessionId) -> Result<(), Reject> {
        Ok(()) // or Err(Reject { reason, ref_tag, text }) to refuse logon
    }
    async fn to_app(&self, _msg: &mut Message, _id: &SessionId) -> Result<(), DoNotSend> {
        Ok(()) // or Err(DoNotSend) to suppress the send without consuming a sequence number
    }
    async fn from_app(&self, _msg: &Message, _id: &SessionId) -> Result<(), BusinessReject> {
        Ok(()) // or Err(BusinessReject { .. }) to emit a 35=j Business Message Reject
    }
}
```

See `crates/truefix/examples/` (`executor`, `banzai`, `ordermatch`, `multi_acceptor`) for complete,
runnable programs.

## Supported FIX versions

FIX 4.0 / 4.1 / 4.2 / 4.3 / 4.4 / 5.0 / 5.0SP1 / 5.0SP2 / **FIX Latest** and **FIXT 1.1** (with separate
transport and application dictionaries; `DefaultApplVerID` resolution), via a dual-track dictionary
that generates typed message structs/field enums/repeating-group structs at build time from the same
normalized source the runtime `DataDictionary` validates against. FIX Latest is sourced from FIX
Orchestra XML via a build-tooling-only conversion tool (`--features dict-tooling`), feeding the same
normalized-dictionary pipeline every other version uses.

## Architecture

Async-first engine on **tokio** (TLS via **rustls**), organized as a layered cargo workspace:

| Crate | Responsibility |
|-------|----------------|
| `truefix-core` | Field / FieldMap / Group / Message, field types, SOH codec, BodyLength/CheckSum, typed errors, `MessageCracker` contract |
| `truefix-dict` | Data dictionary: **build-time codegen of typed messages/field-enums/groups + a `crack_<version>` dispatcher**, plus runtime `DataDictionary` validation (dual-track, one source) |
| `truefix-session` | Session state machine, sequence management, admin messages, weekly scheduling + reset semantics, resend/gap-fill, `NextExpectedMsgSeqNum`, chunked resend, typed callback outcomes, reverse-route |
| `truefix-store` | `MessageStore` trait + Memory / File (disk-only reads) / CachedFile (bounded in-memory cache + fsync toggle) / SQL (PostgreSQL/MySQL/SQLite via sqlx) / Noop |
| `truefix-log` | `Log` trait + Screen / File / Tracing / SQL (PostgreSQL/MySQL/SQLite) / Composite, output switches (heartbeat filter, timestamps, visibility), `SessionPrefixLog` decorator |
| `truefix-transport` | Initiator + Acceptor, multi/dynamic sessions, reconnect + multi-endpoint failover, full socket-option set, rustls TLS/mTLS (file or inline PEM bytes, configurable cipher suites), PROXY protocol v1/v2 (trusted-upstream-gated) + forward proxy (SOCKS4/SOCKS5+auth/HTTP CONNECT) for initiators, synchronous-write timeouts, `metrics`-facade export |
| `truefix-config` | `SessionSettings`, `.cfg` parsing with `${name}` interpolation, `resolve()` into a runnable `Engine`-ready configuration, Appendix A key-stance registry |
| `truefix` | Facade: re-exports + `Application` trait + `MessageCracker` + `Engine::start` (one-shot `.cfg`-driven start) |
| `truefix-at` | Ported Acceptance Test suite (release gate) |
| `examples/` (`crates/truefix/examples/`) | `executor`, `banzai`, `multi_acceptor`, `ordermatch` |

### Dual-track data dictionary

Both tracks derive from **one normalized dictionary** (built from the FIX Trading Community's
Orchestra/Repository specs): build-time codegen gives compile-time-typed messages (structs, field-value
enums, typed repeating-group entries, and a `crack_<version>` dispatcher), while the runtime
`DataDictionary` gives version-agnostic validation, custom dictionaries, and user-defined fields — they
cannot diverge because they share a single source (proven by an FNV-1a hash test).

## Roadmap

Delivered **internally dependency-staged** with a verifiable checkpoint per stage.

001 (foundation; see [`plan.md`](specs/001-fix-engine-parity/plan.md)):

```
S0 Workspace & CI ─▶ S1 Core codec ─▶ S2 Minimal session ─▶ S3 Recovery/resend ─▶ S4 Data dictionary
   ─▶ S5 Store/log ─▶ S6 Multi/dynamic/schedule/reconnect ─▶ S7 All versions + codegen
   ─▶ S8 TLS/auth/monitoring ─▶ S9 Acceptance Test suite (release gate)
```

002 (parity completion; see [`plan.md`](specs/002-qfj-parity-completion/plan.md)):

```
G1 Persistent resend ─▶ G2 Group parsing/validation ─▶ G3 Config-driven start ─▶ G4 Inbound integrity
   ─▶ G5 Typed callbacks ─▶ G6 All-version codegen ─▶ G7 TLS/socket/failover ─▶ G8 Schedule reset
   ─▶ G9 Metrics export ─▶ G10 Storage/logging completeness (release gate: AT suite stays green throughout)
```

## Testing & conformance

- Table-driven unit tests for the codec and session state machine.
- Two-process integration tests (real initiator ↔ acceptor) for handshake, heartbeat, resend, TLS/mTLS,
  failover, restart-survivable persistence, and metrics export.
- The **AT suite** (`truefix-at`) ports QuickFIX/QuickFIX-J acceptance scenarios as black-box behavior
  contracts — **353/353 scenario runs** across all 9 targeted FIX versions (fix40/41/42/43/44/50/50SP1/
  50SP2/fixLatest), plus 3 independently-gated special-category suites (`validateChecksum`,
  `timestamps`, `resynch`) and a CI-enforced coverage-regression floor — and is the hard release gate
  (`cargo test -p truefix-at --test conformance`).
- CI runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, `cargo deny`, the AT
  suite, a dedicated `sql` job exercising PostgreSQL/MySQL/SQLite store+log backends against real
  service containers, and a `dict-tooling` job exercising the FIX Orchestra conversion tool (see
  [`docs/acceptance-record.md`](docs/acceptance-record.md) for the full mapping).
- **Benchmarks** (`cargo bench -p truefix-core`, `cargo bench -p truefix-session`) are
  observation/regression tools, not CI-blocking gates — no numeric latency SLO is enforced; they exist
  to make performance regressions visible across changes, not to fail a build on their own.

## Building & MSRV

```sh
cargo build --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p truefix-store -p truefix-log --features sql   # PostgreSQL/MySQL gated on availability
cargo test -p truefix-dict --features dict-tooling            # Orchestra conversion tool
cargo test -p truefix-at --test conformance                  # AT suite (release gate)
cargo bench -p truefix-core                                   # codec throughput (non-gating)
cargo bench -p truefix-session                                # session round-trip latency (non-gating)
cargo deny check        # license/advisory gate (requires cargo-deny)
```

- **Edition**: 2021. **MSRV**: Rust **1.96** (declared in `workspace.package.rust-version`; build toolchain
  pinned in `rust-toolchain.toml`). The MSRV floor is raised only deliberately and noted here when it
  changes.
- `unsafe` code is forbidden workspace-wide (`unsafe_code = "forbid"`); `unwrap`/`expect`/`panic`/indexing
  are denied on non-test builds of every crate (Constitution Principle I).

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
