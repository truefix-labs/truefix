# Contract: truefix-config (SessionSettings & Appendix A keys)

Covers FR-I1…I3, SC-004. The parity acceptance rule for configuration.

## Provided behavior
- **Load** `.cfg` with `[DEFAULT]` + repeated `[SESSION]` sections; defaults inherited, per-session
  overrides win (FR-I1).
- **Interpolation**: `${name}` resolved from a defined variable source (FR-I3).
- **Programmatic config**: a builder produces the same settings model without a file (FR-I3).
- **Key contract (FR-I2 / SC-004)**: every key in spec **Appendix A** is either:
  - **Implemented** — parsed into the settings model and honored by the owning crate, OR
  - **Documented unsupported-with-reason** — recognized and reported per a discoverable stance
    (no silent unknown-key behavior beyond the configured `AllowUnknownMsgFields`-style policy).
- **v1 stance**: all Appendix A groups implemented except the **Sleepycat/JE** key group
  (`SleepycatDatabaseDir`, `SleepycatMessageDbName`, `SleepycatSequenceDbName`), which is
  documented unsupported-with-reason (deferred). SQL/JDBC keys are implemented (in scope).

## Key→owner mapping (for traceability; full list in Appendix A)
| Group | Owning crate |
|-------|--------------|
| Identity & type, session behavior, scheduling, validation toggles | truefix-config → truefix-session/dict |
| Acceptor/dynamic, initiator, socket, SSL/TLS, proxy | truefix-config → truefix-transport |
| File/SQL store keys | truefix-config → truefix-store |
| Screen/File/SLF4J/SQL log keys | truefix-config → truefix-log |
| Sleepycat keys | documented unsupported-with-reason (v1) |

## Acceptance hooks
- Load a `.cfg` exercising a key from every group; assert applied or documented-unsupported.
- `${name}` interpolation test; defaults/override precedence test.
- A coverage test enumerates Appendix A keys and asserts each has a known stance (SC-004).
