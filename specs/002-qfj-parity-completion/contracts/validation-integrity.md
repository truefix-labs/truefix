# Contract: Inbound Integrity, Rejection Layers, Garbled, Reverse Route, Precision, Switches (US4/US11; FR-006–011)

## Behaviour

1. **Garbled frames (FR-006)**: bad checksum or incorrect body length ⇒ when `RejectGarbledMessage=Y`,
   emit a session Reject (35=3); when `N` (default), drop the frame **without advancing the sequence
   number** (current safe behaviour preserved).
2. **Integrity checks (FR-007)** — each producing the FIX-correct outcome:
   - BeginString correctness, BodyLength correctness, CheckSum validity.
   - `CheckCompID`: SenderCompID/TargetCompID match the session profile, else reject/refuse.
   - SendingTime range (CheckLatency/MaxLatency) ⇒ SendingTime-accuracy reason.
   - First-three-fields order (8,9,35); header/body field order (`ValidateFieldsOutOfOrder`).
   - Repeated-tag detection.
3. **Two rejection layers (FR-008)**: session-level Reject (35=3) vs Business Message Reject (35=j),
   each failure routed to the correct layer.
4. **Reverse routing (FR-011)**: replies/rejects to messages carrying routing header fields reverse the
   routing pairs (OnBehalfOf↔DeliverTo …); empty routing tags ⇒ no reversal, no error.
5. **Timestamp precision (FR-009)**: emitted SendingTime carries the configured precision
   (SECONDS/MILLIS/MICROS/NANOS; default MILLIS); inbound parses any precision without loss.
6. **Correctness switches (FR-010)**: `PersistMessages`, `HeartBeatTimeoutMultiplier`,
   `TestRequestDelayMultiplier` honoured instead of fixed constants.

## Acceptance (maps to spec US4/US11 scenarios; SC-004/SC-005)

Each malformed class reaches its FIX-correct outcome; reverse-routed replies carry reversed routing
fields; timestamps round-trip at each precision; switches change timeout/heartbeat/persistence behaviour.

## Test hooks — new AT scenarios

`3b_InvalidChecksum`, `2m_BodyLengthValueNotCorrect`, `1d_InvalidLogonWrongBeginString`,
`1c_InvalidSenderCompID`/`1c_InvalidTargetCompID`, `2o_SendingTimeValueOutOfRange`,
`2t_FirstThreeFieldsOutOfOrder`, `14g_HeaderBodyTrailerFieldsOutOfOrder`, `14h_RepeatedTag`,
`2d_GarbledMessage`/`3c_GarbledMessage` (RejectGarbledMessage on/off), `ReverseRoute`,
`ReverseRouteWithEmptyRoutingTags`; plus session-level precision/switch tests.
