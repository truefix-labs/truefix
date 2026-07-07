# Contracts: Audit 006 Remediation

This directory defines the behavior contracts that `/speckit-tasks` should convert into tests and
implementation tasks.

- [session-lifecycle.md](./session-lifecycle.md): session protocol, logout/logon,
  `SequenceReset`, `PossResend`, cancel-on-disconnect.
- [transport-engine.md](./transport-engine.md): accept loops, TLS handshake timeout, bounded
  staging, shutdown, dynamic acceptor service factories, scheduled TLS initiator, backlog.
- [store-log.md](./store-log.md): store transactionality, file corruption recovery, duplicate
  detection, file growth limits, log timestamps/write errors.
- [config-defaults.md](./config-defaults.md): strict configuration parsing, duplicate sessions,
  default parity changes, socket option diagnostics.
- [dictionary-codec-api.md](./dictionary-codec-api.md): field parsing, checksum, dictionary
  validation, section-order policy, nested groups, required header/trailer fields.

## AT Scenario Guidance

Protocol-facing items should gain AT coverage when they represent externally visible FIX session
behavior: logout/logon handling, sequence reset behavior, schedule-driven logout, `PossResend`,
and validation rejection. Store/log/config-only items are generally unit or integration tests unless
they affect peer-visible FIX messages.

## Public API Compatibility Placeholders

- `truefix-session`: document additive application-facing `PossResend` and cancel-on-disconnect
  surfaces before marking their implementation complete.
- `truefix-transport`: document additive handle/session identity, TLS scheduled initiator, backlog,
  and shutdown behavior changes before marking transport tasks complete.
- `truefix`: document engine-level session query, acceptor handle, skipped-session, and shutdown
  semantics before marking engine tasks complete.
- `truefix-store`: document duplicate detection and file growth policy surfaces before marking store
  tasks complete.
- `truefix-log`: document file growth policy and backend option threading before marking log tasks
  complete.
