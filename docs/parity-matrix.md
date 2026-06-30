# Parity Traceability Matrix

Maps the spec parity baselines to where they are realized and verified (T096; parity.md
CHK033/CHK036). This is the human-readable companion to the machine-checked registries
(`truefix_config::APPENDIX_A_KEYS`, the AT scenario list).

## Appendix A — configuration keys → owning crate

Every key is enumerated with a stance in `crates/truefix-config/src/keys.rs`
(`APPENDIX_A_KEYS`), and `tests/key_coverage.rs` asserts none is silently unrecognized (SC-004).

| Appendix A group | Owning crate(s) | Notes |
|------------------|-----------------|-------|
| identity, session behavior, validation toggles | `truefix-session`, `truefix-dict` | session config + dictionary validation |
| scheduling (StartTime/EndTime/Weekdays/TimeZone/NonStop) | `truefix-session` (`schedule`) | `Schedule::is_in_session` |
| acceptor / dynamic, initiator, socket, SSL/TLS | `truefix-transport` | routing, dynamic sessions, allow-list, rustls, `SocketOptions` |
| proxy | — | documented unsupported (not implemented) |
| file store, SQL store | `truefix-store` | SQL behind the `sql` feature |
| file/screen/facade log | `truefix-log` | tracing facade; SLF4J-specific keys documented unsupported |
| Sleepycat/JE | — | documented unsupported (deferred v1) |

Stances: **Implemented** (honored), **Recognized** (parsed; behavior partial/pending), or
**Unsupported(reason)**. See the registry for per-key detail.

## Appendix B — AT scenarios → fixtures

The AT runner (`truefix-at`) drives a real acceptor as a black box. Authored scenarios live in
`crates/truefix-at/src/scenarios.rs`; the conformance test (`tests/conformance.rs`) runs the matrix
across the target versions and is the CI gate.

| Scenario (behaviour) | Versions | Status |
|----------------------|----------|--------|
| 1a ValidLogonWithCorrectMsgSeqNum | 4.2, 4.4 | ✅ |
| 1a ValidLogonMsgSeqNumTooHigh (→ ResendRequest) | 4.2, 4.4 | ✅ |
| 2b MsgSeqNumTooHigh (→ ResendRequest) | 4.2, 4.4 | ✅ |
| 2c MsgSeqNumTooLow (→ Logout + disconnect) | 4.2, 4.4 | ✅ |
| 4b ReceivedTestRequest (→ Heartbeat echo) | 4.2, 4.4 | ✅ |
| 13b UnsolicitedLogoutMessage (→ Logout + disconnect) | 4.2, 4.4 | ✅ |
| Remaining 67 server scenarios + special-category suites | all | ⏳ to port onto the runner |

## Codec reference vectors

`crates/truefix-core/tests/fixtures/reference/` holds QuickFIX/J-sourced wire vectors,
cross-validated in `tests/reference_vectors.rs` (byte-exact BodyLength/CheckSum).
