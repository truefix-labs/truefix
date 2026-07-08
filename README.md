# TrueFix

> A production-grade FIX protocol engine in Rust, at feature parity with QuickFIX/J — first-class
> acceptor/sell-side support, a dual-track data dictionary, and FIX conformance (acceptance tests) as the
> release gate.

[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)
![Status](https://img.shields.io/badge/status-implemented-brightgreen.svg)
![Rust](https://img.shields.io/badge/rust-edition%202024-informational.svg)

**Status**: QuickFIX/J parity is implemented, followed by four independent post-parity audit
remediation passes, across nine specs:
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
all 9 FIX versions — the full 373/373-scenario AT suite),
[`specs/006-audit-remediation/`](specs/006-audit-remediation/) (a five-independent-review audit
plus a supplementary Chinese-language pass: session anti-replay/desync fixes, SubID/LocationID/
Qualifier routing correctness, silent store-failure/spurious-reset fixes, orphaned-task cleanup,
JDBC-grammar and FileStore-atomicity fixes, and a dormant-dictionary-feature/stale-provenance
sweep — 405/405-scenario AT suite),
[`specs/007-second-audit-remediation/`](specs/007-second-audit-remediation/) (a second-pass audit
of defects not already covered by 006: data-loss, protocol-breaking-handshake, resource-leak, and
DoS-adjacent fixes; a QuickFIX/J/Go-parity split two-file sequence-number store layout with
transparent single-file auto-migration; and trading-hours schedule awareness — 424/424-scenario AT
suite),
[`specs/009-audit-005-remediation/`](specs/009-audit-005-remediation/) (a four-pass cross-referenced
audit: a Mongo sequence-persistence total-failure fix, an unauthenticated multi-session-acceptor
pre-Logon DoS close, `BeginSeqNo=0`/acceptor-`ResetOnLogon`/gap-fill-verification-bypass fixes,
FIXT 1.1 header/decode gaps, a native-cert-backed TLS default trust store, and dictionary-tooling/
AT-harness-coverage hardening — 444/444-scenario AT suite), and
[`specs/010-audit-006-remediation/`](specs/010-audit-006-remediation/) (session lifecycle/
`SequenceReset`/`PossResend`/cancel-on-disconnect fixes, per-dynamic-identity acceptor persistence,
config-default/store/transport/log operational parity with QuickFIX/J, and core codec/dictionary
strictness plus file log/store growth bounds and duplicate-sequence detection — 483/483-scenario AT
suite).

## Why TrueFix

Existing Rust FIX libraries are either buy-side only or pre-1.0 and not production-trusted. TrueFix aims
to be the first Rust FIX engine that is:

- **Production-ready** — stable, documented public API; no `panic`/`unwrap`/`expect` on critical paths;
  typed, recoverable errors; structured observability (session state, sequence numbers, connection health,
  `metrics`-facade export).
- **Acceptor-first** — the sell-side/acceptor is a first-class citizen, on equal footing with the
  initiator (multi-session, dynamic sessions).
- **Conformance-gated** — protocol correctness is verified by a ported FIX **Acceptance Test (AT)** suite
  (483/483 scenario runs across all 9 targeted FIX versions), which is the hard release gate.

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

### Custom `MessageStore`/`Log` backends

The built-in store/log backends (`Memory`/`File`/`CachedFile`/`Noop`/`Sql`/`Mssql`/`Redb`/`Mongo`
for storage; `Screen`/`File`/`Tracing`/`Composite`/`Sql`/`Mssql` for logging) cover most deployments,
but `MessageStore` and `Log` are both plain, fully public, object-safe traits — a proprietary audit
sink or a backend not on the built-in list is a normal trait implementation away, with two supported
paths from the `.cfg`-driven `Engine::start` entry point (not just the lower-level
`truefix::transport::Services` API):

1. **`StoreConfig::Custom(Arc<dyn MessageStore>)`** — pass an already-built store through
   `truefix_store::build_store`/anywhere else a `StoreConfig` is constructed programmatically.
2. **`Engine::start_with_overrides`** — layers a caller-supplied `Services` (with `store`/`log` set)
   on top of whatever a session's `.cfg` block would otherwise resolve, keyed by the session's label
   (`"{BeginString}:{SenderCompID}->{TargetCompID}"`). When both a `.cfg`-driven backend and an
   override are present for the same session, **the override wins**.

```rust
use std::collections::HashMap;
use std::sync::Arc;

use truefix::config::SessionSettings;
use truefix::transport::Services;
use truefix::Engine;

let settings = SessionSettings::parse(cfg_text)?;
let mut overrides = HashMap::new();
overrides.insert(
    "FIX.4.4:SERVER->CLIENT".to_owned(),
    Services {
        store: Some(my_custom_store as Arc<dyn truefix_store::MessageStore>),
        log: Some(my_custom_log as Arc<dyn truefix_log::Log>),
        ..Services::default()
    },
);
let engine = Engine::start_with_overrides(&settings, app, overrides).await?;
```

Combining an override with a `.cfg` setting that only makes sense for the built-in backend it
replaces (e.g. a custom `log` override alongside `FileLogPath`'s `MaxFileLogSize`/
`FileLogMaxGenerations`/`FileLogRollIntervalSecs` tuning keys) is rejected at startup with a typed
`ConfigError` rather than silently ignoring the now-irrelevant setting. The override mechanism
currently covers non-grouped acceptor sessions and initiators (one session per port — the common
case); acceptor-group/dynamic-template sessions still use the lower-level
`truefix::transport::{AcceptorBuilder, Services}` API directly for the same effect. See
`crates/truefix/tests/custom_backend_injection.rs` for a complete, runnable example exercising
every case above (`cargo test -p truefix --test custom_backend_injection`).

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
| `truefix-binary` | FAST/SBE binary protocol codecs, schema/template parsing models, typed binary decode errors, and `Message` conversion helpers |
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

### `UseDataDictionary` / `DataDictionary` / `ValidateIncomingMessage`

```ini
UseDataDictionary=Y
DataDictionary=dict/binance-oe.fixdict
ValidateIncomingMessage=N
```

- **`UseDataDictionary=Y`** attaches a runtime `DataDictionary` to the session at all. This is the
  switch that turns on group-aware decoding (`decode_with_groups`): header/trailer repeating groups
  and, since feature 011, body-level repeating groups too (e.g. `NoPartyIDs`) are structured against
  the dictionary's declared members/delimiters instead of landing as a flat, unstructured field list.
  **Default: `N`/absent** — no dictionary is attached, matching pre-existing behavior: messages
  still decode (SOH/tag parsing, `BodyLength`/`CheckSum` framing), but every field — including
  repeating groups — stays flat, and no content validation of any kind runs.
- **`DataDictionary=<path or bundled version>`** names which dictionary to load once
  `UseDataDictionary=Y` is set — either a bundled version string (`FIX.4.4`, `FIX.5.0SP2`, ...) or a
  filesystem path to a `.fixdict`/vendor XML source, resolved relative to where the process runs.
  `dict/binance-oe.fixdict` above is an example of a **user-supplied vendor dictionary** — e.g.
  Binance's own FIX OE spec, converted (via `truefix-dict generate-dict --format qfj` or the
  vendor-XML path) into the normalized `.fixdict` grammar and placed at that path by the operator; it
  is not bundled with this repo, but once converted it loads and behaves exactly like a bundled
  version string. For a real FIXT 1.1 split, use `AppDataDictionary`/`TransportDataDictionary` instead
  (or alongside) — see the Quick start section above. **Required when `UseDataDictionary=Y`** —
  omitting it is a config error (`ConfigError::MissingRequired`), it does not silently fall back to
  "no dictionary".
