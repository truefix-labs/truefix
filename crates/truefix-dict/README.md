# truefix-dict

FIX data-dictionary parsing, validation, conversion, and typed Rust code generation.

## Current scope

- runtime `DataDictionary` validation for fields, messages, components, and repeating groups;
- normalized `.fixdict` parsing used by runtime and code-generation paths;
- classic QuickFIX XML and FIX Orchestra conversion under `dict-tooling`;
- generated typed messages, enum fields, group entries, factories, and cracker dispatch.

The optional `dict-tooling` feature enables dictionary conversion tooling. See the
[workspace README](../../README.md#truefix-dict-cli) for commands.

```sh
cargo test -p truefix-dict
cargo test -p truefix-dict --features dict-tooling
```
