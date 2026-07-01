//! `truefix-core` — FIX message model, field types, and the SOH wire codec.
//!
//! Provides [`Field`], [`FieldMap`], [`Group`], and [`Message`] (header/body/trailer),
//! plus [`encode`]/[`decode`] that compute and verify BodyLength (tag 9) and CheckSum
//! (tag 10). All parsing returns typed errors and never panics (Constitution Principle I).
//!
//! Repeating-group *structure* is modelled here; dictionary-driven group *parsing* from a
//! flat wire message arrives with `truefix-dict` (Stage S4). At this layer, decoding yields
//! flat ordered fields, which still round-trips byte-for-byte.
//!
//! Design: `specs/001-fix-engine-parity/`.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod codec;
pub mod cracker;
pub mod error;
pub mod factory;
pub mod field;
pub mod field_map;
pub mod framing;
pub mod group;
pub mod message;
pub mod tags;

pub use codec::{decode, encode};
pub use cracker::MessageCracker;
pub use error::{DecodeError, FieldError};
pub use factory::MessageFactory;
pub use field::Field;
pub use field_map::FieldMap;
pub use framing::frame_length;
pub use group::Group;
pub use message::Message;
pub use tags::SOH;
