//! T007 (US1) — start a working acceptor + initiator entirely from a `.cfg`, with no code beyond
//! providing the Application (FR-013/014; SC-001).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use truefix::config::SessionSettings;
use truefix::{Application, Engine, SessionId};

struct App {
    logons: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }
}

/// Grab an ephemeral port, then release it for the acceptor to bind.
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_starts_acceptor_and_initiator_from_cfg() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n"
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let logons = Arc::new(AtomicU32::new(0));
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
        }),
    )
    .await
    .expect("engine starts from cfg");

    // Both the acceptor session and the initiator session should complete a logon.
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        logons.load(Ordering::SeqCst),
        2,
        "both sessions should log on when started from the .cfg"
    );

    engine.shutdown();
}
