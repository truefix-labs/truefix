//! `truefix-session` — the FIX session layer.
//!
//! Stage S2 provides the sans-IO [`Session`] state machine (logon/heartbeat/test-request/
//! logout), the [`Application`] callback trait, [`SessionId`], and [`SessionConfig`].
//! Sequence-gap recovery, persistence, scheduling, and the remaining session semantics arrive
//! in later stages.
//!
//! Audit 006 additions: inbound `PossResend(97)` reaches [`Application::from_app`]/
//! [`Application::from_admin`] unfiltered (NEW-106 — the session layer never pre-filters it), and
//! [`Application::on_cancel_on_disconnect`] gives integrators an operator-visible hook to run
//! their own cancel-on-disconnect workflow using their own order source (NEW-105 — the session
//! layer deliberately does not track open-order state or synthesize business cancels itself).
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
pub mod schedule;
pub mod schedule_reset;
pub mod session_id;
pub mod state;
mod tags;
mod time_util;

pub use application::Application;
pub use config::{Role, SessionConfig, TimeStampPrecision};
pub use schedule::Schedule;
pub use schedule_reset::{
    ScheduleAction, decide as decide_schedule_action, decide_recurring_reset,
};
pub use session_id::SessionId;
pub use state::{Action, DisconnectReason, Event, Session, SessionState, SessionStatus};
