//! The SOH wire codec: [`encode`] and [`decode`].

mod decode;
mod encode;

pub use decode::{decode, decode_with_groups};
pub use encode::encode;
