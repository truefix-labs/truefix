# Contract: Correctness Bugs (US1, US2; FR-001–FR-006a)

## Surface

```text
// truefix-config
fn strip_comment(raw: &str) -> &str;   // CHANGED behavior, unchanged signature
fn is_sql_scheme(url: &str) -> bool;   // CHANGED: recognizes jdbc: prefixes too
fn is_mssql_scheme(url: &str) -> bool; // CHANGED: recognizes jdbc:sqlserver:// too
fn splice_credentials(url: &str, user: Option<&str>, password: Option<&str>) -> String;  // NEW

pub struct ResolvedSession {
    // ...existing fields...
    pub acceptor_template: bool,               // NEW
    pub allowed_remote_addresses: Vec<IpAddr>, // NEW
}
pub enum ConfigError {
    // ...existing variants...
    AmbiguousAcceptorTemplate { addr: SocketAddr },  // NEW
}

// truefix-core
pub fn data_field_for_length(tag: u32) -> Option<u32>;  // CHANGED: 13th pair, 93 => 89

// truefix (facade)
// Engine::start's acceptor branch restructured — no public signature change, internal control flow only.
```

## Behaviour

1. **`#` truncation (BUG-01/FR-001)**: `strip_comment` treats `#` as a comment start only when it's
   the first non-whitespace character of the raw `.cfg` line. A `#` appearing after a `key=value`
   pair's `=` is preserved verbatim in the value.
2. **`Signature`/`SignatureLength` mapping (BUG-02/FR-002)**: `data_field_for_length(93)` returns
   `Some(89)`. The decoder's length-prefixed-field handling (`decode.rs:230`) picks this up
   unconditionally — no dictionary-level opt-in needed, matching the other 12 pairs' treatment.
3. **`JdbcURL` scheme recognition (BUG-04/FR-003)**: `is_sql_scheme`/`is_mssql_scheme` additionally
   match `jdbc:postgresql://`/`jdbc:postgres://`/`jdbc:mysql://`/`jdbc:sqlite:`/`jdbc:h2:` (sql-family)
   and `jdbc:sqlserver://` (mssql-family), checked as a distinct group before the existing sqlx-native
   checks. `jdbc_store_config` strips the `jdbc:` prefix before constructing the underlying connection
   string.
4. **Credential splicing (BUG-04/FR-004)**: when the (prefix-stripped) URL's authority has no
   `user:pass@` segment, and both `JdbcUser` and `JdbcPassword` are set in the session's raw `.cfg`
   map, `splice_credentials` inserts them into the URL's authority before it reaches
   `StoreConfig::Sql`/`Mssql`. A URL that already embeds credentials is left untouched (no
   double-splicing, no silent override of an explicit inline credential).
5. **`AcceptorBuilder` wiring (BUG-03/FR-006)**: `Engine::start`'s acceptor branch groups resolved
   acceptor sessions by `rs.address`. A size-1 group with none of `AllowedRemoteAddresses`/
   `DynamicSession`/`AcceptorTemplate` set keeps today's `Acceptor::bind_with` path unchanged. Any
   other group builds one `AcceptorBuilder`, registering every member session, the group's dynamic
   template (if exactly one member declares one), the union of every member's allow-list entries, and
   TLS (if any member enables it) — then serves once for the whole group.
6. **Ambiguous template detection (FR-006, part of BUG-03)**: if more than one session in a group sets
   `DynamicSession=Y`/`AcceptorTemplate`, `Engine::start` returns `ConfigError::AmbiguousAcceptorTemplate
   { addr }` — a typed startup error, never silently resolved by picking one member's template.

## No breaking changes

- `strip_comment`/`is_sql_scheme`/`is_mssql_scheme` keep their existing signatures — behavior-only
  changes, and every existing `.cfg` value that worked before this feature (no `#` after `=`, no
  `jdbc:`-prefixed URLs since none existed before) parses identically.
- `ResolvedSession`'s two new fields and `ConfigError`'s one new variant are additive to `pub`-fields
  structs / a non-exhaustively-matched enum, matching feature 004's own established verification method
  (grep-confirmed no exhaustive external match on either type exists in this workspace).
- `data_field_for_length`'s signature is unchanged; it simply now returns `Some` for one more input.

## Acceptance (maps to spec US1/US2 scenarios)

- A `.cfg` `Password=ab#cd` value round-trips to exactly `ab#cd` (SC-001). ✔
- A `Signature`(89)/`SignatureLength`(93) field pair with an embedded SOH byte decodes byte-identically
  (SC-002). ✔
- An unmodified QuickFIX/J-style `.cfg` (`jdbc:...` URL + separate `JdbcUser`/`JdbcPassword`) starts a
  working SQL-backed session (SC-003). ✔
- `AllowedRemoteAddresses`/`DynamicSession`/`AcceptorTemplate` set in a `.cfg`-only multi-session
  acceptor actually govern connection acceptance and template resolution (SC-004/SC-022). ✔

## Test hooks

- `truefix-config`: `.cfg` parsing tests for `#`-in-value (extends the existing comment-handling test
  in `lib.rs`'s own test module); `JdbcURL` scheme-recognition + credential-splicing mapping tests
  (extends `store_and_log_mapping.rs`'s existing `JdbcURL` test block, US3-feature-004-era).
- `truefix-core`: a decode test round-tripping a `Signature`/`SignatureLength` pair with an embedded
  SOH byte (extends `field_types.rs` or a new `signature_length.rs`).
- `truefix`/`truefix-transport`: an `Engine::start`-level integration test proving a `.cfg`-only
  multi-session acceptor (2+ `[SESSION]` blocks sharing one `SocketAcceptPort`, one with
  `AllowedRemoteAddresses` set) actually enforces the allow-list and resolves the dynamic template
  (extends `multi_dynamic.rs`'s existing pattern, now reachable from `.cfg` instead of only the direct
  `AcceptorBuilder` Rust API).
