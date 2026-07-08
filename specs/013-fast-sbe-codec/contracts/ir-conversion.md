# Contract: `truefix-binary::ir` — `Message` Conversion

Traces: spec.md FR-006, FR-011; Clarifications Q3; research.md Decision 1.

## API

```rust
// crates/truefix-binary/src/ir.rs — conceptual signatures (concrete generic bounds are a task-level detail)
pub fn decode_into_message<C: BinaryCodec>(codec: &C, bytes: &[u8]) -> Result<truefix_core::Message, C::Error>;
pub fn encode_from_message<C: BinaryCodec>(codec: &C, message: &truefix_core::Message, template_id: u32) -> Result<Vec<u8>, C::Error>;
```

## Behavior Contract

1. `decode_into_message` builds the resulting `Message`'s `header`/`body`/`trailer` `FieldMap`s
   using only `Field::new`/typed constructors and `FieldMap::set`/`add_group` — the same public API
   any other `Message` producer uses. The result is indistinguishable (by field content and group
   structure) from an equivalent message decoded from SOH text, for any Application code reading it
   (spec US3 Scenario 1).
2. `encode_from_message` reads the source `Message` via `FieldMap::members()` (not `.fields()`, which
   would silently skip repeating groups) so that every `Group` entry is visible to the FAST/SBE
   encoder (spec US3 Scenario 2 — the requirement that outbound bytes are genuinely FAST/SBE-encoded,
   not a passthrough).
3. For the representative message subset (Clarifications Q3: `NewOrderSingle`(D), `ExecutionReport`
   (8), a repeating-group market-data message with at least one nested group), round-tripping
   through `decode_into_message` then `encode_from_message` (or the reverse order) preserves the
   full field set and group structure with zero information loss (spec US3 Scenario 4; SC-004).
4. If a source `Message` contains a construct with no representation in the target FAST template /
   SBE schema (e.g. a field the template/schema doesn't declare), `encode_from_message` MUST return
   a named typed error identifying the unsupported construct — MUST NOT silently drop the field or
   produce a binary message missing that data (spec US3 Scenario 5).
5. Neither function panics, `unwrap`s, or `expect`s on malformed input or an unrepresentable
   construct (Constitution Principle I) — every failure mode above is a typed `Result::Err`.

## Scope Note

Per Clarifications Q3, this contract's conformance testing (and therefore `/speckit-tasks`' test
tasks) is scoped to the three representative message types listed above. Extending IR coverage to
the full FIX44 dictionary (or other versions/vendor dictionaries) is explicitly out of scope for
this feature and MUST NOT be silently expanded during implementation — additional message-type
coverage is a separate, future feature.
