# Implementation Plan: FAST/SBE Binary Protocol Codec (`truefix-binary`)

**Branch**: `013-fast-sbe-codec` | **Date**: 2026-07-08 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/013-fast-sbe-codec/spec.md`

## Summary

Add a new crate `truefix-binary` implementing FAST (FIX Adapted for STreaming) and SBE (Simple
Binary Encoding) codecs, selectable per-session via a new static `Protocol` config option
(`Soh`/`Fast`/`Sbe`, default `Soh`, no runtime negotiation — Clarifications Q2). Both codecs
convert directly to/from the existing `truefix_core::Message`/`FieldMap`/`Group`/`Field` API — no
new intermediate-representation struct is introduced (see research.md Decision 1): that API is
already wire-format-agnostic (raw-byte-preserving `Field`, arbitrary `FieldMap::set`/`add_group`
construction, `members()` traversal), so "IR" (FR-006) is realized as a pair of conversion
functions (`decode_into_message`/`encode_from_message`) rather than a parallel data type. FAST
templates and SBE schemas are parsed by two new `truefix-dict` modules gated behind the existing
`dict-tooling` feature (reusing its already-vetted `quick-xml` dependency and content-sniffing
convention), producing `FastTemplateSet`/`SbeSchemaSet` models that a session can load a *set* of
(FR-012 — selection by inline template/schema ID, not one-template-per-session). `truefix-transport`
gains a `Protocol`-aware branch in `classify_buffered`'s framing/decode step; `truefix-session`/
`truefix-config` gain the `Protocol` field and its `.cfg` plumbing; the `truefix` facade re-exports
`truefix-binary` and wires it into `Engine::start_impl`. Decode failures, unknown template/schema
IDs, and Encoding Context resets emit structured `tracing`+`metrics` (FR-013), following this
codebase's existing `report_write_failure`/`metrics_export.rs` pattern.

## Technical Context

**Language/Version**: Rust edition 2024, workspace MSRV `1.96`, `unsafe_code = "forbid"`.

**Primary Dependencies**: New crate `truefix-binary` depends on `truefix-core` (`Message`,
`FieldMap`, `Group`, `Field` — all already-public types) and `truefix-dict` (new
`FastTemplateSet`/`SbeSchemaSet` parsed models, consumed as data, not by re-implementing XML
parsing). No new external dependency: FAST/SBE template/schema XML parsing reuses `truefix-dict`'s
existing `dict-tooling`-gated `quick-xml` (already Apache-2.0 OR MIT vetted); typed errors use
`thiserror`; observability uses `tracing`+`metrics` (all three already workspace dependencies).
`truefix-session`, `truefix-config`, `truefix-transport`, and the `truefix` facade crate are
modified, not newly added.

**Storage**: N/A. The FAST `Encoding Context` (per-template previous-value cache) is in-memory only
and resets on reconnect (spec Edge Cases / Clarifications) — no `truefix-store` changes.

**Testing**: `cargo test` per crate. Table-driven unit tests in `truefix-binary` (FAST
operator/PMap/stop-bit encode-decode, SBE flyweight fixed-offset/group/VarData encode-decode,
multi-template/schema selection by inline ID, malformed-input error paths) and in `truefix-dict`
(FAST template / SBE schema XML parsing, mirroring `orchestra.rs`'s existing test structure).
Process-level integration tests in `truefix-transport`/`truefix-session` covering: a `Protocol`
mismatch between two sessions failing Logon with a named typed error, and an end-to-end FAST and
SBE session round-trip — both exercised on **both** acceptor and initiator sessions (Constitution
Principle VI). **No new `truefix-at` scenarios are anticipated**: the AT suite ports QuickFIX/J's
SOH-only acceptance behavior, which has no FAST/SBE session concept to port from, and the default
`Protocol = Soh` path is unchanged — this must be confirmed as a zero-regression run of the
existing 483/483 AT suite, not by adding new AT scenarios.

**Target Platform**: Server-side Rust library/engine; existing workspace targets.

**Project Type**: Cargo workspace library/engine. One new crate (`truefix-binary`); five existing
crates modified (`truefix-dict`, `truefix-session`, `truefix-config`, `truefix-transport`, `truefix`
facade).

**Performance Goals**: No blocking numeric target (Clarifications Q3 — matches the project's
existing `cargo bench` convention of observation/regression, not a CI gate). The one concrete
non-timing target is SC-003 (SBE field reads produce zero heap allocation beyond the input buffer),
verified by an allocation-counting test, not a benchmark threshold.

**Constraints**: Critical-path codec/IR-conversion code MUST avoid panic/unwrap/expect/unreachable/
unchecked indexing (Constitution Principle I); malformed binary input MUST produce typed errors
(FR-009); decode failures, unknown template/schema IDs, and Encoding Context resets MUST emit
structured `tracing`+`metrics` (FR-013); `unsafe_code = forbid` applies to `truefix-binary` like
every other workspace crate, so SBE's "zero-copy flyweight" reads must be built on safe slice/
byte-order APIs, not raw pointer casts.

**Scale/Scope**: 3 user stories (P1 FAST codec, P2 SBE codec, P3 IR conversion + `Protocol`
integration); representative IR-conversion coverage limited to 3 message types (Clarifications Q3:
`NewOrderSingle`(D), `ExecutionReport`(8), a repeating-group market-data message).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Status |
|---|---|---|
| I. Production-Ready First | New public API (`BinaryCodec` trait, `FastTemplateSet`/`SbeSchemaSet`, `Protocol` config) MUST be documented; critical-path encode/decode stays panic-free with typed errors (FR-009); decode failures/context resets/unknown template IDs get structured `tracing`+`metrics` (FR-013), closing what would otherwise be a new observability gap. | PASS |
| II. Protocol Correctness First | FAST/SBE behavior is grounded in the FIX Trading Community FAST/SBE specifications (cited in spec.md), with byte-exact conformance against public reference vectors or spec-embedded examples (Clarifications Q1). This does not change AT-covered SOH session-layer semantics — `Protocol = Soh` (default) behavior is unchanged, confirmed by re-running the existing 483/483 AT suite with zero regressions; no new AT scenarios exist to port since QuickFIX/J has no FAST/SBE session concept. | PASS |
| III. License & Provenance | FAST-template/SBE-schema XML parsing reuses `truefix-dict`'s already-vetted `quick-xml` dependency — no new external dependency. Any vendored reference conformance fixture (OpenFAST test vectors / Real Logic SBE example schema) MUST first be verified Apache-2.0/MIT-compatible before being committed as a test fixture; if not, the acceptance criterion falls back to the FAST/SBE specification documents' own embedded examples (already decided in spec Clarifications Q1). This license check is an explicit Phase 0 research task, not assumed. | PASS (conditional on Phase 0 license verification, tracked in research.md) |
| IV. Dual-Track Data Dictionary | The existing SOH codegen/runtime-`DataDictionary` dual-track is untouched by this feature. `FastTemplateSet`/`SbeSchemaSet` are a structurally different model (operator/offset-driven, not SOH tag=value) parsed by `truefix-dict` as their own loader functions, not folded into the `.fixdict`-text-emitting `generate-dict --format` family (research.md Decision 2) — this feature does not add compile-time codegen for FAST/SBE messages (out of scope per Clarifications Q2's static-protocol framing), so literal "codegen MUST exist" does not apply to this new model; the single-normalized-source spirit is honored by every session loading the same parsed `FastTemplateSet`/`SbeSchemaSet` regardless of bundled-name vs. file-path config. | PASS (scope note, not a violation) |
| V. Test Discipline | Table-driven unit tests for the codec/parser layers; process-level integration tests for `Protocol` mismatch rejection and end-to-end FAST/SBE sessions, both acceptor and initiator. No new AT scenarios (see Testing above), confirmed in Phase 0 research. | PASS |
| VI. Acceptor/Initiator Parity | FR-010 requires both codec directions on both roles; integration tests explicitly cover both. | PASS |
| VII. Inventory-Based Completeness | Not an inventory/audit-driven feature (no `docs/todo/NEW-*` items); traceability is to `docs/roadmap.md` Phase 2 §2.1–2.4 stage goals (2a/2b/2c), cited in spec.md. | PASS (N/A framing, consistent with prior non-audit features e.g. 011) |

**Result**: No gate failures. Complexity Tracking is empty (the new-crate-plus-five-touched-crates
footprint is inherent to `Protocol` genuinely spanning config → transport → session → facade, not
avoidable complexity; `truefix-binary` as an independent crate is already the recorded decision
R-FUTURE-01 in `docs/roadmap.md`).

## Project Structure

### Documentation (this feature)

```text
specs/013-fast-sbe-codec/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md         # Phase 1 output
├── contracts/
│   ├── README.md
│   ├── binary-codec-api.md
│   ├── fast-template-sbe-schema-parsing.md
│   ├── protocol-config-and-transport.md
│   └── ir-conversion.md
├── checklists/
│   └── requirements.md
└── tasks.md              # created by /speckit-tasks, not by this command
```

### Source Code (repository root)

```text
crates/
├── truefix-binary/                # NEW crate
│   ├── Cargo.toml                 # deps: truefix-core, truefix-dict, thiserror, tracing, metrics
│   │                               # (all already-vetted workspace deps; no new external dependency)
│   ├── src/
│   │   ├── lib.rs                 # BinaryCodec trait (encode(&Message, TemplateId) -> Result<Vec<u8>, _>;
│   │   │                           # decode(&[u8]) -> Result<(Message, TemplateId), _>), shared by fast/sbe
│   │   ├── fast/
│   │   │   ├── mod.rs             # FastCodec: BinaryCodec impl
│   │   │   ├── presence_map.rs    # PMap bit read/write
│   │   │   ├── varint.rs          # stop-bit variable-length integer encode/decode
│   │   │   ├── context.rs         # EncodingContext: per-template-id previous-value cache,
│   │   │   │                       # in-memory, reset on reconnect (FR-013 emits tracing on reset)
│   │   │   └── error.rs           # FastCodecError (thiserror; Truncated/InvalidStopBit/
│   │   │                           # UnknownTemplateId/... — mirrors truefix_core::DecodeError shape)
│   │   ├── sbe/
│   │   │   ├── mod.rs             # SbeCodec: BinaryCodec impl
│   │   │   ├── flyweight.rs       # zero-copy buffer view (safe slice/byte-order ops, no raw pointers)
│   │   │   └── error.rs           # SbeCodecError (thiserror)
│   │   └── ir.rs                  # decode_into_message()/encode_from_message(): conversion functions
│   │                               # over truefix_core::Message/FieldMap/Group/Field (FR-006) —
│   │                               # no separate IR struct (research.md Decision 1)
│   └── tests/
│       ├── fast_roundtrip.rs
│       ├── fast_delta.rs
│       ├── fast_multi_template.rs # FR-012: selection by inline template ID, unknown-ID error
│       ├── sbe_roundtrip.rs
│       ├── sbe_multi_schema.rs    # FR-012: selection by templateId/schemaId, unknown-ID error
│       ├── ir_conversion.rs       # US3: NewOrderSingle(D)/ExecutionReport(8)/MD-with-repeating-group
│       └── observability.rs       # FR-013: tracing/metrics emission on decode failure & context reset
│
├── truefix-dict/                  # MODIFIED
│   ├── src/
│   │   ├── fast_template.rs       # NEW, dict-tooling-gated: FastTemplateSet model +
│   │   │                           # load_fast_templates_from_file (parallel to, not part of,
│   │   │                           # the orchestra/fix_repository/vendor_xml .fixdict-text family)
│   │   └── sbe_schema.rs          # NEW, dict-tooling-gated: SbeSchemaSet model +
│   │                               # load_sbe_schemas_from_file
│   └── tests/
│       ├── fast_template_parse.rs
│       └── sbe_schema_parse.rs
│
├── truefix-session/               # MODIFIED
│   └── src/
│       ├── config.rs              # + pub protocol: Protocol field on SessionConfig (default Soh);
│       │                           # + enum Protocol { Soh, Fast, Sbe } (mirrors TimeStampPrecision)
│       └── state.rs               # on_logon: optional defense-in-depth Protocol-mismatch guard,
│                                   # secondary to transport's decode-stage enforcement (research.md
│                                   # Decision 4)
│
├── truefix-config/                # MODIFIED
│   └── src/
│       ├── keys.rs                 # + "Protocol" (and FAST-template-set/SBE-schema-set path) keys
│       │                           # registered in APPENDIX_A_KEYS
│       └── builder.rs              # + protocol_key() parse helper (mirrors precision_key);
│                                   # + FAST template set / SBE schema set loading (mirrors
│                                   # resolve_validator/load_dictionary_value's bundled-name-then-
│                                   # filesystem-fallback pattern)
│
├── truefix-transport/             # MODIFIED
│   └── src/lib.rs                  # classify_buffered: branch on session Protocol for framing+decode
│                                   # (FAST/SBE via truefix-binary); new ProtocolMismatch-shaped
│                                   # error path for the "wrong protocol, undecodable bytes" case
│                                   # (research.md Decision 4) — replaces the SOH-only assumption in
│                                   # today's unconditional decode/decode_with_groups call
│
└── truefix/                       # MODIFIED (facade)
    ├── Cargo.toml                  # + truefix-binary dependency
    └── src/lib.rs                  # + `pub use truefix_binary as binary;`;
                                    # Engine::start_impl consults resolved Protocol to select
                                    # SOH vs. FAST vs. SBE wiring per session

