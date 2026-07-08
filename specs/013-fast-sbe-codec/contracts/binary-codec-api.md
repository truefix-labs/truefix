# Contract: `truefix-binary` — `BinaryCodec`, `FastCodec`, `SbeCodec`

Traces: spec.md FR-001–FR-005, FR-009, FR-010, FR-012, FR-013; research.md Decisions 1, 5.

## API

```rust
pub trait BinaryCodec {
    type Error: std::error::Error;
    fn encode(&self, message: &truefix_core::Message, template_id: u32) -> Result<Vec<u8>, Self::Error>;
    fn decode(&self, bytes: &[u8]) -> Result<(truefix_core::Message, u32), Self::Error>;
}

pub struct FastCodec { /* see data-model.md */ }
pub struct SbeCodec { /* see data-model.md */ }

pub enum FastCodecError {
    Truncated { offset: usize },
    InvalidStopBit { offset: usize },
    PresenceMapMismatch { field: u32, offset: usize },
    UnknownTemplateId { id: u32 },
}

pub enum SbeCodecError {
    Truncated { offset: usize },
    BlockLengthMismatch { declared: u32, actual: u32 },
    UnknownTemplateId { id: u32 },
    VarDataLengthMismatch { field: String, declared: u32, actual: u32 },
}
```

## Behavior Contract — FAST (`FastCodec`)

1. Given a `FastTemplateSet` containing a template that declares `copy`/`increment`/`delta`/
   `default` operators, `FastCodec::decode` on a matching byte-accurate FAST-encoded message
   produces a `Message` whose fields match the template's declared field values exactly, including
   values recovered via `copy`/`delta`/`increment` from `EncodingContext` rather than present on the
   wire (spec US1 Acceptance Scenario 2).
2. Across consecutive messages under the same template ID and the same `FastCodec` instance
   (same `EncodingContext`), a field using `copy` (unchanged) or `increment` (fixed delta) is
   correctly recovered when the presence map indicates it was omitted (US1 Scenario 3).
3. A nullable field (presence-map bit `0`, no operator) decodes to an explicit null representation
   distinguishable from "field absent because dictionary doesn't declare it" (US1 Scenario 4) — MUST
   NOT be represented the same way `truefix_core::FieldMap::contains` represents "tag not present."
4. `FastCodec::encode` on a `Message` decoded by this same codec/template MUST reproduce the
   original byte sequence exactly (round-trip, US1 Scenario 5) against the reference encoding
   selected by Clarifications Q1 (contracts/fast-template-sbe-schema-parsing.md governs which
   source that is).
5. A truncated byte stream or one with an invalid stop-bit sequence MUST return
   `FastCodecError::Truncated`/`InvalidStopBit` (with `offset`), never panic, `unwrap`, or `expect`
   (Constitution Principle I; US1 Scenario 6).
6. A message whose inline template ID is not present in this `FastCodec`'s loaded `FastTemplateSet`
   MUST return `FastCodecError::UnknownTemplateId` (FR-012; US1 Scenario 7) — MUST NOT fall back to
   decoding against an arbitrary loaded template.
7. An `EncodingContext` reset (new connection after a disconnect) MUST emit the FR-013 structured
   `tracing`/`metrics` event (see protocol-config-and-transport.md) before the next message under
   that template ID is decoded/encoded.

## Behavior Contract — SBE (`SbeCodec`)

1. Given a `SbeSchemaSet` declaring fixed fields, one repeating group (block length + num groups),
   and one `VarData` field, `SbeCodec::decode` on a matching byte-accurate SBE-encoded message
   correctly reads every fixed field, every group entry (including nested groups/multiple `VarData`
   fields), and the `VarData` payload, in schema-declared order (US2 Scenarios 1–3).
2. Reading a decoded message's fields MUST NOT copy or reallocate the message payload itself — the
   flyweight view borrows the input `&[u8]` for the lifetime of the read (SC-003; US2 Scenario 2).
   This MUST be implemented with safe slice/byte-order operations only (`unsafe_code = "forbid"`
   applies to `truefix-binary` like every other workspace crate).
3. `SbeCodec::encode` on a `Message` decoded by this same codec/schema MUST reproduce the original
   byte sequence exactly against the reference encoding selected by Clarifications Q1 (US2
   Scenario 4).
4. A message whose declared `blockLength`/`numGroups`/`VarData` length is inconsistent with the
   actual buffer content MUST return a typed error (`Truncated`/`BlockLengthMismatch`/
   `VarDataLengthMismatch`) before any out-of-bounds read is attempted — never read past the
   provided buffer (US2 Scenario 5).
5. A message whose `templateId`/`schemaId` is not present in this `SbeCodec`'s loaded
   `SbeSchemaSet` MUST return `SbeCodecError::UnknownTemplateId` (FR-012; US2 Scenario 6) — MUST NOT
   fall back to decoding against an arbitrary loaded schema.

## Acceptor/Initiator Parity (Constitution Principle VI, FR-010)

Every behavior above MUST be exercised by tests constructing both an acceptor-role and an
initiator-role session/codec instance — the codec API itself has no role concept (it operates on
bytes/templates, not connection roles), so parity is a *test-coverage* requirement, not an API
requirement: both roles must be able to call `encode` and `decode` with identical, symmetric
results for the same template/schema and message content.
