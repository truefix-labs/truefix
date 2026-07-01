//! Metrics export via the `metrics` facade (FR-023; Constitution Principle I observability),
//! complementing the existing [`crate::Monitor`].
//!
//! Signals are labelled by `SessionID` and emitted unconditionally at their natural call sites
//! (independent of whether a [`crate::Monitor`] is configured), so any recorder an operator
//! installs (Prometheus, StatsD, ...) observes them. With no recorder installed, `metrics`' default
//! no-op recorder makes these calls effectively free.

use truefix_session::{SessionId, SessionState, SessionStatus};

fn session_label(id: &SessionId) -> String {
    id.to_string()
}

/// Record the session-state and sequence-number gauges for `id`.
pub(crate) fn record_status(id: &SessionId, status: &SessionStatus) {
    let label = session_label(id);
    let state_value = match status.state {
        SessionState::Disconnected => 0.0,
        SessionState::AwaitingLogon => 1.0,
        SessionState::LoggedOn => 2.0,
        SessionState::AwaitingLogout => 3.0,
    };
    metrics::gauge!("truefix_session_state", "session" => label.clone()).set(state_value);
    metrics::gauge!("truefix_next_sender_seqnum", "session" => label.clone())
        .set(status.next_sender_seq as f64);
    metrics::gauge!("truefix_next_target_seqnum", "session" => label)
        .set(status.next_target_seq as f64);
}

/// Increment the sent-message counter for `id`.
pub(crate) fn record_sent(id: &SessionId) {
    metrics::counter!("truefix_messages_sent_total", "session" => session_label(id)).increment(1);
}

/// Increment the received-message counter for `id`.
pub(crate) fn record_received(id: &SessionId) {
    metrics::counter!("truefix_messages_received_total", "session" => session_label(id))
        .increment(1);
}

/// Increment the reconnect counter for `id`.
pub(crate) fn record_reconnect(id: &SessionId) {
    metrics::counter!("truefix_reconnects_total", "session" => session_label(id)).increment(1);
}
