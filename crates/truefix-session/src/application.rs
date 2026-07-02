//! The integrator callback surface.
//!
//! **Breaking change (feature 002 / FR-016)**: the fault-returning callbacks now return typed
//! outcomes (`Reject`/`DoNotSend`/`BusinessReject`) instead of `Result<(), String>`. See
//! `MIGRATION.md`.

use truefix_core::{BusinessReject, DoNotSend, Message, Reject};

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

    /// Called immediately before a full reset (explicit `Session::reset()`, or an internally
    /// triggered one — logon-time `ResetSeqNumFlag`, `ResetOnLogout`/`ResetOnDisconnect`,
    /// `ForceResendWhenCorruptedStore`) clears the durable store, so an integrator can capture or
    /// react to pre-reset state (US10, FR-013). Fired by the transport layer, since `Session`
    /// itself is deliberately sans-IO and never holds an `Application` handle — the same sans-IO
    /// boundary `Action::ResetStore` already applies to the store reset itself.
    async fn on_before_reset(&self, _session: &SessionId) {}

    /// Called before an outbound admin message is sent (allows mutation, e.g. credentials).
    async fn to_admin(&self, _message: &mut Message, _session: &SessionId) {}

    /// Called for an inbound admin message. Returning `Err(Reject)` refuses the session: on a
    /// Logon, the engine sends a Logout carrying the reject's text (if any) and disconnects
    /// (e.g. failed authentication) (FR-016).
    // `from_admin` is the established FIX callback name; it intentionally takes `&self`.
    #[allow(clippy::wrong_self_convention)]
    async fn from_admin(&self, _message: &Message, _session: &SessionId) -> Result<(), Reject> {
        Ok(())
    }

    /// Called before an outbound application message is sent. Returning `Err(DoNotSend)`
    /// suppresses the message: it is not written to the wire and not persisted as sent (FR-016).
    async fn to_app(&self, _message: &mut Message, _session: &SessionId) -> Result<(), DoNotSend> {
        Ok(())
    }

    /// Called for an inbound application message. Returning `Err(BusinessReject)` makes the
    /// engine emit a Business Message Reject (35=j) carrying the given reason and reference tag
    /// (FR-016).
    // `from_app` is the established FIX callback name; it intentionally takes `&self`.
    #[allow(clippy::wrong_self_convention)]
    async fn from_app(
        &self,
        _message: &Message,
        _session: &SessionId,
    ) -> Result<(), BusinessReject> {
        Ok(())
    }
}
