//! FAST encoding context.

use std::collections::BTreeMap;

/// Previous-value cache used by FAST stateful operators.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncodingContext {
    previous_values: BTreeMap<u32, Vec<u8>>,
}

impl EncodingContext {
    /// Create an empty encoding context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the previous raw value for `tag`.
    pub fn previous(&self, tag: u32) -> Option<&[u8]> {
        self.previous_values.get(&tag).map(Vec::as_slice)
    }

    /// Store the previous raw value for `tag`.
    pub fn set_previous(&mut self, tag: u32, value: Vec<u8>) {
        self.previous_values.insert(tag, value);
    }

    /// Clear all previous values and emit the FR-013 reset event.
    pub fn reset(&mut self, session: &str, template_id: u32) {
        self.previous_values.clear();
        tracing::warn!(
            target: "truefix_binary",
            session = session,
            template_id = template_id,
            "FAST encoding context reset"
        );
        metrics::counter!("truefix_binary_context_resets_total", "session" => session.to_owned())
            .increment(1);
    }
}
