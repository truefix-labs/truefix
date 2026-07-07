# Contracts: Body-Level Repeating Group Decoding & Vendor Dictionary Support

This directory defines the behavior contracts that `/speckit-tasks` should convert into tests and
implementation tasks.

- [core-members-api.md](./core-members-api.md): `truefix-core`'s new `FieldMap::members()`/
  `MemberRef` public API and the body-restructuring primitive it enables.
- [decode-and-restructure.md](./decode-and-restructure.md): `classify_buffered`'s single- and
  FIXT-dual-dictionary decode behavior, and the FIXT post-decode application-dictionary
  restructuring step.
- [validation.md](./validation.md): `DataDictionary::validate`'s structured-group validation path,
  the removal of the now-dead flat group walk, and the `present()` group-aware presence fix.
- [vendor-xml-dictionary.md](./vendor-xml-dictionary.md): the new `vendor_xml` converter, its
  `dict-tooling` gating, `load_from_file`'s content-sniffing, and the CLI's `--format vendor-xml`.

## AT Scenario Guidance

No new AT scenarios are required (research.md R7): this feature changes decode-time structure
visibility and dictionary-loading format, not session-layer protocol behavior. Existing AT coverage
must continue to pass unchanged (spec.md SC-002).

## Public API Compatibility Placeholders

- `truefix-core`: `FieldMap::members()`/`MemberRef` are additive public API — document with full
  doc comments (Constitution Principle I) before marking the corresponding task complete.
- `truefix-dict`: `vendor_xml::convert`/`VendorXmlError` are additive public API (behind
  `dict-tooling`); `load_from_file`'s expanded format support and the CLI's new `--format` value are
  additive, backward-compatible surface. `DataDictionary::validate`'s public signature is unchanged.
- `truefix-transport`/`truefix-session`: no public API changes anticipated — `HeaderTrailerGroupsOnly`
  is crate-private, and the FIXT restructuring step is an internal call site. Confirm during
  implementation and note here if a public surface change turns out to be needed.
