# Operation Inventory Contract

The implementation maintains a machine-readable manifest independently derived from
`python-okx@fa8d738`. Each record contains `source_domain`, `source_operation`, `transport`,
`native_entrypoint`, `auth`, `rate_limit_class`, `write_safety`, `test_evidence`, and `status`.

Coverage tests fail when any of the 264 REST baseline operations or real-time `order`,
`batch-orders`, `cancel-order`, `batch-cancel-orders`, `amend-order`, `batch-amend-orders`, or
`mass-cancel` commands lacks a manifest record, native entrypoint, or test evidence. Rust names
may differ; unapproved omissions may not.
