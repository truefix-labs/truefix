//! T022 — two-process (two-task) logon -> heartbeat -> logout over real TCP.
//!
//! Runs an acceptor and an initiator on loopback and drives a full session handshake, sustained
//! heartbeats, and a graceful bilateral logout. Validated on FIX 4.2 and FIX 4.4.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use truefix_core::{Message, Reject};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{connect_initiator, Acceptor};

#[derive(Default)]
struct Counters {
    logged_on: AtomicBool,
    logged_out: AtomicBool,
    heartbeats: AtomicUsize,
}

struct TestApp {
    c: Arc<Counters>,
}

#[async_trait::async_trait]
impl Application for TestApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.c.logged_on.store(true, Ordering::SeqCst);
    }
    async fn on_logout(&self, _s: &SessionId) {
        self.c.logged_out.store(true, Ordering::SeqCst);
    }
    async fn from_admin(&self, msg: &Message, _s: &SessionId) -> Result<(), Reject> {
        if msg.msg_type() == Some("0") {
            self.c.heartbeats.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
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

async fn run_handshake(begin_string: &str) {
    let acc_counters = Arc::new(Counters::default());
    let init_counters = Arc::new(Counters::default());

    let mut acc_cfg = SessionConfig::new(begin_string, "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 1;
    let mut init_cfg = SessionConfig::new(begin_string, "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 1;

    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(TestApp {
            c: acc_counters.clone(),
        }),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let handle = connect_initiator(
        addr,
        init_cfg,
        Arc::new(TestApp {
            c: init_counters.clone(),
        }),
    )
    .await
    .unwrap();

    // Both sides log on.
    assert!(
        wait_until(
            || acc_counters.logged_on.load(Ordering::SeqCst)
                && init_counters.logged_on.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "{begin_string}: both sides should log on"
    );

    // Heartbeats flow both ways (hb interval = 1s).
    assert!(
        wait_until(
            || acc_counters.heartbeats.load(Ordering::SeqCst) >= 1
                && init_counters.heartbeats.load(Ordering::SeqCst) >= 1,
            Duration::from_secs(5),
        )
        .await,
        "{begin_string}: both sides should exchange heartbeats"
    );

    // Initiator starts a graceful logout; both sides log out.
    handle.logout().await;
    assert!(
        wait_until(
            || acc_counters.logged_out.load(Ordering::SeqCst)
                && init_counters.logged_out.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "{begin_string}: both sides should log out"
    );

    handle.join().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn logon_heartbeat_logout_fix42() {
    run_handshake("FIX.4.2").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn logon_heartbeat_logout_fix44() {
    run_handshake("FIX.4.4").await;
}
