//! The SOH wire codec: [`encode`] and [`decode`].

mod decode;
mod encode;

pub use decode::decode;
pub use encode::encode;
