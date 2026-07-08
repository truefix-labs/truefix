# Phase 0 Research: FAST/SBE Binary Protocol Codec

Each section resolves one open technical question from `plan.md`'s Technical Context, grounded in
the actual current code (file paths and line references as of this feature's branch point), not in
`docs/roadmap.md`'s proposed-but-unverified module layout.

## Decision 1: No separate IR struct — `truefix_core::Message`/`FieldMap`/`Group`/`Field` *is* the IR

**Decision**: FR-006's "intermediate representation" is realized as two conversion functions in
`truefix-binary::ir` — `decode_into_message(&FastTemplateSet | &SbeSchemaSet, &[u8]) -> Result<(Message, TemplateId), _>`
and `encode_from_message(&FastTemplateSet | &SbeSchemaSet, &Message, TemplateId) -> Result<Vec<u8>, _>`
— operating directly on `truefix_core::Message`'s existing public API. No new `Ir`/`BinaryMessage`
struct is introduced.

**Rationale**: Inspecting `crates/truefix-core/src/{message,field_map,field,group}.rs` shows the
existing API is already wire-format-agnostic:
- `Field` (`field.rs:13`) stores raw bytes verbatim and exposes typed constructors/accessors
  (`Field::new/string/int/bytes/utc_timestamp/...`, `as_str/as_int/as_decimal/...`), all returning
  `Result<_, FieldError>` — this is exactly what a FAST/SBE decoder needs to hand back a value
  without knowing anything about SOH.
- `FieldMap` (`field_map.rs:25`) supports arbitrary construction (`add_field`, `set`, `add_group`)
  and full-structure traversal (`members()`, yielding `MemberRef::Field`/`MemberRef::Group` — the
  accessor that sees the *entire* wire structure, unlike `fields()` which skips groups) — everything
  an encoder needs to walk an arbitrary decoded message back into bytes.
- `Group` (`group.rs:19`) is a builder (`Group::new(count_tag).add_entry(FieldMap)...`) consumed via
  `into_parts()` — exactly the shape a FAST/SBE repeating-group decoder needs to produce.
- `Message` (`message.rs:13`) is just three public `FieldMap`s (`header`/`body`/`trailer`).

Introducing a separate `BinaryMessage`/IR struct (as `docs/roadmap.md` §2.4's proposed `ir.rs`
loosely suggested before this research) would mean building a second generic field/group container
with the same capabilities as `FieldMap`, then converting *that* into `Message` as a second step —
a redundant type with no additional capability, and a second place round-trip fidelity could be
lost. Building directly into `Message` is both less code and structurally safer.

**Alternatives considered**:
- A dedicated `BinaryIr` struct mirroring `FieldMap` — rejected: duplicates `FieldMap`'s role,
  adds a lossy extra conversion hop, and existing `Field`'s raw-byte-preserving round-trip design
  (see `field.rs` doc comment) already delivers the property such a struct would exist to provide.
- Adding a new `Codec`/`Encoding` trait to `truefix-core` for pluggable wire formats — rejected for
  this feature: there is no such trait today (`encode`/`decode`/`decode_with_groups` in
  `codec/{encode,decode}.rs` are concrete SOH-only free functions); the one existing "core defines a
  narrow trait, a downstream crate implements it" precedent is `group::GroupSpec` (`group.rs:12`,
  implemented by `truefix-dict::DataDictionary`), but that trait's job (advertise group definitions
  during *SOH* decode) doesn't generalize to "select an entire wire format." Since `Protocol` is a
  static per-session choice (Clarifications Q2), `truefix-binary` can simply provide free
  functions/a crate-local trait without touching `truefix-core`'s public surface at all.

## Decision 2: FAST templates / SBE schemas parsed in `truefix-dict`, not `truefix-binary`, and not folded into the `.fixdict`-text converter family

