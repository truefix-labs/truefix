# Security and reliability audits

TrueFix treats protocol correctness, resource bounds, and recoverable failure as production safety
concerns. The workspace forbids `unsafe`, and critical non-test code denies `unwrap`, `expect`,
`panic`, and unchecked indexing through lint policy.

## Audit themes

Successive repository audits examined and remediated issues in:

- replay and duplicate-message handling, sequence resets, gap fills, and resend verification
- pre-logon authentication and malformed-input resource exhaustion
- session identity routing, dynamic acceptor isolation, and restart persistence
- orphaned task cleanup, deterministic startup, bounded channels, and file growth
- TLS trust defaults, mTLS configuration, proxy boundaries, and secret-safe diagnostics
- store atomicity, silent persistence failures, database URL parsing, and migration behavior
- dictionary, framing, checksum, timestamp, and FIXT validation strictness

The detailed finding inventories live in [`todo/`](todo/). Their completion marks describe the audit
in which they were resolved, not a perpetual third-party certification. The associated regression
evidence is summarized in the [acceptance record](acceptance-record.md) and
[conformance guide](conformance.md).

## Policy and verification

- [No-panic audit](no-panic-audit.md) documents the critical-path lint policy.
- `cargo clippy --workspace --all-targets -- -D warnings` enforces the configured lint gates.
- `cargo test --workspace` runs unit and integration coverage.
- `cargo test -p truefix-at --test conformance` runs the black-box FIX release gate.
- Feature-gated database and live-network tests require their documented services or explicit opt-in.

Security reports should avoid public disclosure of exploitable details until maintainers have had a
reasonable opportunity to investigate. Repository contact and reporting instructions should be used
when available.
