# Contract: FIXT 1.1, Dictionary Validation, and Codegen/Converter Tooling

**Requirements**: FR-006 (partial, see below), FR-007, FR-018, FR-019 (US1); FR-023–FR-025,
FR-035–FR-038 (US2); FR-052–FR-057, FR-066–FR-069, FR-072, FR-079 (US3)
**Research**: research.md §R1 (`is_header`)

## FIXT 1.1 header routing (FR-007, `NEW-55`)

**Contract**: `tags::is_header` includes `1128`, `1129`, `1130`, `1156`, and `1351`-`1355`
(`NoApplIDs` and members) — a FIXT 1.1 message's `ApplVerID`/`ApplReportID`/`LastApplVerID`/
`ApplExtID`/`NoApplIDs` land in `message.header`, not `message.body`.

**Protocol-behavioral**: yes. **AT scenario required** — a FIX 5.x message with `ApplVerID` set
must show it resolvable from `msg.header.get(1128)` for downstream FIXT dictionary resolution to
work at all (this is the prerequisite for `NEW-58` below).

## FIXT group-aware decode engagement (`NEW-58`, part of FR-006's neighboring transport fix — see `store-and-transport.md` for the rest of FR-006's scope)

**Contract**: `classify_buffered` engages `HeaderTrailerGroupsOnly` group-aware decoding whenever
`fixt_dictionaries` is configured, not only when `services.validator` is set.

**Protocol-behavioral**: yes. **AT scenario required** — a FIXT 1.1 dual-dictionary session with a
header/trailer repeating group (e.g. `NoHops`) must decode it structurally.

## FIX 5.0/SP1/SP2 dispatch (FR-018, `NEW-06`)

**Contract**: Generated `crack_fix50`/`crack_fix50sp1`/`crack_fix50sp2` resolve by `ApplVerID`
(1128), not solely `BeginString` — a FIX 5.0SP2 message dispatches into FIX 5.0SP2 structs, not
FIX 5.0's.

**Protocol-behavioral**: yes, but codegen-generated-code behavior. Unit test against generated code
for all three sub-versions is the primary vehicle; AT scenario optional (confirm at
`/speckit-tasks` — depends on whether the AT harness already exercises FIX 5.0SP1/SP2 typed
dispatch, likely tied to `NEW-28`'s harness-coverage fix in `hardening-and-tooling.md`).

## `MultipleCharValue` per-token enum check (FR-019, `NEW-07`)

**Contract**: `FieldDef::allows()` checks a `MultipleCharValue` value per space-delimited token,
consistent with `value_ok`'s existing per-token handling.

**Protocol-behavioral**: yes, narrow (validation-only). Table-driven unit test in
`truefix-dict::model`.

## Dictionary validation and field-type items (US2)

| FR | `NEW-ID` | Contract | Test type |
|---|---|---|---|
| FR-023 | 19 | Header-level `NoHops(627)` groups are validated the same as body groups | Unit |
| FR-024 | 20 | `MonthYear`/`LocalMktDate` are format-validated | Unit |
| FR-025 | 21 | Tag `0` is rejected as invalid during decode | Unit — **protocol-behavioral, AT scenario optional** (confirm at tasks time; likely covered by extending an existing malformed-tag scenario) |
| FR-035 | 64 | `application_for` treats empty-string `ApplVerID` as absent, falling back to `DefaultApplVerID` | Unit |
| FR-036 | 65 | Cyclic group member definitions are cycle-guarded / recursion-bounded | Unit (crafted cyclic dictionary fixture — Principle I "never panics" regression test) |
| FR-037 | 66, 67 | `DayOfMonth` range-checked 1-31; `Length`/`SeqNum`/`NumInGroup` reject negatives | Unit |
| FR-038 | 68 | Non-numeric group count field type-checked as `IncorrectDataFormat` on the grouped-decode path | Unit |
| FR-039 (session-protocol.md) | 69 | Empty-valued fields inside group entries rejected under `TagSpecifiedWithoutValue` | Unit — **AT scenario required**, listed in session-protocol.md's AT-impact table |

## Codegen / converter tooling (US3 — `dict-tooling` feature, tooling-only per source audit)

| FR | `NEW-ID` | Contract | Test type |
|---|---|---|---|
| FR-052 | 05 | `fix_repository.rs::convert()` excludes the count field from members, delimiter = first remaining field | Unit — byte-compare against shipped `.fixdict` |
| FR-053 | 60 | `orchestra.rs::map_type` supports the same type tokens `fix_repository::map_type` does | Unit |
| FR-054 | 61 | `build.rs` declares `cargo:rerun-if-changed=src/codegen.rs` | Manual/CI verification (edit codegen.rs without touching a `.fixdict`, confirm rebuild) |
| FR-055 | 78 | `codegen.rs::parse_dict` errors on unknown directives / malformed lines, consistent with runtime `parser::parse` | Unit |
| FR-056 | 79 | `codegen.rs` enum-variant sanitization deduplicates colliding identifiers | Unit (crafted colliding-label dictionary fixture) |
| FR-057 | 82 | `generate_code` CLI uses consistent byte handling between runtime-hash and codegen-hash for non-UTF-8 input | Unit (non-UTF-8 fixture) |
| FR-066 | 43 | Runtime `parser.rs` errors on duplicate tag/name/msg_type definitions | Unit |
| FR-067 | 44 | `strip_comment()` doesn't truncate a STRING enum value containing `#` | Unit |
| FR-068 | 45 | `parse_fix_begin_string` distinguishes FIX 5.0/SP1/SP2 | Unit |
| FR-069 | 46 | Circular `${var}` config references error instead of emitting literal unresolved text | Unit |
| FR-072 | 53 | `DataDictionary::extend()` detects `field_by_name` collisions and recomputes `member_tags` | Unit |
| FR-079 | 04 | `tokenize_validated`/`decode` enforce `MAX_BODY_LEN` independently of `frame_length` | Unit — downgraded scope (API-consistency only, not a reachable DoS per third-pass correction) |

None of the codegen/converter items reach the shipped `.fixdict` files or runtime `DataDictionary`
path used by a deployed engine — no AT scenario is expected for that subgroup (per source document's
own Tooling-Only classification, Constitution Principle IV's dual-track concern doesn't apply since
these fix codegen *robustness*, not a runtime/codegen semantic divergence).
