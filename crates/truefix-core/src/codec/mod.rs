//! The SOH wire codec: [`encode`] and [`decode`].

mod decode;
mod encode;

pub use decode::{decode, decode_with_groups, restructure_groups};
pub use encode::{encode, encode_with_order};
