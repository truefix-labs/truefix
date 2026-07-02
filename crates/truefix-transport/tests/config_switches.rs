//! T024 (US4) — the three session-config switches that need the transport/store layer:
//! `RefreshOnLogon` (real store re-fetch on logon), `ForceResendWhenCorruptedStore`, and
//! `LogMessageWhenSessionNotFound` (acceptor/routing-level, on `Services`, not `SessionConfig`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use truefix_log::Log;
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_store::{FileStore, MessageStore};
use truefix_transport::{connect_initiator_with, Acceptor, AcceptorBuilder, Services};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("truefix-cs-{}-{nanos}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Default)]
struct Flags {
    logged_on: AtomicBool,
}

struct App {
    f: Arc<Flags>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.f.logged_on.store(true, Ordering::SeqCst);
    }
}

async fn wait_until<F: Fn() -> bool>(cond: F, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    cond()
}

/// A `Log` that records every `on_event` text for assertions.
#[derive(Clone, Default)]
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

/// `RefreshOnLogon` — a session whose store sequence numbers were changed *after* the connection
/// was established (simulating an operator externally correcting them, or a template-based
/// acceptor reusing store state written by another process) picks up the refreshed value right at
/// logon completion, not just at connect time.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_on_logon_re_reads_the_store_at_logon_completion() {
    let acc_dir = unique_dir();
    let init_dir = unique_dir();
    let acc_store = Arc::new(FileStore::open(&acc_dir).unwrap());
    let init_store = Arc::new(FileStore::open(&init_dir).unwrap());

    // Simulate the store having advanced sequence state *before* this connection is established
    // (e.g. another process/tool corrected it) beyond what a plain connect-time seed would need.
    acc_store.set_next_sender_seq(5).await.unwrap();
    acc_store.set_next_target_seq(5).await.unwrap();

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 5;
    acc_cfg.reset_on_logon = false;
    acc_cfg.refresh_on_logon = true;
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 5;
    init_cfg.reset_on_logon = false;
    // The initiator must start at the same sequence the acceptor will expect (5), since the
    // acceptor's store was pre-advanced above and RefreshOnLogon re-reads it at logon time.
    init_store.set_next_sender_seq(5).await.unwrap();
    init_store.set_next_target_seq(5).await.unwrap();

    let acc_flags = Arc::new(Flags::default());
    let init_flags = Arc::new(Flags::default());

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(App {
            f: acc_flags.clone(),
        }),
        Services {
            store: Some(acc_store.clone() as Arc<dyn MessageStore>),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let handle = connect_initiator_with(
        addr,
        init_cfg,
        Arc::new(App {
            f: init_flags.clone(),
        }),
        Services {
            store: Some(init_store.clone() as Arc<dyn MessageStore>),
            ..Services::default()
        },
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || acc_flags.logged_on.load(Ordering::SeqCst)
                && init_flags.logged_on.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on using the refreshed sequence numbers"
    );

    handle.logout().await;
    handle.join().await;
    let _ = std::fs::remove_dir_all(&acc_dir);
    let _ = std::fs::remove_dir_all(&init_dir);
}

/// `ForceResendWhenCorruptedStore` — when the acceptor's store reports a recovered corruption, the
/// session forces a full reset (sequence 1) rather than trusting the recovered state, so both
/// sides realign via a fresh `ResetSeqNumFlag` logon.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn force_resend_when_corrupted_store_forces_a_reset() {
    let acc_dir = unique_dir();
    let init_dir = unique_dir();

    // Pre-populate the acceptor's store, then corrupt its trailing record before the connection.
    {
        let s = FileStore::open(&acc_dir).unwrap();
        s.save(1, b"x").await.unwrap();
        s.set_next_sender_seq(2).await.unwrap();
    }
    let body = acc_dir.join("body");
    let mut bytes = std::fs::read(&body).unwrap();
    bytes.extend_from_slice(&[9, 9, 9]);
    std::fs::write(&body, bytes).unwrap();

    let acc_store = Arc::new(FileStore::open(&acc_dir).unwrap());
    assert!(acc_store.was_corrupted());
    let init_store = Arc::new(FileStore::open(&init_dir).unwrap());

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 5;
    acc_cfg.force_resend_when_corrupted_store = true;
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 5;

    let acc_flags = Arc::new(Flags::default());
    let init_flags = Arc::new(Flags::default());

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(App {
            f: acc_flags.clone(),
        }),
        Services {
            store: Some(acc_store.clone() as Arc<dyn MessageStore>),
            ..Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let handle = connect_initiator_with(
        addr,
        init_cfg,
        Arc::new(App {
            f: init_flags.clone(),
        }),
        Services {
            store: Some(init_store.clone() as Arc<dyn MessageStore>),
            ..Services::default()
        },
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || acc_flags.logged_on.load(Ordering::SeqCst)
                && init_flags.logged_on.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on after the forced reset"
    );

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !acc_store.was_corrupted(),
        "the forced reset should clear the corruption flag along with the store"
    );

    handle.logout().await;
    handle.join().await;
    let _ = std::fs::remove_dir_all(&acc_dir);
    let _ = std::fs::remove_dir_all(&init_dir);
}

/// `LogMessageWhenSessionNotFound` — a connection whose Logon matches neither a static session nor
/// a dynamic template gets logged before being refused, when the acceptor is configured to do so.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_message_when_session_not_found_records_the_refusal() {
    let log = RecordingLog::default();
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(App {
            f: Arc::new(Flags::default()),
        }),
    )
    .await
    .unwrap()
    // No `with_session`/`with_dynamic_template`: every connection is refused.
    .with_services(Services {
        log: Some(Arc::new(log.clone())),
        log_message_when_session_not_found: true,
        ..Services::default()
    });
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    // A bare initiator attempt against an acceptor with no matching session/template.
    let init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    let handle = connect_initiator_with(
        addr,
        init_cfg,
        Arc::new(App {
            f: Arc::new(Flags::default()),
        }),
        Services::default(),
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || !log.events.lock().unwrap().is_empty(),
            Duration::from_secs(5),
        )
        .await,
        "the refusal should be logged"
    );
    let found = log
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| e.contains("no session found"));
    assert!(found);

    handle.join().await;
}
