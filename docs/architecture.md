# Architecture

TrueFix is an async-first, Tokio-based FIX engine. The facade crate starts complete initiator and
acceptor deployments from QuickFIX/J-style `.cfg` files, while the lower layers remain available for
applications that need custom wiring.

## Workspace layers

| Crate | Responsibility |
| --- | --- |
| `truefix` | Facade, `Application`, `MessageCracker`, and `.cfg`-driven `Engine` |
| `truefix-core` | Fields, messages, repeating groups, SOH codec, framing, and typed errors |
| `truefix-dict` | Runtime validation, normalized dictionaries, typed code generation, and dictionary CLI |
| `truefix-session` | Sans-I/O session state machine, sequencing, recovery, resend, and schedules |
| `truefix-transport` | Tokio TCP/TLS, initiator/acceptor, reconnect, proxies, and backpressure |
| `truefix-config` | `.cfg` parsing, interpolation, validation, and runnable session resolution |
| `truefix-store` | Memory, file, cached-file, SQL, MSSQL, redb, MongoDB, and custom stores |
| `truefix-log` | Screen, file, tracing, database, and composite logs |
| `truefix-binary` | FAST and SBE codecs and schema models |
| `truefix-at` | Maintainer-facing black-box acceptance test runner |

## Configuration-driven engine

`Engine::start` resolves each configured session into networking, dictionary, store, log, and
schedule services. It supports initiators, single- and multi-session acceptors, dynamic acceptor
templates, failover endpoints, TLS/mTLS, and feature-gated database backends.

Applications needing proprietary persistence or audit sinks can implement the public, object-safe
`MessageStore` and `Log` traits. `Engine::start_with_overrides` injects those services per session;
lower-level transport builders support fully custom composition.

## Dual-track dictionaries

One normalized dictionary source feeds both tracks:

- Build-time code generation produces typed messages, field enums, repeating-group entries, and
  version-specific `MessageCracker` dispatch.
- Runtime `DataDictionary` validation supports version-agnostic messages, custom dictionaries, and
  user-defined fields.

Bundled coverage includes FIX 4.0 through FIX 5.0 SP2 and FIXT 1.1. FIX Latest is generated from FIX
Orchestra input. The `truefix-dict` CLI can convert QuickFIX-schema XML or Orchestra XML, validate a
normalized dictionary, and generate Rust code.

## Operational design

- Tokio supplies async task and I/O orchestration; rustls supplies TLS and mTLS.
- Admin/session traffic is isolated from the bounded application-message channel.
- Persistence and logging are traits, so built-in and application-specific backends use the same
  session machinery.
- Metrics use the `metrics` facade and diagnostics use structured `tracing` events.
- Workspace code forbids `unsafe`; critical non-test crates deny panic-prone operations by lint.

See [Getting started](getting-started.md) for configuration examples and [Conformance](conformance.md)
for the release gates.