**Decision**: Two new `truefix-dict` modules, `fast_template.rs` and `sbe_schema.rs`, both gated
behind the existing `dict-tooling` feature, provide `FastTemplateSet`/`SbeSchemaSet` model types
and `load_fast_templates_from_file`/`load_sbe_schemas_from_file` functions. These are **not** wired
into `generate-dict`'s `--format` family (`orchestra`/`fix-repository`/`vendor-xml`, all of which
convert vendor XML into normalized `.fixdict` text) and **not** wired into `load_from_file`'s
content-sniffing dispatch (which returns a `DataDictionary`).

**Rationale**: `crates/truefix-dict/Cargo.toml` already gates `quick-xml` behind `dict-tooling`
(`quick-xml = { workspace = true, optional = true }`, `dict-tooling = ["dep:quick-xml"]`) explicitly
to keep XML parsing out of the runtime engine's default dependency graph — build/tooling-time only.
Reusing this exact gate for FAST/SBE avoids a second, duplicate feature-gating mechanism in a second
crate for the same class of concern (Constitution Principle III's dependency-minimization stance).
However, `FastTemplateSet`/`SbeSchemaSet` are **not** SOH message dictionaries — `.fixdict` text has
no representation for a FAST field's operator (copy/increment/delta/default) or an SBE field's fixed
byte offset/byte order/block length, so folding them into the existing `--format`/`load_from_file`
paths (both of which produce or consume `.fixdict`-shaped data) would be a lossy, awkward fit, not a
genuine third `--format` option. They get their own loader functions returning their own model
types instead, following the same per-converter pattern (own `thiserror` error enum, e.g.
`orchestra::OrchestraError`'s `Xml { pos, reason }`/`MissingAttr`/`UnknownType`/`UnknownRef`/
`NotANumber` shape) without pretending to be a fourth `.fixdict`-text converter.

**Alternatives considered**:
- Parsing FAST/SBE XML directly in `truefix-binary` with its own `quick-xml` feature gate —
  rejected: duplicates dependency-gating machinery `truefix-dict` already solved, and splits "XML
  dictionary/schema parsing" concerns across two crates for no benefit.
- Folding FAST/SBE into the `generate-dict --format` converter family, emitting `.fixdict` text —
  rejected: `.fixdict` text has no field for FAST's per-field operator or SBE's fixed offset/byte
  order; forcing the fit would either lose information or require inventing new `.fixdict` syntax
  that every other consumer of that format would need to understand.

## Decision 3: `Protocol` config — new `SessionConfig` field, new `.cfg` key, mirrors `TimeStampPrecision`

**Decision**: Add `pub protocol: Protocol` to `truefix-session::SessionConfig` (`config.rs`,
default `Protocol::Soh`), a new `enum Protocol { Soh, Fast, Sbe }`, a new `"Protocol"` `.cfg` key
registered in `truefix-config::keys::APPENDIX_A_KEYS`, and a `protocol_key()` parse helper in
`truefix-config::builder::resolve_one` mirroring the existing `precision_key` helper
(case-insensitive enum-string matching, `builder.rs` ~line 483-498) used for `TimeStampPrecision`.
Associated FAST-template-set / SBE-schema-set file paths are resolved the same way
`resolve_validator`/`load_dictionary_value` already resolve a `DataDictionary` path (`builder.rs`
~line 677-783): try a bundled-name lookup first, fall back to filesystem loading via the new
`truefix-dict` loader functions from Decision 2.

**Rationale**: `TimeStampPrecision` (`config.rs:161`, `Seconds | Milliseconds | Microseconds |
Nanoseconds`) is the closest existing precedent for a small enum session-config field parsed from a
`.cfg` string — reusing its exact shape (struct field + enum + `builder.rs` parse-helper + `keys.rs`
registration) keeps `Protocol` consistent with how every other typed config option in this codebase
is added, rather than inventing a new configuration mechanism. `APPENDIX_A_KEYS` requires every
recognized key to be registered (`Implemented`/`Recognized`/`Unsupported`), so `"Protocol"` must
appear there for the config loader to accept it at all.

**Alternatives considered**: A separate `[binary]`/`[protocol]` config section outside
`SessionConfig` — rejected: every other per-session option (including comparable small-enum options
like `TimeStampPrecision`) lives directly on `SessionConfig`, resolved through the same
`resolve_one`/`APPENDIX_A_KEYS` machinery; a parallel mechanism would fork config resolution for no
benefit and complicate `Engine::start_impl`'s per-session wiring.

## Decision 4: `Protocol` mismatch at Logon is primarily a transport-layer decode error, not (only) an `on_logon` guard

**Decision**: `truefix-transport`'s `classify_buffered` (`lib.rs:1266-1364`) gains a `Protocol`-aware
branch: for `Protocol = Fast`/`Sbe` sessions, framing and decoding go through `truefix-binary`
instead of `truefix_core::framing::frame_length`/`decode`/`decode_with_groups`. A new error variant
(e.g. `ProtocolMismatch` or an equivalent FAST/SBE decode-error class distinguished from ordinary
truncated/malformed input) is the **primary** enforcement point for FR-007's "Logon MUST fail with a
named typed error." A secondary, defense-in-depth guard MAY also be added to
`truefix-session::state.rs::on_logon` (following its existing sequential `reject_logon(&Reject{...})`
guard pattern — see the already-implemented guards for out-of-sequence Logon, bad `HeartBtInt`,
etc., starting at `state.rs:1554`), but it is not the primary layer.

**Rationale**: `classify_buffered` (`lib.rs:1305-1317`) unconditionally frames and decodes every
inbound byte stream as SOH today; there is no existing `Protocol` concept anywhere in
`truefix-transport`. Two sessions configured with different `Protocol`s produce byte streams that
are fundamentally undecodable under each other's grammar (SOH tag=value text vs. FAST's
presence-map/stop-bit binary vs. SBE's fixed-offset binary) — this mismatch surfaces at the framing/
decode step, *before* any `Message` (let alone a recognizable Logon) could ever reach
`truefix-session`'s state machine. `on_logon`'s existing guards all operate on an already-
successfully-decoded Logon message; a byte-level protocol mismatch never produces one. The
project's existing "malformed first message" handling precedent (`lib.rs:2037-2081`'s
`route_and_run`: "a non-Logon first message... closes the socket immediately") is the model this
follows — decode failure at the transport layer is already treated as connection-fatal, and a
`Protocol`-mismatch decode failure is just a specific, clearly-labeled instance of that.

