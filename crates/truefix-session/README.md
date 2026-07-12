# truefix-session

The TrueFix session layer: sequencing, resend handling, scheduling, and session-state transitions.
It is responsible for protocol session semantics and is composed with storage, logging, and
transport crates by the facade.

Most applications should depend on the [`truefix`](https://crates.io/crates/truefix) facade crate.
See the [TrueFix repository](https://github.com/truefix-labs/truefix) for engine examples.
