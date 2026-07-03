# Contract: Dictionary/Codec Correctness (US7)

**Covers**: FR-030 through FR-034. **Source**: `docs/todo/003.md` `BUG-12`, `GAP-27`, `GAP-26`,
`B22`, `GAP-56`. **Grounding**: research.md §R7.

**Files**: `crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/codegen.rs`,
`crates/truefix-transport/src/lib.rs`, `crates/truefix-dict/src/validate.rs`,
`crates/truefix-core/src/tags.rs`, `crates/truefix-dict/dict-src/normalized/*.fixdict`.

## Behavioral contract changes

| # | Change | Contract |
|---|---|---|
| 1 | `FieldType::value_ok` | `UtcTimeOnly`/`UtcDate` gain real format checks (via existing `Field::as_utc_time_only`/`as_utc_date_only` parsers) — malformed values now fail dictionary validation instead of unconditionally passing. `UtcDateOnly` deliberately left unchanged (matches QFJ's own unchecked behavior). |
| 2 | Codegen's generated `encode()` | For messages declaring a custom `fieldOrder`, calls `encode_with_order` with a codegen-emitted order constant instead of the plain order-agnostic `encode()` — the declared order now actually reaches the wire |
| 3 | Production decode path (`truefix-transport`) | Switches from flat `decode()` to `decode_with_groups`, using the session's `DataDictionary` as `GroupSpec`. `tags::is_header`/`is_trailer` gain `NoHops`/`HopCompID`/`HopSendingTime`/`HopRefID`. `validate_groups` gains visibility into group content (currently filtered out by `.fields()`). |
| 4 | `data_field_for_length` | Gains `EncodedLeg*` pairs (618→619, 620→621, 445→446, 1039→1040) — embedded SOH bytes in these fields' content no longer cause incorrect framing |
| 5 | Shipped `.fixdict` provenance headers | Text updated to name the current regenerating tool (`fix_repository.rs`/`truefix-dict-cli`) instead of the deleted `qfj_xml.rs` — **content unchanged**, header-comment-only edit |

## No breaking changes

No public type shape change. `encode_with_order`/`decode_with_groups`/`GroupSpec` already exist
(feature 005) — this feature wires already-correct primitives into the real production path for the
first time; no new primitive is introduced. `FieldType`/`FieldDef` enums unchanged (no new variant).

## Constitution Principle IV (Dual-Track Data Dictionary)

Row 2 and row 3 are precisely dual-track-integrity fixes: today codegen's `encode()` and the
production decode path silently diverge from the runtime `DataDictionary`'s own capabilities. This
feature closes that divergence — the codegen (build-time) and runtime-`DataDictionary` tracks now
actually share behavior for `fieldOrder` and header/trailer groups, as Principle IV already requires.

## Acceptor/Initiator parity

All 5 changes are role-agnostic — dictionary validation, codec encode/decode, and shipped dictionary
content apply identically regardless of session role.
