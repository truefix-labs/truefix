//! T054 — composite fan-out + message/event stream separation.

use std::sync::{Arc, Mutex};

use truefix_log::{CompositeLog, FileLogOptions, Log, LogConfig, build_log};

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
fn composite_fans_out_and_separates_streams() {
    let a = CapturingLog::default();
    let b = CapturingLog::default();
    let composite = CompositeLog::new(vec![Box::new(a.clone()), Box::new(b.clone())]);

    composite.on_incoming("8=FIX.4.4|35=A");
    composite.on_outgoing("8=FIX.4.4|35=0");
    composite.on_event("logged on");

    for log in [&a, &b] {
        let msgs = log.messages.lock().unwrap();
        let evts = log.events.lock().unwrap();
        // both sinks received the fan-out
        assert_eq!(msgs.len(), 2, "messages fanned out");
        assert_eq!(evts.len(), 1, "events fanned out");
        // message vs event streams are separated
        assert!(msgs.iter().any(|m| m.starts_with("I ")));
        assert!(msgs.iter().any(|m| m.starts_with("O ")));
        assert_eq!(evts[0], "logged on");
        assert!(!evts.iter().any(|e| e.contains("35=")));
    }
}

#[test]
fn file_log_separates_message_and_event_files() {
    let dir = std::env::temp_dir().join(format!("truefix-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let log = build_log(&LogConfig::File {
        dir: dir.clone(),
        options: FileLogOptions::default(),
    })
    .unwrap();
    log.on_incoming("8=FIX.4.4|35=D");
    log.on_event("session reset");
    drop(log);

    let messages = std::fs::read_to_string(dir.join("messages.log")).unwrap();
    let events = std::fs::read_to_string(dir.join("event.log")).unwrap();
    assert!(messages.contains("I 8=FIX.4.4|35=D"));
    assert!(!messages.contains("session reset"));
    assert!(events.contains("session reset"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn factory_builds_composite() {
    let log = build_log(&LogConfig::Composite(vec![
        LogConfig::Screen,
        LogConfig::Tracing,
    ]))
    .unwrap();
    log.on_event("ok"); // must not panic
}
