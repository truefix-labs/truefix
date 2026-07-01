//! Message construction contract.

use crate::message::Message;

/// Constructs empty messages for a given protocol version and message type.
///
/// The dictionary-backed implementation (typed messages) arrives in `truefix-dict`
/// (Stage S4); this is the core contract skeleton.
pub trait MessageFactory {
    /// Create an empty message for `begin_string` (e.g. `"FIX.4.4"`) and `msg_type`.
    fn create(&self, begin_string: &str, msg_type: &str) -> Message;
}
