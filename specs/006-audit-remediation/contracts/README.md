# Contracts Index — Feature 006

Each file documents a changed or fixed public/internal-behavior contract this feature introduces,
grouped by theme (mirroring `docs/todo/003.md`'s own P0/P1/P2 grouping and spec.md's user-story
ordering). Every change is additive or internal-behavior-only — see each file's own note; the sole
disclosed public-API-surface growth across the whole feature is `MssqlStore::parse_url`'s dual
grammar (`config-compat.md`). `session-protocol.md` is the one file whose changes are genuinely
protocol-behavioral (Constitution Principle II) — it's the only theme required to grow the AT
conformance suite, not merely keep it green.

| File | Covers (User Stories) | Theme |
|------|------------------------|-------|
| [session-protocol.md](./session-protocol.md) | US1 | Low-seq Logon rejection, SequenceReset anti-replay, malformed ResendRequest, ResetOnLogon reconnect-loop fix, gap-fill validation, per-connection timer reset, Logout-before-disconnect, latency-check fix — **protocol-behavioral, grows the AT suite** |
| [routing-and-lifecycle.md](./routing-and-lifecycle.md) | US2, US5 | SubID/LocationID/Qualifier routing key, SO_REUSEADDR, Engine::start partial-failure cleanup, group ContinueInitializationOnError, Engine::shutdown() doc |
| [store-persistence.md](./store-persistence.md) | US3 | Swallowed store-error logging, creation_time consumption, FileStore/CachedFileStore atomicity, BodyLog fsync/reset ordering, MssqlStore identifier validation |
| [config-compat.md](./config-compat.md) | US4 | JdbcURL scheme table (H2 removal), MSSQL semicolon grammar, credential percent-encoding |
| [network-hardening.md](./network-hardening.md) | US6 | Bounded frame size, proxy connect timeout, CipherSuites validation, PROXY-header timeout/sizing, framing-recovery precision |
| [dictionary-codec.md](./dictionary-codec.md) | US7 | UtcTimeOnly/UtcDate validation, production fieldOrder/group-decode wiring, EncodedLeg* pairs, provenance headers |
| [completeness-and-harness.md](./completeness-and-harness.md) | US8, US9 | Recurring sequence reset, multi-tag LogonTag, FIXT ApplVerID negotiation, wildcard templates, log-backend selection, IANA timezones, env-var fallback, AT regression floor, MinQty scenario |
| [tooling-hygiene.md](./tooling-hygiene.md) | US10 | Dictionary-source recursion guard, doc-accuracy fix, BodyLog TOCTOU close |
