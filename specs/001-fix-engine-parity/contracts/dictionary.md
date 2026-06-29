# Contract: truefix-dict (Dual-track data dictionary)

Covers FR-A2…A4, FR-C1…C5. Constitution Principle IV (codegen + runtime coexist).

## Provided behavior

- **Normalized dictionary source**: a TrueFix-owned format derived at build time from FIX
  Orchestra/Repository specs (not QuickFIX/J XML; Constitution III). Single source of truth.
- **Build-time codegen** (`build.rs`): generates strongly-typed per-version message/field structs from
  the normalized source (e.g. `fix44::NewOrderSingle`). Generated code carries the source version/hash.
- **Runtime `DataDictionary`**: load a dictionary (bundled or custom/extended) and `validate(&Message)`
  returning typed validation results that the session maps to session-level Reject or Business Message
  Reject with the spec-correct reason.
- **Validation toggles** honored individually (FR-C3): `UseDataDictionary`, `ValidateFieldsOutOfOrder`,
  `ValidateFieldsHaveValues`, `ValidateUnorderedGroupFields`, `ValidateUserDefinedFields`,
  `ValidateIncomingMessage`, `ValidateChecksum`, `AllowUnknownMsgFields`, `CheckLatency`/`MaxLatency`,
  `CheckCompID`, `FirstFieldInGroupIsDelimiter`. Each toggle's accept/reject effect is specified & tested.
- **Two rejection layers** (FR-C4): (a) dictionary/validation failure vs (b) FIX protocol-level basic
  validity — distinct, separately testable outcomes.
- **FIXT 1.1**: separate Transport and Application dictionaries; Application dictionary resolved via
  `DefaultApplVerID`/`ApplVerID` (FR-A3/A4).
- **UDF & custom dictionaries**: user-defined fields and extended dictionaries loadable at runtime (FR-C5).

## Dual-track invariant
A build assertion confirms codegen output and the runtime model originate from the same normalized
dictionary version/hash, so typed messages and runtime validation cannot disagree.

## Acceptance hooks
- Parse each bundled version dictionary; validate conforming/non-conforming messages per toggle.
- FIXT split + DefaultApplVerID resolution test.
- Codegen/runtime same-source assertion in CI.