Cargo.toml (workspace root)         # + truefix-binary = { path = "crates/truefix-binary" } entry in
                                    # [workspace.dependencies], alongside the other 7 truefix-* crates
```

**Structure Decision**: One new crate (`truefix-binary`) following this repository's established
"new protocol-layer concern → new crate" pattern (matches `truefix-dict`/`truefix-store`/`truefix-log`
being independent crates); FAST-template/SBE-schema XML parsing is added to the *existing*
`truefix-dict` crate rather than duplicated in the new crate, reusing its already-vetted
`dict-tooling` feature gate. `Protocol` plumbing touches exactly the crates that already own each
layer of session config → wire decode → facade wiring, matching this repository's established
pattern (e.g. feature 011's plan, which touched `truefix-core`/`truefix-dict`/`truefix-transport`/
`truefix-session` for a comparable cross-cutting change).

## Phase 0: Research Summary

See [research.md](./research.md). Key resolved decisions:

- **No separate IR struct**: `truefix_core::Message`/`FieldMap`/`Group`/`Field`'s existing public
  API (raw-byte-preserving fields, arbitrary field/group construction, full-structure `members()`
  traversal) is already wire-format-agnostic, so FR-006's "intermediate representation" is realized
  as conversion functions in `truefix-binary::ir`, not a new parallel data type.
- FAST-template/SBE-schema XML parsing lives in `truefix-dict` (new `fast_template.rs`/
  `sbe_schema.rs` modules, gated behind the existing `dict-tooling` feature) as a **parallel**
  loader family, not folded into the `.fixdict`-text-emitting `orchestra`/`fix_repository`/
  `vendor_xml` converters — those two model shapes are not translatable into `.fixdict` text
  without losing FAST's operator semantics or SBE's fixed-offset/group layout.
- `Protocol`'s natural home is a new `SessionConfig` field (mirrors the existing `TimeStampPrecision`
  enum-config-field pattern) plus a new `.cfg` key and `builder.rs` parse helper; the session-level
  file-loading of FAST template sets / SBE schema sets mirrors `resolve_validator`'s existing
  bundled-name-then-filesystem-fallback pattern for `DataDictionary`.
- A genuine byte-level `Protocol` mismatch (e.g. an SOH peer talking to a `Protocol = Fast`-
  configured session) fails at `truefix-transport`'s framing/decode step *before* a `Message` ever
  reaches `truefix-session::state.rs::on_logon` — so FR-007's "Logon MUST fail with a named typed
  error" is primarily enforced by a new transport-layer decode error variant, with an optional
  defense-in-depth `on_logon` guard as a secondary layer, not the primary one.
- Typed-error and observability conventions to follow: `truefix_core::DecodeError`'s per-variant
  structured-context shape for binary decode errors; `truefix_dict::OrchestraError`'s XML-parse-
  error shape for template/schema parsing errors; `truefix-log`'s `report_write_failure` (paired
  `tracing::error!` + `metrics::counter!`) and `truefix-transport`'s `metrics_export.rs` (session-
  labelled counters/gauges) for FR-013's observability requirement.
- No new AT (`truefix-at`) scenarios are needed: confirmed the AT suite's scope is QuickFIX/J-ported
  SOH session behavior, which has no FAST/SBE concept, and the default `Protocol = Soh` path is
  unchanged by this feature.
- Open item carried forward, not yet resolved by research alone: the actual license text of the
  candidate OpenFAST test-vector source and Real Logic `simple-binary-encoding` example schemas
  must be checked for Apache-2.0/MIT compatibility before either is vendored as a test fixture
  (Constitution Principle III); until confirmed, tasks MUST default to the spec's embedded-example
  fallback (Clarifications Q1).

## Phase 1: Design Summary

See [data-model.md](./data-model.md) for the affected entities/types and [contracts/](./contracts/)
for the per-crate API contracts. The design is additive at nearly every touched public API surface:
`SessionConfig` gains one new field with a default that preserves today's behavior exactly; `.cfg`
gains one new (optional) key; `truefix-dict` gains two new modules with no change to existing
converters; `truefix-transport`'s `classify_buffered` gains a new branch selected by the (default-
`Soh`) `Protocol` field, so the existing SOH-only call path is unchanged when `Protocol = Soh`. The
only non-additive change is internal: `classify_buffered`'s framing/decode branch grows a match arm,
not a signature change.

## Post-Design Constitution Check

| Principle | Re-check | Status |
|---|---|---|
| I | `contracts/binary-codec-api.md` and `contracts/ir-conversion.md` require typed errors on every fallible path and specify the exact `tracing`+`metrics` shape for FR-013 (decode failure, unknown template/schema ID, context reset). | PASS |
| II | `contracts/binary-codec-api.md` cites the FIX Trading Community FAST/SBE spec sections each encode/decode rule implements; `data-model.md` grounds `EncodingContext`/`PresenceMap`/flyweight layout in those specs, not in QuickFIX/J-observed behavior. | PASS |
| III | `contracts/fast-template-sbe-schema-parsing.md` restates the license-verification gate for any vendored reference fixture as an explicit contract clause, with the spec-embedded-example fallback as the default until verified. | PASS (conditional, tracked) |
| IV | `contracts/fast-template-sbe-schema-parsing.md` keeps `FastTemplateSet`/`SbeSchemaSet` as `truefix-dict`-owned parsed models consumed identically by `truefix-binary` regardless of load path, with no change to the existing SOH codegen/`DataDictionary` dual-track. | PASS |
| V | `quickstart.md` enumerates the unit/integration test slices per user story; confirms no new AT scenarios (research.md). | PASS |
| VI | `contracts/protocol-config-and-transport.md` requires `Protocol` mismatch rejection and FAST/SBE round-trip tests on both acceptor and initiator sessions. | PASS |
| VII | N/A — no inventory linkage for this feature (roadmap-driven, not an audit pass), consistent with the pre-design check. | PASS |

## Complexity Tracking

No constitution violations require justification.
