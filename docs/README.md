# TrueFix documentation

This index separates current user documentation from historical design and audit records. Current
facts are derived from the workspace manifests and public code as of 2026-07-20.

## Current documentation

- [Workspace README](../README.md): project overview, install, quick start, and project positioning.
- [Getting Started](getting-started.md): runnable Futu, IB TWS, Binance, OKX, and IG workflows.
- [Architecture](architecture.md): engine layers, dictionaries, and extension points.
- [Conformance](conformance.md): black-box FIX release gates and validation commands.
- [QuickFIX/J parity](quickfixj-parity.md): compatibility scope and historical evidence.
- [Security audits](security-audit.md): safety policy and audit trail.
- Each crate's `README.md`: crate-specific public surface, optional features, and validation command.

## Project status

All workspace crates currently share version `0.1.4`, Rust edition 2024, and MSRV 1.96.

| Area | Crate(s) | Current implementation |
| --- | --- | --- |
| FIX engine facade | `truefix` | `.cfg`-driven engine plus lower-level public APIs |
| FIX model and codec | `truefix-core` | tag-value messages, framing, validation, groups |
| Session semantics | `truefix-session` | sans-I/O state machine, scheduling, sequencing, recovery |
| Networking | `truefix-transport` | TCP/TLS initiator/acceptor, proxies, reconnect, multi/dynamic sessions |
| Dictionaries | `truefix-dict` | runtime validation, normalized dictionaries, XML/Orchestra tooling, codegen |
| Persistence and logs | `truefix-store`, `truefix-log` | file/memory plus feature-gated databases and custom traits |
| Binary codecs | `truefix-binary` | FAST and SBE codecs; no exchange multicast/session adapter |
| Acceptance tests | `truefix-at` | maintainer-facing black-box scenario runner |
| Broker/exchange clients | Futu, TWS, OKX, IG client crates | independent native SDKs; no shared gateway abstraction yet |

There is currently no `truefix-gateway`, Tauri desktop application, unified instruments/provider
registry, news provider, or AI trading-agent crate in the workspace. Those remain product ideas.

## Historical records

- [Acceptance record](acceptance-record.md) and [parity matrix](parity-matrix.md) preserve evidence
  accumulated during earlier feature work. Embedded test counts and dependency versions are dated.
- [No-panic audit](no-panic-audit.md) records the lint-based critical-path policy.
- [Roadmap](roadmap.md) and [`todo/`](todo/) contain proposals and audit findings. Completion marks
  mean resolved in that historical audit, not a fresh 2026-07-20 verification.
- [`news_vender.md`](news_vender.md) is product research, not an implemented feature list.

## Repository verification

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Feature-gated database integration tests require their backend configuration. Live client tests
additionally require explicit credentials and network opt-in.
