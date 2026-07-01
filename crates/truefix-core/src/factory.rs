//! Message construction contract.
//!
//! The dictionary-backed, strongly-typed construction lives in `truefix-dict` (feature 002/US6):
//! each targeted version generates a `<Message>::new()` per message type that stamps `MsgType`
//! (e.g. `truefix_dict::fix44::NewOrderSingle::new()`), wrapping a generic [`Message`] so the
//! result encodes byte-identically with the generic codec path (FR-021). This trait is a
//! version-agnostic construction contract for callers who only have `begin_string`/`msg_type` at
//! hand (e.g. a generic factory keyed off dictionary metadata) and don't need the typed accessors.

use crate::message::Message;

/// Constructs empty messages for a given protocol version and message type.
pub trait MessageFactory {
    /// Create an empty message for `begin_string` (e.g. `"FIX.4.4"`) and `msg_type`.
    fn create(&self, begin_string: &str, msg_type: &str) -> Message;
}
