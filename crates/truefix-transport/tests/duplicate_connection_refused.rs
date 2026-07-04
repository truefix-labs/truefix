//! T027/T028 (US1, feature 007): a second inbound connection presenting an already-connected
//! `SessionId` is refused before its Logon is processed (BUG-32/FR-008) — previously a second
//! connection for the same identity was routed and accepted exactly like the first, corrupting
//! sequence-number bookkeeping shared between the two competing connections.
//!
//! Covers both acceptor code paths that can accept inbound connections: the multi-session
//! `AcceptorBuilder`/`route_and_run` path (`Registry.active`) and the single-session `Acceptor`
//! path (`Acceptor::serve`'s own active-flag), since `docs/todo/004.md`'s BUG-32 names both.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{connect_initiator, Acceptor, AcceptorBuilder};

struct CountApp {
    logons: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Application for CountApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
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

fn acc(sender: &str, target: &str) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", sender, target, Role::Acceptor);
    c.heartbeat_interval = 30;
    c
}

fn init(sender: &str, target: &str) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", sender, target, Role::Initiator);
    c.heartbeat_interval = 30;
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_connection_for_an_already_connected_session_is_refused_multi_session() {
    let acc_logons = Arc::new(AtomicUsize::new(0));
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(CountApp {
            logons: acc_logons.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc("SERVER", "CLIENT"));
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    // First connection: logs on normally.
    let first_logons = Arc::new(AtomicUsize::new(0));
    let _h1 = connect_initiator(
        addr,
        init("CLIENT", "SERVER"),
        Arc::new(CountApp {
            logons: first_logons.clone(),
        }),
    )
    .await
    .unwrap();
    assert!(
        wait_until(
            || acc_logons.load(Ordering::SeqCst) >= 1,
            Duration::from_secs(5)
        )
        .await,
        "first connection must log on"
    );

    // Second connection, same SessionId (CLIENT->SERVER), while the first is still active: must
    // be refused before its Logon is processed, so the acceptor's logon count must not advance
    // past 1.
    let second_logons = Arc::new(AtomicUsize::new(0));
    let _h2 = connect_initiator(
        addr,
        init("CLIENT", "SERVER"),
        Arc::new(CountApp {
            logons: second_logons.clone(),
        }),
    )
    .await
    .unwrap();

    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(
        acc_logons.load(Ordering::SeqCst),
        1,
        "a competing second connection for the same already-connected SessionId must be refused \
         before its Logon is processed, not accepted as a second logon"
    );
    assert_eq!(
        second_logons.load(Ordering::SeqCst),
        0,
        "the second (refused) connection's initiator must never see a completed logon"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_connection_for_an_already_connected_session_is_refused_single_session() {
    let acc_logons = Arc::new(AtomicUsize::new(0));
    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc("SERVER", "CLIENT"),
        Arc::new(CountApp {
            logons: acc_logons.clone(),
        }),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let first_logons = Arc::new(AtomicUsize::new(0));
    let _h1 = connect_initiator(
        addr,
        init("CLIENT", "SERVER"),
        Arc::new(CountApp {
            logons: first_logons.clone(),
        }),
    )
    .await
    .unwrap();
    assert!(
        wait_until(
            || acc_logons.load(Ordering::SeqCst) >= 1,
            Duration::from_secs(5)
        )
        .await,
        "first connection must log on"
    );

    let second_logons = Arc::new(AtomicUsize::new(0));
    let _h2 = connect_initiator(
        addr,
        init("CLIENT", "SERVER"),
        Arc::new(CountApp {
            logons: second_logons.clone(),
        }),
    )
    .await
    .unwrap();

    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(
        acc_logons.load(Ordering::SeqCst),
        1,
        "the single-session Acceptor must also refuse a competing second connection while the \
         first is still active"
    );
    assert_eq!(
        second_logons.load(Ordering::SeqCst),
        0,
        "the second (refused) connection's initiator must never see a completed logon"
    );
}
