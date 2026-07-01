# Contract: All-Message Typed Codegen + MessageCracker (US6; FR-020/021/022)

Completes constitution Principle IV (dual-track). Per Clarifications: **every** message in the bundled
dictionary subsets is generated.

## Generated surface (per targeted version)

- **Field module**: typed field accessors + value enumerations (e.g. `Side::Buy`, `OrdType::Market`)
  from the dictionary's allowed values.
- **Message structs**: one per MsgType, with typed accessors for fields, groups, and components; a thin
  wrapper over the generic `Message`.
- **Group / Component structs**: typed, possibly nested, reflecting the dictionary group/component model.

## MessageCracker

```text
trait MessageCracker {
    fn crack(&self, msg: &Message, id: &SessionId);  // dispatch by (BeginString, MsgType) to a typed handler
}
```

## Behaviour / invariants

1. A generated typed message **encodes/decodes byte-identically** with the generic codec path (FR-021) —
   because it wraps the same generic `Message`.
2. The **dual-track FNV-1a hash** continues to prove codegen artifacts and the runtime `DataDictionary`
   derive from one normalized source (Principle IV).
3. `MessageCracker::crack` dispatches an incoming message to the handler typed for its MsgType/version
   (replaces the current empty-shell trait).
4. Generation is build-time from `dict-src/normalized/*.fixdict`; **no third-party data is copied**
   (Principle III).

## Acceptance (maps to spec US6 scenarios)

- Build emits typed message/group/component/field-enum artifacts for each targeted version. ✔
- Typed message round-trips byte-identically with the generic path. ✔
- Cracker dispatches by MsgType/version to a typed handler. ✔
- Dual-track hash confirms one source. ✔

## Test hooks

Codegen golden tests (compile + shape), a round-trip equality test (typed vs generic bytes), a cracker
dispatch test, and the existing dual-track hash assertion.
