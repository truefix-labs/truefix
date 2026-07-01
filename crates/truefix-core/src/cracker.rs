//! Typed message dispatch contract.

use crate::message::Message;

/// Dispatches an inbound message to a per-message-type handler.
///
/// Full typed dispatch over the generated messages arrives with the dictionary codegen
/// (Stage S4); this is the core contract skeleton.
pub trait MessageCracker {
    /// Error a handler may return (e.g. a business reject).
    type Error;

    /// Dispatch `message` to the handler for its message type.
    fn crack(&mut self, message: &Message) -> Result<(), Self::Error>;
}
