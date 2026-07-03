# TrueFix

> A production-grade FIX protocol engine in Rust, at feature parity with QuickFIX/J — first-class
> acceptor/sell-side support, a dual-track data dictionary, and FIX conformance (acceptance tests) as the
> release gate.

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)
![Status](https://img.shields.io/badge/status-implemented-brightgreen.svg)
![Rust](https://img.shields.io/badge/rust-edition%202021-informational.svg)

**Status**: QuickFIX/J parity is implemented across three specs:
[`specs/001-fix-engine-parity/`](specs/001-fix-engine-parity/) (foundation: codec, session state
machine, recovery/resend, data dictionary, storage/logging, multi/dynamic sessions, all FIX
versions + codegen, TLS/auth/monitoring, the AT suite),
[`specs/002-qfj-parity-completion/`](specs/002-qfj-parity-completion/) (session-owned persistent
resend, dictionary-driven group validation, `.cfg`-driven one-shot engine start, inbound
integrity/reject-layers/reverse-route, typed callback outcomes, all-message typed codegen +
`MessageCracker`, TLS/mTLS-from-config, weekly schedule windows, metrics export, multi-backend SQL
storage/logging), and
[`specs/003-qfj-full-parity-closure/`](specs/003-qfj-full-parity-closure/) (durable resend
completion, field-order/extra validation toggles, the remaining session config switches, a
dictionary `component` model + runtime dictionary loading, additional field types, FIX Latest via
FIX Orchestra, extended application hooks, the full 353/353-scenario AT suite across all 9 FIX
versions, a session benchmark, network hardening (PROXY protocol, forward proxy, inline PEM, cipher
suites, synchronous writes), the `truefix-dict` CLI, bounded inbound backpressure, and an MSSQL
store/log backend),
[`specs/004-engine-wiring-extra-backends/`](specs/004-engine-wiring-extra-backends/) (initiator
failover and `.cfg`-driven dictionary/SQL-backend selection wired into `Engine::start`,
`ContinueInitializationOnError`, `RedbStore`/`RedbLog`, `MongoStore`/`MongoLog`), and
[`specs/005-engine-gap-remediation/`](specs/005-engine-gap-remediation/) (session-state-machine
resend veto + GapFill substitution, PossDup anti-replay, duplicate-Logon rejection, automatic
inbound chunked-resend continuation, real multi-session `.cfg` acceptor wiring + JDBC-URL
drop-in compatibility, `SessionQualifier`/sub-ID/location-ID identity, stepped reconnect backoff +
local bind + connect timeout, store creation-time + atomic save, structured log timestamp/session-
identity columns, and full dictionary/codec model completeness — 11 new field types, open enums,
per-group child dictionaries, repeating-group CRUD, header/trailer groups, custom field order,
dictionary version metadata, value labels, and QuickFIX/J-scale bundled dictionary content across
all 9 FIX versions — the full 373/373-scenario AT suite).

## Why TrueFix

Existing Rust FIX libraries are either buy-side only or pre-1.0 and not production-trusted. TrueFix aims
to be the first Rust FIX engine that is:

- **Production-ready** — stable, documented public API; no `panic`/`unwrap`/`expect` on critical paths;
  typed, recoverable errors; structured observability (session state, sequence numbers, connection health,
  `metrics`-facade export).
- **Acceptor-first** — the sell-side/acceptor is a first-class citizen, on equal footing with the
  initiator (multi-session, dynamic sessions).
- **Conformance-gated** — protocol correctness is verified by a ported FIX **Acceptance Test (AT)** suite
  (373/373 scenario runs across all 9 targeted FIX versions), which is the hard release gate.

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
`SocketConnectHost1`/`SocketConnectPort1` (etc.) add initiator failover endpoints — `Engine::start`
automatically reconnects through them on connection loss, no code required; `UseDataDictionary=Y` (plus
`DataDictionary`/`AppDataDictionary`/`TransportDataDictionary`, a bundled version string like `FIX.4.4`
or a file path) wires real dictionary validation into the session; `JdbcURL=postgres://...`/
`mysql://...`/`sqlite:...`/`mssql://...` **or a standard JDBC-style `jdbc:postgresql://...`/
`jdbc:mysql://...`/`jdbc:sqlserver://...`/`jdbc:hsqldb:...` URL** (with credentials from separate
`JdbcUser`/`JdbcPassword` keys, matching real QuickFIX/J `.cfg` files verbatim) selects a SQL/MSSQL
store and log by URL scheme alone (behind the `sql`/`mssql` features respectively) — plus the JDBC
connection-pool tuning keys (`JdbcMaxActiveConnection`, etc.) and store table-name overrides
(`JdbcStoreMessagesTableName`/`JdbcStoreSessionsTableName`); `ContinueInitializationOnError=Y` lets the
rest of a multi-session engine start even if one session's config or connection fails;
`AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` in `.cfg` sessions that share a
`SocketAcceptPort` now build a real multi-session `AcceptorBuilder` — not just a single-session bind;
`SessionQualifier` (with `SenderSubID`/`SenderLocationID`/`TargetSubID`/`TargetLocationID`) lets two
sessions differing only by qualifier log on independently; `ReconnectInterval` accepts a stepped
backoff array (sticking at the last value), and `SocketLocalHost`/`SocketLocalPort`/
`SocketConnectTimeout` bind an initiator's outbound connection and bound its connect attempt;
`FileLogPath` builds a working session log with creation-time persisted and restored across restarts;
`InChanCapacity=<n>` bounds the inbound application-message channel (admin/session traffic always
travels on its own unbounded channel, so it's never starved by a full application channel) — no code
beyond `Application` required for any of it
(FR-001/002/003/004/012/013/014/015/016/017/019/020/021/026).

Session-state-machine correctness is enforced automatically, with no extra code: a stale
application-message resend can be vetoed by returning `Err(DoNotSend)` from `to_app` (the session
substitutes a `GapFill` instead of resending it); an inbound `PossDupFlag=Y` message with
`OrigSendingTime` after its own `SendingTime` is rejected and the session disconnects; a duplicate
Logon on an already-logged-on session is rejected rather than silently ignored; an inbound chunked
resend automatically continues to the next chunk without waiting for the counterparty to re-request it.

The `Application` trait's callbacks return typed outcomes rather than a bare `Result<(), String>`:

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

## `truefix-dict` CLI

A standalone binary wrapping the same parse/codegen logic `build.rs` uses internally — no parallel
implementation. Requires `--features dict-tooling`:

```sh
cargo run -p truefix-dict --features dict-tooling --bin truefix-dict -- <subcommand> ...

# Convert a FIX Orchestra XML source into the normalized `.fixdict` grammar.
truefix-dict generate-dict --source orchestra.xml --out normalized.fixdict

# Convert a classic QuickFIX-schema XML source (QuickFIX/J's own bundled dictionaries) instead —
# requires --version since QuickFIX XML carries major/minor/servicepack as separate attributes.
truefix-dict generate-dict --format qfj --source FIX44.xml --version FIX.4.4 --out FIX44.fixdict

# Generate the same typed Rust code build.rs would (structs, field enums, group entries,
# a MessageCracker-style crack_<name> dispatcher) from a `.fixdict`, standalone.
truefix-dict generate-code --dict normalized.fixdict --out generated.rs [--name FIX44]

# Parse-check a dictionary and print its field/message counts and dual-track content hash.
truefix-dict validate --dict normalized.fixdict
```

## Architecture

Async-first engine on **tokio** (TLS via **rustls**), organized as a layered cargo workspace:

| Crate | Responsibility |
|-------|----------------|
| `truefix-core` | Field / FieldMap / Group / Message, field types, SOH codec, BodyLength/CheckSum, typed errors, `MessageCracker` contract |
| `truefix-dict` | Data dictionary: **build-time codegen of typed messages/field-enums/groups + a `crack_<version>` dispatcher**, plus runtime `DataDictionary` validation (dual-track, one source) |
| `truefix-session` | Session state machine, sequence management, admin messages, weekly scheduling + reset semantics, resend/gap-fill, `NextExpectedMsgSeqNum`, chunked resend, typed callback outcomes, reverse-route |
| `truefix-store` | `MessageStore` trait + Memory / File (disk-only reads) / CachedFile (bounded in-memory cache + fsync toggle) / SQL (PostgreSQL/MySQL/SQLite via sqlx, `--features sql`) / MSSQL (via tiberius, `--features mssql`) / `Redb` (embedded transactional KV via `redb`, `--features redb`) / `Mongo` (via the `mongodb` driver, `--features mongodb`) / Noop |
| `truefix-log` | `Log` trait + Screen / File / Tracing / SQL (PostgreSQL/MySQL/SQLite, `--features sql`) / MSSQL (`--features mssql`) / `Redb` (`--features redb`) / `Mongo` (`--features mongodb`) / Composite, output switches (heartbeat filter, timestamps, visibility), `SessionPrefixLog` decorator |
| `truefix-transport` | Initiator + Acceptor, multi/dynamic sessions, reconnect + multi-endpoint failover, full socket-option set, rustls TLS/mTLS (file or inline PEM bytes, configurable cipher suites), PROXY protocol v1/v2 (trusted-upstream-gated) + forward proxy (SOCKS4/SOCKS5+auth/HTTP CONNECT) for initiators, synchronous-write timeouts, bounded inbound application-message backpressure (`InChanCapacity`, admin traffic never starved), `metrics`-facade export |
| `truefix-config` | `SessionSettings`, `.cfg` parsing with `${name}` interpolation, `resolve()` into a runnable `Engine`-ready configuration, Appendix A key-stance registry |
| `truefix` | Facade: re-exports + `Application` trait + `MessageCracker` + `Engine::start` (one-shot `.cfg`-driven start) |
| `truefix-at` | Ported Acceptance Test suite (release gate) |
| `examples/` (`crates/truefix/examples/`) | `executor`, `banzai`, `multi_acceptor`, `ordermatch` |

`RedbStore`/`RedbLog` and `MongoStore`/`MongoLog` are library-level only — selected via
`StoreConfig::Redb`/`Mongo`/`LogConfig`-adjacent constructors through the direct Rust API, not from a
`.cfg` key. `RedbStore` replaces QuickFIX/J's obsolete `SleepycatStore` (Berkeley DB JE) with a modern,
license-clean embedded transactional store; `MongoStore`/`MongoLog` match QuickFIX/Go's own MongoDB
option. Neither introduces new `.cfg` surface, matching the existing precedent that `SqlLog`/`MssqlLog`
also aren't fully `.cfg`-selectable beyond `JdbcURL`'s scheme dispatch (only `Screen`/`File`/`Tracing`/
`Composite` logs are).

### Dual-track data dictionary

Both tracks derive from **one normalized dictionary** (built from the FIX Trading Community's
Orchestra/Repository specs, plus a QuickFIX-XML converter for the 8 non-Latest bundled versions —
`crates/truefix-dict/src/qfj_xml.rs`, `--features dict-tooling`): build-time codegen gives
compile-time-typed messages (structs, field-value enums, typed repeating-group entries, and a
`crack_<version>` dispatcher), while the runtime `DataDictionary` gives version-agnostic validation,
custom dictionaries, and user-defined fields — they cannot diverge because they share a single source
(proven by an FNV-1a hash test). Field-type coverage matches QuickFIX/J: 27 `FieldType`s including
`PriceOffset`/`LocalMktDate`/`DayOfMonth`/`UtcDate`/`Time`/`Currency`/`Exchange`/
`MultipleValueString`/`MultipleStringValue`/`MultipleCharValue`/`Country`, open-enum fields, per-group
child dictionaries (deep nested-group validation), header/trailer repeating groups, custom
per-message field emission order, structured dictionary version metadata (validated against a
message's `BeginString`), and value→label lookup. Every bundled dictionary (FIX 4.0 through
5.0SP2, plus FIXT 1.1) now matches QuickFIX/J's real field/message scale (e.g. FIX 4.4: 953
fields/92 messages; FIX 5.0SP2: 1610 fields/110 messages) rather than a documented subset.

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

003 (full parity closure; see [`plan.md`](specs/003-qfj-full-parity-closure/plan.md)):

```
G2 Durable resend completion ─▶ G1a AT coverage broadening ─▶ G3 Field-order/extra validation toggles
   ─▶ G4 Remaining session switches ─▶ G5 Dictionary components ─▶ G6 Runtime dictionary loading
   ─▶ G7 Field type completeness ─▶ G8 FIX Latest (Orchestra) ─▶ G9 Extended application hooks
   ─▶ G1b AT full closure (353/353 across 9 versions, release gate) ─▶ G10 Session benchmark
   ─▶ G11 Network hardening ─▶ G12 `truefix-dict` CLI ─▶ G13/G14 Backpressure + MSSQL SQL backend
```

004 (engine wiring & extra backends; see [`plan.md`](specs/004-engine-wiring-extra-backends/plan.md)):

```
W1 Initiator failover wired into Engine ─▶ W2 .cfg dictionary/validator wiring
   ─▶ W3 .cfg SQL backend selection (JdbcURL) ─▶ W4 ContinueInitializationOnError
   ─▶ W5 RedbStore/RedbLog ─▶ W6 MongoStore/MongoLog
   (release gate: 353/353-scenario AT suite stays green and unmodified — no protocol behavior touched)
```

005 (engine gap remediation; see [`plan.md`](specs/005-engine-gap-remediation/plan.md)):

```
G1 Correctness bugs (# truncation, Signature/93) ─▶ G2 .cfg keys that misrepresent behavior
   (multi-session AcceptorBuilder, JDBC URLs) ─▶ G3 Session protocol-correctness safeguards
   (resend veto+GapFill, PossDup anti-replay, duplicate-Logon) ─▶ G4 Inbound chunked-resend
   continuation ─▶ G5 Session identity (SessionQualifier) ─▶ G6 Initiator connection robustness
   (reconnect backoff, local bind, connect timeout) ─▶ G7 Store/log persistence hardening
   ─▶ G9 Dictionary and codec model completeness (11 field types, open enums, child dictionaries,
   group CRUD, header/trailer groups, field order, version metadata, QFJ-scale bundled content)
   ─▶ G8 Config-key stance registry accuracy sweep
   (release gate: 373/373-scenario AT suite — grew from 353, a deliberate disclosed floor bump)
```

## Testing & conformance

- Table-driven unit tests for the codec and session state machine.
- Two-process integration tests (real initiator ↔ acceptor) for handshake, heartbeat, resend, TLS/mTLS,
  failover, restart-survivable persistence, and metrics export.
- The **AT suite** (`truefix-at`) ports QuickFIX/QuickFIX-J acceptance scenarios as black-box behavior
  contracts — **373/373 scenario runs** across all 9 targeted FIX versions (fix40/41/42/43/44/50/50SP1/
  50SP2/fixLatest), plus 3 independently-gated special-category suites (`validateChecksum`,
  `timestamps`, `resynch`) and a CI-enforced coverage-regression floor — and is the hard release gate
  (`cargo test -p truefix-at --test conformance`).
- CI runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, `cargo deny`, the AT
  suite, a dedicated `sql` job exercising PostgreSQL/MySQL/SQLite store+log backends, an `mssql` job
  exercising the MSSQL store+log backend, a `redb` job (no service container needed — `redb` is
  embedded), a `mongo` job exercising the MongoDB store+log backend, each against real service
  containers where applicable, and a `dict-tooling` job exercising the FIX Orchestra conversion tool
  (see [`docs/acceptance-record.md`](docs/acceptance-record.md) for the full mapping).
- **Benchmarks** (`cargo bench -p truefix-core`, `cargo bench -p truefix-session`) are
  observation/regression tools, not CI-blocking gates — no numeric latency SLO is enforced; they exist
  to make performance regressions visible across changes, not to fail a build on their own.

## Building & MSRV

```sh
cargo build --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p truefix-store -p truefix-log --features sql     # PostgreSQL/MySQL gated on availability
cargo test -p truefix-store -p truefix-log --features mssql   # MSSQL gated on availability
cargo test -p truefix-store -p truefix-log --features redb    # embedded, runs unconditionally
cargo test -p truefix-store -p truefix-log --features mongodb # MongoDB gated on availability
cargo test -p truefix-dict --features dict-tooling         # Orchestra conversion tool + CLI
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
