# Contract: Benchmark, Dictionary CLI, Inbound Backpressure, MSSQL/Oracle (US11, US13, US14; FR-014, FR-018–FR-021)

## Surface

```text
// benches/session.rs (new file, cargo bench target)
fn bench_session_round_trip(c: &mut Criterion);   // observation tool only — no CI-gating threshold

// truefix-session::config
struct SessionConfig {
    // ...
    in_chan_capacity: Option<usize>,   // NEW; None = unbounded (today's behavior)
}

// truefix-dict CLI (new binary target)
truefix-dict generate-dict --source <orchestra-or-fpl-path> --out <normalized.fixdict>
truefix-dict generate-code --dict <normalized.fixdict> --out <generated.rs>
truefix-dict validate --dict <normalized.fixdict>   // prints syntax errors or dual-track hash

// truefix-store / truefix-log — new URL-scheme branches
"mssql://..." | "sqlserver://..."   -> tiberius-backed SqlStore/SqlLog
"oracle://..."                       -> deferred pending Phase 0 license review (may be documented-only)
```

## Behaviour

1. `benches/session.rs` measures round-trip latency (message in → processed → response out) for a
   representative message mix; results are for manual regression comparison — **not** a CI pass/fail gate
   (Clarifications).
2. `in_chan_capacity` bounds the **application**-message channel only; a separate, prioritized admin
   channel (see data-model.md) ensures heartbeat/TestRequest/ResendRequest/Logon/Logout traffic is never
   starved by a full application queue. `None` preserves today's unbounded behavior.
3. The `truefix-dict` CLI wraps the same parsing/codegen logic `build.rs` already uses internally — no
   parallel implementation. `validate` reports syntax errors or, on success, the dual-track hash.
4. `SqlStore`/`SqlLog` behave equivalently across PostgreSQL/MySQL/SQLite/MSSQL (and Oracle, if not
   deferred) — same stored-data shape, differing only in the connection/query layer (see
   data-model.md's "Additional SQL Backend Records").

## Acceptance (maps to spec US11/US13/US14 scenarios)

- Benchmark runs reproducibly; two unchanged-code runs show consistent distributions with no numeric
  gate enforced (SC-010). ✔
- Bounded application channel applies backpressure under a slow consumer without starving admin traffic;
  unbounded (default) is unaffected (SC-013). ✔
- CLI generates a `.fixdict` from an Orchestra/FPL source, generates matching typed Rust code, and
  validates a dictionary with its dual-track hash (SC-012). ✔
- SQL store/log suite passes equivalently against MSSQL wherever available in CI/test environments; same
  for Oracle unless downgraded to deferred per the Phase 0 license review (SC-014). ✔

## Test hooks

`cargo bench` run (manual regression comparison, not CI-gating); channel-saturation integration test with
a slow consumer + concurrent admin traffic; CLI subcommand tests against a sample Orchestra fixture and a
sample `.fixdict`; SQL conformance suite parameterized across backends, each gated on service
availability (matching the PostgreSQL/MySQL precedent from feature 002).
