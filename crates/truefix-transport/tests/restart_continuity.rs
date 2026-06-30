//! T061 — store/log wired into the transport: sequence numbers persist across a session.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_store::{FileStore, MessageStore};
use truefix_transport::{connect_initiator_with, Acceptor, Services};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("truefix-tx-{}-{nanos}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Default)]
struct Flags {
    logged_on: AtomicBool,
    logged_out: AtomicBool,
}

struct App {
    f: Arc<Flags>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.f.logged_on.store(true, Ordering::SeqCst);
    }
    async fn on_logout(&self, _s: &SessionId) {
        self.f.logged_out.store(true, Ordering::SeqCst);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sequence_numbers_persist_to_store_across_session() {
    let acc_dir = unique_dir();
    let init_dir = unique_dir();

    let acc_store = Arc::new(FileStore::open(&acc_dir).unwrap());
    let init_store = Arc::new(FileStore::open(&init_dir).unwrap());

    let acc_services = Services {
        store: Some(acc_store.clone() as Arc<dyn MessageStore>),
        log: None,
    };
    let init_services = Services {
        store: Some(init_store.clone() as Arc<dyn MessageStore>),
        log: None,
    };

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 1;
    acc_cfg.reset_on_logon = false;
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 1;
    init_cfg.reset_on_logon = false;

    let acc_flags = Arc::new(Flags::default());
    let init_flags = Arc::new(Flags::default());

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(App {
            f: acc_flags.clone(),
        }),
        acc_services,
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
        init_services,
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
        "both sides should log on"
    );

    // Let a few heartbeats flow so sequence numbers advance and are persisted.
    tokio::time::sleep(Duration::from_millis(2500)).await;

    handle.logout().await;
    wait_until(
        || init_flags.logged_out.load(Ordering::SeqCst),
        Duration::from_secs(5),
    )
    .await;
    handle.join().await;

    // Reopen the initiator store from disk and confirm the advanced sequence number survived.
    let reopened = FileStore::open(&init_dir).unwrap();
    let persisted = reopened.next_sender_seq().await.unwrap();
    assert!(
        persisted >= 3,
        "sequence numbers should persist and advance (got {persisted})"
    );

    let _ = std::fs::remove_dir_all(&acc_dir);
    let _ = std::fs::remove_dir_all(&init_dir);
}
