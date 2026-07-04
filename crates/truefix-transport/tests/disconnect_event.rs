//! T029/T030/T031 (US1, feature 007): `ResetOnDisconnect` is honored when the TCP connection drops
//! unexpectedly (not a graceful Logout) (BUG-33/FR-009) — previously `run_connection`'s shutdown
//! tail never dispatched any event to the `Session` state machine on a raw drop, so
//! `enter_disconnected()` (which actually checks `reset_on_disconnect`/`reset_on_logout`) was
//! unreachable from that path and the configured reset was silently skipped.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use truefix_session::{Application, Event, Role, Session, SessionConfig, SessionId};
use truefix_store::{FileStore, MessageStore};
use truefix_transport::{Acceptor, Services};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "truefix-tx-disc-{}-{nanos}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Default)]
struct Flags {
    logged_on: AtomicBool,
}

struct App {
    f: Arc<Flags>,
}

#[async_trait::async_trait]
impl Application for App {
    async fn on_logon(&self, _s: &SessionId) {
        self.f.logged_on.store(true, Ordering::SeqCst);
    }
}

/// Build the raw wire bytes of a fresh initiator's first Logon, without going through
/// `truefix-transport` at all -- this lets the test hold the *only* handle to the raw
/// `TcpStream` (no split, no background reader task), so dropping it is a genuine, complete TCP
/// close, unlike aborting a managed `SessionHandle` (whose split read-half lives in a separately
/// spawned task and would keep the socket half-open).
fn raw_logon_bytes(init_cfg: SessionConfig) -> Vec<u8> {
    let mut session = Session::new(init_cfg);
    let actions = session.handle(Event::Connected);
    for action in actions {
        if let truefix_session::Action::Send(msg) = action {
            return msg.encode();
        }
    }
    panic!("Event::Connected on a fresh initiator session must produce a Logon Send action");
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

/// A force-dropped (aborted, not gracefully logged-out) TCP connection must still honor
/// `ResetOnDisconnect` on the surviving peer, exactly as a graceful Logout-driven disconnect would.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_on_disconnect_is_honored_when_the_tcp_connection_is_force_dropped() {
    let acc_dir = unique_dir();
    let init_dir = unique_dir();

    let acc_store = Arc::new(FileStore::open(&acc_dir).unwrap());

    let acc_services = Services {
        store: Some(acc_store.clone() as Arc<dyn MessageStore>),
        ..Services::default()
    };

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 5;
    acc_cfg.reset_on_logon = false;
    acc_cfg.reset_on_disconnect = true; // the behavior under test -- NOT reset_on_logout
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 5;
    init_cfg.reset_on_logon = false;

    let acc_flags = Arc::new(Flags::default());

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(App {
            f: acc_flags.clone(),
        }),
        acc_services,
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    // Connect and log on with a raw, unmanaged `TcpStream` -- see `raw_logon_bytes`'s doc for why
    // this (rather than `connect_initiator_with`) is required to produce a genuine full TCP close.
    let logon_bytes = raw_logon_bytes(init_cfg);
    let mut raw = TcpStream::connect(addr).await.unwrap();
    raw.write_all(&logon_bytes).await.unwrap();
    raw.flush().await.unwrap();

    assert!(
        wait_until(
            || acc_flags.logged_on.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "the acceptor should log on upon receiving the raw client's initial Logon"
    );

    // The Logon itself is persisted to the acceptor's store under seq 1, so there is something to
    // clear once the reset fires.
    tokio::time::sleep(Duration::from_millis(300)).await;
    {
        let out = acc_store.next_sender_seq().await.unwrap();
        assert!(out > 1, "the acceptor should have persisted its Logon");
    }

    // Force-drop: close the raw TCP connection outright (no Logout exchanged), simulating a real
    // crash/network drop from the acceptor's point of view. This test owns the *only* handle to
    // `raw` (never split, no background reader task holding a second reference), so this is a
    // genuine, complete close -- both directions.
    drop(raw);

    // Give the acceptor's connection task a moment to observe the closed socket and run its
    // shutdown tail.
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let reopened = FileStore::open(&acc_dir).unwrap();
    let next_out = reopened.next_sender_seq().await.unwrap();
    assert_eq!(
        next_out, 1,
        "ResetOnDisconnect should reset the acceptor's persisted sequence numbers to 1 even when \
         the TCP connection was force-dropped rather than gracefully logged out"
    );
    let stored = reopened.get(1, 100).await.unwrap();
    assert!(
        stored.is_empty(),
        "ResetOnDisconnect should clear previously-persisted message bodies from the store on a \
         raw drop, got {} entries",
        stored.len()
    );

    let _ = std::fs::remove_dir_all(&acc_dir);
    let _ = std::fs::remove_dir_all(&init_dir);
}
