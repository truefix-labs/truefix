//! `truefix` — the engine facade.
//!
//! Re-exports the public surface of the layered crates so integrators depend on one crate.
//! Implement [`Application`] for your callbacks, build a [`SessionConfig`], and run an
//! [`Acceptor`] or [`connect_initiator`].
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

pub use truefix_core::{
    self as core, decode, encode, DecodeError, Field, FieldError, FieldMap, Group, Message,
    MessageCracker, MessageFactory,
};
pub use truefix_session::{
    self as session, Action, Application, Event, Role, Session, SessionConfig, SessionId,
    SessionState,
};
pub use truefix_transport::{self as transport, connect_initiator, Acceptor, SessionHandle};

pub use truefix_config as config;
