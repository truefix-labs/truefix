//! Thin Futu OpenD protocol client.
#![recursion_limit = "512"]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub(crate) mod actor;
pub mod client;
pub mod codec;
pub mod error;
pub(crate) mod handle;
pub mod proto_id;
pub mod push;
pub mod quote;
pub mod trade;
pub mod transport;

pub mod pb;
pub mod rpc;

pub use client::{FutuClient, FutuClientConfig};
pub use error::{FutuError, FutuResult};
pub use push::Push;
