//! File log: separate `messages.log` and `event.log` streams (FR-H2).
//!
//! NEW-156 (feature 012): writes are queued onto a bounded `mpsc` channel and persisted by a
//! background task, mirroring `SqlLog`/`MssqlLog`/`MongoLog`/`RedbLog`'s existing channel +
//! background-writer shape — `on_incoming`/`on_outgoing`/`on_event` never touch the filesystem
//! directly, so they never block the calling async task on disk I/O. Each blocking file write
//! inside the background task is further wrapped in `tokio::task::spawn_blocking`, matching
//! `RedbLog`'s treatment of its own blocking backend API. A background write/flush failure is
//! surfaced via a structured `tracing::error!` event and a `truefix_log_write_failures_total`
//! metrics counter (FR-009) instead of being silently dropped.
//!
//! NEW-157 (feature 012): [`RetentionPolicy`] adds a composable generation-count and/or
//! time-based rolling policy on top of the pre-existing `MaxFileLogSize` size trigger, opt-in
//! only (`FileLogOptions::retention: None` preserves today's single-backup-overwrite behavior
//! exactly). See [`RotatingFile::rotate`] for the naming/pruning scheme.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::mpsc;

use crate::{Log, LogError, is_heartbeat};

const ASYNC_LOG_CHANNEL_CAPACITY: usize = 1024;

/// Composable retention policy for [`FileLog`]'s rotated backups (NEW-157). Both fields are
/// independently optional and may be set together (e.g. "roll daily, keep the last 30 dated
/// files").
#[derive(Debug, Clone, Copy, Default)]
pub struct RetentionPolicy {
    /// Keep at most this many rotated backups, pruning the oldest once exceeded. `Some(0)` is
    /// rejected at [`FileLog::open_with_options`] time (indistinguishable from "no policy", so
    /// treated as a config error rather than silently behaving like `None`).
    pub generations: Option<u32>,
    /// Roll onto a new dated backup once this much wall-clock time has elapsed since the active
    /// file's current interval began, in addition to (not instead of) the existing
    /// `MaxFileLogSize` size trigger. A raw [`Duration`] (rather than a fixed `Daily`/`Hourly`
    /// enum) so operators can pick any interval; `Some(Duration::ZERO)` is rejected as invalid.
    pub roll_interval: Option<Duration>,
}

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
    /// NEW-157 (feature 012): composable generation-count and/or time-based retention beyond the
    /// single-backup-overwrite default. `None` (the default) preserves today's exact behavior.
    pub retention: Option<RetentionPolicy>,
}

impl Default for FileLogOptions {
    fn default() -> Self {
        Self {
            include_heartbeats: true,
            include_timestamp: false,
            include_milliseconds: false,
            max_size_bytes: None,
            retention: None,
        }
    }
}

