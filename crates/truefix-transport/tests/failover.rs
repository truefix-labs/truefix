//! T064 (US10) — multi-endpoint initiator failover: the primary is unreachable, so the initiator
//! rotates to a configured backup endpoint (FR-019).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{connect_initiator_reconnecting_multi, Acceptor, Services};

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

/// A port with nothing listening on it (used as the deliberately-unreachable primary endpoint).
fn unreachable_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l); // freed immediately; nothing will be listening here
    port
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_fails_over_to_a_backup_endpoint() {
    let logged_on = Arc::new(AtomicBool::new(false));

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(FlagApp {
            on: logged_on.clone(),
        }),
    )
    .await
    .unwrap();
    let good_addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let primary = std::net::SocketAddr::new("127.0.0.1".parse().unwrap(), unreachable_port());

    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 30;
    init_cfg.reconnect_interval = 1;

    let handle = connect_initiator_reconnecting_multi(
        vec![primary, good_addr], // primary is unreachable; the backup is the real acceptor
        init_cfg,
        Arc::new(FlagApp {
            on: Arc::new(AtomicBool::new(false)), // unused on the client's own flag
        }),
        Services::default(),
    );

    assert!(
        wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "the initiator should fail over to the backup endpoint and log on"
    );

    // `stop()` only prevents the *next* reconnect attempt; the current connection is healthy and
    // stays open indefinitely (30s heartbeat), so it is intentionally not joined here — the test
    // runtime drops the spawned task when the test function returns.
    handle.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn all_endpoints_unreachable_keeps_retrying_without_panicking() {
    let addrs = vec![
        std::net::SocketAddr::new("127.0.0.1".parse().unwrap(), unreachable_port()),
        std::net::SocketAddr::new("127.0.0.1".parse().unwrap(), unreachable_port()),
    ];
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.reconnect_interval = 1;

    struct NoopApp;
    #[async_trait::async_trait]
    impl Application for NoopApp {}

    let handle = connect_initiator_reconnecting_multi(
        addrs,
        init_cfg,
        Arc::new(NoopApp),
        Services::default(),
    );
    // Give it a couple of retry cycles; the important assertion is simply that nothing panics
    // and the loop can still be stopped cleanly.
    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.stop();
    handle.join().await;
}
