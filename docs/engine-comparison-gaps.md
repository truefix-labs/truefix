# TrueFix vs QuickFIX/J vs QuickFIX/Go — Verified Gap List (2026-07-02)

> Supersedes the untracked scratch files `docs/codec-dict-comparison.md` and
> `docs/todo-gap-analysis-2026070217.md` (both deleted; their still-valid content is folded in here).
> Every item below was verified against the **current source code on `main`** (post feature 004 —
> `.cfg` dictionary/failover/SQL-backend wiring, `ContinueInitializationOnError`, `RedbStore`/`RedbLog`,
> `MongoStore`/`MongoLog`), not against git history. Citations are `file:line` against:
> - **TF** = this repo, `crates/`
> - **QFJ** = `thrdpty/quickfixj` (`quickfixj-core/src/main/java/quickfix/…`)
> - **QFGo** = `thrdpty/quickfix` (`*.go`)
>
> Filtered out before this list was compiled: items with an equivalent TrueFix mechanism under a
> different name (e.g. `is_admin_type` ≈ QFJ's `messageCategory`), items inappropriate for a Rust
> codebase (JVM reflection, JMX, OSGi, SLF4J categories — already covered in
> `docs/todo-gap-analysis.md`'s "不适合 Rust" section, not repeated here), and TrueFix's own unique
> advantages (dual-track dictionary hash, unconditional checksum verification, two-layer session/business
> reject, `extend()` dictionary merge with conflict detection, `rust_decimal` numeric consistency,
> component cycle detection, default-no-op generated cracker) — these are strengths, not TODOs.
> GAP-01 through GAP-06 (the 2026-07-02-morning gap list) are **closed** by feature 004 and are not
> repeated; see `docs/todo-gap-analysis.md`. One caveat found during this pass: GAP-05's "`.cfg`-only
> SQL backend selection via `JdbcURL`" only recognizes TrueFix's own sqlx-native URL scheme, not the
> actual JDBC URL format real QuickFIX/J `.cfg` files use — see **BUG-04** below.
>
> **Update (005, 2026-07-03)**: every P0 item (`BUG-01`–`BUG-04`, `GAP-07`, `GAP-08`, `GAP-09`,
> `GAP-18a`) and the entire P1 codec/dictionary cluster (`GAP-22`–`GAP-29`, `GAP-32`, `GAP-33`) plus
> `GAP-14`/`15`/`16`/`17`/`38`/`39`/`41`/`47` and the four stance-registry documentation-accuracy notes
> are now **closed** — see `specs/005-engine-gap-remediation/` (spec FR-001–FR-031, tasks.md T001–T099).
> Struck through below with an inline closure note; anything not struck through remains open.

---

## P0 — Real bugs

### ~~BUG-01~~: `#` inside a config value is truncated, not just whole-line comments — **closed (005, FR-001, T003)**

`crates/truefix-config/src/lib.rs:109` calls `strip_comment(raw)` (`lib.rs:177-182`) **before** the
`key=value` split (`lib.rs:121`). `strip_comment` finds the *first* `#` anywhere on the line and drops
everything after it — so `Password=ab#cd` silently becomes `Password=ab`.

Verified against both references — neither truncates mid-value:
- **QFJ** (`SessionSettings.java`'s hand-rolled tokenizer): a value token is read via
  `isValueCharacter` (`:650-652`, `!isEndOfStream && !isNewLineCharacter` — `#` is a value character),
  so `#` only starts a comment when a new *label* token is expected (`:630-634`), never mid-value.
- **QFGo** (`settings.go:100,113`): `commentRegEx = ^#.*` only matches a line that **starts** with `#`;
  the actual line grammar `settingRegEx = ^([^=]*)=(.*)$` (`:101`) captures everything after the first
  `=` verbatim, `#` included.

**Fix**: only treat `#` as a comment start when it's the first non-whitespace character of the line
(matching both references), not anywhere after a `=`.

**File**: `crates/truefix-config/src/lib.rs` `strip_comment()`

### ~~BUG-02~~: `Signature` (tag 89) length-field mapping missing — `SignatureLength` (93) exception not handled — **closed (005, FR-002, T004)**

`data_field_for_length()` (`crates/truefix-core/src/tags.rs:66-81`) has 12 length→data pairs but is
missing tag 93 → 89. The convention every other pair follows is `lengthTag = dataTag - 1`; tag 89 is the
one documented exception (its length field is 93, not 88).

QFJ special-cases exactly this (`Message.java:945-955`):
```java
int lengthField = tag - 1;
if (tag == 89) { lengthField = 93; }
```

Because `data_field_for_length` is consulted unconditionally at decode time
(`crates/truefix-core/src/codec/decode.rs:230`, independent of which dictionary is loaded), a message
containing a `Signature` value with an embedded SOH byte would be mis-tokenized by TrueFix's decoder —
a real (if rarely triggered) protocol-correctness edge case, not merely a missing dictionary entry.

**File**: `crates/truefix-core/src/tags.rs` `data_field_for_length()`

### ~~BUG-03~~: Three `.cfg` keys marked `Implemented` are actually unreachable from `.cfg` — **closed (005, FR-005/006/006a, T005–T010)**

`AllowedRemoteAddresses` / `DynamicSession` / `AcceptorTemplate` are all `Impl` in
`crates/truefix-config/src/keys.rs:133-135`, implying `Engine::start` honors them. It doesn't:
`Engine::start` (`crates/truefix/src/lib.rs:297`) only ever calls `Acceptor::bind_with` — a
**single-session** bind. `AcceptorBuilder` (`crates/truefix-transport/src/lib.rs`), the type that
actually implements multi-session routing, dynamic templates, and the allow-list, is referenced only
from tests and `crates/truefix/examples/multi_acceptor.rs` — never from the `.cfg`-driven path. There
is no field on `ResolvedSession` (`crates/truefix-config/src/builder.rs:28-63`) for any of the three,
and `builder.rs` never reads these three key names at all.

This is worse than a `Recognized`-should-be-`Implemented` mismatch (the direction every other stance
bug in this list runs) — it's the registry **overstating** coverage, which is exactly what the
Appendix A key-stance contract (Constitution: no key silently misrepresented) exists to prevent.

**Fix options**: either wire `Engine::start` to build multi-session acceptors via `AcceptorBuilder` when
these keys are present (real fix), or (interim, cheap) downgrade the three keys to `Recognized` with a
comment matching this finding, so the registry stops asserting something untrue.

**File**: `crates/truefix-config/src/keys.rs:133-135`, `crates/truefix/src/lib.rs:297`

### ~~BUG-04~~: `JdbcURL` scheme dispatch doesn't recognize the URL format real QuickFIX/J `.cfg` files use — **closed (005, FR-003/004, T011–T012)**

TrueFix's `is_sql_scheme`/`is_mssql_scheme` (`crates/truefix-config/src/builder.rs:872-881`) match only
sqlx-native URL schemes with embedded credentials: `postgres://user:pass@host/db`,
`mysql://user:pass@host/db`, `sqlite:...`, `mssql://user:pass@host/db`, `sqlserver://...`. Real
QuickFIX/J `.cfg` files use the standard **JDBC** URL grammar instead —
`jdbc:<subprotocol>://host[:port]/db[;params]` — with credentials supplied via *separate*
`JdbcUser`/`JdbcPassword` keys, never embedded in the URL string. Verified directly against QFJ's own
source and its own test fixtures, not assumed:
- `JdbcUtil.java:69-72` reads `SETTING_JDBC_DRIVER`/`SETTING_JDBC_CONNECTION_URL`/`SETTING_JDBC_USER`/
  `SETTING_JDBC_PASSWORD` as four independent settings and combines them via
  `HikariConfig.setJdbcUrl`/`setUsername`/`setPassword` (`JdbcUtil.java:82-140`, `createPooledDataSource`).
- QFJ's own acceptance-test server config: `JdbcURL=jdbc:mysql://localhost/quickfix`
  (`quickfixj-core/src/test/java/quickfix/test/acceptance/ATServer.java:121`).
- QFJ's JDBC store/log test fixture: `HSQL_CONNECTION_URL = "jdbc:hsqldb:mem:quickfixj"`
  (`quickfixj-core/src/test/java/quickfix/JdbcTestSupport.java:38`).

None of `jdbc:mysql://`, `jdbc:postgresql://`, `jdbc:sqlserver://`, `jdbc:hsqldb:`, etc. match any of
TrueFix's scheme checks — a real, unmodified QuickFIX/J `.cfg` file pointed at TrueFix hits
`ConfigError::UnsupportedBackend { scheme: "jdbc" }` instead of actually selecting a SQL backend. This
is close to the **opposite** of what US3/GAP-05 (feature 004) set out to deliver
("`.cfg`-only SQL backend selection **via `JdbcURL`**" — the whole point being drop-in config
compatibility with QuickFIX/J). This wasn't caught during feature 004's design or implementation:
`specs/004-engine-wiring-extra-backends/research.md` §3 only weighed dispatching on `JdbcDriver` vs. on
`JdbcURL`'s own scheme — it never checked what a *real* QuickFIX/J `JdbcURL` value actually contains.

This also means the `JdbcUser`/`JdbcPassword` `Recognized` stance rationale currently in
`crates/truefix-config/src/keys.rs:288-291` ("nothing left to configure, the URL already carries
user/password inline") rests on the same wrong assumption: once `jdbc:` URLs are actually accepted,
those two keys become genuinely necessary to consume (their values need splicing into the connection
string sqlx/tiberius expect), not merely redundant.

**Fix**: recognize `jdbc:postgresql://`/`jdbc:mysql://`/`jdbc:sqlserver://`/`jdbc:hsqldb:`-style prefixes
in addition to the current sqlx-native ones, strip the `jdbc:` prefix, and splice in `JdbcUser`/
`JdbcPassword` (when present and not already embedded) before constructing `StoreConfig::Sql`/`Mssql`.
TrueFix's existing sqlx-native URL format should stay supported alongside it (additive, not breaking)
for users who prefer to write it directly.

**File**: `crates/truefix-config/src/builder.rs` `is_sql_scheme()`/`is_mssql_scheme()`/
`jdbc_store_config()`; `crates/truefix-config/src/keys.rs:288-291` (stance-comment correction once fixed)

---

## P0 — Protocol-correctness gaps (session layer)

### ~~GAP-07~~: No application veto on stale application-message resend — **closed (005, FR-007, T022–T024)**

`build_resend` (`crates/truefix-session/src/state.rs:625-654`) unconditionally resends stored
application messages during gap-fill — it never gives the `Application` trait a chance to suppress a
now-stale order/message and substitute a gap-fill instead.

- QFJ: `Session.java:1432` (`resendApproved`, calls `application.toApp`; returning `DoNotSend` skips the
  resend), invoked from the resend loop at `Session.java:2409`.
- QFGo: `session.go:263-273` (`resend()` calls `s.application.ToApp(...)`; `false` suppresses).

**File**: `crates/truefix-session/src/state.rs` `build_resend()`

### ~~GAP-08~~: No `OrigSendingTime`-vs-`SendingTime` anti-replay check on PossDup-too-low messages — **closed (005, FR-008/009, T025–T027)**

When an inbound message has `seq < expected` **and** `PossDupFlag=Y`, TrueFix's `on_received`
(`crates/truefix-session/src/state.rs:511-520`, the `Ordering::Less` + `poss_dup` branch) just returns
`Vec::new()` — no validation call at all, so no `OrigSendingTime` check happens on this specific path.
This is a distinct, earlier code path from the already-implemented `ValidationOptions.
requires_orig_sending_time` dictionary toggle (see `docs/todo-gap-analysis.md` TODO-09) — that toggle
only fires inside `validate()`, which this branch never reaches.

- QFJ: `Session.java:2580-2601` (`validatePossDup`) rejects and logs out if
  `origSendingTime > sendingTime`.
- QFGo: `in_session.go:361-388` rejects if `OrigSendingTime` is missing or
  `sendingTime.Before(origSendingTime)`.

Related: TrueFix also has no config switch to *require* `OrigSendingTime`'s presence specifically on
this early-drop path (QFJ: `SETTING_REQUIRES_ORIG_SENDING_TIME`, `Session.java:367,2594-2599`) — bundle
with this fix rather than tracking separately.

**File**: `crates/truefix-session/src/state.rs` `on_received()`, `Ordering::Less` branch

### ~~GAP-09~~: No inbound chunked-resend auto-continuation — **closed (005, FR-011, T030–T034)**

TrueFix only auto-chunks the **outbound** side (`resend_request_chunk_size`); the inbound side requires
the counterparty to manually issue the next `ResendRequest` after one chunk is satisfied.
`on_resend_request` (`state.rs:600-623`) only answers the request it received; `on_sequence_reset`
(`state.rs:661-679`) just advances `next_in_seq` with no follow-up request.

- QFJ: `Session.java:1556-1570` (gap-fill `SequenceReset` triggers an automatic next-chunk
  `sendResendRequest`) and `:1855-1861` (same when a resent message crosses a chunk boundary).
- QFGo: `resend_state.go:44-73` (`FixMsgIn` auto-calls `session.sendResendRequest` on reaching
  `currentResendRangeEnd`).

**File**: `crates/truefix-session/src/state.rs` `build_resend()`/`on_sequence_reset()`

### ~~GAP-18a~~: Acceptor doesn't reject a duplicate Logon on an already-logged-on session — **closed (005, FR-010, T028)**

`on_logon`'s state/role match (`crates/truefix-session/src/state.rs:712-732`) falls through to a no-op
(`_ => {}`) when a second Logon arrives while already `LoggedOn` — no reject, no disconnect. QFJ closes
the connection outright: `AcceptorIoHandler.java:76-81`. This belongs at the session-state layer (not
transport, despite superficially looking like an acceptor-connection concern), since it's about
*state*, not *routing*.

**File**: `crates/truefix-session/src/state.rs` `on_logon()`

---

## P1 — Feature completeness: session layer

- **GAP-12**: `LogonTag` supports exactly one `(tag, value)` pair
  (`crates/truefix-session/src/config.rs:96-97`, `Option<(u32, String)>`). QFJ's `logonTags` is a
  `List<StringField>` (`Session.java:477`) reading `LogonTag`, `LogonTag1`, `LogonTag2`, … .
- **GAP-13** (QFJ-only difference): `RejectReason::code()` (`crates/truefix-dict/src/model.rs:341-358`)
  always returns a reason code with no FIX-version awareness. QFJ filters reason codes unsupported by
  the session's `beginString` before stamping tag 373 (`Session.java:1658-1679`). QFGo has no such
  filter either — this is a QFJ-only refinement, low priority.
- **GAP-18c**: FIXT `DefaultApplVerID` is only a static `.cfg` value feeding `truefix-dict`, never
  auto-extracted from an inbound Logon's tag 1137. QFJ: `AcceptorIoHandler.java:94-101`.
- **GAP-11** (QFGo-only, optional): no `ResetSeqTime` — resetting sequence numbers mid-session while
  keeping the connection open (`session_state.go:160-179`, sends `Logon(ResetSeqNumFlag=Y)` at a daily
  scheduled time). TrueFix requires disconnect+reconnect to reset. Zero hits in `truefix-session` for
  anything resembling this. Not present in QFJ either.
- **GAP-46** (QFGo-only, optional): no `HeartBtIntOverride` (forcibly correcting a misbehaving
  counterparty's declared HeartBtInt — `session_factory.go:575-581`, `session.go:548`). Zero hits.
- ~~**GAP-47**~~ — **closed (005, FR-012/013, T035–T039)** (new, found while auditing every `Recognized` key in `keys.rs`): `SenderSubID` /
  `SenderLocationID` / `TargetSubID` / `TargetLocationID` / `SessionQualifier` are all `Recognized`
  but genuinely cannot be wired as-is — this goes deeper than a missing `builder.rs` read (confirmed
  zero hits for all five in `crates/truefix-config/src/builder.rs`). `SessionId`
  (`crates/truefix-session/src/session_id.rs:8-23`) *does* have fields for all five, but its only
  constructor, `SessionId::new()` (`:27-42`), unconditionally sets them to `None` — there is no
  builder path that ever populates them, from `.cfg` or otherwise (confirmed zero
  `SessionId { .. }` struct-literal constructions anywhere in `crates/truefix-config`/
  `crates/truefix`). `SessionConfig` (`crates/truefix-session/src/config.rs:19-25`) doesn't even
  *have* fields for sub-ID/location-ID/qualifier — only `begin_string`/`sender_comp_id`/
  `target_comp_id`. Practical consequence: TrueFix cannot configure two sessions that share the same
  BeginString/SenderCompID/TargetCompID but differ by `SessionQualifier` (QFJ's mechanism for
  disambiguating exactly that case), and the sub-ID/location-ID fields that GAP-20's routing gap
  references are never populated in the first place, not just unused for routing. QFJ: all five are
  ordinary `SessionID` constructor parameters (`SessionID.java`), read from `.cfg` in
  `SessionFactory.java`.

---

## P1 — Feature completeness: transport/network layer

- ~~**GAP-14**~~ — **closed (005, FR-014, T040/T043)**: No incrementing reconnect-backoff array — one
  fixed `reconnect_interval` for every retry
  (`crates/truefix-transport/src/lib.rs:1432,1462`, and the TLS variant at `:1484,1518`). QFJ's
  `ReconnectInterval` parses to `int[]` (`AbstractSocketInitiator.java:120,252-267`) and steps up per
  attempt, sticking at the last value (`IoSessionInitiator.java:318-321`).
- ~~**GAP-15**~~ — **closed (005, FR-015, T041/T044)**: No `SocketLocalHost`/`SocketLocalPort` (local
  outbound bind address) on any initiator
  connect path — only `TcpStream::connect(addr)` (`lib.rs:320,338,1442,1495`). The `.cfg` keys are
  `Recognized`-only (`crates/truefix-config/src/keys.rs:145-146`). QFJ:
  `AbstractSocketInitiator.java:130,196-214`.
- ~~**GAP-16**~~ — **closed (005, FR-016, T042/T045)**: `SocketConnectTimeout` is `Recognized`
  (`keys.rs:150`) but never consumed — no
  `tokio::time::timeout` wraps any `TcpStream::connect` call anywhere, including the feature-004
  failover-wiring path in `crates/truefix/src/lib.rs`. QFJ: `AbstractSocketInitiator.java:128,180`,
  `IoSessionInitiator.java:170`.
- **GAP-17** — **partially addressed (005)**: `AllowedRemoteAddresses` is one flat, builder-global list
  (`AcceptorBuilder.allowed_remotes`, `lib.rs:1265,1274`, checked at `:1369`) shared by every
  session on that acceptor — not per-session. QFJ: `Session.java:462,562,3112-3115`
  (`isAllowedForSession`), checked per-session from `AcceptorIoHandler.findQFSession`
  (`AcceptorIoHandler.java:121-140`). No longer moot: BUG-03's fix (005, FR-006/006a) now wires
  `Engine::start` through a real `AcceptorBuilder` per `SocketAcceptPort` group, and
  `AllowedRemoteAddresses` is honored — but only as the **union** of every member session's list within
  that group (`crates/truefix/src/lib.rs:348-355`), not enforced per individual `SessionID` the way QFJ
  does. Still open; per spec.md's Assumptions, deliberately folded into (not solved by) FR-006's scope
  rather than tracked as its own requirement this round.
- **GAP-19**: Dynamic-session templates only substitute `begin_string`/`sender_comp_id`/
  `target_comp_id` (`dynamic_config`, `lib.rs:1366-1377`, `template: Option<SessionConfig>` at
  `:1175,1218-1221`) — no wildcard (`*`) pattern matching on SubID/LocationID mapping to different
  templates. QFJ: `DynamicAcceptorSessionProvider.java` (`List<TemplateMapping>`, `isMatching` wildcard
  match across all 7 `SessionID` components).
- **GAP-20** (QFGo-only difference): Acceptor routing (`route_and_run`, `lib.rs:1332-1335`) keys only
  on BeginString/SenderCompID/TargetCompID (tags 49/56). QFGo additionally matches SubID/LocationID
  (`acceptor.go:284-324`).

---

## P1 — Feature completeness: codec / dictionary layer

(Verified via a full read of `crates/truefix-core`, `crates/truefix-dict` against QFJ's
`Message.java`/`DataDictionary.java`/`FieldType.java` and QFGo's `message.go`/`datadictionary.go`/
`validation.go` — full per-concern comparison tables in this document's git history if a wider
citation set is ever needed; kept terse here since these didn't change with feature 004.)

- ~~**GAP-22**~~ — **closed (005, FR-022, T069)**: 11 `FieldType` variants QFJ has that TrueFix doesn't
  (`crates/truefix-dict/src/model.rs`,
  16 variants vs QFJ's 27): `PRICEOFFSET, LOCALMKTDATE, DAYOFMONTH, UTCDATE, TIME, CURRENCY, EXCHANGE,
  MULTIPLEVALUESTRING, MULTIPLESTRINGVALUE, MULTIPLECHARVALUE, COUNTRY`. All are currently treated as
  raw strings — no format validation. `MULTIPLEVALUESTRING`'s space-split enum semantics (`
  DataDictionary.java:463-475`) ride on this and are also absent.
- ~~**GAP-23**~~ — **closed (005, FR-023, T070–T071)**: No `__ANY__`/`allowOtherValues` open-enum
  sentinel (`DataDictionary.java:64,459-465`) —
  TrueFix's `FieldDef.values` is always a closed enum.
- ~~**GAP-24**~~ — **closed (005, FR-024, T072–T073)**: No per-group child dictionary — TrueFix's
  `GroupDef.members` is a flat `Vec<u32>`
  (`model.rs:370`); QFJ's `GroupInfo.dataDictionary` (`DataDictionary.java:1341-1349`) enables deep
  nested-group validation.
- ~~**GAP-25**~~ — **closed (005, FR-025, T074)**: Repeating-group API is add-only
  (`crates/truefix-core/src/field_map.rs:64-67`); QFJ/QFGo
  both have replace/remove/get-by-index (`FieldMap.java:657-706`, `repeating_group.go:111-122`).
- ~~**GAP-26**~~ — **closed (005, FR-026, T075)**: Header/trailer repeating groups aren't supported at
  the core codec layer
  (`crates/truefix-core/src/codec/decode.rs:69-77` keeps them flat) — e.g. `NoHops` (tag 504) can't be
  parsed as a group in the header. QFJ: `Message.java:250-312,658-660`. QFGo: `tag.go:51`.
- ~~**GAP-27**~~ — **closed (005, FR-027, T076–T078)**: No per-message custom `fieldOrder` — TrueFix
  emits in insertion order
  (`Vec<Member>`); QFJ: `int[] fieldOrder` + `FieldOrderComparator` (`FieldMap.java:52,116-132`).
- ~~**GAP-28**~~ — **closed (005, FR-028, T079)**: No structured dictionary version metadata
  (major/minor/service-pack/extension-pack) —
  TrueFix's `DataDictionary` has only a `version: String` (`model.rs:154`). QFJ:
  `DataDictionary.java:90-96`. Blocks GAP-32.
- ~~**GAP-29**~~ — **closed (005, FR-030, T081)**: No value→label name lookup (`FieldDef.values` stores
  raw values only, `model.rs:98`).
  QFJ: `valueNames` (`DataDictionary.java:107,245-258`). QFGo: `Enum.Description`.
- **GAP-31** (low priority): TrueFix truncates timestamp precision to nanoseconds
  (`crates/truefix-core/src/field.rs:195-208`); QFJ additionally supports picoseconds
  (`UtcTimestampConverter.java:46`, `LENGTH_INCL_PICOS = 30`). Practically never triggered. Out of scope
  for 005 (spec.md Assumptions).
- ~~**GAP-32**~~ — **closed (005, FR-029, T080)**: No validation that a message's `BeginString` matches
  the loaded dictionary's version
  (`crates/truefix-dict/src/validate.rs`). QFJ: `DataDictionary.java:632-639`. QFGo has no equivalent
  either — this is a QFJ-only refinement. Depends on GAP-28.
- ~~**GAP-33**~~ — **closed (005, FR-031, T082–T089)**: Bundled `.fixdict` sources are documented
  **subsets**, not full FIX specs
  (`crates/truefix-dict/dict-src/normalized/FIX44.fixdict:1` said "subset" in its own comment). QFJ and
  QFGo both ship full dictionaries. Affects real-world field/message coverage. Closed via a new
  QFJ-XML→`.fixdict` converter (`crates/truefix-dict/src/qfj_xml.rs`) regenerating all 8 non-Orchestra
  bundled dictionaries to real QFJ scale (FIX40: 139 fields/27 messages … FIX50SP2: 1610 fields/110
  messages), enforced going forward by `crates/truefix-dict/tests/dictionary_coverage.rs`.
- **GAP-34** (low priority, tooling): No `toXML` diagnostic message dump. QFJ: `Message.java:325-435`.

---

## P1 — Feature completeness: store / log / config layer

- **GAP-10**: `TimeZone` only accepts a numeric `+HH:MM`/`-HH:MM` offset
  (`crates/truefix-config/src/builder.rs:1024-1045`, `parse_utc_offset`) — IANA zone names like
  `America/New_York` are explicitly rejected (this is disclosed in the function's own doc comment, not
  silent). No DST-aware time-zone handling. QFJ uses `java.util.TimeZone`; QFGo uses `time.LoadLocation`.
  `chrono-tz` would close this.
- ~~**GAP-38**~~ — **closed (005, FR-017, T049–T051)**: Session creation time is never persisted
  anywhere (no `.session` file, no
  `creation_time` column in any SQL/MSSQL/Redb/Mongo schema). QFJ/QFGo persist and update it on reset.
- ~~**GAP-39**~~ — **closed (005, FR-018, T052)**: `save()` and the sequence-number increment are two
  independent, non-transactional writes
  in every SQL-family store (`crates/truefix-store/src/sql.rs`). QFGo wraps both in one transaction
  (`sql_store.go:358-391`). A crash between the two calls can leave `seq` advanced with the message
  body unsaved. Applies to `SqlStore`/`MssqlStore`; `RedbStore`'s `reset()` is already correctly atomic
  (single write transaction, feature 004) but its `save`/`set_next_*` are still separate calls like the
  SQL stores. `MongoStore`'s `save_and_advance_sender` intentionally left at the trait default (no
  multi-doc transaction guarantee without a replica set) — disclosed, not a silent gap.
- **GAP-40**: `Log` trait (`crates/truefix-log/src/lib.rs`) has only `on_incoming`/`on_outgoing`/
  `on_event` — no severity levels (`on_error_event`/`on_warn_event`) and no `clear()`/cleanup API. QFJ:
  `Log.java:30,58,66`. Out of scope for 005 (spec.md Assumptions — trait-signature change touching
  every log backend, no reported incident).
- ~~**GAP-41**~~ — **closed (005, FR-019, T053–T058)**: Every structured log backend — SQL, MSSQL, and
  (new in feature 004) `RedbLog`/`MongoLog` —
  stores only `(id, text)` with **no timestamp and no session-identity column**
  (`crates/truefix-log/src/sql.rs:159-183`, confirmed the same shape propagated to
  `crates/truefix-log/src/redb.rs`/`mongo.rs`). QFJ/QFGo JDBC-style log tables carry
  `(time, <session-id columns>, text)`. This makes cross-session audit/replay from a shared log table
  impossible without external correlation.
- **GAP-42** (low priority): Every background-writer log backend (`SqlLog`, `MssqlLog`, and now
  `RedbLog`/`MongoLog`) uses `mpsc::unbounded_channel` — unbounded memory growth risk if the DB falls
  behind under sustained load. A bounded `tokio::sync::mpsc::channel(N)` would cap it (at the cost of
  needing a drop/backpressure policy, unlike `InChanCapacity`'s bounded application-message channel
  which already exists for the analogous inbound problem).
- **GAP-44**: `${var}` interpolation (`crates/truefix-config/src/lib.rs:201-229`,
  `interpolate_value`) only resolves against the settings map itself — no fallback to environment
  variables or system properties. QFJ's default `SessionSettings()` constructor seeds interpolation
  from `System.getProperties()` (`SessionSettings.java:97-99`), so `.cfg` files using `-D` JVM
  properties don't have a working TrueFix equivalent.
- **GAP-45** (low priority, architectural): `SessionSettings` is an immutable post-parse snapshot
  (`crates/truefix-config/src/lib.rs:92-155` — only `parse`/`default_section`/`sessions`, no mutation
  API). QFJ's `SessionSettings.setString`/`removeSection` support runtime session add/remove. This is a
  deliberate load-once-then-build-`Engine` design; treating it as a gap to fix would be a real
  architecture change, not a small patch — flagged for awareness, not recommended as near-term work.
- **GAP-21** (narrowed by feature 004): `ScreenLog`/`TracingLog`/`CompositeLog` still can't be selected
  from `.cfg` (`resolve_log` only ever builds `FileLog` or, since feature 004, the `JdbcURL`-driven
  `SqlLogSpec`) — `ScreenLog*` keys are `Recognized`-only. `SqlLog`'s `.cfg` path is now solved (US3);
  this item is genuinely smaller than before feature 004, not closed.
- **GAP-37** (low priority / possibly not applicable): `MessageStore` trait has no `incr_next_*`,
  `refresh()`, or `close()`. QFJ has all three, but they exist there partly to support QFJ's
  multi-threaded session model; TrueFix's sans-IO, single-owner session architecture (Constitution-level
  design choice) means the caller already fully owns get-then-set sequencing with no concurrent-writer
  race to guard against — worth a deliberate "no-op, documented" decision (matching how
  `ClosedResendInterval`/`MaxScheduledWriteRequests` were already handled) rather than literally
  porting `incr*`. `get_creation_time`/GAP-38 above is the one sub-part worth actually adding.

---

## P2 — Low priority / optional

- **GAP-35** (QFGo-only): No `TZTIMEONLY`/`TZTIMESTAMP`/`LANGUAGE`/`XMLDATA` `FieldType` recognition
  (`validation.go:413-426`). TrueFix treats all four as raw strings.
- **GAP-36**: No offline/streaming FIX log-file batch parser (QFJ:
  `FIXMessageDecoder.extractMessages`, mmap-based, `:303-339` — used for log replay/audit tooling).
  TrueFix's `frame_length` (`crates/truefix-core/src/codec/framing.rs`) is a stateless single-message
  extractor; a caller-side streaming wrapper would need to be built on top for this use case.

---

## Documentation-accuracy notes (not functional gaps, but worth a follow-up pass over `keys.rs`)

- `EndpointIdentificationAlgorithm` is `Recognized` (`keys.rs:204`) but rustls's certificate verifier
  already performs WebPKI hostname verification implicitly against the configured `ServerName` for
  every TLS connection TrueFix makes — there's no algorithm-selection knob to wire because there's only
  one behavior, and it's already active. The key would be more accurately `Unsupported` with a reason
  ("implicit via rustls, no selectable algorithm"), matching how `KeyStoreType`/
  `KeyManagerFactoryAlgorithm`/`TrustManagerFactoryAlgorithm` are already marked.
- `SLF4JLogPrependSessionID`/`SLF4JLogHeartbeats` (`keys.rs:266-267`) are `Recognized` while their
  sibling `SLF4JLog*Category` keys are `Unsupported` with a "tracing facade" rationale. This isn't
  actually a bug — `TracingLog` does have a directly-equivalent `include_heartbeats` option
  (`crates/truefix-log/src/tracing_log.rs:9-19`), so `Recognized` (parsed, not yet `.cfg`-wired) is the
  technically correct stance — but it's only reachable once GAP-21 is fixed (`TracingLog` isn't
  `.cfg`-selectable at all today), so don't wire these two in isolation.
- `ScreenLogShowEvents`/`ScreenLogShowHeartBeats`/`ScreenLogShowIncoming`/`ScreenLogShowOutgoing`/
  `ScreenIncludeMilliseconds` (`keys.rs:240-244`) checked and confirmed in the same category as the
  two `SLF4J*` keys just above: `ScreenLogOptions` (`crates/truefix-log/src/screen.rs:8-19`) already
  has a field for exactly each one of these five, 1:1. `Recognized` is correct; the actual blocker is
  the same GAP-21 (`ScreenLog` isn't `.cfg`-selectable either).

The following four were found by systematically checking every `Recognized` key in `keys.rs` (not just
the ones already flagged by earlier passes), prompted directly by this question. **All four closed
(005, FR-020/021, T090–T094)**: the first three keys downgraded to `Unsupported` with the documented
reason, and the JDBC pool/table-name keys threaded through `StoreConfig::Sql`/`Mssql`'s new
`sessions_table`/`messages_table`/`session_id`/`pool` fields and promoted to `Implemented`.

- `SocketAcceptProtocol` / `SocketConnectProtocol` (`keys.rs:132,149`) should almost certainly be
  `Unsupported`, not `Recognized`. In QFJ these select between `ProtocolFactory.SOCKET` and
  `ProtocolFactory.VM_PIPE` (`quickfixj-core/src/main/java/quickfix/mina/ProtocolFactory.java:58-59`)
  — an in-JVM-process transport with no meaningful Rust equivalent. `docs/todo-gap-analysis.md`
  already lists `VM_PIPE (JVM 内)` under "不适合 Rust 生态" for other keys; these two keys describe
  the exact same choice and should get the same treatment, not sit as `Recognized` implying future
  work is possible.
- `JdbcDataSourceName` (`keys.rs:293`) should be `Unsupported` alongside its already-`Unsupported`
  siblings `JndiContextFactory`/`JndiProviderURL` (`keys.rs:308-317`), not `Recognized`. All three are
  parts of the *same* QFJ mechanism — looking up a pre-configured JNDI `DataSource` by name
  (`JdbcUtil.java:51-66`, `getJNDIDataSource`, triggered exactly when `SETTING_JDBC_DS_NAME` /
  `JdbcDataSourceName` is set) — and JNDI has no Rust equivalent (the existing `Unsup` reason
  "JNDI data-source lookup is not applicable in Rust" applies verbatim to this key too). Currently
  inconsistent: two-thirds of one mechanism are `Unsupported`, one-third is `Recognized`.
- `JdbcConnectionTestQuery` (`keys.rs:306`) probably belongs in the same `Unsupported` category, for a
  different reason: unlike the other `Jdbc*Connection*` pool-tuning keys (see below), there is no
  matching field anywhere in `SqlPoolOptions` (`crates/truefix-store/src/sql.rs:29-40`) or
  `SqlLogPoolOptions` (`crates/truefix-log/src/sql.rs:30-40`) — because `sqlx`'s pool already validates
  connection liveliness automatically before handing one out (`test_before_acquire`, defaults `true`;
  confirmed in the vendored `sqlx-core-0.9.0/src/pool/options.rs:46,149`), with no string-based
  custom-query hook exposed at the `.cfg` level for this to map onto (only a Rust closure via
  `.before_acquire()`, which isn't `.cfg`-expressible). `Recognized` currently implies "parseable,
  wiring pending"; there is nothing to wire.
- `JdbcMaxActiveConnection`/`JdbcMaxConnectionLifeTime`/`JdbcMinIdleConnection`/
  `JdbcConnectionTimeout`/`JdbcConnectionIdleTimeout`/`JdbcConnectionKeepaliveTime` and
  `JdbcStoreMessagesTableName`/`JdbcStoreSessionsTableName`/`JdbcSessionIdDefaultPropertyValue`
  (`keys.rs:294-305`) are correctly `Recognized`, but the existing stance comment above them
  (`keys.rs:268-288`, written for BUG-04's "nothing left to configure" framing) undersells how cheap
  wiring them would be: unlike the *log* side (`SqlLogSpec`, the `.cfg`-facing struct feature 004
  added, which genuinely has no pool-settings or table-name fields), the **store** side's
  `SqlStoreConfig` (`crates/truefix-store/src/sql.rs:56-67`) already has `sessions_table`/
  `messages_table`/`session_id`/`pool: SqlPoolOptions` — every one of these keys' targets already
  exists as a Rust field. The only reason they're not `.cfg`-reachable is that `resolve_store`'s
  `sql_store_config()`/`mssql_store_config()` (`crates/truefix-config/src/builder.rs:901-921`) only
  ever construct the coarse `StoreConfig::Sql { url }`/`StoreConfig::Mssql { url }` enum variants
  (bare URL, nothing else) instead of threading a full `SqlStoreConfig` through. Worth fixing alongside
  BUG-04 rather than treated as a separate, larger effort.
- `Description` (`keys.rs:52`) checked and found to be correctly `Recognized` with nothing to fix: even
  in QFJ this key is purely cosmetic metadata — its own doc comment says "Used by external tools"
  (`Session.java:298-300`), it has no effect on session behavior in QFJ either. Lowest priority in this
  entire list; not worth a GAP number.

This closes out a full pass over every `Recognized`-stance key currently in `keys.rs` (prompted
directly by this question) — every `Rec` entry has now been checked at least once against the actual
Rust code and, where relevant, against QFJ/QFGo source; nothing else was found beyond what's captured
above and in the GAP/BUG entries earlier in this document.

---

## Considered and explicitly not tracked

- **`TAG_APPEARS_MORE_THAN_ONCE` detected at parse-time vs. validate-time**: QFJ catches this during
  parsing (`Message.java:716-721`); TrueFix catches it during `validate()`
  (`crates/truefix-dict/src/validate.rs:82-91`). Same message is rejected either way — a timing
  difference with no observable behavioral difference from the counterparty's perspective. Not tracked.
- **Non-ASCII/charset-aware length and checksum computation** (QFJ: `CharsetSupport`,
  `Message.java:964-965`): TrueFix is byte-only, same as QFGo. FIX is ASCII/Latin-1 in practice; low
  enough value that it's noted here for completeness (per this audit's instructions) rather than given
  a GAP number.
- **`toRawString`/original-message preservation** (QFJ: `Message.java:235-237`): TrueFix's raw-byte
  `Field` storage (`field.rs:10-11`) already gives strictly stronger round-trip fidelity than QFJ's
  re-encode-on-`toString` approach — this is a TrueFix advantage, not a gap.
- **Admin/app `messageCategory`** (QFJ: `DataDictionary.java:100,326-341`): TrueFix already has the
  equivalent `is_admin_type` function (`crates/truefix-session/src/state.rs:884`).
