# Phase 1 Data Model: Engine Gap Remediation

**Feature**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

This translates the spec's Key Entities (plus the concrete shapes research.md's investigation
surfaced) into grounded types. Every entity here is additive to the existing `truefix-core`/
`truefix-session`/`truefix-transport`/`truefix-config`/`truefix-store`/`truefix-log`/`truefix-dict`
types found during Phase 0 — no existing public type's shape is removed or narrowed.

## `strip_comment` (extends `truefix-config::lib`)

```text
fn strip_comment(raw: &str) -> &str {
    if raw.trim_start().starts_with('#') { "" } else { raw }
}
```

- No new type. Behavior change only: `#` is a comment start exactly when it's the first non-whitespace
  character of the *raw* line, never mid-value after a `=`.

## `data_field_for_length` (extends `truefix-core::tags`)

```text
93 => 89,   // NEW: SignatureLength -> Signature (the one QFJ-documented exception to lengthTag = dataTag - 1)
```

- No new type. One additional match arm in the existing 12→13-pair table.

## `SessionId` construction (extends `truefix-session::session_id`)

```text
SessionId {
    begin_string: String,           // unchanged
    sender_comp_id: String,         // unchanged
    sender_sub_id: Option<String>,      // unchanged field, NEWLY populated
    sender_location_id: Option<String>, // unchanged field, NEWLY populated
    target_comp_id: String,         // unchanged
    target_sub_id: Option<String>,      // unchanged field, NEWLY populated
    target_location_id: Option<String>, // unchanged field, NEWLY populated
    session_qualifier: Option<String>,  // unchanged field, NEWLY populated
}

impl SessionId {
    pub fn new(begin_string, sender_comp_id, target_comp_id) -> Self   // unchanged: still defaults
                                                                          // the other 5 to None
    pub fn new_full(                                                    // NEW
        begin_string: impl Into<String>,
        sender_comp_id: impl Into<String>,
        sender_sub_id: Option<String>,
        sender_location_id: Option<String>,
        target_comp_id: impl Into<String>,
        target_sub_id: Option<String>,
        target_location_id: Option<String>,
        session_qualifier: Option<String>,
    ) -> Self
}
```

- **No field added** — every field already existed (feature 002-era design). Only a new constructor.
- `Hash`/`Eq`/`PartialEq` already derive across all 8 fields; no change needed for two sessions
  differing only by `session_qualifier` to compare as distinct.

## `SessionConfig` growth (extends `truefix-session::config`)

```text
SessionConfig {
    // ...existing fields unchanged (begin_string, sender_comp_id, target_comp_id, role, etc.)...
    sender_sub_id: Option<String>,          // NEW (US5)
    sender_location_id: Option<String>,     // NEW (US5)
    target_sub_id: Option<String>,          // NEW (US5)
    target_location_id: Option<String>,     // NEW (US5)
    session_qualifier: Option<String>,      // NEW (US5)
    reconnect_interval_steps: Vec<u32>,     // NEW (US6/FR-014); empty = today's single-interval behavior
    local_bind_addr: Option<SocketAddr>,    // NEW (US6/FR-015)
    connect_timeout: Option<Duration>,      // NEW (US6/FR-016)
}
```

- All 8 new fields default to their zero-value (`None`/empty `Vec`), preserving today's behavior for
  every `.cfg` that doesn't set the corresponding new key — matches spec.md's "existing `.cfg` files
  continue to work" Assumption.
- `SessionConfig::session_id()` (wherever it constructs a `SessionId` today) is updated to call
  `SessionId::new_full` with the 5 identity fields instead of `SessionId::new`.

## `Action::Send` origin tracking (extends `truefix-session::state`)

```text
pub enum SendOrigin {
    Live,                    // NEW: today's only implicit behavior, now named
    Resend { seq: u64 },     // NEW: set exactly by build_resend's send_raw calls
}

pub enum Action {
    Send { message: Message, origin: SendOrigin },   // CHANGED shape: Message(Message) -> struct variant
    Disconnect,               // unchanged
    ResetStore,                // unchanged
}
```

- `Action` is `truefix-session`-internal (re-exported opaquely to `truefix-transport`, never
  constructed outside this workspace) — this reshaping is not a breaking change to any external API,
  confirmed by there being no external crate/consumer that pattern-matches `Action::Send`'s payload
  directly. `/speckit-tasks` may instead choose an additive `Action::Resend(Message, u64)` variant
  alongside the unchanged `Action::Send(Message)` — both are valid per research.md §5's "Alternatives
  considered"; whichever is chosen, `perform_actions`'s `Action::Send`-handling arm needs the resend/
  live distinction available to it.
