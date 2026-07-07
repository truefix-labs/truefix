//! File log: separate `messages.log` and `event.log` streams (FR-H2).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
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
    /// `MaxFileLogSize` (NEW-108, audit 006): rotate `messages.log`/`event.log` once either
    /// exceeds this many bytes -- the current file is renamed to `<name>.1` (replacing any
    /// previous backup) and a fresh file is opened. `None` (the default) preserves the previous
    /// unbounded-growth behavior; this is log rotation only, distinct from message-store body
    /// retention (which must preserve resend/recovery semantics and is handled separately).
    pub max_size_bytes: Option<u64>,
}

impl Default for FileLogOptions {
    fn default() -> Self {
        Self {
            include_heartbeats: true,
            include_timestamp: false,
            include_milliseconds: false,
            max_size_bytes: None,
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
    messages: Mutex<RotatingFile>,
    events: Mutex<RotatingFile>,
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
        let messages = RotatingFile::open(dir.join("messages.log"), options.max_size_bytes)?;
        let events = RotatingFile::open(dir.join("event.log"), options.max_size_bytes)?;
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

/// A single log stream file with optional size-bounded rotation (`MaxFileLogSize`, NEW-108, audit
/// 006): once the file would exceed `max_size_bytes`, it's renamed to `<name>.1` (replacing any
/// previous backup) and a fresh file is opened in its place. `max_size_bytes: None` never rotates,
/// matching the previous unconditional-append behavior exactly.
struct RotatingFile {
    path: PathBuf,
    file: File,
    size: u64,
    max_size_bytes: Option<u64>,
}

impl RotatingFile {
    fn open(path: PathBuf, max_size_bytes: Option<u64>) -> Result<Self, LogError> {
        let file = open_append(&path)?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            path,
            file,
            size,
            max_size_bytes,
        })
    }

    /// Write `line` (plus a trailing newline), rotating first if the file has already reached
    /// `max_size_bytes` -- so no single file ever grows past that bound, at the cost of the
    /// rotation happening at most one line late (checked before, not after, this write).
    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        if let Some(max) = self.max_size_bytes
            && self.size >= max
        {
            self.rotate()?;
        }
        writeln!(self.file, "{line}")?;
        self.size += line.len() as u64 + 1;
        Ok(())
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        let mut backup = self.path.clone().into_os_string();
        backup.push(".1");
        // Best-effort: a failure here (e.g. the backup path briefly locked on some platform)
        // should not prevent the log from continuing to accept writes to the existing file.
        let _ = std::fs::rename(&self.path, PathBuf::from(backup));
        self.file = open_append(&self.path).map_err(|e| match e {
            LogError::Io(msg) => std::io::Error::other(msg),
        })?;
        self.size = 0;
        Ok(())
    }
}

/// NEW-124 (audit 006): a write failure (disk full, permission revoked, ...) is no longer
/// completely invisible -- printed to stderr, matching QFJ's `FileLog` (which catches
/// `IOException` and calls `System.err.println(...)`), instead of silently discarded via a bare
/// `let _ = ...`.
fn write_line(file: &Mutex<RotatingFile>, line: &str) {
    let Ok(mut f) = file.lock() else {
        eprintln!("truefix-log: poisoned file log mutex, dropping line: {line}");
        return;
    };
    if let Err(e) = f.write_line(line) {
        eprintln!("truefix-log: failed to write log line: {e}");
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
        // NEW-125 (audit 006): events are always timestamped, matching QFJ's `FileLog.onEvent()`
        // (unconditional `forceTimestamp=true`) -- `include_timestamp` governs message logging
        // only, not event logging.
        write_line(
            &self.events,
            &format!(
                "{}{text}",
                format_timestamp_prefix(self.options.include_milliseconds)
            ),
        );
    }
}
