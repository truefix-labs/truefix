# Contracts: FAST/SBE Binary Protocol Codec

This directory defines the behavior contracts that `/speckit-tasks` should convert into tests and
implementation tasks.

- [binary-codec-api.md](./binary-codec-api.md): the new `truefix-binary` crate's `BinaryCodec`
  trait, `FastCodec`/`SbeCodec`, `EncodingContext`, and their error types.
- [fast-template-sbe-schema-parsing.md](./fast-template-sbe-schema-parsing.md): the new
  `truefix-dict` modules (`fast_template.rs`/`sbe_schema.rs`), their `dict-tooling` gating, and the
  reference-fixture licensing gate.
- [protocol-config-and-transport.md](./protocol-config-and-transport.md): the new `Protocol`
  session-config option, `.cfg` plumbing, `classify_buffered`'s protocol-aware branch, and the
  Logon protocol-mismatch error path.
- [ir-conversion.md](./ir-conversion.md): `truefix-binary::ir`'s `Message` conversion functions and
  the representative message-type coverage (Clarifications Q3).

## AT Scenario Guidance

No new AT scenarios are required (research.md Decision 6): this feature adds a new, independently
specified binary-protocol conformance mechanism (spec.md SC-001/SC-002 against FAST/SBE reference
encodings), not new QuickFIX/J-ported session-layer behavior. Existing AT coverage (483/483
scenarios) MUST continue to pass unchanged with the default `Protocol = Soh` — this is a
zero-regression check, not a new-scenario-authoring task.

## Public API Compatibility Placeholders

- `truefix-core`: **no changes**. `Message`/`FieldMap`/`Group`/`Field` are consumed as-is by
  `truefix-binary` (research.md Decision 1); nothing in this feature requires a new public method
  or a signature change here. Confirm during implementation and note here if a gap is found.
- `truefix-dict`: `fast_template`/`sbe_schema` modules, `FastTemplateSet`/`SbeSchemaSet`, and their
  error types are wholly new, additive public API (behind `dict-tooling`). No existing `truefix-dict`
  public API (`load_from_file`, `generate-dict --format`, `DataDictionary`) changes shape.
- `truefix-session`: `SessionConfig` gains one new field (`protocol: Protocol`, default `Soh`) and
  one new public enum (`Protocol`) — additive, and the default preserves every existing caller's
  behavior exactly.
- `truefix-config`: one new `.cfg` key (`"Protocol"`) registered in `APPENDIX_A_KEYS`; additive.
- `truefix-transport`: no public API signature changes anticipated — `classify_buffered` is a
  crate-internal function; its new branch is an internal implementation detail. Confirm during
  implementation and note here if a public surface change turns out to be needed (e.g. a new public
  error variant on whatever error type `read_loop`/session-establishment already surfaces to
  callers).
- `truefix` (facade): new `pub use truefix_binary as binary;` re-export — additive.
