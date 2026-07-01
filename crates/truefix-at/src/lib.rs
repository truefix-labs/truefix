//! `truefix-at` — the Acceptance Test (AT) conformance suite and runner.
//!
//! The runner drives a real [`truefix-transport`](truefix_transport) acceptor as a black-box,
//! scripting a client connection that sends messages and asserts the engine's responses (and
//! disconnect behaviour). Scenarios are authored as independent behaviour contracts — no
//! QuickFIX/J source or test code is copied (Constitution Principle III).
//!
//! Per-version applicability is expressed by tagging each scenario with the FIX versions it
//! targets; [`run_report`] runs the matrix and reports pass/fail.
//!
//! Design: `specs/001-fix-engine-parity/`.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod runner;
pub mod scenarios;

pub use runner::{
    run_report, run_scenario, start_acceptor, ExpectMsg, Scenario, ScenarioResult, Step,
};
