# Quickstart: Validating the FAST/SBE Binary Protocol Codec

This is a validation guide, not an implementation guide — it documents runnable checks that prove
each user story works end-to-end once implemented. See [contracts/](./contracts/) for the detailed
behavior each check exercises.

## Prerequisites

- Rust workspace builds: `cargo build --workspace`.
- The new crate builds standalone: `cargo build -p truefix-binary`.
- FAST-template/SBE-schema parsing requires the `dict-tooling` feature (reused from `truefix-dict`):
  `cargo build -p truefix-dict --features dict-tooling`.
- Reference-fixture licensing (contracts/fast-template-sbe-schema-parsing.md items 6–8) MUST be
  resolved before running the SC-001/SC-002 conformance checks below against any vendored external
  fixture — until then, use the FAST/SBE spec documents' own embedded example encodings.

## User Story 1 — FAST template-driven codec

1. Run the FAST codec unit tests:
   ```sh
   cargo test -p truefix-binary --test fast_roundtrip
   cargo test -p truefix-binary --test fast_delta
   cargo test -p truefix-binary --test fast_multi_template
   ```
2. Manually verify: parse a FAST template XML declaring `copy`/`increment`/`delta`/`default`
   fields, decode a matching sample byte stream, and confirm every field value is recovered
   correctly (including values omitted from the wire and recovered from `EncodingContext`).
   Re-encode the decoded result and confirm byte-for-byte equality with the reference source
   selected per Clarifications Q1.
3. Confirm malformed-input handling: feed a truncated/invalid-stop-bit byte stream and confirm a
   named `FastCodecError` variant is returned, not a panic — check with:
   ```sh
   cargo test -p truefix-binary --test fast_roundtrip -- malformed
   ```
4. Confirm multi-template selection (FR-012): load a set of ≥2 templates, decode messages
   referencing each by inline template ID, and confirm an unknown ID returns
   `FastCodecError::UnknownTemplateId` rather than decoding against an arbitrary loaded template.

**Expected outcome**: SC-001 (byte-exact FAST round-trip) and SC-005 (zero panics on malformed
input), for User Story 1's independent test scope (no SBE, no session/transport integration).

**Implementation result (2026-07-08)**: PASS.
`cargo test -p truefix-binary --test fast_roundtrip --test fast_delta --test fast_multi_template --test observability`
passed, and `cargo test -p truefix-dict --features dict-tooling --test fast_template_parse` passed.
FAST fixtures use the Clarifications Q1 spec-embedded-example fallback because OpenFAST's public
license is MPL, not Apache-2.0/MIT-compatible for vendored fixtures.

## User Story 2 — SBE schema-driven zero-copy codec

1. Run the SBE codec unit tests:
   ```sh
   cargo test -p truefix-binary --test sbe_roundtrip
   cargo test -p truefix-binary --test sbe_multi_schema
   ```
2. Manually verify: parse an SBE schema XML declaring fixed fields, a repeating group, and a
   `VarData` field; decode a matching sample message and confirm every fixed field, group entry,
   and `VarData` payload is read correctly. Re-encode and confirm byte-for-byte equality with the
   reference source selected per Clarifications Q1.
3. Confirm the zero-copy property (SC-003) with an allocation-counting test or profiler run over
   the decode path — no heap allocation beyond the input buffer.
4. Confirm malformed-input handling (declared `blockLength`/`numGroups`/`VarData` length
   inconsistent with actual content) returns a typed error before any out-of-bounds read.
5. Confirm multi-schema selection (FR-012): analogous to FAST's step 4, using `templateId`/
   `schemaId`.

**Expected outcome**: SC-002 (byte-exact SBE round-trip) and SC-003 (zero extra heap allocation),
independent of FAST or session/transport integration.

**Implementation result (2026-07-08)**: PASS.
`cargo test -p truefix-binary --test sbe_roundtrip --test sbe_multi_schema` passed, and
`cargo test -p truefix-dict --features dict-tooling --test sbe_schema_parse` passed. SBE fixtures
use an Apache-2.0-compatible Real Logic SBE-style schema shape recorded by T005.

## User Story 3 — IR conversion and `Protocol` session integration

1. Run the IR conversion tests (Clarifications Q3's representative subset):
   ```sh
   cargo test -p truefix-binary --test ir_conversion
   ```
2. Run the `Protocol`-aware transport/session integration tests, both acceptor and initiator roles
   (Constitution Principle VI):
   ```sh
   cargo test -p truefix-transport
   cargo test -p truefix-session
   ```
3. Manually verify end-to-end: configure a session pair with `Protocol=FAST` (or `Protocol=SBE`)
   and matching template/schema paths on both sides; confirm the Application callback on each side
   receives `Message`s field-equivalent to what an SOH session would deliver for the same logical
   content, while the actual bytes on the wire are FAST/SBE-encoded.
4. Confirm the `Protocol`-mismatch path: configure one side `Protocol=SOH` and the other
   `Protocol=FAST`; confirm Logon fails with a named typed error (not a hang, not a misparsed
   message) — see contracts/protocol-config-and-transport.md item 7.
5. Confirm observability (FR-013): trigger a decode failure and an `EncodingContext` reset (e.g. by
   reconnecting), and confirm both produce the structured `tracing`/`metrics` events described in
   contracts/protocol-config-and-transport.md items 10–11 (a test `tracing` subscriber capturing
   events, and/or `metrics`' test recorder, is the mechanism — no manual log inspection required).
6. Confirm zero regression on the default path:
   ```sh
   cargo test -p truefix-at --test conformance
   ```
   MUST still show 483/483 scenarios passing — this feature adds no new AT scenarios (research.md
   Decision 6) and must not regress the existing suite.

**Expected outcome**: SC-004 (lossless IR round-trip for the representative subset), SC-006
(Application-level transparency + correct Logon failure on mismatch), SC-007 (observability), and
zero regression to the existing 483/483 AT suite.

**Implementation result (2026-07-08)**: PASS for the feature-local checkpoint.
`cargo test -p truefix-binary --test ir_conversion --test observability`,
`cargo test -p truefix-config --test protocol_config`, and
`cargo test -p truefix-transport --test protocol_mismatch --test binary_protocol_session` passed.
The transport branch now bypasses SOH resync for `Protocol::Fast`/`Protocol::Sbe` and treats binary
decode failures as connection-fatal.

## Full workspace regression check

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p truefix-at --test conformance
```

**Implementation result (2026-07-08)**: PASS.
`cargo fmt --all --check` passed. `cargo clippy --workspace --all-targets -- -D warnings` passed.
`cargo test --workspace` passed when rerun outside the sandbox because socket-binding integration
tests need local network permissions; the first sandboxed run failed only with `Operation not
permitted` in socket tests. The AT conformance suite ran as part of the successful workspace test
run and passed with no regressions.
