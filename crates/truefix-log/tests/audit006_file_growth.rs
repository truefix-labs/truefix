//! NEW-108: `FileLog` supports a configurable `MaxFileLogSize` rotation policy for
//! `messages.log`/`event.log`, bounding on-disk growth without silently discarding recent
//! output — this is log rotation only, distinct from message-store body retention (which must
//! preserve resend/recovery semantics; covered separately by
//! `truefix-store/tests/audit006_growth_duplicate.rs`).

use std::fs;

use truefix_log::{FileLog, FileLogOptions, Log};

fn temp_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "truefix-audit006-filegrowth-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn audit006_messages_log_rotates_once_max_size_is_reached() {
    let dir = temp_dir("messages");
    let opts = FileLogOptions {
        include_heartbeats: true,
        include_timestamp: false,
        include_milliseconds: false,
        max_size_bytes: Some(32),
    };
    let log = FileLog::open_with_options(&dir, opts).unwrap();
    for i in 0..10 {
        log.on_outgoing(&format!("8=FIX.4.4|35=D|11=ORDER{i}|10=000"));
    }

    let backup = dir.join("messages.log.1");
    assert!(backup.exists(), "expected a rotated backup file to exist");
    let current_len = fs::metadata(dir.join("messages.log")).unwrap().len();
    assert!(
        current_len < 200,
        "current log file should stay bounded after rotation, was {current_len} bytes"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn audit006_event_log_also_rotates_independently_of_messages_log() {
    let dir = temp_dir("events");
    let opts = FileLogOptions {
        include_heartbeats: true,
        include_timestamp: false,
        include_milliseconds: false,
        max_size_bytes: Some(16),
    };
    let log = FileLog::open_with_options(&dir, opts).unwrap();
    for i in 0..10 {
        log.on_event(&format!("session event {i}"));
    }

    assert!(dir.join("event.log.1").exists());
    assert!(!dir.join("messages.log.1").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn audit006_default_options_never_rotate() {
    let dir = temp_dir("unbounded");
    let log = FileLog::open(&dir).unwrap();
    for i in 0..50 {
        log.on_outgoing(&format!("8=FIX.4.4|35=D|11=ORDER{i}|10=000"));
    }

    assert!(!dir.join("messages.log.1").exists());
    let current_len = fs::metadata(dir.join("messages.log")).unwrap().len();
    assert!(
        current_len > 200,
        "expected unbounded growth without a size limit"
    );

    let _ = fs::remove_dir_all(&dir);
}
