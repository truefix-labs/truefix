# FIX conformance

TrueFix uses a black-box Acceptance Test (AT) suite as a release gate. The current repository record
is **483/483 scenario runs passing** across nine targeted application versions: FIX 4.0, 4.1, 4.2,
4.3, 4.4, 5.0, 5.0 SP1, 5.0 SP2, and FIX Latest. FIXT 1.1 transport behavior is exercised with
separate transport and application dictionaries.

The count describes this repository's scenario runs. It is not certification by the FIX Trading
Community, QuickFIX/J, or QuickFIX/Go.

## What the gate covers

- Logon, logout, heartbeat, timeout, and reset behavior
- Sequence-number recovery, resend requests, gap fills, and duplicate handling
- Message framing, checksum, timestamps, dictionaries, validation, and repeating groups
- Initiator and acceptor behavior across the supported FIX versions
- Dedicated checksum-validation, timestamp, and resynchronization categories

Two-process integration tests additionally exercise real TCP sessions, TLS/mTLS, failover,
restart-survivable persistence, and metrics export. Unit and table-driven tests cover codec,
dictionary, configuration, store/log, and session-state behavior.

## Run the gates

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p truefix-at --test conformance
```

Database integrations have feature-specific test commands and may require local services. See the
[acceptance record](acceptance-record.md) for the detailed historical mapping between requirements,
tests, and remediation releases.
