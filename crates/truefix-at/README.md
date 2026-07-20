# truefix-at

Maintainer-facing black-box acceptance suite for the TrueFix FIX engine. It starts a real
`truefix-transport` acceptor, drives scripted client steps, and reports each scenario/version run.
Applications normally do not depend on this crate.

## Public surface

The crate exports `Scenario`, `Step`, `ExpectMsg`, `ScenarioResult`, `run_scenario`, `run_report`,
and `start_acceptor`. Scenarios cover framing, session behavior, dictionaries, configuration, and
transport behavior across the versions declared by each scenario.

## Verify

```sh
cargo test -p truefix-at
cargo test -p truefix-at --test conformance
```

See the [workspace guide](../../docs/getting-started.md) and [acceptance history](../../docs/acceptance-record.md).
