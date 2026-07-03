# Provenance: FIX Unified Repository 2010 Edition

Per Constitution Principle III (License & Provenance Discipline), these files are sourced directly
from the **official** FIX Trading Community distribution, not from any third-party reference engine's
own bundled/redistributed copy (e.g. `thrdpty/quickfix`'s `spec/FIX*.xml`, which ships under the
restrictive "QuickFIX Software License" and is explicitly off-limits as a data source — see
`.gitignore`'s note on `thrdpty/` and `docs/todo/002.md`'s BUG-history for the finding that motivated
this correction).

- **Source**: `https://fixtrading.org/packages/fix-4-0-fix-5-0-sp2-unified-repository/`
  (download package `fix_repository_2010_edition_20200402.zip`, ~2.7 MB, "FIX 4.0 - FIX 5.0 SP2 Unified
  Repository", FIX Trading Community's official repository download page).
- **License**: FIX Trading Community distributes the current-format Unified Repository/Orchestra
  package under the **Apache License, Version 2.0** (confirmed via `fixtrading.org/standards/fix-orchestra/`
  and the `FIXTradingCommunity/fix-repository` GitHub org's own `LICENSE` file, © FIX Protocol Limited).
  The individual per-file XML comment headers additionally carry FIX Protocol Ltd's original
  data-reproduction grant ("FIX Protocol Limited grants permission to... reproduce the FIX Protocol
  specification..., provided that... the specification itself is 'Copyright FIX Protocol Limited'"),
  retained verbatim in each vendored file below — both are satisfied by this vendoring (files are kept
  byte-identical to the official download, with attribution intact).
- **Downloaded**: 2026-07-03.
- **Scope**: `Fields.xml` / `Enums.xml` / `Components.xml` / `Messages.xml` / `MsgContents.xml` /
  `Datatypes.xml` for each of `FIX.4.0`, `FIX.4.1`, `FIX.4.2`, `FIX.4.3`, `FIX.4.4`, `FIX.5.0`,
  `FIX.5.0SP1`, `FIX.5.0SP2`, `FIXT.1.1` — the `Base` edition tree from the 2010 Edition zip (the
  `Abbreviations.xml`/`Categories.xml`/`Sections.xml` metadata files present for some versions are not
  vendored; they're descriptive-only and unused by `fix_repository.rs`'s conversion).
- **Consumed by**: `crates/truefix-dict/src/fix_repository.rs` (`--features dict-tooling`), which
  converts this relational schema (fields/enums/components/messages, joined by numeric `ComponentID`
  and `Tag`, with an `Indent`/`Position`-ordered `MsgContents` structure table) into the normalized
  `.fixdict` grammar — the same output format `orchestra.rs` produces from FIX Orchestra XML. See
  `crates/truefix-dict/dict-src/normalized/*.fixdict` for the generated output actually shipped.

`FIX.Latest` is intentionally **not** sourced from here — the 2010 Edition predates FIX.Latest's
Orchestra-only existence; `FIXLATEST.fixdict` continues to come from `dict-src/orchestra/
FIXLATEST.orchestra.xml` via `orchestra.rs`, unaffected by this directory.
