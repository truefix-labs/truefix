# Phase 1 Contracts: Public API Changes

TrueFix is a Rust library; its "interface contract" is the public API surface exposed to
downstream crates. This document lists the additive/changed public items each finding requires, so
the tasks phase and implementation stay within a known, reviewable surface (Constitution Principle
I: public API MUST be stable and documented; breaking changes MUST follow semver and record
migration notes).

## `truefix-log`

- `FileLogOptions` (existing, `crates/truefix-log/src/file.rs`): gains a new field carrying the
  retention policy (`NEW-157`). **Additive**: existing callers constructing `FileLogOptions` via
  `..Default::default()` (the crate's established construction idiom) are unaffected; callers using
  exhaustive struct-literal construction without `..Default::default()` will need a source update
  (documented as a migration note per Principle I, since `FileLogOptions` is `pub` and currently has
  no `#[non_exhaustive]`).
- `FileLog`/`Log for FileLog`: **behavior-only change** (`NEW-156`) — no signature change.
  `Log::shutdown()` (already part of the trait, default no-op) gains a real `FileLog` override,
  matching the existing override pattern on `SqlLog`/`MssqlLog`/`MongoLog`/`RedbLog`.
- A new `tracing`/`metrics` observability emission point inside the background writer (`NEW-156`
  clarification: FR-009) — internal behavior, not a new public item.

## `truefix-store`

- `StoreConfig` (existing, `crates/truefix-store/src/lib.rs`): new variant `Custom(Arc<dyn
  MessageStore>)` (`NEW-158`). **Additive** to the enum's variant set, but **behavior-changing** for
  any exhaustive `match` on `StoreConfig` in downstream code (the enum has no
  `#[non_exhaustive]` today) — a compile-time break for exhaustive matchers, documented as a
  migration note; existing matches with a `_ =>` arm are unaffected.
- `build_store` (existing, `crates/truefix-store/src/lib.rs:188`): gains a `Custom` arm
  (pass-through); no signature change.

## `truefix-config`

- `ResolvedSession` (existing, `crates/truefix-config/src/builder.rs`): new field for a
  caller-supplied custom log (`NEW-158`, see `data-model.md`). **Additive** field on a
  `#[derive(Debug, Clone)]` struct with no existing `#[non_exhaustive]` — same struct-literal
  caveat as `FileLogOptions`.
- `ConfigError` (existing): new variant for the fail-fast validation added by `NEW-158`/FR-010.
  **Additive**, same exhaustive-match caveat.

## `truefix` (top-level `Engine`)

- New `Engine` entry point layering a caller-supplied override (`Services`-shaped) on top of
  `.cfg`-resolved configuration (`NEW-158`, FR-006/FR-012) — purely additive, `Engine::start`'s
  existing signature and behavior are unchanged for callers who don't use the new entry point.
- README/quickstart documentation update describing the new entry point and the `Custom(...)`
  variant (FR-007), since this is the discoverability gap the finding exists to close.

## Non-goals for this contract surface

- No changes to the `MessageStore`/`Log` trait definitions themselves (already object-safe,
  already public — the audit's own finding); this feature only adds ways to *supply* an
  implementation, not new trait methods.
- No changes to the wire-level FIX protocol surface (session state machine, message
  encode/decode) — this feature is entirely about logging/storage construction and rotation, not
  protocol behavior, so Constitution Principle II's "cite FIX spec/reference-implementation
  behavior" requirement does not apply to any requirement in this feature.
