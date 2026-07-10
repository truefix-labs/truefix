# Validation Quickstart

Run from repository root:

```sh
python3 crates/truefix-okx-client/scripts/generate_operation_inventory.py
cargo test -p truefix-okx-client --test operation_inventory
cargo test -p truefix-okx-client --test http_contract
cargo test -p truefix-okx-client
cargo fmt --check
cargo clippy -p truefix-okx-client -- -D warnings
```

The generator reads the pinned `thrdpty/clientapi/python-okx` capability source and refuses to
write the Rust manifest unless it extracts exactly 264 operations and every approved path resolves
to a native Rust service method. Commit the generated file together with source/API changes.

Expected inventory failures are explicit: a changed count reports `WrongCount`, repeated source
identity reports `DuplicateSourceIdentity`, absent method/path/auth/replay/fixture evidence reports
`Unclassified`, and a missing Rust entrypoint reports `Unsupported`.

The HTTP contract fixture checks the exact signed millisecond timestamp, JSON and simulation
headers, empty-query omission, corrected path/verb behavior, and retry safety. Its scripted 429
scenario must capture two identical GETs separated by the server's `Retry-After`; the corresponding
POST scenario must capture exactly one request and return `RateLimited` without replay.

Demo/live smoke runs only with securely supplied credentials. Never place API keys, secrets, or
passphrases in source, fixtures, logs, command history, or generated inventory.
