//! The integrator callback surface.

use truefix_core::Message;

use crate::session_id::SessionId;

/// Application callbacks invoked by the engine over a session's lifecycle.
///
/// All methods are async (FR-F7). Default implementations are no-ops so simple integrators
/// only override what they need. Inbound/outbound *admin* messages flow through
/// [`from_admin`](Application::from_admin) / [`to_admin`](Application::to_admin); application
/// messages through [`from_app`](Application::from_app) / [`to_app`](Application::to_app).
#[async_trait::async_trait]
pub trait Application: Send + Sync {
    /// Called once when a session is created.
    async fn on_create(&self, _session: &SessionId) {}

    /// Called when the session successfully logs on.
    async fn on_logon(&self, _session: &SessionId) {}

    /// Called when the session logs out or disconnects.
    async fn on_logout(&self, _session: &SessionId) {}

    /// Called before an outbound admin message is sent (allows mutation, e.g. credentials).
    async fn to_admin(&self, _message: &mut Message, _session: &SessionId) {}

    /// Called for an inbound admin message. Returning `Err` rejects the message
    /// (e.g. failed authentication on Logon).
    // `from_admin` is the established FIX callback name; it intentionally takes `&self`.
    #[allow(clippy::wrong_self_convention)]
    async fn from_admin(&self, _message: &Message, _session: &SessionId) -> Result<(), String> {
        Ok(())
    }

    /// Called before an outbound application message is sent.
    async fn to_app(&self, _message: &mut Message, _session: &SessionId) {}

    /// Called for an inbound application message.
    // `from_app` is the established FIX callback name; it intentionally takes `&self`.
    #[allow(clippy::wrong_self_convention)]
    async fn from_app(&self, _message: &Message, _session: &SessionId) -> Result<(), String> {
        Ok(())
    }
}
