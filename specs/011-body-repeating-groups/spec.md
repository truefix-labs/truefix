# Feature Specification: Body-Level Repeating Group Decoding & Vendor Dictionary Support

**Feature Branch**: `011-body-repeating-groups`

**Created**: 2026-07-07

**Status**: Draft

**Input**: User description: "外部项目 `binance-fix-client`（基于 truefix 引擎实现 Binance FIX API）审查
`truefix-core`/`truefix-dict`/`truefix-transport` 源码后提交的架构改进建议：body-level repeating
group 在接收路径上永远不会被结构化解析（不论是否挂 DataDictionary），且 `truefix_dict` 无法直接读取
vendor（如 Binance）发布的 FIX 字典 XML。建议给出四步修复方案（`FieldMap::members()`公开接口 /
`classify_buffered`去除 body 范围限制 / 重写 `validate()`与`validate_group(s)` / 新增 vendor XML 字
典解析器），并要求评估后采纳。"

## Clarifications

### Session 2026-07-07

- Q: The proposal bundles (A) `FieldMap::members()`, (B) removing `classify_buffered`'s
  header/trailer-only restriction, (C) rewriting `validate()`/`validate_group(s)`, and (D) a new
  vendor-XML dictionary parser. (D) has real license/provenance judgment calls (Constitution
  Principle III) — the project previously *removed* a module (`qfj_xml`) that converted
  vendor/QuickFIX-shaped XML for exactly this reason, even though this proposal's target files are
  Binance's own, independently-licensed files, not QuickFIX's private spec files. Should this
  feature cover the full proposal or just the core group-structuring fix (A/B/C)? → A: Full
  proposal (A/B/C/D) — vendor-XML dictionary ingestion is in scope.
- Q: The proposal explicitly flags FIXT 1.1 dual-dictionary body-group attribution (choosing the
  session vs. application dictionary by `ApplVerID` during `classify_buffered`) as unsolved extra
  complexity that "shouldn't block the main path." Should this feature also solve it? → A: In
  scope — FIXT 1.1 sessions must also get structured body groups, not just single-dictionary
  sessions.

## User Scenarios & Testing *(mandatory)*

TrueFix is a FIX protocol engine library. Its "users" for this feature are engineers who embed
TrueFix in a FIX counterparty integration (session-layer code, message handlers, codegen-generated
message accessors) and configure it with a `DataDictionary`. Today, any inbound message whose body
contains a repeating group (e.g., `NoMDEntries`, `NoOrders`, `NoMiscFees`, `NoPartyIDs`) has that
group's structure silently discarded on the receive path — regardless of whether a dictionary is
attached — so both hand-written code reading `FieldMap::group()`/`get_group()` and codegen-generated
typed accessors that read the same structured interface get an empty/degenerate result with no
error. This feature makes body-level repeating groups first-class on the receive path, for both
single-dictionary and FIXT 1.1 dual-dictionary sessions, and lets a runtime dictionary be loaded
directly from a vendor-published FIX dictionary XML file (e.g., Binance's `spot-fix-oe.xml`/
`spot-fix-md.xml`) instead of requiring a hand-authored translation first.

### User Story 1 - Structured body-level repeating groups on receive, single dictionary (Priority: P1)

An engineer has a session configured with one runtime `DataDictionary` (the common, non-FIXT case).
When a counterparty sends a message whose body contains a repeating group, the engineer needs every
entry of that group to be visible through the engine's structured group accessors (both the
low-level `FieldMap` group API and codegen-generated typed accessors) — not just the first entry,
and not silently dropped.

**Why this priority**: This is the core defect the proposal identifies: it is a correctness bug with
no error signal (silent data loss), it affects every counterparty whose FIX dictionary uses standard
or custom body-level groups, and every other part of the proposal (FIXT support, vendor dictionaries)
only matters once this base case is fixed.

