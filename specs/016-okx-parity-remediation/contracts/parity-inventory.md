# Parity Inventory Contract

Each Python-baseline operation has exactly one record with source identity, current protocol method/path, authentication class, replay classification, Rust entrypoint, and local fixture ID. A record cannot be complete when any field is missing. Rust-only server endpoints are invalid; convenience methods must map only to approved records.

Authenticated REST requests use UTC timestamps with exactly three fractional millisecond digits.
Every request declares JSON content type and explicitly sends `x-simulated-trading` as `1` for
Demo or `0` for live. Empty query values are omitted before signing. Safe reads may retry one
transient failure after rate-limit throttling; writes are never automatically replayed.