- New method: `Session::gap_fill_after_veto(&mut self, seq: u64) -> Action` (thin wrapper around the
  existing private `gap_fill` helper), called by `perform_actions` when a resend-originated `to_app`
  veto occurs.

## `Session::reject_logon` reuse (no shape change, new call sites)

```text
// Existing method, unchanged signature:
pub fn reject_logon(&mut self, reject: &truefix_core::Reject) -> Vec<Action>
```

- Two new internal call sites (not through `Application::from_admin`):
  - `on_received`'s `Ordering::Less` + `poss_dup` branch, on an `OrigSendingTime > SendingTime`
    violation (US3/FR-008).
  - `on_logon`'s already-`LoggedOn` case (US3/FR-010).
- `SessionConfig` gains one new field for FR-009 (`RequiresOrigSendingTime` on this specific code
  path) — exact name/reuse-vs-new decision deferred to `/speckit-tasks` per research.md §6.

## `AcceptorBuilder`-backed multi-session `.cfg` startup (extends `truefix::Engine::start`, `truefix-config::builder`)

```text
// New in truefix-config::builder::ResolvedSession (or read directly from the raw .cfg map at
// Engine::start time — exact layer deferred to /speckit-tasks):
acceptor_template: bool,              // NEW: DynamicSession=Y / AcceptorTemplate present
allowed_remote_addresses: Vec<IpAddr>, // NEW: AllowedRemoteAddresses, parsed

// New error variant on truefix_config::ConfigError:
ConfigError::AmbiguousAcceptorTemplate { addr: SocketAddr }   // NEW
```

```text
// Engine::start's acceptor branch, restructured:
group resolved acceptor sessions by rs.address
for each group:
    if group.len() == 1 && no special keys set on that session:
        Acceptor::bind_with(...)   // UNCHANGED path
    else:
        AcceptorBuilder::bind(addr, app.clone())
            .with_session(...)      // once per group member
            .with_dynamic_template(...)   // if exactly one member sets it
            .allow_remotes(union of every member's AllowedRemoteAddresses)
            .with_tls(...)          // if any member sets SocketUseSSL=Y
            .serve()
```

- No new public type beyond the one new `ConfigError` variant; the grouping/dispatch logic lives
  entirely inside `Engine::start`'s existing function body.

## `JdbcURL` scheme recognition + credential splicing (extends `truefix-config::builder`)

```text
fn is_sql_scheme(url: &str) -> bool {
    // unchanged sqlx-native checks, plus:
    url.starts_with("jdbc:postgresql://") || url.starts_with("jdbc:postgres://")
        || url.starts_with("jdbc:mysql://") || url.starts_with("jdbc:sqlite:")
        || url.starts_with("jdbc:h2:")
}
fn is_mssql_scheme(url: &str) -> bool {
    // unchanged sqlx-native checks, plus:
    url.starts_with("jdbc:sqlserver://")
}
fn splice_credentials(url: &str, user: Option<&str>, password: Option<&str>) -> String  // NEW
```

- `jdbc_store_config` (and its `mssql`/`sql`-gated helpers) call `splice_credentials` after stripping
  the `jdbc:` prefix, when the URL's authority has no `user:pass@` segment and `JdbcUser`/
  `JdbcPassword` are both present in the session's raw `.cfg` map.

## `StoreConfig::Sql`/`Mssql` field growth (extends `truefix-store`)

```text
StoreConfig::Sql {
    url: String,                              // unchanged
    sessions_table: Option<String>,           // NEW (US8/FR-021), None = SqlStoreConfig's existing default
    messages_table: Option<String>,           // NEW
    session_id: Option<String>,               // NEW
    pool: Option<SqlPoolOptions>,              // NEW (SqlPoolOptions already exists, US8/FR-021)
}
// StoreConfig::Mssql grows the equivalent fields for MssqlStoreConfig.
```

- Every new field is `Option`-wrapped and defaults to `None`, which `build_store`'s dispatch maps onto
  `SqlStoreConfig`'s/`MssqlStoreConfig`'s own existing defaults — zero behavior change for a bare
  `JdbcURL` with none of the new keys set.

## `MessageStore` trait growth (extends `truefix-store::lib`)

```text
pub trait MessageStore: Send + Sync {
    // ...existing methods unchanged...
    fn creation_time(&self) -> Option<time::OffsetDateTime> { None }   // NEW, defaulted
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        // NEW, defaulted: calls save() then set_next_sender_seq(seq + 1) sequentially
        self.save(seq, message).await?;
        self.set_next_sender_seq(seq + 1).await
    }
}
```

- `SqlStore`/`MssqlStore` override `save_and_advance_sender` with a single transaction; `RedbStore`
  overrides it with a single `redb` write transaction (mirroring its existing `reset()`).
