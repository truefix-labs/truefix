//! Typed message dispatch contract.
//!
//! The concrete, generated dispatch mechanism lives in `truefix-dict` (feature 002/US6,
//! FR-020/022): each targeted version generates a `<Version>MessageHandler` trait (one method per
//! message type, default no-op) and a `crack_<version>(message, &mut handler) -> bool` function
//! that routes an inbound [`Message`] to the matching method by `(BeginString, MsgType)` — e.g.
//! `truefix_dict::fix44::crack_fix44`. This trait is a version-agnostic marker for integrators who
//! want a single type across versions; implement it by delegating to the generated `crack_*`
//! function(s) for the versions you support.

use crate::message::Message;

/// A version-agnostic marker for a type that dispatches inbound messages to typed handlers.
///
/// Implementors typically delegate to one or more generated `crack_<version>` functions from
/// `truefix-dict` (see the module docs above).
pub trait MessageCracker {
    /// Attempt to dispatch `message` to a typed handler. Returns whether a handler matched.
    fn crack(&mut self, message: &Message) -> bool;
}
