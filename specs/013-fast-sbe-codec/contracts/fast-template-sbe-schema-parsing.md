# Contract: `truefix-dict` — FAST Template / SBE Schema Parsing

Traces: spec.md FR-001, FR-004, FR-008; Clarifications Q1; research.md Decision 2.

## API

```rust
// crates/truefix-dict/src/fast_template.rs (dict-tooling gated)
pub fn load_fast_templates_from_file(path: &Path) -> Result<FastTemplateSet, FastTemplateError>;
pub fn parse_fast_templates(xml: &str) -> Result<FastTemplateSet, FastTemplateError>;

// crates/truefix-dict/src/sbe_schema.rs (dict-tooling gated)
pub fn load_sbe_schemas_from_file(path: &Path) -> Result<SbeSchemaSet, SbeSchemaError>;
pub fn parse_sbe_schemas(xml: &str) -> Result<SbeSchemaSet, SbeSchemaError>;
```

## Behavior Contract

1. Both modules are gated behind the existing `dict-tooling` Cargo feature (no new feature flag,
   no new external dependency — reuses the already-`dict-tooling`-gated `quick-xml`). Without
   `dict-tooling` enabled, these modules do not exist; callers needing FAST/SBE support MUST enable
   the feature, matching how `orchestra`/`vendor_xml` already work.
2. `parse_fast_templates`/`parse_sbe_schemas` are **not** wired into `truefix-dict::load_from_file`'s
   content-sniffing dispatch and **not** exposed as `generate-dict --format` values — they are a
   parallel loader family producing `FastTemplateSet`/`SbeSchemaSet`, never `.fixdict` text
   (research.md Decision 2). A FAST template XML file handed to `load_from_file` MUST continue to
   fail as an ordinary unrecognized/parse-error `.fixdict` load — it MUST NOT be silently
   misinterpreted as a `DataDictionary`.
3. A template/schema XML declaring a field operator, data type, or structure this feature does not
   implement MUST fail parsing with a named `FastTemplateError`/`SbeSchemaError` variant
   (`UnknownOperator`/`UnknownDataType`/etc.) — never succeed with a silently-incomplete model, and
   never defer the failure to encode/decode time (spec.md Edge Cases).
4. A duplicate template/message ID within one loaded set MUST fail parsing
   (`DuplicateTemplateId { id }`), not silently keep the first or last occurrence.
5. Every error variant carries enough structured context (element name, attribute, byte/element
   position) to locate the offending XML content without re-parsing — mirrors
   `orchestra::OrchestraError`'s shape exactly (research.md Decision 5).

## Reference-Fixture Licensing Gate (Constitution Principle III — NON-NEGOTIABLE)

6. Before any external FAST template XML or SBE schema XML (e.g. OpenFAST's test-vector corpus or
   Real Logic `simple-binary-encoding`'s example schemas) is committed to this repository as a test
   fixture, its license MUST be confirmed compatible with Apache-2.0 OR MIT (no copyleft terms that
   would propagate into TrueFix's dual license). This check MUST happen before the corresponding
   `/speckit-tasks` task that adds the fixture file is marked complete — it is not satisfied by
   merely *referencing* the external project in documentation.
7. If a candidate external fixture's license is **not** confirmed compatible (or the check has not
   yet been completed), the corresponding conformance test (spec.md SC-001/SC-002) MUST use the
   FAST/SBE specification documents' own embedded example encodings instead (Clarifications Q1's
   fallback) — the acceptance criterion is never skipped or weakened, only its fixture source
   changes.
8. This gate applies per-fixture-file, not once for the whole feature: a FAST fixture and an SBE
   fixture may end up on different sides of this check (e.g. one license-compatible, the other
   falling back to spec-embedded examples) — each is recorded independently in the corresponding
   task's completion notes.
