//! T050 (US10) — `Application::on_before_reset` fires before every reset (FR-013).
//!
//! **Design note**: `Session` is deliberately sans-IO and never holds an `Application` handle
//! (established during US2 for `Action::ResetStore`), so `on_before_reset` cannot literally be
//! invoked "inside `Session::reset()`" as originally sketched — it is the transport layer that
//! drives `Application`, so it is the transport that calls the hook, immediately before each of
//! the three places a reset actually takes effect: the explicit `Monitor::reset()` control path,
//! the `ForceResendWhenCorruptedStore` internal trigger, and `Action::ResetStore` (the declarative
//! signal for every other internally-triggered reset — logon-time `ResetSeqNumFlag`,
//! `ResetOnLogout`/`ResetOnDisconnect`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{Acceptor, Monitor};

#[derive(Default)]
struct Counts {
    before_reset: AtomicU32,
    logged_on: AtomicU32,
}

struct App {
    c: Arc<Counts>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.c.logged_on.fetch_add(1, Ordering::SeqCst);
    }
    async fn on_before_reset(&self, _s: &SessionId) {
        self.c.before_reset.fetch_add(1, Ordering::SeqCst);
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

/// The explicit, operator-triggered reset path (`Monitor::reset`, i.e. `Control::Reset`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_before_reset_fires_for_an_explicit_monitor_reset() {
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 5;
    acc_cfg.reset_on_logon = false; // isolate the explicit reset from logon-time ResetSeqNumFlag
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.reset_on_logon = false;

    let acc_counts = Arc::new(Counts::default());
    let init_counts = Arc::new(Counts::default());
    let monitor = Monitor::new();

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(App {
            c: acc_counts.clone(),
        }),
        truefix_transport::Services {
            monitor: Some(monitor.clone()),
            ..truefix_transport::Services::default()
        },
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let _handle = truefix_transport::connect_initiator(
        addr,
        init_cfg,
        Arc::new(App {
            c: init_counts.clone(),
        }),
    )
    .await
    .unwrap();

    let logged_on = wait_until(
        || acc_counts.logged_on.load(Ordering::SeqCst) > 0,
        Duration::from_secs(5),
    )
    .await;
    assert!(logged_on, "acceptor side should log on");

    let acc_id: Vec<_> = monitor.sessions();
    assert_eq!(acc_id.len(), 1);
    assert_eq!(acc_counts.before_reset.load(Ordering::SeqCst), 0);

    assert!(monitor.reset(&acc_id[0]).await);
    assert!(
        wait_until(
            || acc_counts.before_reset.load(Ordering::SeqCst) > 0,
            Duration::from_secs(5),
        )
        .await,
        "on_before_reset should fire for an explicit Monitor::reset"
    );
    assert_eq!(acc_counts.before_reset.load(Ordering::SeqCst), 1);
}

/// An internally-triggered reset (`ResetOnLogout`, surfaced to the transport as
/// `Action::ResetStore`) also fires the hook — not just the explicit `Monitor::reset` path.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_before_reset_fires_for_an_internally_triggered_reset() {
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 5;
    acc_cfg.reset_on_logon = false;
    acc_cfg.reset_on_logout = true; // the internal trigger under test
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.reset_on_logon = false;

    let acc_counts = Arc::new(Counts::default());
    let init_counts = Arc::new(Counts::default());

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(App {
            c: acc_counts.clone(),
        }),
        truefix_transport::Services::default(),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let handle = truefix_transport::connect_initiator(
        addr,
        init_cfg,
        Arc::new(App {
            c: init_counts.clone(),
        }),
    )
    .await
    .unwrap();

    assert!(
        wait_until(
            || acc_counts.logged_on.load(Ordering::SeqCst) > 0
                && init_counts.logged_on.load(Ordering::SeqCst) > 0,
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on"
    );
    assert_eq!(acc_counts.before_reset.load(Ordering::SeqCst), 0);

    handle.logout().await;

    assert!(
        wait_until(
            || acc_counts.before_reset.load(Ordering::SeqCst) > 0,
            Duration::from_secs(5),
        )
        .await,
        "on_before_reset should fire for the ResetOnLogout-triggered internal reset"
    );
}
