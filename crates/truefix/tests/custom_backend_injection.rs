//! NEW-158 (feature 012): a caller using the `.cfg`-driven `Engine::start` path can supply a
//! custom `MessageStore`/`Log` implementation, through `StoreConfig::Custom(...)` (at the
//! `truefix_store::build_store` level) and/or `Engine::start_with_overrides` (FR-006). The
//! builder-level override wins over whatever `.cfg` would otherwise have resolved (FR-012), and a
//! custom log override alongside `FileLogPath`'s FileLog-specific tuning settings is a startup
//! error rather than a silent no-op (FR-010). Behavior is identical for acceptor and initiator
//! sessions (FR-008).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use truefix::config::SessionSettings;
use truefix::transport::Services;
use truefix::{Application, Engine, SessionId};
use truefix_log::Log;
use truefix_store::{MessageStore, StoreConfig, StoreError};

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

struct App {
    logons: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }
}

/// A `MessageStore` that delegates to an in-memory backend while recording every call, so a test
/// can prove a particular instance -- not just "some store" -- was actually used.
#[derive(Default)]
struct RecordingStore {
    calls: Arc<Mutex<Vec<&'static str>>>,
    next_sender: Mutex<u64>,
    next_target: Mutex<u64>,
}

impl RecordingStore {
    fn new(calls: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            calls,
            next_sender: Mutex::new(1),
            next_target: Mutex::new(1),
        }
    }
}

#[async_trait::async_trait]
impl MessageStore for RecordingStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        self.calls.lock().unwrap().push("next_sender_seq");
        Ok(*self.next_sender.lock().unwrap())
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        self.calls.lock().unwrap().push("next_target_seq");
        Ok(*self.next_target.lock().unwrap())
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.calls.lock().unwrap().push("set_next_sender_seq");
        *self.next_sender.lock().unwrap() = seq;
        Ok(())
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.calls.lock().unwrap().push("set_next_target_seq");
        *self.next_target.lock().unwrap() = seq;
        Ok(())
    }
    async fn save(&self, _seq: u64, _message: &[u8]) -> Result<(), StoreError> {
        self.calls.lock().unwrap().push("save");
        Ok(())
    }
    async fn get(&self, _begin: u64, _end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        self.calls.lock().unwrap().push("get");
        Ok(Vec::new())
    }
    async fn reset(&self) -> Result<(), StoreError> {
        self.calls.lock().unwrap().push("reset");
        *self.next_sender.lock().unwrap() = 1;
        *self.next_target.lock().unwrap() = 1;
        Ok(())
    }
}

#[derive(Default)]
struct RecordingLog {
    events: Arc<Mutex<Vec<String>>>,
}

impl Log for RecordingLog {
    fn on_incoming(&self, _message: &str) {}
    fn on_outgoing(&self, _message: &str) {}
    fn on_event(&self, text: &str) {
        self.events.lock().unwrap().push(text.to_owned());
    }
}

async fn wait_for_logons(logons: &AtomicU32, expected: u32) {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < expected {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        logons.load(Ordering::SeqCst),
        expected,
        "expected sessions to log on"
    );
}

// --- T033: StoreConfig::Custom(...) is honored by build_store ---

#[tokio::test]
async fn store_config_custom_variant_is_honored_by_build_store() {
    let calls: Arc<Mutex<Vec<&'static str>>> = Arc::default();
    let store: Arc<dyn MessageStore> = Arc::new(RecordingStore::new(calls.clone()));
    let built = truefix_store::build_store(&StoreConfig::Custom(store))
        .await
        .unwrap();

    built.next_sender_seq().await.unwrap();
    built.set_next_sender_seq(5).await.unwrap();

    let recorded = calls.lock().unwrap();
    assert!(recorded.contains(&"next_sender_seq"));
    assert!(recorded.contains(&"set_next_sender_seq"));
}

// --- T034: Engine::start_with_overrides uses the supplied custom MessageStore/Log ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_start_with_overrides_uses_the_custom_store_and_log() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n"
    );
    let settings = SessionSettings::parse(&cfg).unwrap();

    let store_calls: Arc<Mutex<Vec<&'static str>>> = Arc::default();
    let custom_store: Arc<dyn MessageStore> = Arc::new(RecordingStore::new(store_calls.clone()));
    let log_events: Arc<Mutex<Vec<String>>> = Arc::default();
    let custom_log: Arc<dyn Log> = Arc::new(RecordingLog {
        events: log_events.clone(),
    });

    let mut overrides = HashMap::new();
    overrides.insert(
        "FIX.4.4:SERVER->CLIENT".to_owned(),
        Services {
            store: Some(custom_store),
            log: Some(custom_log),
            ..Services::default()
        },
    );

    let logons = Arc::new(AtomicU32::new(0));
    let engine = Engine::start_with_overrides(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
        }),
        overrides,
    )
    .await
    .expect("engine starts with overrides");

    wait_for_logons(&logons, 2).await;
    engine.shutdown();

    assert!(
        !store_calls.lock().unwrap().is_empty(),
        "the acceptor session's custom store should have been used"
    );
    assert!(
        !log_events.lock().unwrap().is_empty(),
        "the acceptor session's custom log should have received at least one event"
    );
}

