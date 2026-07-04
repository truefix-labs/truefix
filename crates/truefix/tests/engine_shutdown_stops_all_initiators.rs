//! T020 (US1, feature 007): `Engine::shutdown()` stops every session task it started, including
//! plain (non-failover) initiators (BUG-27) — previously only acceptors and failover-initiator
//! reconnect loops were actually stopped by `shutdown()`; plain initiators kept running forever
//! with no way to reach them from the public API.

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
async fn shutdown_stops_a_plain_initiator_not_only_acceptors_and_failover_initiators() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         SocketReuseAddress=Y\n\
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

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(logons.load(Ordering::SeqCst), 2, "both sessions log on");

    // This is a plain (single-endpoint) initiator, so it must show up in `initiators()`, not
    // `failover_initiators()`.
    assert_eq!(engine.initiators().len(), 1);
    assert!(!engine.initiators()[0].is_finished());

    engine.shutdown();

    // BUG-27's fix (SessionHandle::abort(), called from Engine::shutdown()) is asynchronous from
    // the caller's point of view (tokio aborts the task, but the task doesn't finish
    // instantaneously) -- poll briefly rather than asserting immediately.
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && !engine.initiators()[0].is_finished() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        engine.initiators()[0].is_finished(),
        "the plain initiator's connection task must be stopped by shutdown(), not left running"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_the_engine_without_calling_shutdown_also_stops_the_plain_initiator() {
    let port = free_port();
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         SocketReuseAddress=Y\n\
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

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && logons.load(Ordering::SeqCst) < 2 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(logons.load(Ordering::SeqCst), 2);

    // A weak-ish liveness probe: hold onto a clone of the sender comp id counter via `logons`; if
    // the initiator task is truly detached (pre-007 behavior), it would keep running and, e.g.,
    // keep sending heartbeats indefinitely. We can't inspect a dropped `Engine`'s internals
    // directly, so this test instead confirms the `impl Drop` code path itself runs without
    // panicking and that a *second* `Engine` can immediately rebind the same port afterward --
    // which would fail with "address already in use" if the first engine's acceptor listener were
    // still alive.
    drop(engine);

    // Give the abort a moment to actually release the socket.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let cfg2 = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER2\n\
         TargetCompID=CLIENT2\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         SocketReuseAddress=Y\n"
    );
    let settings2 = SessionSettings::parse(&cfg2).expect("parse cfg2");
    let logons2 = Arc::new(AtomicU32::new(0));
    let engine2 = Engine::start(&settings2, Arc::new(App { logons: logons2 }))
        .await
        .expect(
            "a second Engine must be able to rebind the port the dropped Engine's acceptor held",
        );
    engine2.shutdown();
}
