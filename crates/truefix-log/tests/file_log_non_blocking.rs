//! NEW-156 (feature 012): `FileLog` writes must never block the calling async task on the
//! underlying disk I/O (FR-001/FR-003/FR-009, SC-001, SC-005).
//!
//! These tests simulate a slow/contended disk by pointing `messages.log` at a Unix FIFO (named
//! pipe) drained by a background OS thread at a deliberately slow, but always-progressing, rate —
//! this reproduces real backpressure from the kernel (writes to a full pipe genuinely block) with
//! a bounded worst-case duration, so a pre-fix run fails these assertions without ever hanging.
//! Gated to Unix since it relies on the `mkfifo` binary; CI (`.github/workflows/ci.yml`) runs
//! `ubuntu-latest` only.

#![cfg(unix)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use truefix_log::{FileLog, FileLogOptions, Log};

fn unique_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "truefix-file-log-non-blocking-{name}-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn make_fifo(path: &Path) {
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .expect("mkfifo must be available on unix test hosts");
    assert!(status.success(), "mkfifo failed for {path:?}");
}

/// Opens `path` (a FIFO) for reading and drains it a little at a time, sleeping between reads —
/// a bounded-but-slow consumer, matching the "slow/network-backed log volume" scenario `NEW-156`
/// describes. Always keeps draining, so nothing waiting on this reader can hang forever.
fn spawn_slow_reader(path: PathBuf) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut f = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => return,
        };
        let mut buf = [0u8; 512];
        loop {
            match f.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => break,
            }
        }
    })
}

// --- T002: on_outgoing must return quickly even when the underlying disk write is slow ---

#[tokio::test]
async fn on_outgoing_returns_quickly_even_when_the_underlying_disk_write_is_slow() {
    let dir = unique_dir("t002-latency");
    std::fs::create_dir_all(&dir).unwrap();
    make_fifo(&dir.join("messages.log"));
    let _reader = spawn_slow_reader(dir.join("messages.log"));

    let log = FileLog::open(&dir).await.unwrap();

    let big = "x".repeat(2048);
    let start = Instant::now();
    for _ in 0..128 {
        log.on_outgoing(&big);
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(300),
        "on_outgoing must return without waiting on the underlying (slow) disk write; took \
         {elapsed:?} for 128 calls"
    );

    log.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

// --- T003: a scheduled heartbeat-equivalent task must not be starved by slow FileLog writes ---

#[tokio::test(flavor = "current_thread")]
async fn on_outgoing_does_not_starve_other_scheduled_work() {
    let dir = unique_dir("t003-starvation");
    std::fs::create_dir_all(&dir).unwrap();
    make_fifo(&dir.join("messages.log"));
    let _reader = spawn_slow_reader(dir.join("messages.log"));

    let log = Arc::new(FileLog::open(&dir).await.unwrap());

    let ticks: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let ticks2 = ticks.clone();
    let heartbeat = tokio::spawn(async move {
        for _ in 0..40 {
            ticks2.lock().unwrap().push(Instant::now());
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    let log2 = log.clone();
    let writer = tokio::spawn(async move {
        let big = "x".repeat(2048);
        for _ in 0..128 {
            log2.on_outgoing(&big);
        }
    });

    let _ = tokio::join!(heartbeat, writer);

    let (tick_count, max_gap) = {
        let ticks = ticks.lock().unwrap();
        let mut max_gap = Duration::ZERO;
        for pair in ticks.windows(2) {
            let gap = pair[1].duration_since(pair[0]);
            if gap > max_gap {
                max_gap = gap;
            }
        }
        (ticks.len(), max_gap)
    };
    assert!(tick_count >= 2, "heartbeat task must have recorded ticks");
    assert!(
        max_gap < Duration::from_millis(200),
        "a scheduled heartbeat-equivalent task must not be starved by FileLog writes to a slow \
         disk; largest observed gap between ticks was {max_gap:?}"
    );

    log.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

// --- T004: sustained write pressure must apply bounded backpressure, not unbounded queueing ---

#[tokio::test]
async fn sustained_write_pressure_applies_bounded_backpressure_not_unbounded_queueing() {
    let dir = unique_dir("t004-backpressure");
    std::fs::create_dir_all(&dir).unwrap();
    make_fifo(&dir.join("messages.log"));

    // A reader that opens the fifo (unblocking FileLog::open) but never drains it -- once the OS
    // pipe buffer and the bounded async channel both fill, every excess entry must be dropped
    // rather than blocking or growing memory without bound.
    let reader_path = dir.join("messages.log");
    let _reader = std::thread::spawn(move || {
        let _f = std::fs::File::open(&reader_path);
        std::thread::sleep(Duration::from_secs(3));
    });

    let log = FileLog::open(&dir).await.unwrap();

    let big = "x".repeat(256);
    let start = Instant::now();
    for _ in 0..5000 {
        log.on_outgoing(&big);
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(500),
        "submitting far more entries than the bounded channel can hold must stay fast (excess \
         entries dropped, not queued/blocked without bound); took {elapsed:?} for 5000 calls"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// --- T005: a background write/flush failure is surfaced via structured tracing + metrics ---

struct SharedBufWriter(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for SharedBufWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_background_write_failure_is_surfaced_via_tracing_and_metrics() {
    let dir = unique_dir("t005-observability");
    std::fs::create_dir_all(&dir).unwrap();

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    // Installing a global recorder can only happen once per process; ignore if another test in
    // the same binary already installed one (matches the precedent in
    // crates/truefix-transport/tests/metrics.rs). No other test in this file installs a recorder.
    let _ = recorder.install();

    let buf: Arc<Mutex<Vec<u8>>> = Arc::default();
    let buf2 = buf.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || SharedBufWriter(buf2.clone()))
        .with_max_level(tracing::Level::ERROR)
        .without_time()
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            max_size_bytes: Some(1),
            ..FileLogOptions::default()
        },
    )
    .await
    .unwrap();

    // Remove the whole directory out from under the open FileLog so the next triggered rotation's
    // rename/reopen fails with a real I/O error.
    std::fs::remove_dir_all(&dir).unwrap();

    log.on_outgoing("8=FIX.4.4|35=D|");
    log.on_outgoing("8=FIX.4.4|35=D|"); // size already > max_size_bytes=1 -> triggers rotate()

    log.shutdown().await;

    let captured = String::from_utf8_lossy(&buf.lock().unwrap()).to_string();
    assert!(
        captured.to_uppercase().contains("ERROR"),
        "expected a structured tracing ERROR event for the background write failure, got: \
         {captured:?}"
    );

    let snapshot = snapshotter.snapshot().into_hashmap();
    let failures: u64 = snapshot
        .iter()
        .filter_map(|(k, (_, _, v))| {
            if k.key().name() == "truefix_log_write_failures_total" {
                match v {
                    DebugValue::Counter(c) => Some(*c),
                    _ => None,
                }
            } else {
                None
            }
        })
        .sum();
    assert!(
        failures >= 1,
        "expected the truefix_log_write_failures_total counter to increment, got {failures}"
    );
}
