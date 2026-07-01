# Contract: Dictionary-Driven Group Parsing & Validation (US3; FR-004/005)

## Surface

- `decode_with_dict(bytes, &DataDictionary) -> Result<Message, DecodeError>` (`truefix-core`).
- Group validation integrated into `DataDictionary::validate` against the structured result.

## Behaviour

1. **Decode**: on a group-count tag, consume the declared number of entries; each entry is delimited by
   the dictionary's first-field (delimiter) for that group. Nested groups recurse; components expand to
   their member fields/groups. Without a dictionary, the existing flat decode is used.
2. **Validation toggles**:
   - count vs entries: declared count ≠ parsed entries ⇒ Reject (count-mismatch reason).
   - `FirstFieldInGroupIsDelimiter`: an entry whose first field isn't the delimiter ⇒ Reject.
   - `ValidateUnorderedGroupFields`: out-of-order members ⇒ Reject when on; accepted when off.
3. **Edge cases**: zero declared count ⇒ empty group (no entries); nested groups parsed recursively.

## Acceptance (maps to spec US3 scenarios + Appendix B 14i/14j/21/QFJ934)

- Correct group ⇒ decoded + accepted. ✔
- Wrong count ⇒ count-mismatch Reject. ✔
- Missing delimiter (toggle on) ⇒ Reject. ✔
- Out-of-order members (toggle on) ⇒ Reject; (off) ⇒ accepted. ✔
- Nested / zero-count ⇒ handled. ✔

## Test hooks

Table-driven core decode tests (correct + each malformed variant), dict validation tests per toggle, and
AT scenarios `14i_RepeatingGroupCountNotEqual`, `14j_OutOfOrderRepeatingGroupMembers`,
`21_RepeatingGroupSpecifierWithValueOfZero`, `QFJ934_MissingDelimiterNestedRepeatingGroup`.
