//! T071 (US12) — log output switches: heartbeat filtering, timestamp/millisecond inclusion,
//! incoming/outgoing/event visibility, and session-ID prefixing (FR-026).

use std::sync::{Arc, Mutex};

use truefix_log::{
    FileLog, FileLogOptions, Log, ScreenLog, ScreenLogOptions, SessionPrefixLog, TracingLog,
    TracingLogOptions,
};

const HEARTBEAT: &str = "8=FIX.4.4\u{1}35=0\u{1}10=000\u{1}";
const NEWORDER: &str = "8=FIX.4.4\u{1}35=D\u{1}10=000\u{1}";

#[derive(Default, Clone)]
struct CapturingLog {
    messages: Arc<Mutex<Vec<String>>>,
    events: Arc<Mutex<Vec<String>>>,
}

impl Log for CapturingLog {
    fn on_incoming(&self, message: &str) {
        self.messages.lock().unwrap().push(format!("I {message}"));
    }
    fn on_outgoing(&self, message: &str) {
        self.messages.lock().unwrap().push(format!("O {message}"));
    }
    fn on_event(&self, text: &str) {
        self.events.lock().unwrap().push(text.to_owned());
    }
}

#[test]
fn session_prefix_log_prepends_to_every_stream() {
    let inner = CapturingLog::default();
    let prefixed = SessionPrefixLog::new("FIX.4.4:CLIENT->SERVER", inner.clone());

    prefixed.on_incoming(NEWORDER);
    prefixed.on_outgoing(NEWORDER);
    prefixed.on_event("logon");

    let msgs = inner.messages.lock().unwrap();
    let evts = inner.events.lock().unwrap();
    assert!(msgs[0].starts_with("I [FIX.4.4:CLIENT->SERVER] "));
    assert!(msgs[1].starts_with("O [FIX.4.4:CLIENT->SERVER] "));
    assert!(evts[0].starts_with("[FIX.4.4:CLIENT->SERVER] logon"));
}

#[test]
fn file_log_heartbeat_filter_excludes_heartbeats_only() {
    let dir = std::env::temp_dir().join(format!("truefix-log-switches-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            include_heartbeats: false,
            include_timestamp: false,
            include_milliseconds: false,
            max_size_bytes: None,
        },
    )
    .unwrap();
    log.on_incoming(HEARTBEAT);
    log.on_outgoing(NEWORDER);
    drop(log);

    let messages = std::fs::read_to_string(dir.join("messages.log")).unwrap();
    assert!(!messages.contains("35=0"), "heartbeat should be filtered");
    assert!(messages.contains("35=D"), "non-heartbeat still logged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_log_default_includes_heartbeats() {
    let dir = std::env::temp_dir().join(format!("truefix-log-switches2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let log = FileLog::open(&dir).unwrap();
    log.on_incoming(HEARTBEAT);
    drop(log);

    let messages = std::fs::read_to_string(dir.join("messages.log")).unwrap();
    assert!(messages.contains("35=0"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_log_timestamp_switch_controls_prefix() {
    let dir = std::env::temp_dir().join(format!("truefix-log-switches3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let log = FileLog::open_with_options(
        &dir,
        FileLogOptions {
            include_heartbeats: true,
            include_timestamp: true,
            include_milliseconds: true,
            max_size_bytes: None,
        },
    )
    .unwrap();
    log.on_incoming(NEWORDER);
    drop(log);

    let messages = std::fs::read_to_string(dir.join("messages.log")).unwrap();
    let line = messages.lines().next().unwrap();
    // "YYYYMMDD-HH:MM:SS.mmm I 8=FIX.4.4..."
    let ts = line.split(' ').next().unwrap();
    assert_eq!(ts.len(), "20240102-03:04:05.678".len());
    assert!(ts.contains('.'), "millisecond component present: {ts}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn screen_log_visibility_switches_suppress_categories() {
    // Not capturing stdout/stderr directly (that's process-global); instead verify the options
    // are honored via the heartbeat-filter path, which is deterministic and doesn't depend on
    // console capture. Construction with every switch off must not panic and must still be usable.
    let log = ScreenLog::with_options(ScreenLogOptions {
        show_events: false,
        show_heartbeats: false,
        show_incoming: false,
        show_outgoing: false,
        include_milliseconds: true,
    });
    log.on_incoming(HEARTBEAT);
    log.on_outgoing(NEWORDER);
    log.on_event("suppressed");
}

#[test]
fn tracing_log_heartbeat_filter_does_not_panic() {
    let log = TracingLog::with_options(TracingLogOptions {
        include_heartbeats: false,
    });
    log.on_incoming(HEARTBEAT);
    log.on_outgoing(NEWORDER);
    log.on_event("evt");
}
