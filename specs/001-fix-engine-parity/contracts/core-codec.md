# Contract: truefix-core (Message model & codec)

Covers FR-B1…B9. No panic on any parse path (SC-005).

## Provided behavior

- **Construct/inspect messages**: build `Message` with header/body/trailer regions; get/set typed fields;
  add and iterate nested repeating `Group`s. Field order within a region is preserved.
- **Encode** `Message → bytes`: emits SOH-delimited wire form, header fields in FIX-required order,
  computes BodyLength(9) and CheckSum(10). For the canonical set (NewOrderSingle, ExecutionReport, Logon,
  Heartbeat, ResendRequest, SequenceReset, Reject) the bytes — including BodyLength/CheckSum — MUST be
  byte-identical to QuickFIX/J for the same logical message (SC-002).
- **Decode** `bytes → Result<Message, DecodeError>`: parses fields and nested groups; verifies
  BodyLength/CheckSum; preserves original textual field forms for byte-exact re-encode (round-trip, FR-B9).
- **Field types & converters**: Int, Float/Decimal (rust_decimal), Char, Boolean, String, Data/RawData,
  UtcTimestamp (ns store, ≤ ps accept), UtcTimeOnly, UtcDateOnly, MonthYear, etc. Each converter returns
  `Result<_, FieldError>`.

## Error contract

- `DecodeError` (typed enum): GarbledMessage, BodyLengthMismatch, ChecksumMismatch, TruncatedInput,
  FieldError{tag,kind}, … — governed by `RejectGarbledMessage` at the session layer.
- No `panic!`/`unwrap`/`expect`/`unreachable!`/panicking-index on encode/decode (Constitution I).

## Acceptance hooks
- Table-driven unit tests per field type and per canonical message.
- Round-trip property: `decode(encode(m)) == m` and `encode(decode(b)) == b` for valid `b`.
- Byte-exact BodyLength/CheckSum comparison vectors vs reference output.
