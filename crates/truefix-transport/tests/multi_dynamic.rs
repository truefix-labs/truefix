//! T062 / T063 — multi-session routing, dynamic sessions, and allow-listing (SC-003).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{connect_initiator, AcceptorBuilder, SessionHandle};

struct CountApp {
    logons: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Application for CountApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }
}

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
async fn static_and_dynamic_sessions_route_correctly() {
    let acc_logons = Arc::new(AtomicUsize::new(0));
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(CountApp {
            logons: acc_logons.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc("SERVER1", "CLIENT1"))
    .with_session(acc("SERVER2", "CLIENT2"))
    .with_dynamic_template(acc("TEMPLATE", "TEMPLATE"));
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut handles: Vec<SessionHandle> = Vec::new();
    let flags: Vec<Arc<AtomicBool>> = (0..3).map(|_| Arc::new(AtomicBool::new(false))).collect();

    // Two static (CLIENT1->SERVER1, CLIENT2->SERVER2) and one dynamic (CLIENT3->SERVER3).
    let routes = [
        ("CLIENT1", "SERVER1"),
        ("CLIENT2", "SERVER2"),
        ("CLIENT3", "SERVER3"),
    ];
    for (i, (sender, target)) in routes.iter().enumerate() {
        let app = Arc::new(FlagApp {
            on: flags[i].clone(),
        });
        handles.push(
            connect_initiator(addr, init(sender, target), app)
                .await
                .unwrap(),
        );
    }

    assert!(
        wait_until(
            || acc_logons.load(Ordering::SeqCst) >= 3
                && flags.iter().all(|f| f.load(Ordering::SeqCst)),
            Duration::from_secs(5),
        )
        .await,
        "all three sessions (2 static + 1 dynamic) should log on"
    );

    drop(handles);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connection_from_disallowed_remote_is_refused() {
    let acc_logons = Arc::new(AtomicUsize::new(0));
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(CountApp {
            logons: acc_logons.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc("SERVER1", "CLIENT1"))
    .allow_remotes(vec!["10.0.0.1".parse().unwrap()]); // loopback is NOT allowed
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let flag = Arc::new(AtomicBool::new(false));
    let _h = connect_initiator(
        addr,
        init("CLIENT1", "SERVER1"),
        Arc::new(FlagApp { on: flag.clone() }),
    )
    .await
    .unwrap();

    // Give it time; the connection must be refused, so no logon ever completes.
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(!flag.load(Ordering::SeqCst), "initiator must not log on");
    assert_eq!(
        acc_logons.load(Ordering::SeqCst),
        0,
        "acceptor must not log on"
    );
}
