# Phase 1 Data Model: TrueFix

Conceptual entities and their fields/relationships/validation/lifecycle. Types are conceptual; concrete
Rust signatures live in `contracts/`. Crate ownership noted per entity.

## Message model (`truefix-core`)

### Field
- **Fields**: `tag: u32`, `value: FieldValue` (typed: Int, Float/Decimal, Char, Boolean, String, Data,
  UtcTimestamp, UtcTimeOnly, UtcDateOnly, MonthYear, …), plus raw textual form for round-trip fidelity.
- **Rules**: tag > 0; value parses per declared FIX type; illegal value → typed `FieldError`
  (never panic). Original bytes retained so re-encode is byte-identical (SC-002, FR-B9).

### FieldMap
- **Fields**: ordered collection of `Field` + nested `Group`s, preserving FIX field order within a region.
- **Relationships**: base of `Message` regions and of `Group` instances.
- **Rules**: ordering preserved; `ValidateFieldsOutOfOrder`/`ValidateUnorderedGroupFields` govern strict
  vs lenient ordering checks (FR-C3).

### Group (repeating, nestable)
- **Fields**: `count_tag: u32`, delimiter tag, ordered list of `FieldMap` entries (each entry may contain
  further nested groups).
- **Rules**: declared count matches entries (`14i`), first field is the delimiter
  (`FirstFieldInGroupIsDelimiter`), members ordered (`14j`); count=0 handled (`21`); missing nested
  delimiter detected (QFJ934).

### Message
- **Fields**: `header: FieldMap`, `body: FieldMap`, `trailer: FieldMap`; derived `msg_type`,
  `begin_string`.
- **Relationships**: built/parsed by codec; classified by `MessageFactory`; dispatched by `MessageCracker`.
- **Rules**: header fields in required order (`14g`, `15`, `2t`); BodyLength(9) & CheckSum(10) auto
  computed and verified (FR-B5); SOH-delimited (FR-B4); RawData length-prefixed handled (FR-B6).

### Codec
- **Inputs/Outputs**: bytes ⇄ `Message`. Computes/validates BodyLength & CheckSum.
- **Rules**: garbled/truncated input → typed `DecodeError` per `RejectGarbledMessage`; no panic on parse
  path (FR-B8, SC-005). Round-trip guarantee (FR-B9).

## Dictionary (`truefix-dict`)

### DataDictionary
- **Fields**: version id; maps of field defs (tag→type/enum/required), component defs, group defs,
  message defs (allowed/required fields & groups per msg type), header/trailer defs.
- **Relationships**: produced by the dictionary parser from the normalized source; consumed by the
  runtime validator and (via build.rs) by codegen.
- **Rules**: validation produces session-level Reject or Business Message Reject with spec-correct reason
  (FR-C4); supports UDF and custom/extended dictionaries (FR-C5). FIXT 1.1 splits Transport vs
  Application dictionaries; `DefaultApplVerID`/`ApplVerID` selects the application dictionary (FR-A3/A4).
- **Dual-track invariant**: codegen output and runtime model derive from the same normalized source
  (same version/hash) — they cannot diverge (Principle IV; research R4).

### Generated typed messages (build.rs output)
- **Fields**: per-version typed structs (e.g. `fix44::NewOrderSingle`) with typed accessors.
- **Rules**: generated solely from the normalized dictionary; no hand-copied definitions.

## Session (`truefix-session`)

### SessionID
- **Fields**: `begin_string`, `sender_comp_id`, `target_comp_id`, optional `sender_sub_id`,
  `target_sub_id`, `sender_location_id`, `target_location_id`, `session_qualifier`.
- **Rules**: identity for routing/lookup; uniqueness per engine; `CheckCompID` governs validation
  (`1c`, `2k`).

### Session
- **Fields**: `id: SessionID`, role (initiator/acceptor), `state`, `next_sender_seq`, `next_target_seq`,
  `heartbeat_interval`, schedule, store handle, log handle, settings, peer connection handle,
  resend/queue buffers.
- **State transitions**: Disconnected → LogonSent/LogonReceived → LoggedOn → LogoutSent/LogoutReceived →
  Disconnected; Reconnecting (initiator). Reset transitions zero sequence numbers and clear store
  (FR-D5, FR-E3).
- **Rules**: ResetSeqNumFlag(141), ResetOn{Logon,Logout,Disconnect}, RefreshOnLogon (FR-D4/D5);
  Heartbeat/TestRequest timing (FR-D6); ResendRequest + chunked resend + gap fill (FR-D7);
  SequenceReset GapFill/Reset (FR-D8); PossDup/PossResend/OrigSendingTime (FR-D9);
  NextExpectedMsgSeqNum 789 for FIX ≥ 4.4 (FR-D10); Logon/Logout timeouts + MaxLatency (FR-D11);
  high/low seq handling + recovery (FR-D12).

### Schedule
- **Fields**: start/end time, start/end day, weekdays, time zone, non-stop flag.
- **Rules**: in-session window computation; scheduled reset = disconnect+reset+clear+reconnect (FR-E1–E3).

### AdminMessage set
- **Entities**: Logon, Logout, Heartbeat, TestRequest, ResendRequest, SequenceReset, Reject,
  BusinessMessageReject — constructed/validated by the session per version.

## Store (`truefix-store`)

### MessageStore (trait + impls)
- **Operations (conceptual)**: get/set next sender & target seq; save/retrieve sent messages by range;
  reset; creation time.
- **Impls**: Memory, File, CachedFile, Sql(sqlx), Noop. Sleepycat deferred (FR-G1).
- **Rules**: persistence sufficient for resend across restart (FR-G2, SC-006);
  `ForceResendWhenCorruptedStore` recovery path.

## Log (`truefix-log`)

### Log (trait + impls)
- **Operations**: log incoming message, outgoing message, event, error event.
- **Impls**: Screen, File, Tracing, Composite (fan-out). Message vs event streams separated (FR-H2).

## Transport (`truefix-transport`)

### Connector (Initiator / Acceptor)
- **Initiator fields**: target host/port, reconnect interval, local bind, socket opts, TLS config.
- **Acceptor fields**: bind address/port, static sessions, dynamic-session template, allowed remote
  addresses, socket opts, TLS config.
- **Rules**: multi-session; dynamic sessions accept unconfigured peers via template (FR-F3); reconnect
  (FR-F4); socket options (FR-F5); TLS both roles (FR-F6); acceptor is first-class peer of initiator
  (Constitution VI).

## Config (`truefix-config`)

### SessionSettings
- **Fields**: default section + per-session sections keyed by SessionID; values keyed by Appendix A keys.
- **Rules**: defaults inherited, per-session overrides win (FR-I1); `${name}` interpolation (FR-I3);
  every Appendix A key recognized or documented unsupported-with-reason (FR-I2); programmatic builder.

## Application surface (`truefix`)

### Application (trait)
- **Callbacks**: `onCreate`, `onLogon`, `onLogout`, `toAdmin`, `fromAdmin`, `toApp`, `fromApp`
  (async — FR-F7, FR-J1).
### MessageCracker
- **Operation**: dispatch typed messages to per-type handlers with configurable fallback (FR-J2).

## Acceptance tests (`truefix-at`)

### AtScenario
- **Fields**: ordered steps (configure session, connect, send message, expect message, expect
  disconnect, comment), target FIX version(s).
- **Rules**: pass = sent-messages + disconnect behavior match expected steps exactly (US12, FR-M1);
  authored as independent black-box fixtures (Constitution III); all targeted versions gate release
  (FR-M3).
