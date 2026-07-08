//! FAST and SBE binary protocol codecs for TrueFix.
//!
//! This crate exposes a shared [`BinaryCodec`] trait implemented by the FAST and SBE codec
//! modules. Codecs operate directly on [`truefix_core::Message`], preserving TrueFix's existing
//! wire-format-neutral field and group representation.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use truefix_core::Message;

/// FAST binary protocol support.
pub mod fast;
/// Message conversion helpers shared by binary codecs.
pub mod ir;
/// SBE binary protocol support.
pub mod sbe;

/// A binary protocol codec that encodes and decodes TrueFix messages by template identifier.
pub trait BinaryCodec {
    /// The typed error returned by this codec.
    type Error: std::error::Error;

    /// Encode a message using `template_id`.
    fn encode(&self, message: &Message, template_id: u32) -> Result<Vec<u8>, Self::Error>;

    /// Decode one binary message, returning the decoded [`Message`] and inline template id.
    fn decode(&self, bytes: &[u8]) -> Result<(Message, u32), Self::Error>;
}
