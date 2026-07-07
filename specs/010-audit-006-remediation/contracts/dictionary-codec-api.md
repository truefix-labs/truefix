# Contract: Dictionary, Codec, And Public API Validation

## Scope

Applies to `truefix-core`, `truefix-dict`, `truefix-config` validation options, and public API
surfaces that expose parsed/validated messages.

## Required Behaviors

- Integer and decimal field accessors must reject whitespace-padded wire values (`NEW-101`).
- Frame detection must verify checksum trailer terminator (`NEW-107`).
- Direct public decode must require checksum values to be exactly three ASCII digits (`NEW-155`).
- Required header/trailer field checks and BeginString dictionary-version checks must be explicit
  and tested (`NEW-127`, `NEW-145`).
- Section-order violations must be rejected through validation policy, preserving decode-time
  diagnostics (`NEW-146`).
- Nested repeating group validation must use the nested group's own definition (`NEW-147`).
- Malformed `SequenceReset` and other admin messages must go through the same dictionary validation
  expectations as comparable inbound messages (`NEW-99`).

## Acceptance Evidence

- Table-driven unit tests for malformed field values, checksum formats, and section-order cases.
- Dictionary fixtures for required header/trailer and nested group validation.
- Integration tests where validation rejection should produce session-level behavior.

## Compatibility Notes

Strict parsing may reject messages previously accepted leniently. That is intentional protocol
parity behavior and must be documented for operators.
