//! `tracing`-facade log (the SLF4J-equivalent).

use crate::{is_heartbeat, Log};

/// Output switches for [`TracingLog`] (FR-026): `SLF4JLogHeartbeats`. Session-ID prefixing
/// (`SLF4JLogPrependSessionID`) is provided generically by wrapping in [`crate::SessionPrefixLog`]
/// rather than as a field here.
#[derive(Debug, Clone, Copy)]
pub struct TracingLogOptions {
    /// `SLF4JLogHeartbeats`: whether Heartbeat (`35=0`) messages are emitted.
    pub include_heartbeats: bool,
}

impl Default for TracingLogOptions {
    fn default() -> Self {
        Self {
            include_heartbeats: true,
        }
    }
}

/// Logs via the `tracing` facade, separating message and event targets.
pub struct TracingLog {
    options: TracingLogOptions,
}

impl Default for TracingLog {
    fn default() -> Self {
        Self::new()
    }
}

impl TracingLog {
    /// Create a tracing log with default (unfiltered) output switches.
    pub fn new() -> Self {
        Self::with_options(TracingLogOptions::default())
    }

    /// Create a tracing log honoring `options` (FR-026).
    pub fn with_options(options: TracingLogOptions) -> Self {
        Self { options }
    }
}

impl Log for TracingLog {
    fn on_incoming(&self, message: &str) {
        if is_heartbeat(message) && !self.options.include_heartbeats {
            return;
        }
        tracing::info!(target: "truefix::message", direction = "incoming", %message);
    }
    fn on_outgoing(&self, message: &str) {
        if is_heartbeat(message) && !self.options.include_heartbeats {
            return;
        }
        tracing::info!(target: "truefix::message", direction = "outgoing", %message);
    }
    fn on_event(&self, text: &str) {
        tracing::info!(target: "truefix::event", %text);
    }
}
