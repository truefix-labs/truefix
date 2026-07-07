# Research: Audit 006 Remediation

## Decision: Keep Remediation In Existing Crates

**Decision**: Implement all fixes inside existing crate ownership boundaries.

**Rationale**: The findings are defects or parity gaps in already-owned surfaces: session state in
`truefix-session`, I/O lifecycle in `truefix-transport`, engine management in `truefix`, stores in
`truefix-store`, logging in `truefix-log`, parsing/validation in `truefix-core` and `truefix-dict`,
settings in `truefix-config`, and AT coverage in `truefix-at`.

**Alternatives considered**: A new orchestration crate was rejected because it would blur ownership
and make state-machine/store fixes harder to test at the source of the behavior.

## Decision: Dynamic Acceptor Persistence Uses Per-Identity Services

**Decision**: `NEW-148` is fixed with a per-dynamic-identity store/services factory.

**Rationale**: The clarified requirement preserves dynamic persistent acceptor support while
removing cross-session sequence/history contamination. Rejecting persistent dynamic templates would
avoid corruption but leave an important parity gap.

**Alternatives considered**: Reject persistent dynamic templates; support only non-persistent
dynamic sessions. Both were rejected during clarification.

## Decision: Implement `CancelOrdersOnDisconnect` and `PossResend(97)`

**Decision**: `NEW-105` and `NEW-106` are implementation scope, not deferred documentation.

**Rationale**: The feature is now explicitly parity-completion oriented. `PossResend` must become
observable to applications for deduplication. Cancel-on-disconnect needs an operator-visible,
testable mechanism that does not pretend the session can synthesize business cancels without an
application-provided open-order source.

**Alternatives considered**: Deferring both as parity gaps, or implementing only `PossResend`. Both
were rejected during clarification.

## Decision: Built-In File Growth Bound

**Decision**: `NEW-108` is implemented with configurable size-limit or rotation behavior for file
logs/stores.

**Rationale**: The user chose built-in behavior rather than documentation-only guidance. The design
must preserve resend/recovery semantics for message stores, so tasks must distinguish simple log
rotation from store body retention/compaction rules.

**Alternatives considered**: Documentation-only external rotation; document now and create a
follow-up feature. Both were rejected during clarification.

## Decision: Duplicate Detection Is Separate From `save()`

**Decision**: `NEW-137` adds a separate duplicate-detection capability such as `contains(seq)` or
equivalent while preserving the existing core `save()` contract.

**Rationale**: This avoids a broad breaking trait change while giving callers and tests a way to
observe duplicate sequence use.

**Alternatives considered**: Changing `MessageStore::save()` to return an overwrite flag. Rejected
to reduce public API churn across all backends.

## Decision: Section Order Enforced In Validation

**Decision**: `NEW-146` remains validation-layer behavior controlled by section-order policy.

**Rationale**: Decode can retain diagnostics and permit callers to inspect malformed messages, while
runtime dictionary validation performs the reject decision consistently with
`ValidateFieldsOutOfOrder`.

**Alternatives considered**: Decode-time rejection or double enforcement. Rejected because it would
weaken diagnostics and duplicate policy.

## Decision: TLS Scheduled Initiator And Backlog Are In Scope

**Decision**: Implement both `NEW-134` TLS scheduled initiator support and `NEW-135` configurable
listener backlog.

**Rationale**: Both are concrete transport parity gaps with clear acceptance tests and no remaining
scope ambiguity after clarification.

**Alternatives considered**: Defer TLS scheduled initiator or defer both as parity gaps. Rejected.

## Dependency Review

**Decision**: Avoid new dependencies during planning.

**Rationale**: Existing workspace dependencies cover async I/O, TLS, persistence, logging, metrics,
and parsing. Any task proposing a new helper crate must include license compatibility, maintenance,
and necessity rationale before implementation.

**Alternatives considered**: Pulling in a log-rotation crate. Rejected at planning time because file
log/store behavior has project-specific resend/recovery constraints.

