//! Audit 006 engine lifecycle coverage for dynamic acceptors and shutdown.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use truefix::config::SessionSettings;
use truefix::{Application, Engine, Role, SessionConfig, SessionId, connect_initiator};

struct App {
    logons: Arc<AtomicU32>,
    logouts: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }

    async fn on_logout(&self, _s: &SessionId) {
        self.logouts.fetch_add(1, Ordering::SeqCst);
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
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
async fn dynamic_acceptor_builds_independent_services_per_identity() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=N\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=TEMPLATE_SERVER\n\
         TargetCompID=TEMPLATE_CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         DynamicSession=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT1\n\
         TargetCompID=SERVER1\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT2\n\
         TargetCompID=SERVER2\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n"
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let logons = Arc::new(AtomicU32::new(0));
    let logouts = Arc::new(AtomicU32::new(0));
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
            logouts,
        }),
    )
    .await
    .expect("engine starts");

    assert!(
        wait_until(
            || logons.load(Ordering::SeqCst) >= 4,
            Duration::from_secs(5)
        )
        .await,
        "two dynamic acceptor sessions and two initiators should all log on"
    );

    engine.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_shutdown_stops_accepted_session_tasks() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=N\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n"
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let engine_logons = Arc::new(AtomicU32::new(0));
    let engine_logouts = Arc::new(AtomicU32::new(0));
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: engine_logons.clone(),
            logouts: engine_logouts,
        }),
    )
    .await
    .expect("engine starts");

    let client_logons = Arc::new(AtomicU32::new(0));
    let client_logouts = Arc::new(AtomicU32::new(0));
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 1;
    init_cfg.reset_on_logon = false;
    let _handle = connect_initiator(
        format!("127.0.0.1:{port}").parse().unwrap(),
        init_cfg,
        Arc::new(App {
            logons: client_logons.clone(),
            logouts: client_logouts.clone(),
        }),
    )
    .await
    .expect("initiator connects");

    assert!(
        wait_until(
            || engine_logons.load(Ordering::SeqCst) > 0 && client_logons.load(Ordering::SeqCst) > 0,
            Duration::from_secs(5),
        )
        .await,
        "engine acceptor and external initiator should log on"
    );

    engine.shutdown();

    assert!(
        wait_until(
            || client_logouts.load(Ordering::SeqCst) > 0,
            Duration::from_secs(5),
        )
        .await,
        "external initiator should observe disconnect when Engine shuts down accepted sessions"
    );
}
