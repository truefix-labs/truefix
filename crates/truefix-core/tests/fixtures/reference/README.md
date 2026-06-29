# Reference wire vectors (T099)

Byte-exact reference messages used by `tests/reference_vectors.rs` to cross-validate that
TrueFix's framing, BodyLength, and CheckSum agree with QuickFIX/J on real data (T010/T072; SC-002).

## Scope (per project decision, 2026-06-29)

These are reference **wire messages (output data)** sourced from QuickFIX/J test resources. Using
QuickFIX/J's emitted wire data / test vectors as fixtures is permitted for this directory. This is a
narrow, data-only allowance: TrueFix's own engine code under `crates/*/src` is still implemented from
the FIX specification and does **not** copy QuickFIX/J source logic (the dual Apache-2.0 OR MIT release
goal depends on that). See `[[license-discipline-t099-scope]]` notes in the constitution discussion.

## Captured vectors

- `fix44_logon_rawdata.fix` — FIX.4.4 Logon-shaped message exercising a length-prefixed RawData field
  (BodyLength 21, CheckSum 086).
- `fix44_newordersingle_group.fix` — FIX.4.4 message with a NoPartyIDs repeating group
  (BodyLength 35, CheckSum 040).

## Format

Each `.fix` file holds the raw SOH-delimited bytes of one message (SOH = 0x01). `decode()` validates
BodyLength + CheckSum, so a successful decode is the cross-check; re-encoding proves byte-exact fidelity.

## Follow-up

Broader per-version coverage of the full canonical set (NewOrderSingle, ExecutionReport, Logon,
Heartbeat, ResendRequest, SequenceReset, Reject across FIX 4.0–5.0SP2 + FIXT 1.1) can be added by
building QuickFIX/J and capturing its output vectors.
