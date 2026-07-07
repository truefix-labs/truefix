# Phase 0 Research: Body-Level Repeating Group Decoding & Vendor Dictionary Support

## R1 — `FieldMap::members()` public API shape and the existing name collision

**Read**: `crates/truefix-core/src/field_map.rs`. `Member` (line 8) is `pub(crate)`. A
crate-private `pub(crate) fn members(&self) -> &[Member]` (line ~152) already exists, with exactly
two consumers, both in `crates/truefix-core/src/codec/encode.rs` (lines 72, 128) — the encoder,
which needs full structural order (fields and groups interleaved) to serialize a message correctly.
`.fields()` (public, line 151) deliberately skips `Member::Group` entries and is used broadly
outside this crate.

**Decision**: Add a new public API:

```rust
pub enum MemberRef<'a> {
    Field(&'a Field),
    Group { count_tag: u32, entries: &'a [FieldMap], declared_count: Option<i64> },
}

impl FieldMap {
    pub fn members(&self) -> impl Iterator<Item = MemberRef<'_>> { ... }
}
```

Since the name `members()` is already taken by the crate-private accessor, rename the existing
`pub(crate) fn members(&self) -> &[Member]` to `pub(crate) fn raw_members(&self) -> &[Member]` and
update its two call sites in `encode.rs`. This is a zero-risk, crate-internal-only rename (no
external caller can reference a `pub(crate)` item). `declared_count` is exposed on `MemberRef::Group`
because it is already tracked per-group (NEW-22, feature 009) and callers inspecting structure
generally want to know when a message's declared `NoXxx` count didn't match its actual entry count.

**Alternatives considered**: Name the new API `iter_members()`/`all_fields()` instead of renaming
the old one. Rejected: the proposal (and the original feature-006 research note it quotes) both use
`members()` for the new, public, structure-preserving accessor; keeping that name for the
public-facing API and renaming the obscure private one is the smaller, more natural diff.

## R2 — `classify_buffered`'s single/transport-dictionary case

**Read**: `crates/truefix-transport/src/lib.rs:1300-1389`. `HeaderTrailerGroupsOnly` is a
crate-private wrapper (not re-exported) that filters `GroupSpec::group_of` down to header/trailer
count tags. `impl GroupSpec for DataDictionary` (`truefix-dict/src/validate.rs:9`) is already the
full, unrestricted implementation — body groups included.

**Decision**: Delete `HeaderTrailerGroupsOnly`. Change the dispatch in `classify_buffered` to:

```rust
match (&services.validator, &services.fixt_dictionaries) {
    (Some((dict, _)), _) => decode_with_groups(raw, dict),
    (None, Some((dicts, _))) => decode_with_groups(raw, dicts.transport()),
    (None, None) => decode(raw),
}
```

For the single-`DataDictionary` case (`services.validator`), this is the entire fix: header, body,
and trailer are now all structured against the one dictionary's full group definitions. For the
FIXT case (`services.fixt_dictionaries`), this still only covers session-layer (transport-dictionary)
messages correctly — application-layer messages need R3 below.

**Alternatives considered**: Keep `HeaderTrailerGroupsOnly` and add a body-scoped variant.
Rejected: there is no remaining reason to scope body out once C (R4) removes the flat validator that
was the original reason for the restriction — an unrestricted pass-through of the dictionary itself
is simpler and has no remaining caller depending on the old scoping.

## R3 — FIXT 1.1 application-dictionary body structuring: where can the negotiated `ApplVerID` be reached?

**Read**: `crates/truefix-transport/src/lib.rs:1166-1173` (`read_loop`) takes `services: Services`
**by value** — a per-connection snapshot cloned once at connection start, run in its own tokio task,
independently of the processing loop. `crates/truefix-session/src/state.rs:176`
(`Session::negotiated_appl_ver_id`) is mutable session state, set at `state.rs:1725` only when the
processing loop (not the reader task) processes an inbound Logon carrying `DefaultApplVerID` (tag
1137). `crates/truefix-session/src/state.rs:1110-1131` (`validate_app`) already resolves the correct
per-message application dictionary — `msg.header.get(APPL_VER_ID).or(self.negotiated_appl_ver_id)`,
then `dicts.application_for(appl_ver_id)` (`crates/truefix-dict/src/fixt.rs:53`) — but only *after*
the message has already been decoded (with whatever grouping `classify_buffered` gave it) and handed
to `Session` by the reader task. The reader task itself has no live channel to `Session`'s mutable
`negotiated_appl_ver_id`: they are split into separate tokio tasks by design (US14, feature 009), and
`Services` carries only the immutable `FixtDictionaries` value, not the session's negotiated state.

