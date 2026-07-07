//! NEW-121: initiator handles expose their `SessionId`.
//! NEW-122/NEW-123: `Engine::monitor()` gives session-state query and control (including acceptor
//! sessions, which previously had no per-session handle from `Engine` at all).
//! NEW-143: `Engine::skipped_sessions()` exposes sessions skipped via
//! `ContinueInitializationOnError=Y`, previously only logged and then discarded.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
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

// --- NEW-122/NEW-123: Engine::monitor() addresses acceptor sessions by SessionId ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audit006_engine_monitor_tracks_and_controls_an_acceptor_session() {
    let acc_port = free_port();
    let cfg = format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nHeartBtInt=30\nResetOnLogon=Y\n\
         [SESSION]\nConnectionType=acceptor\nSenderCompID=SERVER\nTargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\nSocketAcceptPort={acc_port}\n\
         [SESSION]\nConnectionType=initiator\nSenderCompID=CLIENT\nTargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\nSocketConnectPort={acc_port}\n"
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

    assert!(
        wait_until(
            || logons.load(Ordering::SeqCst) >= 2,
            Duration::from_secs(5)
        )
        .await,
        "both sides should log on"
    );

    let acceptor_id = SessionId::new("FIX.4.4", "SERVER", "CLIENT");
    assert!(
        wait_until(
            || engine.monitor().is_connected(&acceptor_id),
            Duration::from_secs(5)
        )
        .await,
        "the acceptor session (previously unaddressable from Engine) must be visible via monitor()"
    );
    assert!(engine.monitor().sessions().contains(&acceptor_id));

    // Force-logout the acceptor session by SessionId, through Engine::monitor() -- NEW-122's
    // missing capability: no way to programmatically manage a specific acceptor session.
    assert!(engine.monitor().force_logout(&acceptor_id).await);
    assert!(
        wait_until(
            || !engine.monitor().is_connected(&acceptor_id),
            Duration::from_secs(5)
        )
        .await,
        "force_logout should disconnect the targeted acceptor session"
    );

    engine.shutdown();
}

// --- NEW-143: skipped sessions are visible to callers, not just logged ---

fn misconfigured_session_block() -> &'static str {
    "\n[SESSION]\n\
     ConnectionType=acceptor\n\
     SenderCompID=BROKEN\n\
     TargetCompID=CLIENT\n\
     SocketAcceptAddress=127.0.0.1\n\
     SocketAcceptPort=1\n\
     UseDataDictionary=Y\n"
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audit006_skipped_sessions_are_programmatically_visible() {
    let good_port = free_port();
    let cfg = format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nHeartBtInt=30\nResetOnLogon=Y\n\
         [SESSION]\nConnectionType=acceptor\nSenderCompID=SERVER\nTargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\nSocketAcceptPort={good_port}\n\
         {broken}\
         ContinueInitializationOnError=Y\n",
        broken = misconfigured_session_block(),
    );
    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: Arc::new(AtomicU32::new(0)),
        }),
    )
    .await
    .expect("engine starts, skipping the misconfigured session");

    assert_eq!(engine.skipped_sessions().len(), 1);
    assert!(engine.skipped_sessions()[0].0.contains("BROKEN"));

    engine.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audit006_no_skipped_sessions_when_everything_resolves() {
    let acc_port = free_port();
    let cfg = format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nHeartBtInt=30\n\
         [SESSION]\nConnectionType=acceptor\nSenderCompID=SERVER\nTargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\nSocketAcceptPort={acc_port}\n"
    );
    let settings = SessionSettings::parse(&cfg).expect("parse cfg");
    let engine = Engine::start(
        &settings,
        Arc::new(App {
            logons: Arc::new(AtomicU32::new(0)),
        }),
    )
    .await
    .expect("engine starts");
    assert!(engine.skipped_sessions().is_empty());
    engine.shutdown();
}

// --- NEW-121: initiator handles expose their SessionId ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audit006_engine_initiator_handle_exposes_session_id() {
    let acc_port = free_port();
    let cfg = format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nHeartBtInt=30\nResetOnLogon=Y\n\
         [SESSION]\nConnectionType=acceptor\nSenderCompID=SERVER\nTargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\nSocketAcceptPort={acc_port}\n\
         [SESSION]\nConnectionType=initiator\nSenderCompID=CLIENT\nTargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\nSocketConnectPort={acc_port}\n"
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

    assert_eq!(engine.initiators().len(), 1);
    let expected = SessionId::new("FIX.4.4", "CLIENT", "SERVER");
    assert_eq!(engine.initiators()[0].session_id(), &expected);

    engine.shutdown();
}
