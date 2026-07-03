//! `truefix-log` — pluggable FIX logging with message/event stream separation (FR-H2).
//!
//! The [`Log`] trait separates **message** logging (inbound/outbound wire messages) from
//! **event** logging (session lifecycle, errors). Implementations: [`ScreenLog`], [`FileLog`],
//! [`TracingLog`], and [`CompositeLog`] (fan-out).
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

mod composite;
mod file;
#[cfg(feature = "mongodb")]
mod mongo;
#[cfg(feature = "mssql")]
mod mssql;
mod prefix;
#[cfg(feature = "redb")]
mod redb;
mod screen;
#[cfg(feature = "sql")]
mod sql;
mod tracing_log;

use std::path::PathBuf;

use thiserror::Error;

pub use composite::CompositeLog;
pub use file::{FileLog, FileLogOptions};
#[cfg(feature = "mongodb")]
pub use mongo::{MongoLog, MongoLogConfig};
#[cfg(feature = "mssql")]
pub use mssql::{MssqlLog, MssqlLogConfig};
pub use prefix::SessionPrefixLog;
#[cfg(feature = "redb")]
pub use redb::{RedbLog, RedbLogConfig};
pub use screen::{ScreenLog, ScreenLogOptions};
#[cfg(feature = "sql")]
pub use sql::{SqlLog, SqlLogConfig, SqlLogPoolOptions};
pub use tracing_log::{TracingLog, TracingLogOptions};

/// Whether a raw FIX wire message is a Heartbeat (`35=0`), used by the `*LogHeartbeats`/
/// `*ShowHeartBeats` switches (FR-026). Matches on an exact SOH-delimited field to avoid
/// false positives from substrings like `3510=0`.
pub(crate) fn is_heartbeat(message: &str) -> bool {
    message.split('\u{1}').any(|field| field == "35=0")
}

/// An error constructing a log.
#[derive(Debug, Error)]
pub enum LogError {
    /// An I/O error (e.g. opening a log file).
    #[error("log I/O error: {0}")]
    Io(String),
}

/// A FIX log sink. Message logging (incoming/outgoing) is kept separate from event logging.
///
/// Methods are best-effort and infallible from the caller's perspective; a sink that cannot
/// write drops the entry rather than failing the session.
pub trait Log: Send + Sync {
    /// Log an inbound wire message.
    fn on_incoming(&self, message: &str);
    /// Log an outbound wire message.
    fn on_outgoing(&self, message: &str);
    /// Log a session event.
    fn on_event(&self, text: &str);
}

impl Log for Box<dyn Log> {
    fn on_incoming(&self, message: &str) {
        (**self).on_incoming(message);
    }
    fn on_outgoing(&self, message: &str) {
        (**self).on_outgoing(message);
    }
    fn on_event(&self, text: &str) {
        (**self).on_event(text);
    }
}

/// Which log backend to construct.
#[derive(Debug, Clone)]
pub enum LogConfig {
    /// Log to stdout/stderr.
    Screen,
    /// Log to files in a directory (`messages.log` + `event.log`).
    File {
        /// Directory holding the log files.
        dir: PathBuf,
    },
    /// Log via the `tracing` facade.
    Tracing,
    /// Fan out to several logs.
    Composite(Vec<LogConfig>),
}

/// Build a boxed [`Log`] from a [`LogConfig`].
pub fn build_log(config: &LogConfig) -> Result<Box<dyn Log>, LogError> {
    Ok(match config {
        LogConfig::Screen => Box::new(ScreenLog::new()),
        LogConfig::File { dir } => Box::new(FileLog::open(dir)?),
        LogConfig::Tracing => Box::new(TracingLog::new()),
        LogConfig::Composite(parts) => {
            let mut logs: Vec<Box<dyn Log>> = Vec::with_capacity(parts.len());
            for part in parts {
                logs.push(build_log(part)?);
            }
            Box::new(CompositeLog::new(logs))
        }
    })
}
