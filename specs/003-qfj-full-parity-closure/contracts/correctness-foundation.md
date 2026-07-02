# Contract: AT Coverage Expansion & Session-Owned Durable Resend (US1, US2; FR-001–FR-005)

## Surface

```text
// truefix-session
impl Session {
    pub fn with_store(config: SessionConfig, store: Arc<dyn MessageStore>) -> Session;  // NEW
    // existing constructor unchanged (store-less path, unaffected)
}

// truefix-at
pub const SUITE_VERSIONS: &[&str]; // extended: fix40/41/42/43/44/50/50SP1/50SP2 (+fixLatest once US9 lands)
pub fn server_suite() -> Vec<Scenario>;          // extended to 73 entries
pub fn validate_checksum_suite() -> Vec<Scenario>; // NEW
pub fn timestamps_suite() -> Vec<Scenario>;        // NEW
pub fn resynch_suite() -> Vec<Scenario>;           // NEW
```

## Behaviour

1. `Session::with_store` attaches a durable `MessageStore` handle. `build_resend(begin, end)` reads
   sent-message bodies from `store.get(begin, end)` rather than the in-memory `BTreeMap`; `reset()` also
   calls `store.reset()`. A `Session` built without a store behaves exactly as it does today (no
   regression for store-less deployments).
2. Every scenario in `server_suite()`/the three special-category suites is run against **every** FIX
   version it targets in `SUITE_VERSIONS`, using each version's already-normalized dictionary (fix40/41/
   43/50/50SP2 loaders are wired into `truefix-at::runner`; fixLatest is wired once US9's dictionary
   lands).

## Acceptance (maps to spec US1/US2 scenarios)

- Restart process (not reconnect) against the same store → ResendRequest for a pre-restart range replays
  byte-identically with `PossDupFlag=Y` (SC-002). ✔
- `reset()` with a store attached clears store state consistently with in-memory state. ✔
- 73 server scenarios + 3 special-category suites pass across every targeted version (SC-001). ✔

## Test hooks

Session-level integration test: kill and restart the process against a shared file/SQL store, issue a
ResendRequest spanning pre-restart sequence numbers. `truefix-at` runs the full expanded suite in CI
across all targeted versions.
