//! T070 — schedule-driven initiator: connect only while in session.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use time::Time;
use truefix_session::{Application, Role, Schedule, SessionConfig, SessionId};
use truefix_transport::{run_scheduled_initiator, AcceptorBuilder, Services};

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
