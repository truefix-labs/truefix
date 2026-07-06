# Contract: Session Protocol Correctness

**Requirements**: FR-002, FR-003, FR-005, FR-008, FR-011, FR-012, FR-016, FR-017 (US1);
FR-022, FR-026, FR-032, FR-034, FR-039, FR-040, FR-041, FR-049, FR-050 (US2)
**Research**: research.md §R1

## Gap-fill `SequenceReset` verification (FR-005, `NEW-84`)

**Contract**: A gap-fill `SequenceReset` is routed through the same `Ordering::Greater`/
`Ordering::Less` dispatch every other message type receives before its `NewSeqNo` is applied — a
too-high gap-fill is queued and answered with a `ResendRequest` (not applied immediately); a
too-low one is rejected. Only once the session's own sequence position matches is `NewSeqNo`
applied per the existing gap-fill logic.

**Protocol-behavioral**: yes. **AT scenario required** — a gap-fill `SequenceReset` arriving with a
too-high `MsgSeqNum` interleaved with other queued messages is the specific failure mode `NEW-84`
describes; needs a scenario proving no message loss occurs.

## `BeginSeqNo=0` resend handling (FR-003, `NEW-02`)

**Contract**: `on_resend_request` treats a `BeginSeqNo=0` the same as `BeginSeqNo=1` — answered
from the first stored message, not silently dropped.

**Protocol-behavioral**: yes. **AT scenario required** — a `ResendRequest` with `BeginSeqNo=0` must
receive a real resend/gap-fill response, not silence.

## Acceptor `ResetOnLogon` (FR-002, `NEW-03`)

**Contract**: An acceptor with `config.reset_on_logon = true` resets both sequence numbers to 1
upon receiving *any* Logon, independent of that Logon's own `ResetSeqNumFlag` value.

**Protocol-behavioral**: yes. **AT scenario required** — acceptor-side, `ResetOnLogon=Y`, inbound
Logon without `ResetSeqNumFlag=Y`, expect a reset anyway.

## Teardown reason routing (FR-008, `NEW-56`; see data-model.md's `DisconnectReason`)

**Contract**: `enter_disconnected` consults `reset_on_logout` only when the teardown followed a
completed graceful Logout exchange, and `reset_on_disconnect` for every other teardown cause
(TCP drop, any error-driven path). The two flags no longer both gate on the same `||` regardless of
cause.

**Protocol-behavioral**: yes — an operator-visible behavior difference depending on which flag is
set and how the session ends. **AT scenario required**: one scenario per flag combination
(`reset_on_logout` alone + graceful Logout; `reset_on_disconnect` alone + TCP drop) proving no
cross-leak.

## Schedule-exit `Logout` (FR-016, `NEW-62`)

**Contract**: A logged-on acceptor session whose schedule window closes sends a `Logout` before
disconnecting, not an abrupt drop.

**Protocol-behavioral**: yes. **AT scenario required** — needs a schedule-configured AT fixture (per
007's plan, prior schedule testing has been transport-integration-test-only; confirm AT runner
support for schedule-configured scenarios at `/speckit-tasks` time, falling back to a
`truefix-transport` integration test if the harness gap still exists).

## Dictionary-invalid Logon rejection path (FR-012, `NEW-63`)

**Contract**: A Logon failing dictionary validation is always routed through the Logout-and-
disconnect rejection path (`reject_logon`), regardless of `disconnect_on_error`.

**Protocol-behavioral**: yes. **AT scenario required** — `disconnect_on_error=false` with a
dictionary-invalid Logon must still produce Logout + disconnect, not a bare Reject leaving the
session stranded.

## UDF validation short-circuit ordering (FR-017, `NEW-17`)

**Contract**: With `validate_user_defined_fields=false`, an empty-valued or repeated UDF is never
rejected — the short-circuit runs before the empty-value/repeated-tag checks, not after.

**Protocol-behavioral**: yes, but narrow (validation-toggle behavior). Table-driven unit test at
the `truefix-dict::validate` level is the primary vehicle; an AT scenario is optional (confirm at
`/speckit-tasks` — likely covered by extending an existing UDF-validation scenario rather than
adding a new one).

## MSSQL/validation config wiring (FR-011, `NEW-10`; see data-model.md's key-registry changes)

**Contract**: `ValidateFieldsHaveValues`, `ValidateUnorderedGroupFields`,
`ValidateUserDefinedFields`, `AllowUnknownMsgFields`, `FirstFieldInGroupIsDelimiter` each have an
observable effect on validation outcome when set in `.cfg`; `ValidateSequenceNumbers`/
`RejectInvalidMessage` are downgraded to `Recognized` (accepted but not enforced) per Clarifications.

**Protocol-behavioral**: partially (the 5 wired keys are genuine validation-outcome changes).
Table-driven config-resolution unit tests suffice; no new AT scenario required (existing dictionary-
validation scenarios already exercise the underlying `ValidationOptions` fields once wired).

## Pre-logon admin message handling (FR-022, `NEW-18`)

**Contract**: A pre-logon `Logout`/`Reject`/`SequenceReset` with a too-high sequence number does not
trigger a `ResendRequest`.

**Protocol-behavioral**: yes. **AT scenario required** — extends the existing pre-logon-message
scenario category (`BUG-42`'s prior fix) to cover admin message types it didn't.

## Group decode/re-encode fidelity (FR-026, `NEW-22`)

**Contract**: A repeating group's declared `NoXxx` count is never silently mutated on re-encode to
match a differing actual entry count.

**Protocol-behavioral**: session/codec-internal round-trip fidelity — table-driven unit test in
`truefix-core::codec`, no AT scenario required.

## `reset_sequences` field clearing (FR-032, `NEW-32`)

**Contract**: An operational `reset()` mid-session clears `resend_target`, `resend_chunk_end`, and
`test_request_outstanding`, matching what a fresh connection (`on_connected`) already clears.

**Protocol-behavioral**: session-internal state hygiene — unit test only.

## `NewSeqNo=0`/missing `NewSeqNo` rejection (FR-034, FR-040, `NEW-34`, `NEW-70`)

**Contract**: A present `NewSeqNo=0` (non-gap-fill) is rejected as an invalid value; a gap-fill
`SequenceReset` missing `NewSeqNo` entirely is also rejected, not silently accepted with no update.

**Protocol-behavioral**: yes. **AT scenario required** — both are required-field/value violations
that currently produce silent no-ops instead of protocol-correct rejections.

## Acceptor `HeartBtInt` omission, no-dictionary case (FR-041, `NEW-71`)

**Contract**: With no dictionary configured, an acceptor rejects a Logon omitting `HeartBtInt`
rather than silently keeping its prior configured interval.

**Protocol-behavioral**: yes, narrow scope (only reachable with no dictionary attached — per the
audit's own third-pass downgrade). Integration test sufficient; AT scenario optional (confirm at
`/speckit-tasks` — likely skippable since the with-dictionary case is already covered).

## Too-high `Logout`/`ResendRequest` handling (FR-049, FR-050, `NEW-85`, `NEW-86`)

**Contract**: A `Logout` with a too-high `MsgSeqNum` after logon is processed immediately (reply,
honor reset flags, disconnect), not queued behind a `ResendRequest`. A too-high `ResendRequest`
already answered immediately is not reprocessed a second time once `drain_queue` reaches it in the
queue — only the sequence counter advances.

**Protocol-behavioral**: yes, both. **AT scenario required** — the hang (Logout case) and the
duplicate-resend-burst (ResendRequest case) are each independently observable wire behaviors.
