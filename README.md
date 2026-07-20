<p align="center">
  <img src="docs/icon/banner_1280x640.png" alt="TrueFix — production-grade FIX engine for Rust" width="100%">
</p>

# TrueFix

**Production-grade FIX engine for Rust.**

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)
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
cargo add truefix
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

The workspace also contains independent Rust clients for
[Futu](crates/truefix-futu-client), [Interactive Brokers TWS](crates/truefix-twsapi-client),
[OKX](crates/truefix-okx-client), and [IG](crates/truefix-ig-client). They are standalone SDKs, not a
single unified gateway.

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