// --- T035: the builder-level override wins over whatever .cfg would otherwise resolve ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn builder_override_wins_over_cfg_resolved_log_kind() {
    let port = free_port();
    // `Log=Screen` selects a built-in ScreenLog for the acceptor -- with no override, events go
    // there. FR-012: an override for the same session must win instead.
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         Log=Screen\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n"
    );
    let settings = SessionSettings::parse(&cfg).unwrap();

    let log_events: Arc<Mutex<Vec<String>>> = Arc::default();
    let custom_log: Arc<dyn Log> = Arc::new(RecordingLog {
        events: log_events.clone(),
    });
    let mut overrides = HashMap::new();
    overrides.insert(
        "FIX.4.4:SERVER->CLIENT".to_owned(),
        Services {
            log: Some(custom_log),
            ..Services::default()
        },
    );

    let logons = Arc::new(AtomicU32::new(0));
    let engine = Engine::start_with_overrides(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
        }),
        overrides,
    )
    .await
    .expect("Log=Screen alongside a log override must not be rejected -- FR-012 says override wins, not an error");

    wait_for_logons(&logons, 2).await;
    engine.shutdown();

    assert!(
        !log_events.lock().unwrap().is_empty(),
        "the override log must have received the acceptor's events instead of the .cfg-selected \
         ScreenLog"
    );
}

// --- T036: a custom log override alongside FileLogPath's tuning settings is a startup error ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_log_override_alongside_file_log_settings_is_a_startup_error() {
    let port = free_port();
    let dir = std::env::temp_dir().join(format!(
        "truefix-custom-log-conflict-{}",
        std::process::id()
    ));
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         FileLogPath={}\n\
         MaxFileLogSize=1024\n",
        dir.display()
    );
    let settings = SessionSettings::parse(&cfg).unwrap();

    let custom_log: Arc<dyn Log> = Arc::new(RecordingLog::default());
    let mut overrides = HashMap::new();
    overrides.insert(
        "FIX.4.4:SERVER->CLIENT".to_owned(),
        Services {
            log: Some(custom_log),
            ..Services::default()
        },
    );

    let logons = Arc::new(AtomicU32::new(0));
    let result = Engine::start_with_overrides(&settings, Arc::new(App { logons }), overrides).await;

    match result {
        Err(truefix::EngineError::Config(
            truefix_config::ConfigError::CustomOverrideWithBuiltinOnlySetting { kind, .. },
        )) => {
            assert_eq!(kind, "log");
        }
        Err(other) => panic!("expected CustomOverrideWithBuiltinOnlySetting, got {other}"),
        Ok(engine) => {
            engine.shutdown();
            panic!(
                "expected Engine::start_with_overrides to reject a custom log override \
                 alongside FileLogPath's tuning settings, but it started successfully"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// --- T037: custom backend injection behaves identically for acceptor and initiator sessions ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_backend_injection_is_identical_for_acceptor_and_initiator() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n"
    );
    let settings = SessionSettings::parse(&cfg).unwrap();

    let acc_events: Arc<Mutex<Vec<String>>> = Arc::default();
    let init_events: Arc<Mutex<Vec<String>>> = Arc::default();
    let mut overrides = HashMap::new();
    overrides.insert(
        "FIX.4.4:SERVER->CLIENT".to_owned(),
        Services {
            log: Some(Arc::new(RecordingLog {
                events: acc_events.clone(),
            })),
            ..Services::default()
        },
    );
    overrides.insert(
        "FIX.4.4:CLIENT->SERVER".to_owned(),
        Services {
            log: Some(Arc::new(RecordingLog {
                events: init_events.clone(),
            })),
            ..Services::default()
        },
    );

    let logons = Arc::new(AtomicU32::new(0));
    let engine = Engine::start_with_overrides(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
        }),
        overrides,
    )
    .await
    .expect("engine starts with per-role overrides");

    wait_for_logons(&logons, 2).await;
    engine.shutdown();

    assert!(
        !acc_events.lock().unwrap().is_empty(),
        "acceptor session's custom log must receive events"
    );
    assert!(
        !init_events.lock().unwrap().is_empty(),
        "initiator session's custom log must receive events"
    );
}