## Audit Disposition Table

| Finding(s) | Domain | Disposition | Evidence target | Status |
|------------|--------|-------------|-----------------|--------|
| NEW-97, NEW-98, NEW-100, NEW-120, NEW-139, NEW-142 | Session lifecycle | Implement | `crates/truefix-session/tests/audit006_session_lifecycle.rs` | Implemented, passing |
| NEW-99, NEW-119, NEW-140 | SequenceReset | Implement | `crates/truefix-session/tests/audit006_sequence_reset.rs` | Implemented, passing |
| NEW-105, NEW-106 | Application parity | Implement | `crates/truefix-session/tests/audit006_application_parity.rs` | Implemented, passing |
| NEW-148, NEW-149, NEW-150, NEW-151, NEW-153 | Engine/transport lifecycle | Implement | `crates/truefix/tests/audit006_engine_lifecycle.rs`, `crates/truefix-transport/tests/audit006_backpressure.rs` | Implemented, passing |
| NEW-152, NEW-154 | FileStore P1 durability | Implement | `crates/truefix-store/tests/audit006_corrupt_tail.rs`, `crates/truefix-store/tests/audit006_seqfile_atomicity.rs` | Implemented, passing |
| NEW-102, NEW-103, NEW-104, NEW-111, NEW-112, NEW-113, NEW-128, NEW-129, NEW-130 | Config/defaults | Implement | `crates/truefix-config/tests/audit006_config_defaults.rs`, `crates/truefix-config/tests/audit006_config_parser.rs` | Implemented, passing. NEW-103's corrected `TestRequestDelayMultiplier` default required updating two `truefix-at` acceptance scenarios (`0_IdleHeartbeatEmitted`, `4_TestRequestOnSilence`) that had encoded the old message ordering — see quickstart.md's Validation Results. |
| NEW-115, NEW-121, NEW-122, NEW-123, NEW-132, NEW-134, NEW-135, NEW-143 | Transport/engine operations | Implement | `crates/truefix-transport/tests/audit006_transport_options.rs`, `crates/truefix/tests/audit006_engine_api.rs` | Implemented, passing |
| NEW-116, NEW-117, NEW-118, NEW-136 | Store operational durability | Implement | `crates/truefix-store/tests/audit006_store_reset.rs`, `crates/truefix-store/tests/audit006_file_store_safety.rs` | Implemented, passing |
| NEW-124, NEW-125, NEW-126 | Logging | Implement | `crates/truefix-log/tests/audit006_file_log.rs` | Implemented, passing |
| NEW-127, NEW-145 | Dictionary required/version validation | Implement | `crates/truefix-dict/tests/audit006_required_version.rs` | Implemented, passing |
| NEW-101, NEW-107, NEW-155 | Core strictness | Implement | `crates/truefix-core/tests/audit006_codec_strictness.rs` | Implemented, passing |
| NEW-108, NEW-137, NEW-138 | File growth and diagnostics | Implement | `crates/truefix-log/tests/audit006_file_growth.rs`, `crates/truefix-store/tests/audit006_growth_duplicate.rs`, `crates/truefix-core/tests/audit006_field_diagnostics.rs` | Implemented, passing |
| NEW-133, NEW-146, NEW-147 | Transport/dictionary hardening | Implement | `crates/truefix-transport/tests/audit006_buffering.rs`, `crates/truefix-dict/tests/audit006_validation.rs` | Implemented, passing. NEW-133's slice-based `classify_buffered` required a `buf.get(..total).unwrap_or(&[])` fallback (not raw indexing) to satisfy `truefix-transport`'s `clippy::indexing_slicing` deny gate — see quickstart.md's Validation Results. |
| NEW-109, NEW-110, NEW-114, NEW-131, NEW-141, NEW-144 | Retired/false positive | Excluded | `docs/todo/006.md` retirement note | Excluded, as planned |
