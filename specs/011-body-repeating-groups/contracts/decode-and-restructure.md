# Contract: `truefix-transport`/`truefix-session` — Receive-Path Group Structuring

Traces: spec.md User Stories 1 & 2, FR-003, FR-004, FR-008, FR-009, FR-010; research.md R2, R3.

## Single-Dictionary Sessions (`services.validator`)

1. `classify_buffered` decodes every inbound frame with `decode_with_groups(raw, dict)`, where `dict`
   is the session's actual `DataDictionary` — not a header/trailer-restricted wrapper.
2. Header, body, and trailer are all structured against `dict`'s full group definitions. A
   multi-entry body-level group (e.g. `NoMDEntries`) decodes to a `Member::Group` with every entry
   present, readable via `.group()`/`.get_group()`/`.members()` and via codegen-generated typed
   accessors.
3. `Message.fields_out_of_order` tracking (GAP-26/FR-032, feature 006) is preserved — unaffected by
   which sections get grouped.
4. No `DataDictionary` attached (`services.validator: None`, `services.fixt_dictionaries: None`):
   decoding is exactly today's flat `decode(raw)` — unchanged (edge case, spec.md).
5. Removing `HeaderTrailerGroupsOnly` must not add allocations to this hot path beyond what
   `decode_with_groups` already performs for header/trailer groups today (NEW-133, feature 006) —
   passing the dictionary directly is a strictly simpler call, not a new allocation source.

## FIXT 1.1 Dual-Dictionary Sessions (`services.fixt_dictionaries`)

1. `classify_buffered` decodes every inbound frame with `decode_with_groups(raw, dicts.transport())`
   — session-layer (admin) messages, and any message before further processing, are structured
   against the transport dictionary exactly as the single-dictionary case would be. This alone is
   already correct and sufficient for session-layer messages (e.g. `NoHops`) — no further step
   applies to them.
2. **Implementation-time correction to the call site (see research.md R3's "Implementation-time
   correction")**: the restructuring step does *not* run inside `Session::on_received`/the
   processing loop's `Event::Received` dispatch. `truefix-transport`'s `handle_inbound` calls
   `Application::from_admin`/`from_app` directly, before `dispatch`/`Session::handle` ever runs —
   there is no `Action::Deliver`; `Action` only ever carries `Send`/`Resend`/`Disconnect`/
   `ResetStore`. The actual, verified call site is `handle_inbound` itself: right after its
   existing `would_reject_before_processing` precheck and before the `is_admin(&msg)`/
   `from_admin`/`from_app` dispatch, it calls the now-`pub` `Session::restructure_fixt_application_body(&mut msg)`.
   That method resolves the message's `ApplVerID` using `validate_app`'s existing precedence (own
   header tag 1128, else `Session::negotiated_appl_ver_id`, else `FixtDictionaries`'s baked-in
   default), looks up `dicts.application_for(appl_ver_id)`, and if a dictionary is found,
   restructures the message's `body` in place via `truefix_core::restructure_groups`.
   `validate_app` (reached afterward, via `dispatch` → `Session::handle` →
   `on_received`) sees the same already-restructured `msg` for free, since `Event::Received(msg)`
   carries the same (by-then-mutated) message — no separate restructuring call is needed inside the
   state machine.
3. If no application dictionary resolves for a message's `ApplVerID` (edge case, spec.md), the
   message's body is left exactly as decoded in step 1 (transport-dictionary-scoped, i.e. effectively
   flat for body content the transport dictionary doesn't declare) — no restructuring, no error,
   consistent with `validate_app`'s existing "treat like the no-dictionary case" fallback for the same
   condition.
4. Messages whose `ApplVerID` can't be resolved (e.g. an explicit per-message tag 1128 naming an
   unregistered application version, taking precedence over an otherwise-valid negotiated default):
   the same fallback applies — the message is left transport-scoped, not rejected or misattributed.
5. When multiple `ApplVerID`s are registered, each message's restructuring step re-resolves
   independently per message (FR-009's "not a single cached choice") — no per-connection caching of
   "the" application dictionary.

## Test Expectations

- Extend `crates/truefix-transport/tests/header_trailer_groups_production.rs`-style coverage (new
  file `body_group_decode_production.rs`) for body-level group fixtures in the single-dictionary
  case, including a nested group.
- New `crates/truefix-transport/tests/fixt_application_group_restructure.rs` for the FIXT
  dual-dictionary case: acceptor and initiator sessions (Principle VI) each receiving an
  application-layer message with a body-level group under a negotiated `ApplVerID`, asserting full
  entry visibility; a companion case for an unresolvable per-message `ApplVerID` fallback, a
  session-layer-message-unaffected regression, and a multi-`ApplVerID`-independence case. (Note:
  this test file lives in `truefix-transport`, not `truefix-session` — the real acceptor/initiator
  networking primitives it needs, e.g. `Acceptor::bind_with`/`connect_initiator_with`, are only
  available there; `truefix-session` has no `tokio` dependency at all.)
