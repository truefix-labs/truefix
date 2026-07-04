//! T070 — schedule-driven initiator: connect only while in session.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use time::Time;
use truefix_session::{Application, Role, Schedule, SessionConfig, SessionId};
use truefix_store::{FileStore, MessageStore};
use truefix_transport::{AcceptorBuilder, Services, run_scheduled_initiator};

struct FlagApp {
    on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for FlagApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.on.store(true, Ordering::SeqCst);
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

async fn start_acceptor() -> SocketAddr {
    let mut template = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    template.check_latency = false;
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            on: Arc::new(AtomicBool::new(false)),
        }),
    )
    .await
    .unwrap()
    .with_dynamic_template(template);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();
    addr
}

fn initiator_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    c.check_latency = false;
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connects_when_in_session() {
    let addr = start_acceptor().await;
    let flag = Arc::new(AtomicBool::new(false));
    let handle = run_scheduled_initiator(
        addr,
        initiator_cfg(),
        Arc::new(FlagApp { on: flag.clone() }),
        Services::default(),
        Schedule::non_stop(),
    );
    assert!(
        wait_until(|| flag.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "scheduled initiator should connect and log on while in session"
    );
    handle.stop();
    handle.join().await;
}

// --- T030 (US3, feature 006): restart mid-window preserves state (BUG-09/GAP-48/FR-014) ---

fn unique_store_dir() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "truefix-scheduled-restart-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_with_a_store_whose_creation_time_is_already_in_the_active_window_does_not_reset() {
    let dir = unique_store_dir();
    let store = FileStore::open(&dir).unwrap();
    let original_creation_time = store.creation_time().await.unwrap();
    let store: std::sync::Arc<dyn MessageStore> = std::sync::Arc::new(store);

    let addr = start_acceptor().await;
    let flag = Arc::new(AtomicBool::new(false));
    // Schedule::non_stop() would never exercise decide_schedule_action's Enter/Exit transitions
    // at all -- use a real, currently-active daily window instead so the seeded `was_in_session`
    // value actually matters (a restart landing inside this window should be indistinguishable
    // from "already running", not treated as a fresh entry).
    let now = time::OffsetDateTime::now_utc();
    let start = (now - Duration::from_secs(3600)).time();
    let end = (now + Duration::from_secs(3600)).time();
    let schedule = Schedule::daily(start, end);

    // ResetOnLogon defaults to true (SessionConfig::default), which would itself force a fresh
    // store reset on every connect via the session-level fix from US1 (T017, Action::ResetStore)
    // -- unrelated to the schedule-level fix under test here. Disable it so only the
    // schedule/store-driven reset path (or lack thereof) can affect the store's creation time.
    let mut cfg = initiator_cfg();
    cfg.reset_on_logon = false;

    let handle = run_scheduled_initiator(
        addr,
        cfg,
        Arc::new(FlagApp { on: flag.clone() }),
        Services {
            store: Some(store.clone()),
            ..Services::default()
        },
        schedule,
    );
    assert!(
        wait_until(|| flag.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "scheduled initiator should still connect and log on inside the active window"
    );
    handle.stop();
    handle.join().await;

    // `reset()` always stamps a fresh creation time (GAP-38, feature 005); an unchanged creation
    // time is direct proof no reset occurred, regardless of how far sequence numbers legitimately
    // advanced from the Logon exchange itself.
    assert_eq!(
        store.creation_time().await.unwrap(),
        original_creation_time,
        "a restart landing inside an already-active schedule window must not spuriously reset \
         the store (which would stamp a fresh creation time)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn does_not_connect_out_of_session() {
    let addr = start_acceptor().await;
    let flag = Arc::new(AtomicBool::new(false));
    // An empty window (start == end) is never in session.
    let schedule = Schedule::daily(
        Time::from_hms(12, 0, 0).unwrap(),
        Time::from_hms(12, 0, 0).unwrap(),
    );
    let handle = run_scheduled_initiator(
        addr,
        initiator_cfg(),
        Arc::new(FlagApp { on: flag.clone() }),
        Services::default(),
        schedule,
    );
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(
        !flag.load(Ordering::SeqCst),
        "scheduled initiator must not connect out of session"
    );
    handle.stop();
    handle.join().await;
}
