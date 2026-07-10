//! Thin TWS/Gateway protocol client.
//!
//! This crate ports the official TWS API client's low-level protocol layer while using `twsapi`
//! naming in TrueFix-owned code.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

/// Connection-oriented TWS API client.
pub mod client;
/// Low-level frame and field encoding helpers.
pub mod comm;
/// Protocol constants.
pub mod constants;
/// Incoming message decoder.
pub mod decoder;
/// Semantic enums for wire-level integer and string values.
pub mod enums;
/// Client-side error types.
pub mod error;
/// TWS callback events.
pub mod events;
/// Incoming and outgoing TWS API message identifiers.
pub mod message;
/// Rust types generated from the official TWS API `.proto` files.
pub mod protobuf {
    #![allow(clippy::tabs_in_doc_comments)]
    include!(concat!(env!("OUT_DIR"), "/protobuf.rs"));
}
/// Request encoders and request payload types.
pub mod requests;
/// Server-version feature gates.
pub mod server_versions;
/// Domain model types used by requests and callbacks.
pub mod types;

pub use client::{ClientConfig, ConnectionState, EventPump, TwsApiClient};
pub use error::{TwsApiError, TwsApiResult};
pub use events::{Event, Wrapper};