**Independent Test**: Can be fully tested by feeding a decoded raw FIX message containing a
multi-entry body-level repeating group (using an existing bundled dictionary, e.g. FIX44's
`NoMDEntries` or a custom test dictionary's group) through the receive path with a single
`DataDictionary` attached, and asserting that all entries are readable via both the structured
`FieldMap` group accessor and (where applicable) the corresponding codegen-generated accessor.

**Acceptance Scenarios**:

1. **Given** a session with one `DataDictionary` attached that declares a body-level repeating
   group, **When** a multi-entry instance of that group is received, **Then** the message's
   structured group accessor returns every entry, in wire order, with each entry's member fields
   intact.
2. **Given** the same setup, **When** the message is read via the flat, group-skipping field
   accessor that existing code already relies on, **Then** that accessor's behavior is unchanged
   (still returns the flat, group-skipping view, still contains every raw tag).
3. **Given** a codegen-generated message type with a typed body-group accessor method, **When** a
   real multi-entry instance of that group is received and read through the generated accessor,
   **Then** the accessor returns every entry (not an empty list).
4. **Given** the same dictionary-attached session, **When** a message with a nested group (a group
   whose entries themselves contain another repeating group) is received, **Then** both levels of
   nesting are structured correctly and all entries at both levels are readable.
5. **Given** dictionary validation runs against a message with a body-level repeating group,
   **When** the group's entries are missing a required member field, have the wrong member order,
   or their count disagrees with the declared count, **Then** validation reports the same class of
   failure it already reports for header/trailer groups today.
6. **Given** a previously-passing validation test suite (well-formed messages with body-level
   groups), **When** it is re-run after this change, **Then** every previously-passing case still
   passes with an equivalent accept/reject outcome.

---

### User Story 2 - Structured body-level repeating groups under FIXT 1.1 dual dictionaries (Priority: P2)

An engineer runs a FIXT 1.1 session where session-layer messages are governed by the transport
dictionary and application-layer messages are governed by an application dictionary selected by
`ApplVerID`. The engineer needs body-level repeating groups in application-layer messages to be
structured using the correct application dictionary for that message, not left flat, and without
regressing existing session-layer (transport-dictionary) group handling.

**Why this priority**: This closes the gap the base fix (User Story 1) does not reach — FIXT 1.1
deployments are a real, named configuration in the engine, and leaving them flat would mean the same
silent-data-loss defect persists for exactly the counterparties (multi-version FIX gateways) most
likely to need it. It depends on User Story 1's plumbing, so it is P2.

**Independent Test**: Can be fully tested by configuring a session with `fixt_dictionaries` (a
transport dictionary plus one or more `ApplVerID`-keyed application dictionaries), sending an
application-layer message with a body-level repeating group under a given `ApplVerID`, and asserting
the group is structured using that `ApplVerID`'s application dictionary — independent of whatever
vendor-XML work User Story 3 adds.

**Acceptance Scenarios**:

1. **Given** a FIXT 1.1 session with a transport dictionary and one application dictionary
   registered for a negotiated `ApplVerID`, **When** an application-layer message with a body-level
   repeating group is received after that `ApplVerID` is known, **Then** the group is structured
   using the application dictionary's group definition, and its entries are fully readable.
2. **Given** the same session, **When** a session-layer message (e.g., one carrying `NoHops`) is
   received, **Then** it continues to be structured using the transport dictionary exactly as
   before (no regression to existing header/trailer group handling).
3. **Given** a FIXT 1.1 session where the `ApplVerID` for an inbound message cannot yet be resolved
   (e.g., prior to completing Logon negotiation), **When** a message is received in that window,
   **Then** the engine falls back to transport-dictionary-scoped structuring for that message rather
   than failing or misattributing it to the wrong dictionary.
4. **Given** multiple registered application dictionaries for different `ApplVerID`s, **When**
   messages under different `ApplVerID`s arrive in the same session, **Then** each message's body
   groups are structured using its own `ApplVerID`'s dictionary, not a single cached choice.

---

### User Story 3 - Load a vendor-published FIX dictionary XML directly (Priority: P3)

An engineer integrating with a counterparty (e.g., Binance) that publishes its FIX dictionary as a
vendor-shaped XML file (nested `<fix><header>/<messages>/<trailer>/<components>/<fields>`
structure, as opposed to the engine's own native dictionary text format) needs to point the engine's
`DataDictionary` configuration directly at that published file — including counterparties' custom,
multi-character message types — and get a working, validating, group-structuring dictionary without
first hand-translating the file.

**Why this priority**: This makes User Story 1/2's fix actually reachable end-to-end for
counterparties that only publish this XML shape; without it, engineers must still hand-author a
translation (today's status quo), which is exactly the manual step the proposal wants removed. It is
P3 because User Stories 1 and 2 are the correctness-critical fix and deliver value even for
engineers who already have (or are willing to hand-write) a compatible dictionary.

**Independent Test**: Can be fully tested by pointing the engine's dictionary loader at a
self-contained vendor-XML fixture file (fields, a message with a body-level group, a component
reference, and a multi-character custom message type) and asserting the loaded dictionary validates
and structures a matching sample message correctly — independent of whether the message came from a
live counterparty.

**Acceptance Scenarios**:

1. **Given** a vendor-published dictionary XML file declaring fields, messages, a body-level
   repeating group, and component references, **When** it is loaded as a runtime `DataDictionary`,
   **Then** the resulting dictionary validates a matching sample message and structures its groups
   correctly, equivalent to an engine-native dictionary describing the same protocol.
2. **Given** a vendor XML file declaring a custom message type with a multi-character `MsgType`
   value, **When** the dictionary is loaded and a message of that type is received, **Then** the
   message is recognized as a known, valid message type rather than rejected as unknown.
3. **Given** a vendor XML file whose messages reference reusable `<component>` blocks or contain
   nested `<group>` definitions, **When** the dictionary is loaded, **Then** those references resolve
   to the same field/group structure as if they had been inlined directly.
4. **Given** a file that is a vendor-shaped XML dictionary (not the engine's native format), **When**
   it is handed to the dictionary loader without any extra format flag, **Then** the loader detects
   the format from the file's content and loads it correctly.
5. **Given** an engineer wants to inspect or version-control the translated form of a vendor XML
   dictionary, **When** they run the engine's dictionary conversion tooling against that file,
   **Then** they get the engine's native dictionary text as output, usable anywhere a native
   dictionary file is accepted today.
6. **Given** a vendor XML file that is malformed or uses a structure the parser cannot interpret
   (e.g., missing required attributes), **When** it is loaded, **Then** the engine reports a clear
   parse error rather than silently producing an incomplete or incorrect dictionary.

---

### Edge Cases

- What happens when a body-level group's declared entry count (the `NoXxx` count-tag value) does
  not match the number of entries actually present on the wire? (Existing declared-vs-actual-count
  tracking behavior must extend to body groups, not just today's header/trailer groups.)
- What happens when a repeating group is nested more than one level deep (a group of groups)?
- What happens when no `DataDictionary` is attached at all? (Behavior must remain exactly today's
  flat decode — this feature must not require a dictionary to be present.)
- What happens when a body-level group tag exists on the wire but is not declared as a group by the
  attached dictionary (e.g., a custom/unknown tag)? (Must not be misinterpreted as a group; falls
  back to being treated as a plain field, as today.)
- What happens to a codegen-generated body-group accessor for a message type whose dictionary has no
  `DataDictionary` attached at runtime? (Documented existing behavior — empty result — must not
  become a panic or a new class of silent failure.)
- What happens when a FIXT 1.1 message's `MsgType` is recognized by the transport dictionary but its
  `ApplVerID` has no registered application dictionary?
- What happens when a vendor XML dictionary and the engine's native dictionary format for the same
  protocol version disagree in some field/group definition? (Out of scope to reconcile — each is
  loaded and used independently; this is a data-entry concern for whoever authors the dictionary,
  not an engine behavior to detect.)
- What happens when a vendor XML file's component reference points to a component name that isn't
  defined anywhere in the file?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The engine MUST provide a public, structured iteration interface over a decoded
  message's header, body, and trailer that yields both plain fields and repeating groups (with
  their per-entry structure), in wire order.
- **FR-002**: The existing flat, group-skipping field iteration interface MUST remain available and
  behaviorally unchanged for existing callers after this feature ships.
- **FR-003**: When a session has exactly one runtime `DataDictionary` attached, every inbound
  message's header, body, and trailer MUST be structured against that dictionary's full group
  definitions — not limited to header/trailer groups as today.
- **FR-004**: Structuring a message's groups on receive MUST NOT alter the raw wire bytes, checksum
  handling, or the set/order of tags visible through the flat field-iteration interface (FR-002).
- **FR-005**: Dictionary validation (required-field, count, member-order, and unknown-field checks)
  MUST evaluate the actual contents of body-level repeating groups, not only header/trailer groups.
- **FR-006**: For any message that passes validation today, validation behavior after this feature
  MUST produce an equivalent accept/reject outcome (no new false rejects or false accepts introduced
  by the validation rewrite).
- **FR-007**: A codegen-generated typed accessor for a body-level repeating group MUST return every
  entry present in a decoded inbound message when a matching `DataDictionary` is attached at
  runtime.
- **FR-008**: Under a FIXT 1.1 session configured with a transport dictionary and one or more
  `ApplVerID`-keyed application dictionaries, an application-layer inbound message's body-level
  groups MUST be structured using the application dictionary resolved for that message's
  `ApplVerID`.
- **FR-009**: Under the same FIXT 1.1 configuration, session-layer inbound messages MUST continue to
  be structured using the transport dictionary, unaffected by application dictionary selection.
- **FR-010**: When a FIXT 1.1 inbound message's `ApplVerID` cannot yet be resolved to a registered
  application dictionary, the engine MUST fall back to transport-dictionary-scoped structuring for
  that message rather than failing the connection or misattributing the message to an unrelated
  dictionary.
- **FR-011**: The engine MUST be able to load a vendor-published FIX dictionary XML file (using the
  nested `<fix><header>/<messages>/<message>/<trailer>/<components>/<fields>` shape, as used by
  Binance's published `spot-fix-oe.xml`/`spot-fix-md.xml`) as a working runtime `DataDictionary`,
  without requiring the caller to pre-translate it into the engine's native dictionary format.
- **FR-012**: Vendor XML dictionary loading MUST support multi-character `MsgType` values (e.g.,
  two- or three-letter custom message types) as first-class message-type identifiers.
- **FR-013**: Vendor XML dictionary loading MUST resolve `<component>` references and nested
  `<group>` definitions (including groups referencing components, and groups nested within groups)
  to the same effective field/group structure as if they had been written out directly.
- **FR-014**: The engine's runtime dictionary file loader MUST detect, from file content, whether a
  given file is a vendor-shaped XML dictionary or the engine's native dictionary format, and load it
  accordingly without requiring a separate format flag at runtime load time.
- **FR-015**: The engine's dictionary-generation command-line tooling MUST offer a way to convert a
  vendor XML dictionary file into the engine's native dictionary text format, alongside its existing
  format converters, so the converted output can be inspected, version-controlled, or fed into
  existing dictionary-based codegen/validation tooling.
- **FR-016**: When a vendor XML dictionary file is malformed or uses a structure the parser does not
  recognize, loading it MUST produce a clear parse error rather than a silently incomplete or
  incorrect dictionary.
- **FR-017**: The vendor XML dictionary parser MUST be implemented independently against the
  published XML shape and the public FIX protocol specification — it MUST NOT copy or mechanically
  translate any third-party FIX engine's bundled private specification files, consistent with the
  project's existing source-provenance rule already applied to its other dictionary format
  converters.

### Key Entities

- **Structured Repeating Group (receive-side)**: The decoded, wire-order-preserving representation
  of a `NoXxx`-style repeating group inside a received message's header, body, or trailer — its
  count tag, its ordered list of entries, and (per existing behavior) whether the declared count
  matched the actual entry count. This feature extends where this representation applies (body, not
  just header/trailer) rather than introducing a new concept.
- **Runtime Dictionary Source**: The on-disk representation of a FIX dictionary usable to configure
  a session — either the engine's native dictionary text format or a vendor-published XML file:
  same resulting in-memory dictionary capabilities (validation + group structuring) regardless of
  which source format was used.
- **Application Dictionary Selection (FIXT 1.1)**: The mapping used at receive time from an inbound
  application-layer message's `ApplVerID` to the application dictionary that governs how its body is
  structured and validated, distinct from the single transport dictionary that governs session-layer
  messages.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For any inbound message containing a body-level repeating group with more than one
  entry, 100% of entries are retrievable through the engine's structured group accessors (both the
  low-level group API and codegen-generated typed accessors), with zero entries silently dropped.
- **SC-002**: 100% of an existing pre-feature validation/acceptance test suite's previously-passing
  cases still pass with equivalent accept/reject outcomes after this feature ships.
- **SC-003**: An engineer can point the engine's dictionary configuration directly at an unmodified,
  vendor-published dictionary XML file and get a fully working (validating, group-structuring)
  dictionary with zero manual translation steps.
- **SC-004**: In a FIXT 1.1 session exercising both session-layer and multiple negotiated
  application-layer message types, every message's body-level groups are structured using the
  correct dictionary for that message, with zero entries silently dropped, matching the
  single-dictionary case's correctness (SC-001).
- **SC-005**: Every malformed or unrecognized vendor XML dictionary file used in testing produces an
  explicit load error rather than a dictionary that silently loads with missing or incorrect
  message/field/group definitions.

## Assumptions

- Single-dictionary (non-FIXT) sessions are the primary and default configuration; the FIXT 1.1
  dual-dictionary case (User Story 2) is additive on top of the same underlying structuring
  mechanism, not a separate implementation.
- "Vendor XML" scope is the nested `<fix>...</fix>` shape as published by Binance and
  similarly-shaped third-party dictionaries — this feature is not a commitment to parse every FIX
  dictionary XML dialect that exists industry-wide.
- Before a FIXT 1.1 session's `ApplVerID` is known for a given message (e.g., prior to completing
  Logon negotiation), falling back to transport-dictionary-scoped structuring is acceptable
  behavior, since there is no application dictionary yet to resolve against.
- The existing flat, group-skipping field-iteration interface is relied upon by existing callers and
  must remain available and unchanged; this feature is additive at that API surface even though the
  receive-path decode and validation behavior underneath it changes.
- Vendor XML dictionary parsing reuses the project's existing build/tooling-time XML-parsing
  dependency and licensing posture already established for its other XML dictionary converters,
  rather than introducing a new dependency family with different license terms.
- Reconciling disagreements between a vendor XML dictionary and the engine's native dictionary for
  the "same" protocol version is out of scope — each loaded dictionary is used independently as
  authored.
