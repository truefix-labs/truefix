# truefix-core

The foundational TrueFix types: FIX messages and fields, plus SOH framing with correct
`BodyLength` and `CheckSum` handling. It is deliberately transport- and runtime-agnostic, making
it useful to low-level integrations and the other TrueFix crates.

For a normal engine integration, start with the [`truefix`](https://crates.io/crates/truefix)
facade crate. Source and examples are in the
[TrueFix repository](https://github.com/truefix-labs/truefix).
