//! T036 (US5, feature 005) — two `[SESSION]` blocks sharing BeginString/SenderCompID/TargetCompID
//! but distinct `SessionQualifier` both start as independently addressable sessions (GAP-47/
//! FR-012/FR-013), rather than one silently shadowing or colliding with the other.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use truefix::config::SessionSettings;
use truefix::{Application, Engine, Role, SessionId};

struct App {
    logons: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_sessions_differing_only_by_qualifier_both_start_and_log_on() {
    let port_a = free_port();
    let port_b = free_port();
    // Both acceptor sessions share BeginString/SenderCompID/TargetCompID; only SessionQualifier
    // and the listening port differ. Each gets its own initiator counterpart.
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
         SessionQualifier=A\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port_a}\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SessionQualifier=B\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port_b}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SessionQualifier=A\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port_a}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SessionQualifier=B\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port_b}\n"
    );

    // The two acceptor `SessionConfig`s must resolve to distinct `SessionId`s (proving the
    // qualifier reaches identity, not just parses without error) before even starting the
    // engine.
    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let resolved = settings.resolve().expect("resolve cfg");
    let acceptor_ids: Vec<_> = resolved
        .iter()
        .filter(|r| r.session.role == Role::Acceptor)
        .map(|r| r.session.session_id())
        .collect();
    assert_eq!(acceptor_ids.len(), 2);
    assert_ne!(acceptor_ids[0], acceptor_ids[1]);

    let logons = Arc::new(AtomicU32::new(0));
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
        }),
    )
    .await
    .expect("engine starts both qualifier-distinguished sessions from the cfg");

    // All four sessions (2 acceptors + 2 initiators) should independently complete a logon.
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < 4 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        logons.load(Ordering::SeqCst),
        4,
        "both qualifier-distinguished session pairs should log on independently"
    );

    engine.shutdown();
}

// --- T024 (US2, feature 006): SessionQualifier-only-distinguished group is rejected (BUG-07/FR-011) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_sessions_sharing_one_port_distinguished_only_by_qualifier_is_a_config_error() {
    let port = free_port();
    // Both acceptor sessions share BeginString/SenderCompID/TargetCompID *and* one
    // SocketAcceptPort -- the only thing distinguishing them is SessionQualifier, which has no
    // wire tag, so no live connection could ever disambiguate which one it's meant for.
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SessionQualifier=A\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SessionQualifier=B\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n"
    );

    struct NoopApp;
    #[async_trait::async_trait]
    impl Application for NoopApp {
        async fn on_logon(&self, _s: &SessionId) {}
    }

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let result = Engine::start(&settings, Arc::new(NoopApp)).await;
    let err = match result {
        Ok(_) => panic!(
            "two sessions sharing one port, distinguished only by SessionQualifier, must be \
             rejected at resolve time -- not silently started with one of them unroutable"
        ),
        Err(e) => e,
    };
    assert!(
        format!("{err}").contains("SessionQualifier"),
        "expected an error naming the SessionQualifier conflict, got: {err}"
    );
}
