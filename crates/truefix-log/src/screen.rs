//! Console log.

use crate::{is_heartbeat, Log};

/// Output switches for [`ScreenLog`] (FR-026): `ScreenLogShowEvents`/`ScreenLogShowHeartBeats`/
/// `ScreenLogShowIncoming`/`ScreenLogShowOutgoing`/`ScreenIncludeMilliseconds`.
#[derive(Debug, Clone, Copy)]
pub struct ScreenLogOptions {
    /// `ScreenLogShowEvents`: whether event lines are printed at all.
    pub show_events: bool,
    /// `ScreenLogShowHeartBeats`: whether Heartbeat (`35=0`) messages are printed.
    pub show_heartbeats: bool,
    /// `ScreenLogShowIncoming`: whether inbound messages are printed at all.
    pub show_incoming: bool,
    /// `ScreenLogShowOutgoing`: whether outbound messages are printed at all.
    pub show_outgoing: bool,
    /// `ScreenIncludeMilliseconds`: whether the timestamp prefix includes milliseconds.
    pub include_milliseconds: bool,
}

impl Default for ScreenLogOptions {
    fn default() -> Self {
        Self {
            show_events: true,
            show_heartbeats: true,
            show_incoming: true,
            show_outgoing: true,
            include_milliseconds: false,
        }
    }
}

/// Logs messages to stdout and events to stderr, with direction prefixes.
pub struct ScreenLog {
    options: ScreenLogOptions,
}

impl Default for ScreenLog {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenLog {
    /// Create a screen log with default (unfiltered) output switches.
    pub fn new() -> Self {
        Self::with_options(ScreenLogOptions::default())
    }

    /// Create a screen log honoring `options` (FR-026).
    pub fn with_options(options: ScreenLogOptions) -> Self {
        Self { options }
    }

    fn timestamp_prefix(&self) -> String {
        crate::file::format_timestamp_prefix(self.options.include_milliseconds)
    }
}

impl Log for ScreenLog {
    fn on_incoming(&self, message: &str) {
        if !self.options.show_incoming {
            return;
        }
        if is_heartbeat(message) && !self.options.show_heartbeats {
            return;
        }
        println!("{}<incoming> {message}", self.timestamp_prefix());
    }
    fn on_outgoing(&self, message: &str) {
        if !self.options.show_outgoing {
            return;
        }
        if is_heartbeat(message) && !self.options.show_heartbeats {
            return;
        }
        println!("{}<outgoing> {message}", self.timestamp_prefix());
    }
    fn on_event(&self, text: &str) {
        if !self.options.show_events {
            return;
        }
        eprintln!("{}<event> {text}", self.timestamp_prefix());
    }
}
