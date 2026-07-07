//! NEW-124: FileLog write failures are surfaced (stderr) instead of silently discarded.
//! NEW-125: FileLog events are always timestamped, independent of `include_timestamp`.
//! NEW-126: `LogConfig::File` threads `FileLogOptions` through to the built backend.

use truefix_log::{FileLog, FileLogOptions, Log, LogConfig, build_log};

fn unique_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "truefix-audit006-log-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// --- NEW-125: events are always timestamped, regardless of include_timestamp ---

#[test]
fn audit006_event_lines_are_always_timestamped_even_with_include_timestamp_off() {
    let dir = unique_dir("event-timestamp");
    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            include_heartbeats: true,
            include_timestamp: false,
            include_milliseconds: false,
            max_size_bytes: None,
        },
    )
    .unwrap();
    log.on_event("session started");
    drop(log);

    let events = std::fs::read_to_string(dir.join("event.log")).unwrap();
    let line = events.lines().next().unwrap();
    // "YYYYMMDD-HH:MM:SS " prefix even though include_timestamp=false.
    let prefix = line.split(' ').next().unwrap();
    assert_eq!(prefix.len(), "20240102-03:04:05".len());
    assert!(line.ends_with("session started"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn audit006_message_lines_still_respect_include_timestamp_off() {
    let dir = unique_dir("message-timestamp");
    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            include_heartbeats: true,
            include_timestamp: false,
            include_milliseconds: false,
            max_size_bytes: None,
        },
    )
    .unwrap();
    log.on_incoming("8=FIX.4.4\u{1}35=D\u{1}");
    drop(log);

    let messages = std::fs::read_to_string(dir.join("messages.log")).unwrap();
    let line = messages.lines().next().unwrap();
    assert!(
        line.starts_with("I "),
        "message lines still respect include_timestamp=false: {line:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// --- NEW-126: LogConfig::File threads FileLogOptions through ---

#[test]
fn audit006_log_config_file_threads_heartbeat_filter_option() {
    let dir = unique_dir("logconfig-options");
    let log = build_log(&LogConfig::File {
        dir: dir.clone(),
        options: FileLogOptions {
            include_heartbeats: false,
            include_timestamp: false,
            include_milliseconds: false,
            max_size_bytes: None,
        },
    })
    .unwrap();
    log.on_incoming("8=FIX.4.4\u{1}35=0\u{1}"); // heartbeat, should be filtered
    log.on_incoming("8=FIX.4.4\u{1}35=D\u{1}");
    drop(log);

    let messages = std::fs::read_to_string(dir.join("messages.log")).unwrap();
    assert!(!messages.contains("35=0"), "heartbeat should be filtered");
    assert!(messages.contains("35=D"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn audit006_log_config_composite_includes_file_options() {
    let dir = unique_dir("logconfig-composite");
    let log = build_log(&LogConfig::Composite(vec![LogConfig::File {
        dir: dir.clone(),
        options: FileLogOptions {
            include_heartbeats: false,
            include_timestamp: true,
            include_milliseconds: false,
            max_size_bytes: None,
        },
    }]))
    .unwrap();
    log.on_incoming("8=FIX.4.4\u{1}35=0\u{1}");
    drop(log);

    let messages = std::fs::read_to_string(dir.join("messages.log")).unwrap();
    assert!(
        messages.is_empty(),
        "heartbeat filter option must reach the composite's File component"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
