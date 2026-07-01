//! `truefix` — the engine facade.
//!
//! Re-exports the public surface of the layered crates so integrators depend on one crate.
//! Implement [`Application`] for your callbacks, build a [`SessionConfig`], and run an
//! [`Acceptor`] or [`connect_initiator`].
//!
//! Design: `specs/001-fix-engine-parity/`.
#![deny(missing_docs)]
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

use std::sync::Arc;

use tokio::task::JoinHandle;

use truefix_config::{ConnectionType, SessionSettings};
use truefix_transport::{connect_initiator_with, Services};

/// An error starting the engine from configuration (FR-015).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// A configuration mapping error (missing/invalid key, unknown ConnectionType).
    #[error(transparent)]
    Config(#[from] truefix_config::ConfigError),
    /// A message store could not be built.
    #[error("store: {0}")]
    Store(String),
    /// A network bind/connect failure.
    #[error("io: {0}")]
    Io(String),
}

/// A running engine: the started acceptor listeners and initiator sessions (FR-013/014).
///
/// Built from a [`SessionSettings`] via [`Engine::start`]; each `[SESSION]` is routed by its
/// `ConnectionType` to an acceptor or initiator with a store built from its configuration.
pub struct Engine {
    acceptors: Vec<JoinHandle<()>>,
    initiators: Vec<SessionHandle>,
}

impl Engine {
    /// Build and start every session described by `settings`, sharing `app` across them.
    ///
    /// All-or-nothing: a configuration-resolution error returns before anything is started
    /// (FR-015). Each session gets a store built from its `FileStorePath`/store keys.
    pub async fn start<A: Application + 'static>(
        settings: &SessionSettings,
        app: Arc<A>,
    ) -> Result<Self, EngineError> {
        let resolved = settings.resolve()?;
        let mut acceptors = Vec::new();
        let mut initiators = Vec::new();
        for rs in resolved {
            let store = truefix_store::build_store(&rs.store)
                .await
                .map_err(|e| EngineError::Store(e.to_string()))?;
            let services = Services {
                store: Some(Arc::from(store)),
                ..Services::default()
            };
            match rs.connection {
                ConnectionType::Acceptor => {
                    let acc = Acceptor::bind_with(rs.address, rs.session, app.clone(), services)
                        .await
                        .map_err(|e| EngineError::Io(e.to_string()))?;
                    acceptors.push(acc.serve());
                }
                ConnectionType::Initiator => {
                    let handle =
                        connect_initiator_with(rs.address, rs.session, app.clone(), services)
                            .await
                            .map_err(|e| EngineError::Io(e.to_string()))?;
                    initiators.push(handle);
                }
            }
        }
        Ok(Self {
            acceptors,
            initiators,
        })
    }

    /// The started initiator session handles (for logout/join).
    pub fn initiators(&self) -> &[SessionHandle] {
        &self.initiators
    }

    /// Abort all acceptor listeners.
    pub fn shutdown(&self) {
        for acceptor in &self.acceptors {
            acceptor.abort();
        }
    }
}
