# Data Model: Audit 006 Remediation

## AuditFinding

- **Fields**: `id` (`NEW-*`), severity, category, owning crate/domain, source evidence,
  disposition, verification artifact.
- **Validation rules**: retired or false-positive IDs from `docs/todo/006.md` are excluded unless
  current local verification finds a distinct defect.
- **Relationships**: maps to one or more RequirementGroups and TestEvidence entries.

## RequirementGroup

- **Fields**: requirement id (`FR-*`), priority tier, affected crate(s), behavior contract, linked
  findings.
- **Validation rules**: each implemented group must have at least one targeted test strategy.
- **Relationships**: contains one or more AuditFindings; produces one or more TestEvidence records.

## SessionIdentity

- **Fields**: FIX identity fields, resolved dynamic/static origin, persistence scope, active state.
- **Validation rules**: dynamic acceptor identities must not share mutable persistent state.
- **State transitions**: unresolved → resolved → active → logout/disconnect → inactive.

## SessionLifecycleEvent

- **Fields**: event type, role (acceptor/initiator), inbound/outbound sequence, reason,
  expected actions.
- **Validation rules**: logout, validation rejection, schedule exit, and shutdown paths must produce
  graceful or explicitly bounded behavior.
- **State transitions**: awaiting logon/logged on/awaiting logout/disconnected transitions remain
  explicit and testable.

## PersistentStoreState

- **Fields**: sender sequence, target sequence, message history, body-file/index state, corruption
  marker or repair state, duplicate detection state.
- **Validation rules**: runtime and durable sequence state must not diverge after reported failures;
  saved messages must remain discoverable after restart; duplicate detection must not change the
  core save contract.
- **Relationships**: belongs to a SessionIdentity; used by resend/recovery workflows.

## FileGrowthPolicy

- **Fields**: enabled flag, size threshold, rotation/limit mode, retention semantics,
  recovery constraints.
- **Validation rules**: file logs may rotate independently; message stores must not rotate or
  truncate data required for valid resend/recovery unless an explicit, tested policy preserves
  protocol semantics.

## OperatorConfiguration

- **Fields**: raw key/value, parsed value, default source, validation error, compatibility note.
- **Validation rules**: invalid booleans/TLS protocol values fail visibly; duplicate sessions fail;
  parity default changes are deterministic and documented.

## PublicApiSurface

- **Fields**: API name, owning crate, compatibility class, docs requirement, test requirement.
- **Validation rules**: API additions for handles, session state, `PossResend`, cancel-on-disconnect,
  duplicate detection, TLS scheduled initiator, and backlog must be documented and covered.

## TestEvidence

- **Fields**: finding id(s), test type, crate, scenario name, role coverage, backend feature gates.
- **Validation rules**: P1 implemented findings require failing-before/passing-after evidence;
  role-sensitive protocol changes need acceptor and initiator coverage where applicable.
