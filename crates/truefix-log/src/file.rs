//! File log: separate `messages.log` and `event.log` streams (FR-H2).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use time::OffsetDateTime;

use crate::{Log, LogError, is_heartbeat};

/// Output switches for [`FileLog`] (FR-026): `FileLogHeartbeats`/`FileIncludeMilliseconds`/
/// `FileIncludeTimeStampForMessages`.
#[derive(Debug, Clone, Copy)]
pub struct FileLogOptions {
    /// `FileLogHeartbeats`: whether Heartbeat (`35=0`) messages are written to `messages.log`.
    pub include_heartbeats: bool,
    /// `FileIncludeTimeStampForMessages`: whether each line is prefixed with a timestamp.
    pub include_timestamp: bool,
    /// `FileIncludeMilliseconds`: whether that timestamp includes milliseconds.
    pub include_milliseconds: bool,
}

impl Default for FileLogOptions {
    fn default() -> Self {
        Self {
            include_heartbeats: true,
            include_timestamp: false,
            include_milliseconds: false,
        }
    }
}

/// `YYYYMMDD-HH:MM:SS[.mmm] ` (trailing space), or empty when timestamps are disabled.
pub(crate) fn format_timestamp_prefix(include_millis: bool) -> String {
    let now = OffsetDateTime::now_utc();
    if include_millis {
        format!(
            "{:04}{:02}{:02}-{:02}:{:02}:{:02}.{:03} ",
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
            now.millisecond()
        )
    } else {
        format!(
            "{:04}{:02}{:02}-{:02}:{:02}:{:02} ",
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        )
    }
}

/// Logs inbound/outbound messages to `messages.log` and events to `event.log`.
pub struct FileLog {
    messages: Mutex<File>,
    events: Mutex<File>,
    options: FileLogOptions,
}

impl FileLog {
    /// Open (creating if needed) the log files in `dir` with default (unfiltered) output switches.
    pub fn open(dir: &Path) -> Result<Self, LogError> {
        Self::open_with_options(dir, FileLogOptions::default())
    }

    /// Open (creating if needed) the log files in `dir`, honoring `options` (FR-026).
    pub fn open_with_options(dir: &Path, options: FileLogOptions) -> Result<Self, LogError> {
        std::fs::create_dir_all(dir).map_err(|e| LogError::Io(e.to_string()))?;
        let messages = open_append(&dir.join("messages.log"))?;
        let events = open_append(&dir.join("event.log"))?;
        Ok(Self {
            messages: Mutex::new(messages),
            events: Mutex::new(events),
            options,
        })
    }

    fn timestamp_prefix(&self) -> String {
        if self.options.include_timestamp {
            format_timestamp_prefix(self.options.include_milliseconds)
        } else {
            String::new()
        }
    }
}

fn open_append(path: &Path) -> Result<File, LogError> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| LogError::Io(e.to_string()))
}

fn write_line(file: &Mutex<File>, line: &str) {
    if let Ok(mut f) = file.lock() {
        let _ = writeln!(f, "{line}");
    }
}

impl Log for FileLog {
    fn on_incoming(&self, message: &str) {
        if is_heartbeat(message) && !self.options.include_heartbeats {
            return;
        }
        write_line(
            &self.messages,
            &format!("{}I {message}", self.timestamp_prefix()),
        );
    }
    fn on_outgoing(&self, message: &str) {
        if is_heartbeat(message) && !self.options.include_heartbeats {
            return;
        }
        write_line(
            &self.messages,
            &format!("{}O {message}", self.timestamp_prefix()),
        );
    }
    fn on_event(&self, text: &str) {
        write_line(&self.events, &format!("{}{text}", self.timestamp_prefix()));
    }
}
