//! Fan-out log.

use crate::Log;

/// Fans every entry out to each wrapped log.
pub struct CompositeLog {
    logs: Vec<Box<dyn Log>>,
}

impl CompositeLog {
    /// Create a composite over `logs`.
    pub fn new(logs: Vec<Box<dyn Log>>) -> Self {
        Self { logs }
    }
}

#[async_trait::async_trait]
impl Log for CompositeLog {
    fn on_incoming(&self, message: &str) {
        for log in &self.logs {
            log.on_incoming(message);
        }
    }
    fn on_outgoing(&self, message: &str) {
        for log in &self.logs {
            log.on_outgoing(message);
        }
    }
    fn on_event(&self, text: &str) {
        for log in &self.logs {
            log.on_event(text);
        }
    }
    async fn shutdown(&self) {
        for log in &self.logs {
            log.shutdown().await;
        }
    }
}
