# Contract: `truefix-dict` — Vendor XML Dictionary Ingestion

Traces: spec.md User Story 3, FR-011 through FR-017; research.md R5, R6.

## Provenance Boundary (Constitution Principle III) — read first

This module parses **vendor-published** dictionary files (e.g. Binance's own `spot-fix-oe.xml`/
`spot-fix-md.xml`, independently licensed by Binance) against their published XML shape and the
public FIX protocol specification. It MUST NOT read, translate, or embed any content from
`thrdpty/quickfix`'s or `thrdpty/quickfixj`'s own bundled `spec/FIX*.xml` files — the private data
files whose translation this project already removed (`qfj_xml`, superseded by `fix_repository.rs`).
Module naming and doc comments must not imply the code is derived from or reads QuickFIX's bundled
files. This boundary is a blocking review criterion for any task implementing this contract.

## `vendor_xml` Module (new, `dict-tooling`-gated)

```rust
pub fn convert(xml: &str) -> Result<String, VendorXmlError>
```

1. Parses the nested shape: `<fix><header>...</header><messages><message name= msgtype= msgcat=>
   ...</message></messages><trailer>...</trailer><components>...</components><fields><field
   number= name= type=><value enum= description=/></field></fields></fix>`.
2. Emits `.fixdict` line-oriented text — the same output contract as `orchestra::convert`/
   `fix_repository::convert` — fed to the existing `crate::parse()`. No parallel `DataDictionary`
   construction path.
3. `<component name= required=>` references translate to `component:Name` text references (not
   inlined) — `.fixdict`'s parser already resolves these recursively.
4. `<group name= required=>` (including nested `<group>` and `<component>` references within a
   group, and groups nested within groups) translate to `.fixdict`'s `group` syntax, member lists
   preserved in order.
5. `<field number= name= type=>` type tokens (`INT`, `LENGTH`, `SEQNUM`, `NUMINGROUP`, `FLOAT`,
   `PRICE`, `QTY`, `CHAR`, `BOOLEAN`, `STRING`, `DATA`, `UTCTIMESTAMP`, etc.) pass through as-is —
   these are standard FIX type names `FieldType::parse` (`model.rs`) already recognizes.
6. Multi-character `msgtype` values (e.g. `XCN`, `XLQ`, `XAK`) are supported as first-class message
   type identifiers — `truefix_dict`'s internal model (`msg_type: String`) has no single-character
   assumption to violate.
7. The root element's version attributes (`type`/`major`/`minor`, and `servicepack` when present)
   are translated into a synthesized `version FIX.M.N[SPk]` directive (research.md R6). Missing or
   unrecognizable version attributes produce `VendorXmlError`, not a guessed/omitted version.
8. Malformed XML, an undefined `<component>`/`<group>` reference, or any other structurally
   unrecognized input produces a `VendorXmlError` — never a partially-populated or silently-incorrect
   `.fixdict` output (spec.md FR-016).

## `load_from_file` (existing function, extended)

1. Content-sniffs the file: text beginning with `<?xml` or `<fix ` is treated as vendor XML and
   routed through `vendor_xml::convert` before `crate::parse()`; everything else is parsed as
   `.fixdict` text exactly as today.
2. This sniffing branch is `#[cfg(feature = "dict-tooling")]`-gated (`vendor_xml` depends on
   `quick-xml`, an optional dependency). Without `dict-tooling` enabled, a vendor-XML-shaped file
   still produces today's ordinary `DictLoadError` (ordinary ".fixdict" parse failure on XML syntax)
   — an explicit, typed error, never a silent partial load.
3. Both existing `load_from_file` call sites — single-dictionary and FIXT dual-dictionary `.cfg`
   loading (`truefix-config/src/builder.rs`'s `load_dictionary_value`) — get vendor-XML support
   transparently, with no change to their own code.

## CLI (`truefix-dict` binary, already `dict-tooling`-gated)

1. `generate-dict --format vendor-xml --source <file.xml> --out <normalized.fixdict>` — new match
   arm in `generate_dict()`, alongside the existing `orchestra` (default) and `fix-repository`
   arms, calling `vendor_xml::convert` and writing its output exactly like the other two formats.
2. The converted `.fixdict` output is a normal input to the existing `generate-code`/`validate`
   subcommands — no special-casing needed downstream.

## Test Expectations

- `crates/truefix-dict/tests/vendor_xml_conversion.rs` (new, mirroring
  `orchestra_conversion.rs`/`fix_repository_group_converter_fix.rs`'s style): a self-contained
  fixture XML with fields, a body-level group, a component reference, a nested group, and a
  multi-character custom `MsgType`, asserting `convert()`'s `.fixdict` output parses successfully and
  produces a `DataDictionary` that validates/structures a matching sample message correctly.
- Error-path tests: malformed XML, missing version attributes, undefined component/group reference
  — each asserting a `VendorXmlError`, not a panic or partial success.
- `crates/truefix-dict/tests/load_from_file.rs`: extend with a vendor-XML-shaped fixture, both with
  and without `dict-tooling` enabled (via a `#[cfg(feature = "dict-tooling")]`-gated test module, if
  the existing test harness already separates feature-gated cases this way).
- `crates/truefix-dict/tests/cli.rs`: extend with a `--format vendor-xml` invocation.
