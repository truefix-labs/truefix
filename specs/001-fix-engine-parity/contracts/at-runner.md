# Contract: truefix-at (Acceptance Test suite)

Covers FR-M1…M3, US12. Highest-priority verification (Constitution II/V). Release gate.

## Scenario format
- An `AtScenario` is an ordered list of steps, authored as an independent black-box fixture
  (Constitution III — no copied source/runner from QuickFIX/J):
  - `configure_session(settings)`
  - `connect` / `disconnect`
  - `initiate(message)` — send a wire message to the engine under test
  - `expect(message_pattern)` — assert the next outbound message matches (fields/values, with allowed
    wildcards for timestamps/seq where the reference suite does)
  - `expect_disconnect`
  - `comment(text)`
- Scenarios are tagged with the FIX version(s) they target.

## Pass criteria
- A scenario **passes** iff the engine's outbound messages and disconnect behavior match the scenario's
  expected steps exactly (US12). Timestamp/seq wildcards follow the reference suite's matching rules.

## Coverage & gate
- Port the **73 distinct server scenarios** (Appendix B) across their applicable versions
  (fix40/41/42/43/44/50/fixLatest) plus the special-category suites (nextExpectedMsgSeqNum,
  lastMsgSeqNumProcessed, resendRequestChunkSize, validateChecksum, rejectGarbledMessages, timestamps,
  resynch).
- **Release gate (FR-M3)**: every targeted FIX version must pass its in-scope scenarios. The only
  permitted exception is an explicitly-listed, justified per-scenario deferral (FR-M2).

## Acceptance hooks
- AT runner executes against a real `truefix-transport` acceptor + the facade Application.
- CI job runs the full AT matrix; a coverage report lists scenario × version pass/deferred status.
