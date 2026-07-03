# Contract: Tooling / Latent-Risk Hygiene (US10)

**Covers**: FR-044 through FR-046. **Source**: `docs/todo/003.md` `GAP-50`/`BUG-18`, `GAP-51`,
`BUG-20`. **Grounding**: research.md §R10.

**Files**: `crates/truefix-dict/src/fix_repository.rs`, `crates/truefix-store/src/file.rs`.

## Behavioral contract changes

| # | Change | Contract |
|---|---|---|
| 1 | `flatten_members` | Gains a depth parameter/guard (bound 16, matching sibling `resolve_entries`) — a self-referencing component chain now fails with a typed error instead of recursing unboundedly. Not live against any of the 9 vendored dictionary sources today; closes the gap before a future/different Repository edition could trigger it. |
| 2 | `parse_messages` doc comment | Corrected to describe the actual `id_by_name`/`by_id` registration behavior (or lack thereof) — no behavior change, doc-accuracy only |
| 3 | `BodyLog::append` | Offset determination moved inside the same lock/critical-section as the write and index-insert, closing a TOCTOU window. Not live against any current call site (all sequential today) — closes the gap before a future concurrent caller is added. |

## No breaking changes

All three are internal-only (private functions/fields, doc comments). No public API affected.

## Out of scope (informational only, per research.md §R10.3)

`GAP-52` (O(fields × enums) enum-emission complexity in `fix_repository.rs::convert`) has no FR in
spec.md's US10 — build-time-only, non-correctness, lowest audit priority. May be addressed
opportunistically while touching `fix_repository.rs` for item 1 above, but is not a required
deliverable of this feature.

## Acceptor/Initiator parity

Not applicable — all three items are tooling/store-internals with no session-role distinction.
