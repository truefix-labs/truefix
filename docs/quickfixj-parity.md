# QuickFIX/J compatibility and parity

TrueFix accepts QuickFIX/J-style session configuration and implements the core operational surface
expected from a full FIX engine: initiator and acceptor roles, persistence, recovery, dictionaries,
schedules, TLS, proxies, multi-session operation, and typed application callbacks.

“Parity” in this repository means inventory-backed compatibility work and black-box behavioral tests.
It does not mean source compatibility, certification, or an assertion that every QuickFIX/J extension
has an identical Rust API.

## Compatibility highlights

- `.cfg`-driven one-shot startup through `Engine::start`
- Familiar session, validation, reconnect, failover, TLS, and socket settings
- JDBC-style URL recognition for feature-gated SQL and MSSQL stores/logs
- Runtime dictionaries for `DataDictionary`, `AppDataDictionary`, and
  `TransportDataDictionary`
- Multi-session acceptors, dynamic session templates, session qualifiers, and identity routing
- Persistent resend and sequence-number state with restart recovery
- Typed equivalents for callback outcomes such as do-not-send and message rejects

Some Rust-native capabilities deliberately use different mechanisms. For example, redb and MongoDB
backends are selected through Rust APIs, and custom stores/logs implement public traits rather than
Java interfaces.

## Historical work

The original roadmap grew through these evidence-backed stages:

| Specs | Focus | AT-suite floor recorded at completion |
| --- | --- | ---: |
| `001` | Codec, sessions, recovery, dictionaries, storage, transport, TLS | Initial suite |
| `002`–`003` | Configuration, typed callbacks/codegen, all versions, operational closure | 353/353 |
| `004`–`005` | Engine wiring, failover, extra backends, identity, dictionary completeness | 373/373 |
| `006` | First independent audit remediation | 405/405 |
| `007` | Second audit remediation and persistence/schedule hardening | 424/424 |
| `009` | Authentication, FIXT, TLS trust, and tooling hardening | 444/444 |
| `010` | Lifecycle, sequence, persistence, configuration, and codec remediation | 483/483 |

There is no `008` feature directory. The detailed evidence remains in the corresponding `specs/`
directories, the [parity matrix](parity-matrix.md), and the [acceptance record](acceptance-record.md).
Counts are historical repository evidence and should be rerun for a release claim.

## Provenance

TrueFix is independently implemented from the FIX protocol specification. QuickFIX/J and
QuickFIX/Go were consulted to understand architecture and externally observable behavior; source
code and private data files were not copied or translated. Dictionary inputs derive from official
machine-readable FIX specifications, with tooling for compatible XML formats.
