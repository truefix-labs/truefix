//! `truefix-session` — the FIX session layer.
//!
//! Stage S2 provides the sans-IO [`Session`] state machine (logon/heartbeat/test-request/
//! logout), the [`Application`] callback trait, [`SessionId`], and [`SessionConfig`].
//! Sequence-gap recovery, persistence, scheduling, and the remaining session semantics arrive
//! in later stages.
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

mod admin;
pub mod application;
pub mod config;
pub mod session_id;
pub mod state;
mod tags;
mod time_util;

pub use application::Application;
pub use config::{Role, SessionConfig};
pub use session_id::SessionId;
pub use state::{Action, Event, Session, SessionState};
