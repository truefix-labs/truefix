# truefix-at

Acceptance and conformance tests for the TrueFix FIX engine. It validates wire framing, session
behavior, dictionaries, configuration, and transport behavior across the supported FIX versions.

This is a maintainer-facing crate, not a normal application dependency. Run the suite from a source
checkout with `cargo test -p truefix-at --test conformance`.

- [Repository and release notes](https://github.com/truefix-labs/truefix)
- [Engine documentation](https://github.com/truefix-labs/truefix/blob/main/docs/getting-started.md)
