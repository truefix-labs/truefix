# Contracts Index — Feature 005

Each file documents a changed or new public-surface contract this feature introduces, grouped by theme
(mirroring `docs/engine-comparison-gaps.md`'s own layer grouping). Every addition is additive — see
each file's "No breaking changes" note. `session-protocol.md` is the one file whose changes are
genuinely protocol-behavioral (Constitution Principle II) rather than pure wiring/config/schema growth
— it's the only theme required to grow the AT conformance suite, not merely keep it green.

| File | Covers (User Stories) | Theme |
|------|------------------------|-------|
| [correctness-bugs.md](./correctness-bugs.md) | US1, US2 | `#`-in-value truncation, Signature/93 length mapping, `AcceptorBuilder` wiring, `JdbcURL` scheme recognition |
| [session-protocol.md](./session-protocol.md) | US3, US4 | Resend veto + gap-fill, PossDup anti-replay, duplicate-Logon rejection, inbound chunk auto-continuation — **protocol-behavioral, grows the AT suite** |
| [session-transport.md](./session-transport.md) | US5, US6 | Session identity (sub-ID/location-ID/qualifier), initiator connection robustness (reconnect backoff, local bind, connect timeout) |
| [store-log-hardening.md](./store-log-hardening.md) | US7 | Creation-time persistence, atomic save+increment, structured log schema widening |
| [stance-registry.md](./stance-registry.md) | US8 | Config-key stance corrections (both directions) + JDBC pool/table-name wiring |
| [dictionary-model.md](./dictionary-model.md) | US9 | 11 new `FieldType`s, open enums, per-group child dictionaries, group CRUD, header/trailer groups, custom field order, version metadata + match, value-label lookup, bundled dictionary content parity with QuickFIX/J |