- Every backend overrides `creation_time()` to actually persist/return a value; the trait-level default
  (`None`) exists purely so this is a non-breaking addition for any external `MessageStore`
  implementor, matching the `was_corrupted()` precedent.

## Structured `Log` schema widening (extends `truefix-log::sql`/`redb`/`mongo`)

```text
// SQL/MSSQL (ensure_table), each table gains two columns:
logged_at   TIMESTAMP  -- server-default-now on insert
session_id  TEXT       -- from the existing session_id parameter, previously unused beyond table selection

// RedbLog: table value type widens from `&str` to a small tuple/struct:
(timestamp: u64, session_id: String, text: String)

// MongoLog: document gains two fields:
{ "logged_at": <timestamp>, "session_id": <string>, "text": <string> }
```

## Config-key stance registry (extends `truefix-config::keys`)

| Key | Before | After | Reason |
|-----|--------|-------|--------|
| `SocketAcceptProtocol` | `Rec` | `Unsupported` | VM_PIPE-only alternative, no Rust equivalent |
| `SocketConnectProtocol` | `Rec` | `Unsupported` | same |
| `JdbcDataSourceName` | `Rec` | `Unsupported` | JNDI-only lookup, no Rust equivalent |
| `JdbcConnectionTestQuery` | `Rec` | `Unsupported` | no `.cfg`-expressible equivalent (`sqlx`'s own automatic `test_before_acquire`) |
| `JdbcURL` | `Impl` (scheme-limited) | `Impl` (both sqlx-native and `jdbc:`-prefixed) | US2/FR-003 |
| `JdbcUser` / `JdbcPassword` | `Rec` | `Impl` | US2/FR-004, now actually consumed |
| `AllowedRemoteAddresses` / `DynamicSession` / `AcceptorTemplate` | `Impl` (unreachable) | `Impl` (actually reachable) | US2/FR-006 — same nominal stance, now honest |
| `JdbcMaxActiveConnection` + 5 sibling pool keys, `JdbcStoreMessagesTableName`/`SessionsTableName`/`SessionIdDefaultPropertyValue` | `Rec` | `Impl` | US8/FR-021 |

## Dictionary/codec model growth (extends `truefix-dict::model`/`parser`/`validate`/`codegen`, `truefix-core::codec`/`field_map`)

```text
FieldType {
    // ...existing 16 variants unchanged...
    PriceOffset, LocalMktDate, DayOfMonth, UtcDate, Time, Currency, Exchange,
    MultipleValueString, MultipleStringValue, MultipleCharValue, Country,   // NEW (11 variants, FR-022)
}

FieldDef {
    tag: u32, name: String, field_type: FieldType, values: Vec<String>,   // unchanged
    open_enum: bool,                                                       // NEW (FR-023), default false
    value_labels: BTreeMap<String, String>,                                 // NEW (FR-030), default empty
}

GroupDef {
    count_tag: u32, delimiter: u32, members: Vec<u32>,   // unchanged
    child: Option<Box<DataDictionary>>,                    // NEW (FR-024), default None
}

MessageDef {
    // ...existing fields unchanged...
    field_order: Option<Vec<u32>>,   // NEW (FR-027), default None = today's insertion order
}

DataDictionary {
    // ...existing fields unchanged...
    version_meta: Option<VersionMeta>,   // NEW (FR-028), default None
}

struct VersionMeta {   // NEW
    major: u8, minor: u8, service_pack: Option<u8>, extension_pack: Option<u8>,
}
```

```text
// truefix-core::field_map — new mutation methods alongside the existing add_group:
impl FieldMap {
    pub fn replace_group(&mut self, count_tag: u32, index: usize, entry: FieldMap)   // NEW (FR-025)
    pub fn remove_group(&mut self, count_tag: u32, index: usize)                       // NEW
    pub fn get_group(&self, count_tag: u32, index: usize) -> Option<&FieldMap>         // NEW
}
```

- **Normalized `.fixdict` grammar** (`parser.rs`) gains four additive directive modifiers: `open` (on a
  field's value list, FR-023), `ordered` (on a `message` block, FR-027), `version-meta` (top-level,
  FR-028), and a label-carrying value syntax (FR-030) — every existing `.fixdict` file parses
  identically without them, per spec.md's Edge Cases.
- **Header/trailer group decode** (FR-026) reuses `decode_with_groups`'s existing `GroupSpec`-driven
  `build_group` machinery — no new type, just a new call site in `truefix-core::codec::decode`.
- **Bundled dictionary content expansion** (FR-031) is a `dict-src/normalized/*.fixdict` content
  change, not a type/schema change — no entity here, tracked purely as a per-version content-diff task
  in `/speckit-tasks`.
