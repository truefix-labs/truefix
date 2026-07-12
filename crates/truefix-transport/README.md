# truefix-transport

The TrueFix initiator/acceptor transport layer, with TLS, reconnects, multi-session operation, and
dynamic session support. It combines the session, configuration, storage, logging, dictionary, and
binary crates into Tokio-based network runtimes.

Most applications should depend on the [`truefix`](https://crates.io/crates/truefix) facade crate.
See the [TrueFix repository](https://github.com/truefix-labs/truefix) for TLS and engine examples.
