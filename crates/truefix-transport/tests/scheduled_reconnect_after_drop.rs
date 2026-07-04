//! T023/T025 (US1, feature 007): a scheduled initiator reconnects after its connection task
//! finishes from a drop (BUG-25), and failed connect attempts back off rather than retrying at a
//! fixed 200ms interval (BUG-94).

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use truefix_session::{Application, Role, Schedule, SessionConfig, SessionId};
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

fn initiator_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    c.check_latency = false;
    c
}

fn free_addr() -> SocketAddr {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnects_after_the_connection_task_finishes_from_a_drop() {
    let raw_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = raw_listener.local_addr().unwrap();
    raw_listener.set_nonblocking(true).unwrap();
    let async_listener = tokio::net::TcpListener::from_std(raw_listener).unwrap();

    let flag = Arc::new(AtomicBool::new(false));
    let handle = run_scheduled_initiator(
        addr,
        initiator_cfg(),
        Arc::new(FlagApp { on: flag.clone() }),
        Services::default(),
        Schedule::non_stop(),
    );

    // Accept the initiator's TCP connection once, then sever it without ever completing a FIX
    // handshake -- this is what a genuine mid-connection drop looks like from the initiator's
    // side. Before BUG-25's fix, `current` would stay `Some(handle)` forever after this, and the
    // reconnect condition would never fire again.
    let (stream, _) = tokio::time::timeout(Duration::from_secs(5), async_listener.accept())
        .await
        .expect("initiator should attempt to connect")
        .unwrap();
    drop(stream);
    drop(async_listener);

    // Give the port a moment to free up, then bring up a real acceptor on the same address for
    // the initiator's reconnect to land on.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut template = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    template.check_latency = false;
    let acceptor = AcceptorBuilder::bind(
        addr,
        Arc::new(FlagApp {
            on: Arc::new(AtomicBool::new(false)),
        }),
    )
    .await
    .unwrap()
    .with_dynamic_template(template);
    acceptor.serve();

    assert!(
        wait_until(|| flag.load(Ordering::SeqCst), Duration::from_secs(10)).await,
        "the scheduled initiator must reconnect once its dropped connection task has finished, \
         not stay permanently stuck believing it's still connected"
    );

    handle.stop();
    handle.join().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_connect_attempts_back_off_before_finding_a_now_available_acceptor() {
    let addr = free_addr(); // freed immediately -- nothing listens here yet, so connects refuse

    let mut cfg = initiator_cfg();
    cfg.reconnect_interval_steps = vec![2]; // back off to 2s after any failed attempt

    let flag = Arc::new(AtomicBool::new(false));
    let handle = run_scheduled_initiator(
        addr,
        cfg,
        Arc::new(FlagApp { on: flag.clone() }),
        Services::default(),
        Schedule::non_stop(),
    );

    // Let at least one connect attempt fail (refused) and enter its 2s backoff window. A refused
    // TCP connect fails fast (well under 300ms).
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Now bring up a real acceptor on the same address. If the loop were still retrying every
    // fixed 200ms (pre-fix), it would connect almost immediately. With the 2s backoff already in
    // effect from the just-failed attempt, it should take close to (up to) that long instead.
    let acceptor_ready_at = std::time::Instant::now();
    let mut template = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    template.check_latency = false;
    let acceptor = AcceptorBuilder::bind(
        addr,
        Arc::new(FlagApp {
            on: Arc::new(AtomicBool::new(false)),
        }),
    )
    .await
    .unwrap()
    .with_dynamic_template(template);
    acceptor.serve();

    assert!(
        wait_until(|| flag.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "the initiator must eventually connect once its backoff window elapses"
    );
    let elapsed = acceptor_ready_at.elapsed();
    assert!(
        elapsed >= Duration::from_millis(800),
        "expected the backed-off retry to take a substantial fraction of the 2s backoff window \
         before connecting, not the pre-fix fixed-200ms cadence; took {elapsed:?}"
    );

    handle.stop();
    handle.join().await;
}
