# Phase 0 Research: QuickFIX/J Parity Completion

Decisions resolving the design unknowns implied by the spec/clarifications. Each: **Decision /
Rationale / Alternatives considered**. Protocol-behaviour decisions cite FIX-spec/reference behaviour
(Principle II); no reference source is copied (Principle III).

## R1 — Session-owned persistent resend (G1, FR-001/002/003)

**Decision**: Inject an `Arc<dyn MessageStore>` into `Session` (constructor `Session::with_store`). The
resend path (`build_resend`) reads sent message bodies from the store keyed by sequence number;
`send_stored` writes the encoded body to the store as it assigns the outbound seq. The in-memory
`BTreeMap` becomes a write-through cache, not the source of truth. The existing `MessageStore` trait is
async; the `Session` engine is sans-IO/sync, so the transport mediates store reads on the session's
behalf during resend (the session emits a `NeedResend{begin,end}` intent the transport fulfils, OR the
session is given a synchronous snapshot accessor). **Chosen shape**: keep `Session` sans-IO — the
transport pre-loads the requested range from the async store and feeds bodies back into a
`build_resend(range, bodies)` call. `PersistMessages=N` ⇒ store writes skipped ⇒ resend gap-fills.

**Rationale**: Preserves the deterministic, unit-testable sans-IO `Session` design from 001 (no async in
the state machine) while making durable storage the source of truth (SC-006). Matches QuickFIX behaviour:
admin messages in a resent range collapse to SequenceReset-GapFill, application messages replay as PossDup.

**Alternatives considered**: (a) make `Session` hold the async store directly — rejected: forces async
into the sans-IO core, breaking the Event→Vec<Action> purity. (b) Keep memory-only and document the
limitation — rejected: violates the already-claimed FR-G2/SC-006.

## R2 — Dictionary-driven group/component parsing (G2, FR-004/005)

**Decision**: Add `decode_with_dict(bytes, &DataDictionary)`. The flat field scan is retained for the
no-dictionary path; when a dictionary is supplied, on encountering a group-count tag the decoder consumes
the declared number of entries, using the dictionary's per-group delimiter (first field) to bound each
entry, recursing for nested groups and expanding components into their fields. Group validation
(`FirstFieldInGroupIsDelimiter`, `ValidateUnorderedGroupFields`, count-vs-entries) runs against the
structured result.

**Rationale**: FIX repeating groups are only parseable with the dictionary (the count tag + delimiter
define boundaries); QuickFIX/J and QuickFIX/Go both decode groups dictionary-driven. Structured groups
are the precondition for the 14i/14j/21/QFJ934 AT scenarios and for the typed codegen group structs (R4).

**Alternatives considered**: heuristic delimiter detection without a dictionary — rejected: ambiguous and
non-conformant. A separate parser crate — rejected: keep it in `truefix-core` codec beside `decode`.

## R3 — Typed callback outcomes (G5, FR-016)

**Decision**: Replace the three `Result<(), String>` callback signatures with typed results:
`from_admin -> Result<(), Reject>`, `to_app -> Result<(), DoNotSend>`, `from_app -> Result<(),
BusinessReject>` (names mirror QuickFIX semantics). `Reject`/`BusinessReject` carry a reason code and
optional reference tag; `DoNotSend` is a marker. The engine acts on each: `Reject` from a logon refuses
the session; `DoNotSend` suppresses the outbound (and it is not stored as sent); `BusinessReject` makes
the engine emit a 35=j carrying the reason+ref tag. This is a **breaking change**; a `MIGRATION.md`
note + minor-version bump (pre-1.0) accompanies it (Principle I).

**Rationale**: Opaque `String` cannot carry a reject reason/ref tag and is currently ignored on the app
path; the typed model is what `contracts/application-api.md` (001) specified and what integrators need.

**Alternatives considered**: additive typed variants alongside the string ones — rejected per
Clarifications (pre-1.0; avoid dual maintenance).

## R4 — Typed codegen for all messages + MessageCracker (G6, FR-020/021/022)

**Decision**: Extend `build.rs` with a `codegen` module that, from the normalized `.fixdict`, emits per
version: a field module (typed accessors + value enums, e.g. `Side::Buy`), per-message structs with
typed field/group/component accessors, and group/component structs. Generated structs build/read a
generic `Message` underneath (thin typed wrapper), guaranteeing byte-identical encode/decode with the
generic path (FR-021). A `MessageCracker` trait dispatches an incoming `Message` to a typed handler keyed
by `(BeginString, MsgType)`. The dual-track FNV-1a hash continues to assert codegen and runtime
`DataDictionary` share one source.

**Rationale**: Completes Principle IV. Wrapping the generic `Message` (rather than a parallel
representation) keeps one wire path → no codegen/runtime divergence and trivially byte-identical output.

**Alternatives considered**: a fully independent typed representation with its own serializer — rejected:
risks dual-track divergence and duplicate codec bugs. Generating only a subset — rejected per
Clarifications ("all messages typed").

## R5 — SQL Postgres/MySQL/SQLite (G10, FR-024)