Two designs were evaluated:

- **(a) Thread negotiated `ApplVerID` back to the reader task.** Add new shared mutable state (e.g.
  an `Arc<ArcSwapOption<String>>` or a `tokio::sync::watch` channel) that the processing loop writes
  to on Logon and `classify_buffered` reads before choosing a dictionary, so body structuring happens
  once, at the true decode point, exactly mirroring the single-dictionary case.
- **(b) Decode flat/transport-scoped in the reader task (as today for the FIXT case), then
  re-structure the already-decoded message's body in the processing loop**, at the point
  `negotiated_appl_ver_id`/the resolved `ApplVerID` is already known — i.e., right where
  `validate_app` already does its dictionary resolution, before that dictionary is used for
  validation and before the message is delivered to the application.

**Decision**: (b). It requires no new cross-task synchronization primitive, keeps the reader/processor
task split exactly as US14 designed it, and reuses `validate_app`'s already-correct
`ApplVerID` resolution precedence (own header tag 1128, else negotiated tag 1137, else
`FixtDictionaries`'s baked-in default) verbatim rather than re-deriving it in a second place. It does
need one new primitive in `truefix-core`: a function that takes an already-decoded, flat `FieldMap`
(a message's `body`) and a `&dyn GroupSpec`, and re-groups its `Member::Field` runs into
`Member::Group` entries wherever a count tag matches — algorithmically the same "count tag +
delimiter + ordered members" logic `decode_section_with_groups` (`truefix-core/src/codec/decode.rs`)
already implements over raw tokens, just applied to an in-memory flat `FieldMap` instead of freshly
tokenized wire bytes. This keeps the "single source of truth" grouping algorithm in one crate
(`truefix-core`), reused by both the decode-time path (R2) and this post-decode correction path,
rather than duplicating group-boundary logic a third time.

Session-layer (admin/transport) messages are unaffected: `validate_app` already special-cases them to
validate against `dicts.transport()` (`BUG-89/FR-011`), and R2 already structures those correctly at
decode time since the transport dictionary needs no per-message resolution.

**Alternatives considered**: (a) was the proposal's own framing ("pick a dictionary during
`classify_buffered` by first extracting `MsgType`/`ApplVerID` from tokenized fields"). It is workable
but adds a new mutable-state handoff between two independently-scheduled tasks purely to save one
in-memory restructuring pass on a comparatively rare message class (FIXT application messages,
already a minority configuration); (b) achieves the same observable outcome (FR-008/FR-009/FR-010,
User Story 2) with strictly less new concurrency surface.

**Implementation-time correction (2026-07-07)**: the exact call site named above — "right where
`validate_app` already does its dictionary resolution, before that dictionary is used for
validation and before the message is delivered to the application" — turned out to be wrong about
*where* delivery happens. `Application::from_admin`/`from_app` are called directly from
`truefix-transport`'s `handle_inbound` (`crates/truefix-transport/src/lib.rs`), **before**
`dispatch`/`Session::handle`/`on_received` ever runs — there is no `Action::Deliver` variant;
`Action` only ever carries `Send`/`Resend`/`Disconnect`/`ResetStore`. A first implementation that
called the new restructuring method from inside `Session::on_received` compiled, and unit-level
reasoning about it looked sound, but an end-to-end acceptor/initiator test proved the application
callback still saw the flat, transport-scoped body — `on_received` runs strictly after delivery,
not before it. The fix: `Session::restructure_fixt_application_body` is `pub`, and
`handle_inbound` calls it directly, right after its existing `would_reject_before_processing`
precheck and before the `is_admin(&msg)`/`from_admin`/`from_app` dispatch — the actual, verified
single correct call site. `validate_app` (inside `on_received`, reached afterward via `dispatch`)
sees the same already-restructured `msg` for free, since `Event::Received(msg)` carries the same
(by-then-mutated) message. See `contracts/decode-and-restructure.md` for the corrected contract.

## R4 — Validation: what actually needs to change once body is structured

**Read**: `crates/truefix-dict/src/validate.rs`. `validate_groups` (line 281) already has **two**
code paths: a structured one (lines 298-305, calling `validate_structured_group` at line 347) added
in feature 007 for `BUG-55/FR-034` — explicitly documented as reachable "when `decode_with_groups`
was called with a `GroupSpec` covering body groups too", which R2/R3 now make the *normal* production
case — and a flat, position-scanning one (lines 307-327, calling `validate_group` at line 383) that
walks `message.body.fields()`/`message.header.fields()` (which never contain `Member::Group`
entries). Once R2/R3 ship, every message reaching `validate()` with a dictionary attached has its
groups already `Member::Group`-structured, so the flat walk's `while let Some(f) = body.get(pos)`
loop never finds a group count tag to act on — it becomes dead code, not a code path still needed
for some other message shape.

A real, previously-latent gap surfaces once body groups are always structured: `present()` (line
586, used by `validate()`'s `mdef.required`/`header_required_tags`/`trailer_required_tags` checks) is
built on `FieldMap::get()`/`contains()`, which only match `Member::Field` entries. A dictionary that
marks a group's own count tag as required at the message level (i.e. the group itself, not merely
its members, is mandatory) would have that requirement silently fail to register as satisfied for a
present-but-structured group, since `.get(count_tag)` returns `None` once that tag's content is a
`Member::Group`. This is unrelated to whether the group's *members* are required (already checked
correctly by `validate_structured_group`'s own `gdef.required` loop) — it is specifically about the
group's count tag being named directly in `mdef.required`/header/trailer-required sets.

**Implementation-time correction (2026-07-07)**: "the flat walk becomes dead code" was wrong.
`DataDictionary::validate` is a general-purpose public API, not exclusively fed by the production
receive path — `group_validation.rs`/`header_group_validation.rs`'s existing test fixtures build
`Message`s directly via `FieldMap::add_field` (flat, never `add_group`/`decode_with_groups`), and a
plain `decode()` with no `GroupSpec` also produces flat content. Removing the flat walk was tried
and immediately regressed four existing tests (`wrong_count_is_rejected`,
`missing_delimiter_is_rejected`, `out_of_order_members_rejected_only_when_toggle_on`,
`nested_group_missing_delimiter_is_rejected`) plus a header-level equivalent — all real,
now-fixed-in-place bugs the deletion would have introduced. The two representations are not
mutually exclusive alternatives from a shared history; they are two genuinely different input
shapes `validate()` must both handle, and the flat walk only ever matches content the structured
loop's `message.header.group(tag)`/`message.body.group(tag)` didn't already consume (a
`Member::Group` is invisible to `.fields()` by construction), so running both unconditionally never
double-processes the same occurrence.

**Decision**:
1. Keep `validate_group` (the flat position-scanning function) and both branches of
   `validate_groups` — the `self.groups.keys()`-driven structured loop (lines 298-305, via
   `validate_structured_group`) for `Member::Group` input, and the flat walk (lines 307-327) for
   `Member::Field` input. Neither is dead; each covers a shape the other cannot see.
2. Fix `present()` to also treat a `Member::Group` with a matching count tag as "present", using the
   new `FieldMap::members()` (R1) instead of `.get()`/`.contains()`. This closes the gap described
   above without changing `DataDictionary::validate`'s public signature.
3. `validate()`'s top-level per-field loop (lines 120-226, walking `.fields()`) is unaffected in
   scope: it already relies on `mdef.member_tags.contains(&tag)` (not raw flat-position adjacency) to
   allow group-member tags to repeat, and per-field type/enum/required checks *inside* a group entry
   are already fully covered by `validate_structured_group`/`check_group_field_value`. No change
   needed there beyond `present()`.

**Pre-existing gap, discovered (not introduced) during implementation, and fixed as part of this
feature — not left as follow-up**: `validate_structured_group` — unchanged by this feature at
first, added in feature 007 — never checked a structured group's declared wire count
(`Member::Group`'s `declared_count`) against its actual `entries.len()`, nor validated member order
within a structured entry; it only checked required-field presence and per-field type/enum. This
gap had existed silently since feature 006/007 first started structuring header/trailer groups on
the production decode path (no existing test exercised count/order detection for *already-structured*
input — every such test, including the header-level `NoHops` mismatch case, built its fixture flat,
exercising the flat `validate_group` walk instead) — but became a **live regression** the moment
R2/R3 made body-level structuring routine on the production receive path: the full AT suite
(`cargo test -p truefix-at`) dropped from 483/483 to 480/483, with `14i_RepeatingGroupCountNotEqual`,
`14j_OutOfOrderRepeatingGroupMembers`, and `QFJ934_MissingDelimiterNestedRepeatingGroup` failing —
proof the gap was live, not theoretical, and squarely inside this feature's FR-006/SC-002
("equivalent accept/reject outcome", "100% of the existing suite still passes") bar. Fixed by:
- Teaching `validate_structured_group` to accept and check `declared_count` (via the new
  `find_structured_group_with_next` helper reading `FieldMap::members()`) against `entries.len()`.
- Rewriting its per-entry walk to iterate the entry's own actual member order (via `.members()`,
  peekable) instead of `gdef.members`' declared order, so out-of-order members are now detected
  (`RepeatingGroupFieldsOutOfOrder`, gated on `validate_unordered_group_fields`, same as the flat
  path).
- A QFJ934-specific refinement: when a group has fewer entries than declared, `build_group` itself
  discards *why* (ran out of wire content vs. a member field appeared where the delimiter was
  expected) — but the symptom survives as this group's very next sibling in the parent's member
  sequence (or, when this group was the last member of its own entry, the *enclosing* group's next
  sibling — `build_group` unwinds every enclosing scope in one pass once a token fails a scope's
  membership check, so the escaped token can surface multiple levels away from the group whose
  entries actually fell short). Peeking that sibling and checking it against this group's own
  `gdef.members` reconstructs QFJ's `FirstFieldInGroupIsDelimiter` signal (reason 15) instead of a
  generic count mismatch (reason 16) for this specific deeply-nested case, matching the flat path's
  behavior for identical input. This peek is narrowly scoped (only fires when entries are
  short *and* the peeked tag is a genuine member of the group in question), so it cannot introduce
  a new false positive for an unrelated, coincidentally-adjacent tag.
- Full suite re-verified green after the fix: `cargo test -p truefix-at` → 483/483 scenario runs
  passed, and `cargo test --workspace --features dict-tooling` shows no regressions elsewhere.

**Alternatives considered**: A full rewrite of `validate()`'s top-level loop to walk `.members()`
recursively (the proposal's original framing, and feature 006 research's `GAP-26` note's own
suggested option). Rejected as broader than necessary: `validate_structured_group` already performs
the equivalent per-field checks for group content; recursively re-walking the same fields through the
top-level loop would either duplicate checks already done or require suppressing them there, adding
risk without a matching behavior gain. The `present()` gap and the count/order gap above are each
fixed directly and narrowly instead.

## R5 — Vendor XML dictionary scope, licensing boundary, and feature gating

**Read**: `crates/truefix-dict/src/fix_repository.rs:1-15` documents that this project already
*removed* a `qfj_xml` module that mechanically converted `thrdpty/quickfix`'s bundled, restrictively-
licensed `spec/FIX*.xml` files, specifically because Constitution Principle III forbids shipping
translations of a third-party FIX engine's private spec/data files. `orchestra.rs`/`fix_repository.rs`
are the replacement pattern: both parse an **independently, differently-licensed** published XML
source and emit `.fixdict` text (`fn convert(...) -> Result<String, Error>`), feeding the existing
`crate::parse()` — no parallel `DataDictionary`-construction path. Both are gated behind the
`dict-tooling` Cargo feature (`crates/truefix-dict/Cargo.toml`), which alone pulls in `quick-xml`,
keeping XML parsing out of the default runtime dependency graph.

**Decision**: `vendor_xml` follows the exact same pattern: `pub fn convert(xml: &str) -> Result<String,
VendorXmlError>`, parsing the nested `<fix><header>/<messages>/<message>/<trailer>/<components>/
<fields>` shape (as published by Binance and other QuickFIX-dialect vendors), emitting `.fixdict`
text fed to `crate::parse()`. This is Binance's own independently-licensed file, parsed via an
independent implementation against the published shape and the public FIX spec — not a translation of
`thrdpty/quickfix`'s bundled data, so it does not reopen the concern that led to `qfj_xml`'s removal;
this reasoning is recorded here explicitly given that history. `vendor_xml` is gated behind the same
`dict-tooling` feature as the other two converters — no new default-graph dependency.

`load_from_file` (`crates/truefix-dict/src/lib.rs:67`) — the single choke point both single- and
FIXT-dual-dictionary `.cfg` loading already goes through (`crates/truefix-config/src/builder.rs:756`,
`load_dictionary_value`) — gets a content-sniffing branch (text starting with `<?xml`/`<fix `) that
calls `vendor_xml::convert` before `parse()`. That branch is itself `#[cfg(feature = "dict-tooling")]`
-gated, since it depends on `vendor_xml`. Consuming binaries must enable `truefix-dict`'s
`dict-tooling` feature for `DataDictionary=<vendor.xml>` to load directly at runtime; without it, a
vendor-XML-shaped file still produces today's ordinary `DictLoadError::Parse` (not a silent partial
load — satisfies FR-016/SC-005), and the file can still be converted offline via the
`dict-tooling`-gated CLI. This is consistent with the project's existing "keep XML parsing out of the
default runtime dependency graph" norm and does not weaken it.

The CLI (`crates/truefix-dict/src/bin/truefix-dict.rs`, already `required-features = ["dict-tooling"]`)
gets `--format vendor-xml` in `generate_dict()`, alongside the existing `orchestra`/`fix-repository`
match arms — same function, same output contract (`.fixdict` text written to `--out`).

**Alternatives considered**: Making `vendor_xml` unconditionally available (not `dict-tooling`-gated)
so `load_from_file` always supports it without a feature flag. Rejected: this would make `quick-xml`
a non-optional dependency of every `truefix-dict` consumer, contradicting the project's explicit,
already-established "XML parsing is build/tooling-time only" boundary (`fix_repository.rs`'s own doc
comment) purely for runtime convenience — a dependency-discipline regression the constitution's
"依赖纪律"/minimality guidance weighs against.

## R6 — Deriving `BeginString`/version from vendor XML

**Read**: `crates/truefix-dict/src/validate.rs:28-31` (`is_legacy_char_lenient`) and the native
`.fixdict` `version FIX.M.N` directive it parses (`parse_fix_begin_string`, line 557) show that
version-dependent runtime behavior already depends on a dictionary's declared version string. Vendor
XML files in this dialect (including Binance's published files) universally declare their FIX version
as attributes on the root element (conventionally `type`/`major`/`minor`, sometimes `servicepack`) —
the same information QuickFIX-family engines use to identify a dictionary's version.

**Decision**: `vendor_xml::convert` reads those root-element attributes and emits a `version FIX.M.N`
(or `FIX.M.NSPk`, when a service pack attribute is present) directive as part of its generated
`.fixdict` text — the same directive line every bundled `.fixdict` already declares — so version-
dependent runtime behavior (legacy CHAR leniency, the `BeginString`-vs-dictionary-version check in
`validate()`) works identically for a vendor-XML-loaded dictionary as for a native one. If the root
element is missing recognizable version attributes, `convert` returns a `VendorXmlError` (FR-016)
rather than emitting an unversioned/guessed directive.

**Alternatives considered**: Treat every vendor-XML-loaded dictionary as unversioned. Rejected:
silently disables `is_legacy_char_lenient`/BeginString-consistency checking for exactly the
dictionaries (older-FIX-version vendor gateways) most likely to need it, with no error signal — the
kind of silent behavior gap this feature is otherwise trying to close.

## R7 — Acceptance-test (AT) impact

**Read**: Constitution Principle II/V requires QuickFIX/J-derived AT scenarios to gate any
session-layer *protocol behavior* change (handshake, sequencing, resend, heartbeat/logout semantics).

**Decision**: No new AT scenarios are required. This feature changes what data is *visible* through
existing structured-group accessors and how a dictionary file can be *loaded* — it does not change
any session-layer protocol behavior (no new message types, no changed handshake/sequencing/resend
semantics, no wire-format change per spec.md FR-004). Existing AT coverage is unaffected and must
continue to pass unchanged (spec.md SC-002).