- **`ValidateIncomingMessage`** controls only the *content*-level checks the attached dictionary
  performs on each inbound message — required fields present, known fields/values, field data types,
  `BeginString`/version match, group field ordering, and the other `Validate*` toggles
  (`ValidateFieldsOutOfOrder`, `ValidateFieldsHaveValues`, `ValidateUserDefinedFields`, etc.). It does
  **not** control structural decoding: even with `ValidateIncomingMessage=N`, an attached dictionary
  is still used to decode repeating groups correctly (`decode_with_groups` runs unconditionally
  whenever a dictionary is attached — see `crates/truefix-transport/src/lib.rs`'s read loop). Setting
  it to `N` simply makes dictionary content-validation a no-op (`DataDictionary::validate` returns
  `Ok(())` immediately) instead of rejecting the message. **Default: `Y`/`true`** whenever
  `UseDataDictionary=Y` is set (matching QuickFIX/J) — content validation runs unless explicitly
  turned off. It has no effect at all when `UseDataDictionary` is absent/`N`, since there is no
  dictionary to validate against either way.

Put together, the config block above is the shape used for a vendor dictionary you trust for
**message structure** (so groups like `NoPartyIDs` come back as real, indexable entries) but not for
strict **content** conformance — e.g. because the vendor's real wire traffic includes fields, enum
values, or field orderings the hand-converted `.fixdict` doesn't perfectly document, and you would
rather let those through than reject otherwise-legitimate messages.

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

### Post-parity audit remediation

001-005 closed out QuickFIX/J feature parity; every feature since has been an independent
source-vs-QuickFIX/J/Go audit pass (own `docs/todo/00N.md` finding inventory, own
`/speckit-clarify` scope decisions, own release-gated AT-suite regression floor) rather than a new
staged roadmap, so these are listed by finding-inventory source instead of a stage diagram:

- **006** (see [`plan.md`](specs/006-audit-remediation/plan.md)): `docs/todo/003.md` — 5
  independent deep-dive reviews plus a supplementary Chinese-language pass (`B1`-`B30`); session
  anti-replay/desync, SubID/LocationID/Qualifier routing, silent store failures, orphaned-task
  leaks, JDBC grammar, and FileStore atomicity (release gate: 405/405-scenario AT suite).
- **007** (see [`plan.md`](specs/007-second-audit-remediation/plan.md)): `docs/todo/004.md` —
  defects not already closed by 006; data loss, protocol-breaking handshakes, resource leaks, a
  DoS-adjacent malformed-input acceptance, a QuickFIX/J/Go-parity split sequence-number store with
  transparent auto-migration, and trading-hours schedule awareness (release gate: 424/424-scenario
  AT suite).
- **009** (see [`plan.md`](specs/009-audit-005-remediation/plan.md)): `docs/todo/005.md` — a
  four-pass cross-referenced audit of all 9 crates; a Mongo sequence-persistence total failure, an
  unauthenticated multi-session-acceptor pre-Logon DoS, `BeginSeqNo=0`/`ResetOnLogon`/gap-fill
  verification-bypass fixes, FIXT 1.1 header/decode gaps, a native-cert-backed TLS default trust
  store, and dictionary-tooling/AT-harness-coverage hardening (release gate: 444/444-scenario AT
  suite).
- **010** (see [`plan.md`](specs/010-audit-006-remediation/plan.md)): `docs/todo/006.md` — session
  lifecycle/`SequenceReset`/`PossResend`/cancel-on-disconnect fixes, per-dynamic-identity acceptor
  persistence, config-default/store/transport/log operational parity with QuickFIX/J, and core
  codec/dictionary strictness plus file log/store growth bounds and duplicate-sequence detection
  (release gate: 483/483-scenario AT suite).

(There is no `specs/008-*`: that slot was not used.)

## Testing & conformance

- Table-driven unit tests for the codec and session state machine.
- Two-process integration tests (real initiator ↔ acceptor) for handshake, heartbeat, resend, TLS/mTLS,
  failover, restart-survivable persistence, and metrics export.
- The **AT suite** (`truefix-at`) ports QuickFIX/QuickFIX-J acceptance scenarios as black-box behavior
  contracts — **483/483 scenario runs** across all 9 targeted FIX versions (fix40/41/42/43/44/50/50SP1/
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
cargo test -p truefix-binary                               # FAST/SBE binary codecs
cargo test -p truefix-at --test conformance                  # AT suite (release gate)
cargo bench -p truefix-core                                   # codec throughput (non-gating)
cargo bench -p truefix-session                                # session round-trip latency (non-gating)
cargo deny check        # license/advisory gate (requires cargo-deny)
```

- **Edition**: 2024. **MSRV**: Rust **1.96** (declared in `workspace.package.rust-version`; build toolchain
  pinned in `rust-toolchain.toml`). The MSRV floor is raised only deliberately and noted here when it
  changes.
- `unsafe` code is forbidden workspace-wide (`unsafe_code = "forbid"`); `unwrap`/`expect`/`panic`/indexing
  are denied on non-test builds of every crate (Constitution Principle I).
- Session `.cfg` defaults to `Protocol=SOH`. `Protocol=FAST` requires `FastTemplatePath`, and
  `Protocol=SBE` requires `SbeSchemaPath`; both paths are resolved during config loading so binary
  sessions fail fast when the selected schema/template input is missing.

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
