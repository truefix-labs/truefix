# truefix-binary

FAST and SBE binary-protocol codecs over `truefix_core::Message`.

Add it when an integration needs a binary codec:

```toml
truefix-binary = "0.1.4"
```

## Current scope

- `BinaryCodec` provides template-ID based encode/decode.
- `fast` implements FAST templates, presence maps, operators, sequences, decimals, and state.
- `sbe` implements SBE schema parsing and fixed/variable-length message encoding.
- `ir` converts between binary schema values and the shared FIX message model.

This crate is a codec layer, not a multicast/session protocol implementation.

## Verify

```sh
cargo test -p truefix-binary
```
