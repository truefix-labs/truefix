//! T149 (NEW-48) — a `Monitor` entry is removed (not merely marked disconnected) once its
//! connection disconnects, so a long-lived acceptor's monitor map doesn't grow monotonically.

use std::sync::Arc;
use std::time::Duration;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{AcceptorBuilder, Monitor, Services, connect_initiator};

struct NoopApp;

#[async_trait::async_trait]
impl Application for NoopApp {
    async fn on_logon(&self, _s: &SessionId) {}
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
async fn disconnected_session_entry_is_removed_not_retained() {
    let monitor = Monitor::new();
    let services = Services {
        monitor: Some(monitor.clone()),
        ..Services::default()
    };

    let mut acc = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc.heartbeat_interval = 30;
    let mut init = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init.heartbeat_interval = 30;

    let acceptor = AcceptorBuilder::bind("127.0.0.1:0".parse().unwrap(), Arc::new(NoopApp))
        .await
        .unwrap()
        .with_session(acc)
        .with_services(services);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let handle = connect_initiator(addr, init, Arc::new(NoopApp))
        .await
        .unwrap();

    let sid = SessionId::new("FIX.4.4", "SERVER", "CLIENT");

    assert!(
        wait_until(|| monitor.sessions().contains(&sid), Duration::from_secs(5)).await,
        "monitor should observe the session registering"
    );

    handle.abort();

    assert!(
        wait_until(
            || !monitor.sessions().contains(&sid),
            Duration::from_secs(5),
        )
        .await,
        "the session's entry must be removed from the monitor on disconnect, not just marked \
         disconnected — otherwise a long-lived acceptor's monitor map grows monotonically"
    );
    assert!(!monitor.is_connected(&sid));
    assert!(monitor.status(&sid).is_none());
}
