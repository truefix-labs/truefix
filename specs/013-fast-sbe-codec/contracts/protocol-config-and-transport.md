# Contract: `Protocol` Session Config, `.cfg` Plumbing, Transport Integration

Traces: spec.md FR-006, FR-007, FR-009, FR-010, FR-011, FR-013; research.md Decisions 3, 4, 5.

## API

```rust
// crates/truefix-session/src/config.rs
pub enum Protocol {
    Soh,   // default
    Fast,
    Sbe,
}

pub struct SessionConfig {
    // ...existing fields unchanged...
    pub protocol: Protocol,  // new; default Protocol::Soh via SessionConfig::new
    // FAST/SBE-specific resolved template/schema set is carried alongside, populated only for the
    // matching Protocol variant — exact field shape is an implementation detail of config.rs.
}
```

```text
# new .cfg key, registered in truefix-config::keys::APPENDIX_A_KEYS
Protocol=SOH|FAST|SBE          # case-insensitive, default SOH if absent
FastTemplatePath=<path>        # required when Protocol=FAST
SbeSchemaPath=<path>           # required when Protocol=SBE
```

## Behavior Contract — Config

1. `Protocol` defaults to `Soh` when the `.cfg` key is absent — every existing `.cfg` file's
   resolved `SessionConfig` is byte-for-byte unchanged by this feature (no regression to any
   existing session).
2. `Protocol=FAST`/`Protocol=SBE` without a corresponding `FastTemplatePath`/`SbeSchemaPath` MUST
   fail config resolution with a named typed error at startup (fail fast, not at first connection).
3. `FastTemplatePath`/`SbeSchemaPath` resolution follows the existing `resolve_validator`/
   `load_dictionary_value` bundled-name-then-filesystem-fallback pattern (research.md Decision 3) —
   no new resolution mechanism.
4. The `.cfg` key parser (`protocol_key`) matches case-insensitively, mirroring `precision_key`'s
   existing convention for `TimeStampPrecision`.

## Behavior Contract — Transport (`classify_buffered`)

5. For a `Protocol::Soh` session, `classify_buffered`'s behavior is **exactly** what it is today —
   this feature must not change the SOH decode path's framing, error types, or performance
   characteristics (FR-011's "transparent to Application" extends to "transparent to the SOH path
   itself").
6. For a `Protocol::Fast`/`Protocol::Sbe` session, `classify_buffered` frames and decodes using
   `truefix-binary`'s `FastCodec`/`SbeCodec` instead of `truefix_core::framing::frame_length`/
   `decode`/`decode_with_groups`.
7. A byte stream that cannot be decoded under the session's configured `Protocol` (e.g. an SOH peer
   connecting to a `Protocol::Fast` session) MUST surface a distinct, named error at this decode
   step — distinguishable from an ordinary truncated/malformed-but-same-protocol input — and MUST
   result in the connection being closed (mirroring the existing "non-Logon first message closes
   the socket" precedent), which is what ultimately manifests as "Logon fails" from the peer's
   perspective (spec US3 Scenario 3; research.md Decision 4).
8. `classify_buffered`'s existing SOH-specific malformed-frame resync heuristic (`next_frame_start`,
   scanning for the next `"8="`) MUST NOT be applied under `Protocol::Fast`/`Protocol::Sbe` — there
   is no equivalent textual marker in either binary format, so a decode failure under those
   protocols is treated as connection-fatal, not resync-and-continue.
9. (Defense-in-depth, optional) If a `Protocol`-mismatch guard is also added to
   `truefix-session::state.rs::on_logon`, it MUST follow the existing `reject_logon(&Reject{...})`
   sequential-guard pattern exactly (same rejection/Logout/disconnect shape as every other existing
   guard in that function) — it is secondary to Behavior Contract item 7, not a replacement for it.

## Behavior Contract — Observability (FR-013)

10. A `FastCodec`/`SbeCodec` decode failure, an `UnknownTemplateId` result, or a `FastCodec`
    `EncodingContext` reset MUST emit a `tracing::error!`/`warn!` event with `target:
    "truefix_binary"` and structured fields including at least the session identifier and (where
    applicable) the template/schema ID and error detail — mirroring `truefix-log`'s
    `report_write_failure` shape exactly.
11. Each such event MUST be paired with a `metrics::counter!` increment labelled by session
    (mirroring `truefix-transport::metrics_export.rs`'s `"session" => session_label(id)` labelling
    convention) — e.g. `truefix_binary_decode_failures_total`. These calls MUST be unconditional
    (cheap no-ops when no metrics recorder is installed), matching the existing convention's
    documented rationale.
