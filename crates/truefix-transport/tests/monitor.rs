//! T079 — operational monitoring: status, sequence numbers, connection health, actions.

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
async fn monitor_reports_status_and_supports_force_logout() {
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

    let _handle = connect_initiator(addr, init, Arc::new(NoopApp))
        .await
        .unwrap();

    // The acceptor session id (our perspective).
    let sid = SessionId::new("FIX.4.4", "SERVER", "CLIENT");

    assert!(
        wait_until(
            || monitor.status(&sid).map(|s| s.logged_on).unwrap_or(false),
            Duration::from_secs(5),
        )
        .await,
        "monitor should observe the session logging on"
    );

    let status = monitor.status(&sid).unwrap();
    assert!(status.logged_on);
    assert!(status.next_sender_seq >= 1);
    assert!(status.next_target_seq >= 1);
    assert!(monitor.is_connected(&sid));
    assert!(monitor.sessions().contains(&sid));

    // Operational force-logout via the monitor.
    assert!(monitor.force_logout(&sid).await);
    assert!(
        wait_until(|| !monitor.is_connected(&sid), Duration::from_secs(5)).await,
        "force_logout should disconnect the session"
    );
}
