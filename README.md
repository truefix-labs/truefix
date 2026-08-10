<p align="center">
  <img src="docs/icon/banner_1280x640.png" alt="TrueFix — production-grade FIX engine for Rust" width="100%">
</p>

# TrueFix

**Production-grade FIX engine for Rust.**

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)
[![crates.io](https://img.shields.io/crates/v/truefix.svg)](https://crates.io/crates/truefix)
![Rust](https://img.shields.io/badge/rust-1.96%2B-informational.svg)
![FIX](https://img.shields.io/badge/FIX-4.0%E2%80%935.0SP2%20%7C%20FIXT%201.1%20%7C%20Latest-brightgreen.svg)

- ✓ FIX 4.0–FIX 5.0 SP2, FIXT 1.1, and FIX Latest
- ✓ Initiator and acceptor, including multi-session and dynamic acceptors
- ✓ QuickFIX/J-compatible `.cfg` configuration
- ✓ Persistent message stores and structured logs
- ✓ TLS and mTLS with rustls
- ✓ Async Tokio runtime
- ✓ 483/483 black-box FIX conformance scenario runs passing

```sh
cargo add truefix@0.1.6
```

```rust
use std::sync::Arc;
use truefix::config::SessionSettings;
use truefix::{Application, Engine};

let settings = SessionSettings::parse(&std::fs::read_to_string("session.cfg")?)?;
let engine = Engine::start(&settings, Arc::new(MyApp)).await?;
```

`MyApp` implements the async [`Application`](crates/truefix/src/lib.rs) callbacks for session and
application messages. Everything else—sessions, networking, TLS, persistence, schedules, reconnect,
and dictionaries—can be selected from configuration.

## Why TrueFix?

TrueFix is for teams that want Rust's memory safety and async ecosystem without giving up the
operational features expected from an established FIX engine.

| Capability | TrueFix | QuickFIX/J | FerrumFIX |
| --- | --- | --- | --- |
| Rust native | **Yes** | No (Java/JVM) | **Yes** |
| Initiator + acceptor | **Built in** | Built in | Library primitives |
| QuickFIX/J `.cfg` files | **Compatible** | Reference format | No |
| Persistent stores | **Memory, file, SQL, MSSQL, redb, MongoDB, custom** | Built in | Application-provided |
| TLS / mTLS | **Built in** | Built in | Integration-dependent |
| Async Tokio integration | **Built in** | No | Runtime-agnostic |
| Black-box conformance gate | **483/483 scenario runs** | Acceptance-tested | Project-specific tests |

This is a project-positioning comparison, not a claim of certification or complete feature identity.
See [QuickFIX/J parity](docs/quickfixj-parity.md) for scope and evidence.

## Quick start

Create `session.cfg`:

```ini
[DEFAULT]
ConnectionType=initiator
ReconnectInterval=5
HeartBtInt=30
FileStorePath=.truefix/store
FileLogPath=.truefix/log
UseDataDictionary=Y
DataDictionary=FIX.4.4

[SESSION]
BeginString=FIX.4.4
SenderCompID=CLIENT
TargetCompID=BROKER
SocketConnectHost=127.0.0.1
SocketConnectPort=9876
```

Implement the application callbacks and run the engine:

```rust,no_run
use std::sync::Arc;

use truefix::config::SessionSettings;
use truefix::{Application, Engine, SessionId};

struct MyApp;

#[async_trait::async_trait]
impl Application for MyApp {
    async fn on_logon(&self, session: &SessionId) {
        println!("logged on: {session}");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = std::fs::read_to_string("session.cfg")?;
    let settings = SessionSettings::parse(&cfg)?;
    let engine = Engine::start(&settings, Arc::new(MyApp)).await?;

    tokio::signal::ctrl_c().await?;
    engine.shutdown();
    Ok(())
}
```

See [Getting started](docs/getting-started.md) and the runnable
[`executor`](crates/truefix/examples/executor.rs), [`banzai`](crates/truefix/examples/banzai.rs),
[`ordermatch`](crates/truefix/examples/ordermatch.rs), and
[`multi_acceptor`](crates/truefix/examples/multi_acceptor.rs) examples.

## What is included?

- Strict SOH codec with `BodyLength`/`CheckSum` validation and typed FIX messages
- Session sequencing, heartbeat, resend, gap-fill, reset, scheduling, and recovery
- Runtime data dictionaries plus generated typed messages, fields, and repeating groups
- Multi-endpoint reconnect, proxy support, socket controls, TLS/mTLS, and bounded backpressure
- Memory, file, cached-file, SQL, MSSQL, redb, MongoDB, and custom store/log backends
- Metrics-facade integration and structured tracing
- FAST and SBE codec crates alongside the tag-value FIX engine

## Workspace crates

Every published crate can be depended on independently. Use `truefix` for a complete FIX engine;
choose a lower-level crate when you only need one layer. Broker clients are standalone SDKs and do
not require the FIX engine.

| Crate | Independently usable | Purpose and supported scope |
| --- | --- | --- |
| [`truefix`](crates/truefix) | Yes — recommended FIX entry point | Facade, application callbacks, `.cfg`-driven engine startup, and re-exports of FIX layers |
| [`truefix-core`](crates/truefix-core) | Yes | Runtime-neutral FIX message model, SOH codec, framing, typed fields, groups, and dispatch |
| [`truefix-dict`](crates/truefix-dict) | Yes | Runtime dictionaries, validation, QuickFIX XML/FIX Orchestra conversion, and typed code generation |
| [`truefix-session`](crates/truefix-session) | Yes, with caller-supplied I/O | Sans-I/O session state machine: logon, sequencing, resend, heartbeat, schedules, and callbacks |
| [`truefix-transport`](crates/truefix-transport) | Yes | Tokio initiator/acceptor runtime, TCP/TLS, proxies, multi-session routing, backpressure, and shutdown |
| [`truefix-config`](crates/truefix-config) | Yes | QuickFIX-compatible `.cfg` parsing, inheritance, validation, schedules, TLS, proxy, store, and log settings |
| [`truefix-store`](crates/truefix-store) | Yes | Memory, file, cached-file, noop, SQL, MSSQL, redb, MongoDB, and custom message stores |
| [`truefix-log`](crates/truefix-log) | Yes | Screen, file, tracing, composite, SQL, MSSQL, redb, MongoDB, and custom FIX logs |
| [`truefix-binary`](crates/truefix-binary) | Yes | FAST and SBE codecs over the shared message model; not a multicast/session transport |
| [`truefix-at`](crates/truefix-at) | Yes, for maintainers/tests | Black-box FIX conformance harness; application runtime dependency is normally unnecessary |
| [`truefix-futu-client`](crates/truefix-futu-client) | Yes | Futu OpenD protobuf client, request correlation, reconnect, quote/trade requests, and pushes |
| [`truefix-twsapi-client`](crates/truefix-twsapi-client) | Yes | Interactive Brokers TWS/IB Gateway wire client for market data, accounts, orders, contracts, and events |
| [`truefix-okx-client`](crates/truefix-okx-client) | Yes | OKX V5 typed REST plus public/private/business WebSocket APIs and 264-operation inventory |
| [`truefix-ig-client`](crates/truefix-ig-client) | Yes | IG REST and Lightstreamer client, v2/v3 authentication, positions, working orders, and live updates |

## Latest release

TrueFix 0.1.6 adds native IG Lightstreamer support, v3-to-CST/XST token exchange, and the complete
working-order REST lifecycle. It also refreshes the documentation for every independently usable
workspace crate. All published workspace crates share the `0.1.6` version.

## Documentation

- [Getting started](docs/getting-started.md) — prerequisites and runnable workflows
- [Architecture](docs/architecture.md) — workspace layers, dictionaries, and extension points
- [Conformance](docs/conformance.md) — the 483-scenario release gate and validation commands
- [QuickFIX/J parity](docs/quickfixj-parity.md) — compatibility scope and historical parity work
- [Security audits](docs/security-audit.md) — safety policy and audit trail
- [Full documentation index](docs/README.md)

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p truefix-at --test conformance
```

TrueFix uses Rust edition 2024 and has an MSRV of Rust 1.96. `unsafe` is forbidden workspace-wide;
critical non-test paths deny panic-prone operations through lint policy.

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.

TrueFix is independently implemented from the FIX specification. QuickFIX/J and QuickFIX/Go were
used to study architecture and protocol behavior; their source and private data files were not
copied or translated.
