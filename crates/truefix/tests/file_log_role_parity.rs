//! NEW-156/NEW-157 (feature 012): `FileLog`'s non-blocking-write, `shutdown()`-drain, and retention
//! behavior must be identical whether the owning session is configured as an acceptor or an
//! initiator (FR-008). Added by a `/speckit-analyze` remediation pass (D1): `spec.md`'s Story 1/2
//! each specify an explicit acceptor+initiator acceptance scenario, but the original task list only
//! exercised this for Story 3 (`NEW-158`). Both sessions are wired identically through the same
//! `.cfg`-driven `Engine::start` path, proving the wiring — not just the design — is symmetric.

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

fn unique_dir(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "truefix-file-log-role-parity-{name}-{}-{nanos}",
        std::process::id(),
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// T007 (US1, `NEW-156`): both an acceptor and an initiator session, each `FileLogPath`-backed and
/// started through the same `.cfg`-driven `Engine::start` path, get a `FileLog` whose
/// `shutdown()` flushes every entry queued before it returns — the same non-blocking,
/// channel-backed contract T002-T006 verify at the `FileLog` unit level, now verified end-to-end
/// for both roles.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_log_shutdown_drain_is_identical_for_acceptor_and_initiator() {
    let port = free_port();
    let acc_dir = unique_dir("acceptor");
    let init_dir = unique_dir("initiator");
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
         FileLogPath={}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n\
         FileLogPath={}\n",
        acc_dir.display(),
        init_dir.display()
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
    assert_eq!(
        logons.load(Ordering::SeqCst),
        2,
        "both the acceptor and initiator session should log on"
    );

    // Flush every log this Engine built (both roles' FileLog instances) before tearing down,
    // mirroring Engine::logs()'s own documented pattern.
    for log in engine.logs() {
        log.shutdown().await;
    }
    engine.shutdown();

    let acc_events = std::fs::read_to_string(acc_dir.join("event.log"))
        .expect("acceptor session's FileLog must have created and flushed event.log");
    let init_events = std::fs::read_to_string(init_dir.join("event.log"))
        .expect("initiator session's FileLog must have created and flushed event.log");
    assert!(
        !acc_events.is_empty(),
        "acceptor session's FileLog must have flushed at least one event"
    );
    assert!(
        !init_events.is_empty(),
        "initiator session's FileLog must have flushed at least one event"
    );

    let _ = std::fs::remove_dir_all(&acc_dir);
    let _ = std::fs::remove_dir_all(&init_dir);
}

/// T023 (US2, `NEW-157`): a generation-count retention policy configured via `.cfg`
/// (`FileLogMaxGenerations`) applies identically to an acceptor and an initiator session started
/// through the same `Engine::start` path -- the retained-backup cap holds for both roles, not just
/// one.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retention_policy_caps_backups_identically_for_acceptor_and_initiator() {
    let port = free_port();
    let acc_dir = unique_dir("acceptor-retention");
    let init_dir = unique_dir("initiator-retention");
    let cfg = format!(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=1\n\
         ResetOnLogon=Y\n\
         MaxFileLogSize=1\n\
         FileLogMaxGenerations=2\n\
         \n\
         [SESSION]\n\
         ConnectionType=acceptor\n\
         SenderCompID=SERVER\n\
         TargetCompID=CLIENT\n\
         SocketAcceptAddress=127.0.0.1\n\
         SocketAcceptPort={port}\n\
         FileLogPath={}\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort={port}\n\
         FileLogPath={}\n",
        acc_dir.display(),
        init_dir.display()
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

    // Let several 1-second heartbeats flow so MaxFileLogSize=1 forces repeated rotation on both
    // sides.
    tokio::time::sleep(Duration::from_millis(3500)).await;

    for log in engine.logs() {
        log.shutdown().await;
    }
    engine.shutdown();

    for dir in [&acc_dir, &init_dir] {
        assert!(
            dir.join("messages.log.1").exists(),
            "{dir:?} should have rotated at least once"
        );
        assert!(
            !dir.join("messages.log.3").exists(),
            "{dir:?} must not exceed the configured FileLogMaxGenerations=2 cap"
        );
    }

    let _ = std::fs::remove_dir_all(&acc_dir);
    let _ = std::fs::remove_dir_all(&init_dir);
}
