# truefix-core

Runtime-agnostic FIX field/message model and tag-value codec.

## Current scope

- `Field`, `FieldMap`, `Group`, and `Message`;
- encode/decode with `BeginString`, `BodyLength`, and three-digit `CheckSum` validation;
- frame-length detection, bounded resynchronization, and group-aware decoding;
- typed field accessors, message factories, and `MessageCracker` dispatch.

This crate does not own sockets, schedules, persistence, or session state.

For a normal engine integration, start with the [`truefix`](https://crates.io/crates/truefix)
facade crate. Source and examples are in the
[workspace guide](../../docs/getting-started.md).

```sh
cargo test -p truefix-core
```