/// `YYYYMMDD-HH:MM:SS[.mmm] ` (trailing space), or empty when timestamps are disabled.
pub(crate) fn format_timestamp_prefix(include_millis: bool) -> String {
    let now = time::OffsetDateTime::now_utc();
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

/// `YYYYMMDD-HHMMSS`, used as a rotated-backup filename suffix when a `roll_interval` is
/// configured -- fine enough granularity to stay unique regardless of the configured interval.
fn format_date_stamp(at: time::OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

/// Which whole `interval`-sized bucket (since the Unix epoch) `at` falls into -- two timestamps
/// map to the same key iff no `interval` boundary separates them. Nanosecond precision (not
/// whole seconds) so sub-second intervals -- used by tests to exercise a roll boundary without
/// waiting real hours/days -- behave correctly too.
fn interval_key(interval: Duration, at: time::OffsetDateTime) -> i128 {
    let interval_ns = interval.as_nanos().max(1);
    at.unix_timestamp_nanos().div_euclid(interval_ns as i128)
}

/// The representative start-of-interval timestamp for `key`, used to format a rotated backup's
/// date-stamp from the *closing* interval (not "now", which may already be in the next one).
fn interval_start(interval: Duration, key: i128) -> time::OffsetDateTime {
    let interval_ns = interval.as_nanos().max(1) as i128;
    time::OffsetDateTime::from_unix_timestamp_nanos(key.saturating_mul(interval_ns))
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.to_path_buf().into_os_string();
    s.push(".");
    s.push(suffix);
    PathBuf::from(s)
}

/// A fully-formatted line destined for either stream, queued onto the background writer's
/// channel. Formatting (timestamp prefix, heartbeat filtering) happens synchronously in the
/// caller so timestamps reflect when the message was actually seen, not when the background task
/// gets around to persisting it.
enum Entry {
    Message(String),
    Event(String),
}

/// Logs inbound/outbound messages to `messages.log` and events to `event.log` via a bounded
/// channel and background writer task (NEW-156).
pub struct FileLog {
    tx: std::sync::Mutex<Option<mpsc::Sender<Entry>>>,
    task: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    options: FileLogOptions,
}

impl FileLog {
    /// Open (creating if needed) the log files in `dir` with default (unfiltered) output switches.
    pub async fn open(dir: &Path) -> Result<Self, LogError> {
        Self::open_with_options(dir, FileLogOptions::default()).await
    }

    /// Open (creating if needed) the log files in `dir`, honoring `options` (FR-026), and spawn
    /// the background writer task (NEW-156). `async` (unlike the pre-NEW-156 synchronous
    /// constructor) purely because spawning that task requires an active Tokio runtime, matching
    /// `RedbLog::connect`/`SqlLog::connect`'s existing async constructors -- opening the files
    /// themselves is still a cheap, synchronous `std::fs` call, not offloaded to `spawn_blocking`.
    pub async fn open_with_options(dir: &Path, options: FileLogOptions) -> Result<Self, LogError> {
        if let Some(retention) = options.retention {
            if retention.generations == Some(0) {
                return Err(LogError::Io(
                    "RetentionPolicy::generations must be at least 1 when set (0 is \
                     indistinguishable from no policy)"
                        .to_owned(),
                ));
            }
            if retention.roll_interval == Some(Duration::ZERO) {
                return Err(LogError::Io(
                    "RetentionPolicy::roll_interval must be greater than zero when set".to_owned(),
                ));
            }
        }
        std::fs::create_dir_all(dir).map_err(|e| LogError::Io(e.to_string()))?;
        let messages = RotatingFile::open(
            dir.join("messages.log"),
            options.max_size_bytes,
            options.retention,
        )?;
        let events = RotatingFile::open(
            dir.join("event.log"),
            options.max_size_bytes,
            options.retention,
        )?;

        let (tx, mut rx) = mpsc::channel::<Entry>(ASYNC_LOG_CHANNEL_CAPACITY);
        let task = tokio::spawn(async move {
            let mut messages = messages;
            let mut events = events;
            while let Some(entry) = rx.recv().await {
                let outcome =
                    tokio::task::spawn_blocking(move || write_entry(messages, events, entry)).await;
                let Ok((m, e, result)) = outcome else {
                    // The blocking closure panicked (a bug elsewhere, not a disk failure) -- we no
                    // longer own `messages`/`events`, so this task cannot continue safely.
                    report_write_failure("file log background writer task panicked");
                    break;
                };
                messages = m;
                events = e;
                if let Err(err) = result {
                    report_write_failure(&err.to_string());
                }
            }
        });

        Ok(Self {
            tx: std::sync::Mutex::new(Some(tx)),
            task: std::sync::Mutex::new(Some(task)),
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

    fn send(&self, entry: Entry) {
        if let Ok(guard) = self.tx.lock()
            && let Some(tx) = guard.as_ref()
        {
            let _ = tx.try_send(entry);
        }
    }
}

/// FR-009: surface a background write/flush failure via structured `tracing` + a `metrics`
/// counter, instead of the pre-NEW-156 `eprintln!`-only fallback (NEW-124, audit 006).
fn report_write_failure(detail: &str) {
    tracing::error!(target: "truefix_log", error = %detail, "failed to write log line");
    metrics::counter!("truefix_log_write_failures_total").increment(1);
}

/// Writes `entry` to whichever of `messages`/`events` it targets, returning both streams back to
/// the caller regardless of which one changed -- keeps the background task the sole owner of both
/// `RotatingFile`s across `spawn_blocking` hops (which require `'static` owned captures).
fn write_entry(
    mut messages: RotatingFile,
    mut events: RotatingFile,
    entry: Entry,
) -> (RotatingFile, RotatingFile, std::io::Result<()>) {
    let result = match &entry {
        Entry::Message(line) => messages.write_line(line),
        Entry::Event(line) => events.write_line(line),
    };
    (messages, events, result)
}

fn open_append(path: &Path) -> Result<File, LogError> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| LogError::Io(e.to_string()))
}

/// A single log stream file with size-bounded rotation (`MaxFileLogSize`, NEW-108) and/or a
/// composable [`RetentionPolicy`] (NEW-157, feature 012).
struct RotatingFile {
    path: PathBuf,
    file: File,
    size: u64,
    max_size_bytes: Option<u64>,
    retention: Option<RetentionPolicy>,
    /// The whole-interval bucket the *active* file's content currently belongs to, when
    /// `retention.roll_interval` is set; re-derived on every `open()` (FR-011/FR-013) rather than
    /// persisted, so a crash or a stopped process never leaves stale in-memory state.
    current_interval_key: Option<i128>,
}

impl RotatingFile {
    fn open(
        path: PathBuf,
        max_size_bytes: Option<u64>,
        retention: Option<RetentionPolicy>,
    ) -> Result<Self, LogError> {
        let file = open_append(&path)?;
        let metadata = file.metadata().ok();
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        // FR-011/FR-013: re-derive which interval the pre-existing content (if any) belongs to
        // from the file's own mtime, rather than assuming "now" -- this is what makes a stale
        // interval (elapsed while the process was stopped) roll exactly once on the first write
        // after restart instead of being silently absorbed into "today".
        let current_interval_key = retention.and_then(|r| r.roll_interval).map(|interval| {
            let at = if size > 0 {
                metadata
                    .and_then(|m| m.modified().ok())
                    .map(time::OffsetDateTime::from)
                    .unwrap_or_else(time::OffsetDateTime::now_utc)
            } else {
                time::OffsetDateTime::now_utc()
            };
            interval_key(interval, at)
        });
        Ok(Self {
            path,
            file,
            size,
            max_size_bytes,
            retention,
            current_interval_key,
        })
    }

    /// Write `line` (plus a trailing newline), rotating first if either the size bound or a
    /// configured roll interval has been crossed -- so no single file ever grows past
    /// `max_size_bytes`, and no file spans more than one configured interval.
    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        if let Some(interval) = self.retention.and_then(|r| r.roll_interval) {
            let now_key = interval_key(interval, time::OffsetDateTime::now_utc());
            if self.current_interval_key.is_some_and(|k| k != now_key) {
                self.rotate()?;
            }
            self.current_interval_key = Some(now_key);
        }
        if let Some(max) = self.max_size_bytes
            && self.size >= max
        {
            self.rotate()?;
        }
        writeln!(self.file, "{line}")?;
        self.size += line.len() as u64 + 1;
        Ok(())
    }

    /// Renames the current file out of the way and opens a fresh one, per the configured
    /// [`RetentionPolicy`]:
    /// - No policy: today's exact pre-NEW-157 behavior -- a single `<name>.1`, overwritten every
    ///   time (`FR-004`'s default-unchanged requirement).
    /// - `generations` only: numeric shifting `<name>.1`..`<name>.N`, oldest dropped.
    /// - `roll_interval` set (with or without `generations`): a date-stamped
    ///   `<name>.YYYYMMDD-HHMMSS` backup named after the *closing* interval; if `generations` is
    ///   also set, dated backups beyond that count are pruned (oldest first).
    ///
    /// Every filesystem-mutating step here happens before `self.size`/`self.current_interval_key`
    /// are updated by the caller, and `open()` always re-derives state from disk rather than
    /// trusting in-memory assumptions -- together these make a crash at any point during this
    /// sequence leave exactly one unambiguous active file on the next `open()` (FR-011).
    fn rotate(&mut self) -> std::io::Result<()> {
        match self.retention {
            Some(RetentionPolicy {
                roll_interval: Some(interval),
                generations,
            }) => {
                let key = self
                    .current_interval_key
                    .unwrap_or_else(|| interval_key(interval, time::OffsetDateTime::now_utc()));
                let stamp = format_date_stamp(interval_start(interval, key));
                let backup = suffixed(&self.path, &stamp);
                // Best-effort: a failure here should not prevent the log from continuing to
                // accept writes to the existing file.
                let _ = std::fs::rename(&self.path, &backup);
                if let Some(n) = generations {
                    self.prune_dated_backups(n);
                }
            }
            Some(RetentionPolicy {
                roll_interval: None,
                generations: Some(n),
            }) => {
                self.shift_numeric_backups(n);
                let _ = std::fs::rename(&self.path, suffixed(&self.path, "1"));
            }
            _ => {
                // No policy (or a policy with both fields unset, which is the same as no policy):
                // preserve the pre-NEW-157 single fixed `.1` backup, overwritten every rotation.
                let _ = std::fs::rename(&self.path, suffixed(&self.path, "1"));
            }
        }
        self.file = open_append(&self.path).map_err(|e| match e {
            LogError::Io(msg) => std::io::Error::other(msg),
        })?;
        self.size = 0;
        Ok(())
    }

    /// Shifts `<name>.1`..`<name>.{n-1}` up by one generation (dropping whatever was at
    /// `<name>.n`), making room for the current file to become the new `<name>.1`. Each rename is
    /// best-effort/no-op if its source doesn't exist yet (early in a file's life).
    fn shift_numeric_backups(&self, n: u32) {
        if n == 0 {
            return;
        }
        let oldest = suffixed(&self.path, &n.to_string());
        let _ = std::fs::remove_file(&oldest);
        for generation in (1..n).rev() {
            let from = suffixed(&self.path, &generation.to_string());
            let to = suffixed(&self.path, &(generation + 1).to_string());
            let _ = std::fs::rename(&from, &to);
        }
    }

    /// Keeps only the `keep` most-recent `<name>.YYYYMMDD-HHMMSS` backups (lexicographic order
    /// matches chronological order for this fixed-width format), deleting older ones. Best-effort
    /// (directory listing/deletion failures are swallowed) since pruning is a retention nicety,
    /// not a correctness requirement -- a few extra retained backups is not data loss.
    fn prune_dated_backups(&self, keep: u32) {
        let Some(parent) = self.path.parent() else {
            return;
        };
        let Some(file_name) = self.path.file_name().and_then(|f| f.to_str()) else {
            return;
        };
        let prefix = format!("{file_name}.");
        let Ok(entries) = std::fs::read_dir(parent) else {
            return;
        };
        let mut dated: Vec<(String, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_owned();
                let suffix = name.strip_prefix(&prefix)?.to_owned();
                let looks_dated = suffix.len() == "YYYYMMDD-HHMMSS".len()
                    && suffix
                        .chars()
                        .enumerate()
                        .all(|(i, c)| if i == 8 { c == '-' } else { c.is_ascii_digit() });
                looks_dated.then_some((suffix, e.path()))
            })
            .collect();
        dated.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, path) in dated.into_iter().skip(keep as usize) {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[async_trait::async_trait]
impl Log for FileLog {
    fn on_incoming(&self, message: &str) {
        if is_heartbeat(message) && !self.options.include_heartbeats {
            return;
        }
        self.send(Entry::Message(format!(
            "{}I {message}",
            self.timestamp_prefix()
        )));
    }
    fn on_outgoing(&self, message: &str) {
        if is_heartbeat(message) && !self.options.include_heartbeats {
            return;
        }
        self.send(Entry::Message(format!(
            "{}O {message}",
            self.timestamp_prefix()
        )));
    }
    fn on_event(&self, text: &str) {
        // NEW-125 (audit 006): events are always timestamped, matching QFJ's `FileLog.onEvent()`
        // (unconditional `forceTimestamp=true`) -- `include_timestamp` governs message logging
        // only, not event logging.
        self.send(Entry::Event(format!(
            "{}{text}",
            format_timestamp_prefix(self.options.include_milliseconds)
        )));
    }
    async fn shutdown(&self) {
        // NEW-91-style contract (see RedbLog/SqlLog's identical shutdown()): dropping the sender
        // closes the channel once the sole clone is gone; the writer task's
        // `while let Some(entry) = rx.recv().await` then drains whatever was already queued and
        // returns, ending the loop (FR-002, SC-002).
        let tx = self.tx.lock().ok().and_then(|mut guard| guard.take());
        drop(tx);
        let task = self.task.lock().ok().and_then(|mut guard| guard.take());
        if let Some(task) = task {
            let _ = task.await;
        }
    }
}
