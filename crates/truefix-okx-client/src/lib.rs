//! Native, typed client for the OKX V5 REST and WebSocket APIs.
//!
//! # Gateway composition
//!
//! This crate is an independent, native OKX client.  It deliberately has no dependency on
//! `truefix-gateway`: applications that use a unified gateway compose an adapter at their own
//! boundary.  The optional projection types expose only the common order, balance, position,
//! fill, and market-data shape.  They are not a replacement for the native records.
//!
//! Product-specific fields (for example, option Greeks, portfolio-margin risk, grid strategy
//! state, RFQ data, and funding/earn attributes) are intentionally non-projectable.  Keep and
//! pass the native OKX value alongside any projection whenever those semantics matter.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod inventory;
pub mod limiter;
pub mod request;
pub mod response;
pub mod services;
pub mod transport;
pub mod types;
pub mod ws;

pub use client::OkxClient;
pub use config::{ClientConfig, Credentials, Environment, LiveTradingConfirmation};
pub use error::{OkxError, OkxResult};