**Alternatives considered**: Enforcing the check purely as an `on_logon` state-machine guard (i.e.,
attempt to decode under the peer's assumed protocol, only reject after a `Message` is constructed)
— rejected as the primary mechanism: it's not reachable, because a genuine cross-protocol byte
stream cannot be decoded into a `Message` in the first place under this design; a guard here would
only ever fire in the vanishingly unlikely case that foreign bytes coincidentally happen to satisfy
the wrong protocol's decode grammar. Kept as an optional secondary layer for defense-in-depth /
symmetry with existing `on_logon` guards, not relied upon as the sole enforcement point.

## Decision 5: Typed-error and observability shapes to reuse

**Decision**: `truefix-binary`'s decode error enums (`FastCodecError`, `SbeCodecError`) follow
`truefix_core::error::DecodeError`'s per-variant structured-context shape (`error.rs:74-153`, e.g.
`GarbledField { offset, reason }`) — concretely: `Truncated { offset }`, `InvalidStopBit { offset }`,
`PresenceMapMismatch { .. }`, `UnknownTemplateId { id }` for FAST; `Truncated { offset }`,
`BlockLengthMismatch { .. }`, `UnknownTemplateId { id }`, `VarDataLengthMismatch { .. }` for SBE.
`truefix-dict`'s new `FastTemplateError`/`SbeSchemaError` (XML parse errors) follow
`orchestra::OrchestraError`'s shape (`Xml { pos, reason }`, `MissingAttr { elem, attr }`,
`UnknownType { id, type_name }`, `UnknownRef { kind, id }`, `NotANumber { elem, attr, value }`).
FR-013's observability requirement is met by combining `truefix-log`'s `report_write_failure`
pattern (`crates/truefix-log/src/file.rs:242-247`: paired `tracing::error!(target: "...", ...)` +
`metrics::counter!("...").increment(1)`) with `truefix-transport::metrics_export.rs`'s session-
labelled counter/gauge convention (`metrics::counter!("truefix_reconnects_total", "session" =>
session_label(id))`) — concretely, decode failures and Encoding Context resets emit
`tracing::error!`/`warn!(target: "truefix_binary", session = %id, template_id = ..., error = %e,
"...")` paired with a `metrics::counter!("truefix_binary_decode_failures_total", "session" => ...)`-
style counter, called unconditionally (the existing convention notes this is "effectively free" with
no recorder installed, per `metrics_export.rs`'s doc comment).

**Rationale**: Every existing error type in this codebase (`DecodeError`, `DictLoadError`,
`OrchestraError`, `VendorXmlError`) is a `thiserror`-derived enum with structured, matchable
variants — reusing that exact shape keeps `truefix-binary`'s errors consistent with the rest of the
workspace's error-handling discipline (Constitution Principle I) rather than inventing a new error
style. Likewise, `report_write_failure` and `metrics_export.rs` are the two concrete precedents in
this codebase for "pair a tracing event with a metrics counter on a failure/notable-event path" —
following them exactly (rather than, say, tracing-only or metrics-only) keeps FR-013 consistent with
how the rest of the engine already surfaces comparable events (write failures, reconnects).

**Alternatives considered**: `Box<dyn Error>` or a single flat error enum shared across FAST and SBE
— rejected: Constitution's "错误模型" constraint explicitly forbids `Box<dyn Error>` masking
critical-path recoverable errors, and FAST/SBE have genuinely different failure modes (stop-bit vs.
block-length/VarData-length) that read better as separate, per-codec enums, matching how
`truefix-dict` already keeps `OrchestraError`/`VendorXmlError` separate per converter rather than
one shared "dictionary error" enum.

## Decision 6: No new `truefix-at` scenarios

**Decision**: This feature adds zero new scenarios to the `truefix-at` acceptance-test suite. The
existing 483/483-scenario suite is re-run as a **zero-regression check** (default `Protocol = Soh`
must produce byte-identical behavior to today).

**Rationale**: The AT suite (Constitution Principle V) ports QuickFIX/J's acceptance behavior;
QuickFIX/J has no FAST/SBE session concept, so there is nothing to port for this feature — its
"protocol correctness" evidence instead comes from conformance against the FIX Trading Community
FAST/SBE specifications and reference encodings (spec.md SC-001/SC-002), which is a different,
already-specified acceptance mechanism. This mirrors feature 011's precedent (`plan.md`: "no
protocol-visible session-lifecycle behavior changes, so no new AT scenarios are anticipated").

**Alternatives considered**: None — this is a factual scoping conclusion (QuickFIX/J's AT corpus
has no binary-protocol scenarios to port), not a judgment call with real alternatives.

## Open item carried into Phase 1 / tasks: reference-fixture licensing

Clarifications Q1 (spec.md) already decided the *policy* (use public reference fixtures if
Apache-2.0/MIT-compatible, else fall back to spec-embedded examples) but the actual license text of
the candidate sources — OpenFAST's test-vector corpus and Real Logic `simple-binary-encoding`'s
example schemas — has **not** been checked as part of this research pass (it requires inspecting
each upstream project's LICENSE file/repository, which is an implementation-time task, not a
design-time one). This is carried forward as an explicit early task (see tasks.md, not produced by
this command) rather than assumed resolved: whichever path SC-001/SC-002's tests actually take
depends on that check's outcome.

### Implementation-time license resolution (2026-07-08)

- FAST: OpenFAST is listed in the public FAST implementation inventory as Mozilla Public License,
  which is not compatible with this repository's Apache-2.0 OR MIT fixture-provenance gate. FAST
  fixtures for this feature therefore use the Clarifications Q1 fallback: independently-authored
  examples based on the FAST specification's embedded examples rather than vendored OpenFAST
  corpus files.
- SBE: Real Logic / Aeron `simple-binary-encoding` advertises Apache-2.0 licensing in its GitHub
  repository README and license metadata. Example schemas from that repository are compatible with
  this repository's fixture-provenance gate when source path and commit/tag are recorded beside the
  fixture.
