//! T009 (US1, feature 004) — a `.cfg`-only initiator with numbered backup endpoints reconnects
//! through a backup after the primary is unreachable, driven entirely by `Engine::start` (FR-001,
//! SC-001), with no additional Rust code beyond `Application`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use truefix::config::SessionSettings;
use truefix::{Application, Engine, SessionId};

struct App {
    acc_logon: Arc<AtomicBool>,
    init_logon: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, id: &SessionId) {
        if id.sender_comp_id == "SERVER" {
            self.acc_logon.store(true, Ordering::SeqCst);
        } else {
            self.init_logon.store(true, Ordering::SeqCst);
        }
    }
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_reconnects_through_a_cfg_backup_endpoint_after_the_primary_is_unreachable() {
    let dead_port = free_port(); // freed immediately, nothing listens here -> connection refused
    let backup_port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         ReconnectInterval=1\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={backup_port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={dead_port}\n\
         SocketConnectHost1=127.0.0.1\n\
         SocketConnectPort1={backup_port}\n"
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let acc_logon = Arc::new(AtomicBool::new(false));
    let init_logon = Arc::new(AtomicBool::new(false));
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            acc_logon: acc_logon.clone(),
            init_logon: init_logon.clone(),
        }),
    )
    .await
    .expect("engine starts from cfg");

    // The initiator has zero single-endpoint sessions (its only session has a backup endpoint),
    // so `initiators()` should be empty and `failover_initiators()` should have exactly one entry.
    assert!(engine.initiators().is_empty());
    assert_eq!(engine.failover_initiators().len(), 1);

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10)
        && !(acc_logon.load(Ordering::SeqCst) && init_logon.load(Ordering::SeqCst))
    {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        acc_logon.load(Ordering::SeqCst) && init_logon.load(Ordering::SeqCst),
        "the initiator should reconnect through the configured backup endpoint and both sides \
         should complete logon, with no Rust code beyond Application callbacks"
    );

    engine.shutdown();
}
