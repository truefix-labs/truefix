//! T061 — store/log wired into the transport: sequence numbers persist across a session.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use truefix_core::{decode, Field, Message};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_store::{FileStore, MessageStore};
use truefix_transport::{connect_initiator_with, Acceptor, Monitor, Services};

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
        ..Services::default()
    };
    let init_services = Services {
        store: Some(init_store.clone() as Arc<dyn MessageStore>),
        log: None,
        ..Services::default()
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

fn new_order(clordid: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "D")); // NewOrderSingle
    m.body.set(Field::string(11, clordid)); // ClOrdID
    m
}

/// T015 (US2) — sent application-message *bodies* (not just sequence numbers) are persisted to the
/// store, so a post-restart ResendRequest can replay them (FR-001/002).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sent_message_bodies_persist_to_store() {
    let acc_dir = unique_dir();
    let init_dir = unique_dir();

    let acc_store = Arc::new(FileStore::open(&acc_dir).unwrap());
    let init_store = Arc::new(FileStore::open(&init_dir).unwrap());
    let monitor = Monitor::new();

    let acc_services = Services {
        store: Some(acc_store.clone() as Arc<dyn MessageStore>),
        ..Services::default()
    };
    let init_services = Services {
        store: Some(init_store.clone() as Arc<dyn MessageStore>),
        monitor: Some(monitor.clone()),
        ..Services::default()
    };

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 5;
    acc_cfg.reset_on_logon = false;
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 5;
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

    // The initiator sends two application orders; the transport persists their bodies to the store.
    let id = SessionId::new("FIX.4.4", "CLIENT", "SERVER");
    monitor.send_app(&id, new_order("ORDER-1")).await;
    monitor.send_app(&id, new_order("ORDER-2")).await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    handle.logout().await;
    wait_until(
        || init_flags.logged_out.load(Ordering::SeqCst),
        Duration::from_secs(5),
    )
    .await;
    handle.join().await;

    // Reopen the store from disk and confirm both order bodies survived.
    let reopened = FileStore::open(&init_dir).unwrap();
    let out = reopened.next_sender_seq().await.unwrap();
    let stored = reopened.get(1, out.saturating_sub(1)).await.unwrap();
    let clordids: Vec<String> = stored
        .iter()
        .filter_map(|(_, b)| decode(b).ok())
        .filter(|m| m.msg_type() == Some("D"))
        .filter_map(|m| {
            m.body
                .get(11)
                .and_then(|f| f.as_str().ok())
                .map(String::from)
        })
        .collect();
    assert!(
        clordids.contains(&"ORDER-1".to_string()) && clordids.contains(&"ORDER-2".to_string()),
        "both order bodies should persist to the store (got {clordids:?})"
    );

    let _ = std::fs::remove_dir_all(&acc_dir);
    let _ = std::fs::remove_dir_all(&init_dir);
}

/// US2 (FR-004) — a `ResetOnLogout`-triggered reset must clear the durable store, not just the
/// in-memory session state, so stale message bodies aren't retained under sequence numbers that
/// get recycled from 1 on the next logon.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_on_logout_clears_the_durable_store() {
    let acc_dir = unique_dir();
    let init_dir = unique_dir();

    let acc_store = Arc::new(FileStore::open(&acc_dir).unwrap());
    let init_store = Arc::new(FileStore::open(&init_dir).unwrap());

    let acc_services = Services {
        store: Some(acc_store.clone() as Arc<dyn MessageStore>),
        ..Services::default()
    };
    let init_services = Services {
        store: Some(init_store.clone() as Arc<dyn MessageStore>),
        ..Services::default()
    };

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 5;
    acc_cfg.reset_on_logon = false;
    acc_cfg.reset_on_logout = true; // the behavior under test
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 5;
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

    // The Logon itself is persisted to the acceptor's store under seq 1, so there is something to
    // clear.
    tokio::time::sleep(Duration::from_millis(300)).await;
    {
        let out = acc_store.next_sender_seq().await.unwrap();
        assert!(out > 1, "the acceptor should have persisted its Logon");
    }

    // Peer-initiated logout: the acceptor did not send Logout first, so it takes the
    // "peer logged out first" branch of `on_logout_msg`, which also calls `enter_disconnected`.
    handle.logout().await;
    wait_until(
        || init_flags.logged_out.load(Ordering::SeqCst),
        Duration::from_secs(5),
    )
    .await;
    handle.join().await;
    // Give the acceptor's connection task a moment to process the inbound Logout and react.
    wait_until(
        || acc_flags.logged_out.load(Ordering::SeqCst),
        Duration::from_secs(5),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let reopened = FileStore::open(&acc_dir).unwrap();
    let next_out = reopened.next_sender_seq().await.unwrap();
    assert_eq!(
        next_out, 1,
        "ResetOnLogout should reset the acceptor's persisted sequence numbers to 1"
    );
    let stored = reopened.get(1, 100).await.unwrap();
    assert!(
        stored.is_empty(),
        "ResetOnLogout should clear previously-persisted message bodies from the store, got {} entries",
        stored.len()
    );

    let _ = std::fs::remove_dir_all(&acc_dir);
    let _ = std::fs::remove_dir_all(&init_dir);
}
