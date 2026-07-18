//! Native, typed client for the IG REST trading API.
//!
//! The client defaults to IG's demo environment. Selecting production requires an explicit
//! acknowledgement because position and order operations can affect live funds.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod client;
pub mod config;
pub mod error;
pub mod types;

pub use client::IgClient;
pub use config::{
    AuthenticationVersion, ClientConfig, Credentials, Environment, LiveTradingConfirmation,
};
pub use error::{IgError, IgResult};
