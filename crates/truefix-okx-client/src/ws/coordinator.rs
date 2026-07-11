//! Applies OKX acknowledgement messages to session and subscription state.

use crate::{
    types::websocket::{RequestId, WsAcknowledgement},
    ws::{
        session::{Session, SessionState},
        subscription::{SubscriptionKey, Subscriptions},
    },
};

/// The state transition performed for one OKX acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcknowledgementOutcome {
    /// The server accepted or rejected a login command.
    Login { success: bool },
    /// A correlated subscribe request completed.
    Subscription {
        request_id: RequestId,
        key: SubscriptionKey,
        success: bool,
    },
    /// An error not otherwise correlated to a subscription request.
    Error { request_id: Option<RequestId> },
    /// OKX requested that the connection be replaced (for example during maintenance).
    Notice,
    /// An acknowledgement that does not mutate session/subscription state.
    Ignored,
}

/// Coordinates protocol acknowledgements with the locally tracked WebSocket state.
///
/// The caller records every outbound subscribe with [`Subscriptions::correlate`], then passes
/// each decoded [`WsAcknowledgement`] to [`Self::apply`]. A successful subscribe becomes routable
/// only after the corresponding acknowledgement arrives.
#[derive(Debug, Default)]
pub struct WsStateCoordinator {
    subscriptions: Subscriptions,
}

impl WsStateCoordinator {
    pub fn new(subscriptions: Subscriptions) -> Self {
        Self { subscriptions }
    }

    pub fn subscriptions(&self) -> &Subscriptions {
        &self.subscriptions
    }

    pub fn subscriptions_mut(&mut self) -> &mut Subscriptions {
        &mut self.subscriptions
    }

    /// Applies one decoded acknowledgement. `code == "0"` is the only successful OKX result.
    pub fn apply(
        &mut self,
        session: &mut Session,
        acknowledgement: &WsAcknowledgement,
    ) -> AcknowledgementOutcome {
        let success = acknowledgement.code.as_deref() == Some("0");
        match acknowledgement.event.as_deref() {
            Some("login") => {
                session.login_acknowledged(success);
                AcknowledgementOutcome::Login { success }
            }
            Some("subscribe") => self.apply_subscription(acknowledgement, success),
            Some("error") => {
                if let Some(request_id) = acknowledgement.request_id.as_ref()
                    && let Some(key) = self.subscriptions.acknowledge_id(request_id, false)
                {
                    return AcknowledgementOutcome::Subscription {
                        request_id: request_id.clone(),
                        key,
                        success: false,
                    };
                }
                // Login replies may be surfaced as a generic error rather than `event: login`.
                if session.state() == SessionState::Authenticating {
                    session.login_acknowledged(false);
                }
                AcknowledgementOutcome::Error {
                    request_id: acknowledgement.request_id.clone(),
                }
            }
            Some("notice") => {
                // A notice asks the client to reconnect; no prior subscriptions are active on
                // the replacement connection, but desired keys remain available for replay.
                self.subscriptions.disconnect();
                session.disconnected();
                AcknowledgementOutcome::Notice
            }
            _ => AcknowledgementOutcome::Ignored,
        }
    }

    fn apply_subscription(
        &mut self,
        acknowledgement: &WsAcknowledgement,
        success: bool,
    ) -> AcknowledgementOutcome {
        let Some(request_id) = acknowledgement.request_id.as_ref() else {
            return AcknowledgementOutcome::Ignored;
        };
        let Some(key) = self.subscriptions.acknowledge_id(request_id, success) else {
            return AcknowledgementOutcome::Ignored;
        };
        AcknowledgementOutcome::Subscription {
            request_id: request_id.clone(),
            key,
            success,
        }
    }
}
