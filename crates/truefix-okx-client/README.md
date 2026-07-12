# truefix-okx-client

Native typed Rust client for OKX V5 REST and WebSocket APIs. It supports explicit live/demo intent,
signed requests, typed services, and non-replaying writes.

Maintain the operation manifest whenever upstream V5 changes: add the source identity, Rust
entrypoint, auth/replay class and fixture evidence together. Compare domain counts against the
recorded `python-okx@fa8d738` baseline; never copy upstream source or tests.

All write operations are non-replaying. Demo is the default; credentials belong in a secret
provider, never this repository.
