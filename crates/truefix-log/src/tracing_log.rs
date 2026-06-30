//! `tracing`-facade log.

use crate::Log;

/// Logs via the `tracing` facade, separating message and event targets.
#[derive(Default)]
pub struct TracingLog;

impl TracingLog {
    /// Create a tracing log.
    pub fn new() -> Self {
        Self
    }
}

impl Log for TracingLog {
    fn on_incoming(&self, message: &str) {
        tracing::info!(target: "truefix::message", direction = "incoming", %message);
    }
    fn on_outgoing(&self, message: &str) {
        tracing::info!(target: "truefix::message", direction = "outgoing", %message);
    }
    fn on_event(&self, text: &str) {
        tracing::info!(target: "truefix::event", %text);
    }
}