**Decision**: Use sqlx with all three drivers behind the `sql` feature. Prefer per-backend pools selected
at runtime from the configured URL scheme (`postgres://`, `mysql://`, `sqlite:`), sharing one schema
abstraction and parameterised, portable SQL (avoid dialect-specific syntax; keep DDL minimal and
hand-written per backend where types differ). Table names and pool settings come from config keys.
Multi-backend tests gate on DB availability (env-provided URLs / CI service containers); SQLite always
runs.

**Rationale**: JDBC-equivalent parity (Principle VII) per Clarifications. sqlx supports all three with
async pools already used for SQLite.

**Alternatives considered**: sqlx `Any` driver — considered; viable but its lowest-common-denominator
typing complicates the existing typed queries, so explicit per-backend pools are clearer. A separate
ORM — rejected (extra dependency, license surface).

## R6 — Settings → engine mapping (G3, FR-013/014/015)

**Decision**: Add a `truefix-config` mapping/builder layer: `EngineConfig::from_settings(&Settings)`
resolves `[DEFAULT]` + per-`[SESSION]` (override precedence) into a list of fully-typed per-session
configs (SessionConfig, store spec, log spec, schedule, transport/TLS, socket options). A typed
`ConfigError { key, session, kind }` is returned on any invalid value, missing required key, or unusable
resource — startup is all-or-nothing (no partial start). `truefix` facade adds `Engine::start(settings)`
that routes each session by `ConnectionType` to acceptor/initiator construction.

**Rationale**: This is the central usability blocker (SC-001) and the dependency for TLS/socket/schedule
config (G7/G8). Fail-fast typed errors match Principle I (no silent ignore, no panic).

**Alternatives considered**: lazily build per-session on connect — rejected: defers config errors to
runtime; all-or-nothing validation up front is safer for operators.

## R7 — TLS from config + mTLS (G7, FR-017)

**Decision**: Build `rustls::ServerConfig`/`ClientConfig` from `.cfg` keys using **rustls-pemfile** to
load key/cert/CA from configured file paths. `NeedClientAuth=Y` ⇒ server uses a client-cert verifier
built from the configured CA roots; client config presents the configured client cert. Min TLS version
and SNI/server-name come from their keys. The pre-built-config code path is retained for programmatic use.

**Rationale**: Secure deployment must be config-driven (FR-F6); rustls-pemfile is the standard
license-compatible loader. mTLS via a CA-roots verifier is the rustls-idiomatic approach.

**Alternatives considered**: native-tls/openssl — rejected: heavier, C dependency, license/portability
cost; rustls is already the stack.

## R8 — Metrics export via facade only (G9, FR-023)

**Decision**: Emit through the `metrics` facade (`gauge!`, `counter!`) with stable metric names labelled
by SessionID: `truefix_session_state`, `truefix_next_sender_seqnum`, `truefix_next_target_seqnum`,
`truefix_messages_sent_total`, `truefix_messages_received_total`, `truefix_reconnects_total`. No exporter
is bundled; operators attach one. The transport updates these alongside the existing `Monitor`.

**Rationale**: Completes Principle I observability without forcing an HTTP-server dependency (per
Clarifications). The `metrics` facade is already a workspace dependency.

**Alternatives considered**: bundling a Prometheus endpoint — rejected per Clarifications (optional/extra
dep).

## R9 — Timestamp precision (G4, FR-009)

**Decision**: Add a `TimeStampPrecision` (SECONDS/MILLIS/MICROS/NANOS) to session config; the UTCTimestamp
formatter emits the configured number of sub-second digits (default MILLIS for QuickFIX/J parity). Inbound
parsing already accepts/truncates sub-second precision; verify round-trip at each level.

**Rationale**: FR-009 + the baseline nanosecond-capability goal; QuickFIX/J default is MILLIS.

**Alternatives considered**: always-nanos — rejected: breaks byte-parity with millis-default peers.

## R10 — Reverse routing (G4, FR-011)

**Decision**: A header helper reverses the routing pair tags (`OnBehalfOfCompID↔DeliverToCompID`,
`OnBehalfOfSubID↔DeliverToSubID`, `OnBehalfOfLocationID↔DeliverToLocationID`) onto generated
replies/rejects; empty routing tags follow the empty-tags rule (no reversal, no error). Exercised by
`ReverseRoute`/`ReverseRouteWithEmptyRoutingTags` AT scenarios.

**Rationale**: Defined FIX routing behaviour for relayed sessions (Appendix B).

**Alternatives considered**: none material.

## R11 — Dependency license audit (Phase 0 gate, Principle III)

**Decision**: Confirm the newly-exercised deps are Apache-2.0/MIT-compatible before use:
`rustls-pemfile` (MIT/Apache-2.0/ISC), `sqlx` Postgres/MySQL drivers (MIT/Apache-2.0), `metrics`
(MIT/Apache-2.0). Re-run `cargo deny` (already in CI). No copyleft introduced.

**Rationale**: Principle III is absolute; new deps must keep the Apache-2.0 OR MIT releasability.

**Alternatives considered**: n/a (gate, not a choice).
