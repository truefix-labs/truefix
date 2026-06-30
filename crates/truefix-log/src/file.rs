//! File log: separate `messages.log` and `event.log` streams (FR-H2).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use crate::{Log, LogError};

/// Logs inbound/outbound messages to `messages.log` and events to `event.log`.
pub struct FileLog {
    messages: Mutex<File>,
    events: Mutex<File>,
}

impl FileLog {
    /// Open (creating if needed) the log files in `dir`.
    pub fn open(dir: &Path) -> Result<Self, LogError> {
        std::fs::create_dir_all(dir).map_err(|e| LogError::Io(e.to_string()))?;
        let messages = open_append(&dir.join("messages.log"))?;
        let events = open_append(&dir.join("event.log"))?;
        Ok(Self {
            messages: Mutex::new(messages),
            events: Mutex::new(events),
        })
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
        write_line(&self.messages, &format!("I {message}"));
    }
    fn on_outgoing(&self, message: &str) {
        write_line(&self.messages, &format!("O {message}"));
    }
    fn on_event(&self, text: &str) {
        write_line(&self.events, text);
    }
}
