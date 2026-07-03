//! T009/T010 (US2, feature 005) — `.cfg`-only multi-session acceptor: `AllowedRemoteAddresses`/
//! `DynamicSession`/`AcceptorTemplate` actually govern `Engine::start`'s acceptor behavior (BUG-03),
//! instead of being silently unreachable. Before this fix, two `[SESSION]` acceptor blocks sharing
//! one `SocketAcceptPort` would fail outright (`Engine::start` called `Acceptor::bind_with` once per
//! session, so the second bind hit "address already in use").

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

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_acceptor_sessions_sharing_one_port_both_start_and_route_correctly() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER1\n\
         TargetCompID=CLIENT1\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER2\n\
         TargetCompID=CLIENT2\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
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
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: logons.clone(),
        }),
    )
    .await
    .expect("engine starts from cfg with two acceptor sessions sharing one port");

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < 4 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        logons.load(Ordering::SeqCst),
        4,
        "both acceptor sessions and both initiator sessions should log on, routed by CompID \
         through the shared multi-session acceptor"
    );

    engine.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn allowed_remote_addresses_on_a_grouped_acceptor_session_refuses_disallowed_connections() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER1\n\
         TargetCompID=CLIENT1\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         AllowedRemoteAddresses=10.0.0.1\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER2\n\
         TargetCompID=CLIENT2\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT1\n\
         TargetCompID=SERVER1\n\
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
    .expect("engine starts");

    // Loopback is excluded from the allow-list one grouped session declares; since the allow-list
    // is enforced at the shared listener, the connection from 127.0.0.1 must never log on.
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(
        logons.load(Ordering::SeqCst),
        0,
        "a connection from a disallowed remote address must be refused before any Logon completes"
    );

    engine.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_sessions_both_declaring_a_dynamic_template_on_one_port_is_a_typed_error() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER1\n\
         TargetCompID=CLIENT1\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         DynamicSession=Y\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER2\n\
         TargetCompID=CLIENT2\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         DynamicSession=Y\n"
    );

    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let result = Engine::start(
        &settings,
        Arc::new(App {
            logons: Arc::new(AtomicU32::new(0)),
        }),
    )
    .await;
    let err = match result {
        Ok(_) => panic!(
            "two dynamic templates on one acceptor group must be a typed error, not an ambiguous \
             silent pick"
        ),
        Err(e) => e,
    };
    assert!(
        format!("{err:?}").contains("AmbiguousAcceptorTemplate")
            || format!("{err}").to_lowercase().contains("ambiguous"),
        "expected an ambiguous-acceptor-template error, got {err:?}"
    );
}
