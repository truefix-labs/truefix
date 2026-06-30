//! Console log.

use crate::Log;

/// Logs messages to stdout and events to stderr, with direction prefixes.
#[derive(Default)]
pub struct ScreenLog;

impl ScreenLog {
    /// Create a screen log.
    pub fn new() -> Self {
        Self
    }
}

impl Log for ScreenLog {
    fn on_incoming(&self, message: &str) {
        println!("<incoming> {message}");
    }
    fn on_outgoing(&self, message: &str) {
        println!("<outgoing> {message}");
    }
    fn on_event(&self, text: &str) {
        eprintln!("<event> {text}");
    }
}
