//! FIX transport: initiator/acceptor, TLS, multi/dynamic sessions.
//!
//! Part of the TrueFix FIX engine. Design: `specs/001-fix-engine-parity/`.
//! This crate is scaffolding (Stage S0); functionality lands in later stages.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]
