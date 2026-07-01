//! Session-ID-prefixing log decorator (`SLF4JLogPrependSessionID` and equivalents, FR-026).

use crate::Log;

/// Wraps any [`Log`] implementation, prepending `[{session_id}] ` to every message/event before
/// delegating. Generic over the wrapped log so it composes with any sink (screen/file/SQL/
/// tracing/composite) without each implementation needing its own session-ID-prefix option.
pub struct SessionPrefixLog<L: Log> {
    session_id: String,
    inner: L,
}

impl<L: Log> SessionPrefixLog<L> {
    /// Wrap `inner`, prefixing every entry with `session_id`.
    pub fn new(session_id: impl Into<String>, inner: L) -> Self {
        Self {
            session_id: session_id.into(),
            inner,
        }
    }
}

impl<L: Log> Log for SessionPrefixLog<L> {
    fn on_incoming(&self, message: &str) {
        self.inner
            .on_incoming(&format!("[{}] {message}", self.session_id));
    }
    fn on_outgoing(&self, message: &str) {
        self.inner
            .on_outgoing(&format!("[{}] {message}", self.session_id));
    }
    fn on_event(&self, text: &str) {
        self.inner
            .on_event(&format!("[{}] {text}", self.session_id));
    }
}
