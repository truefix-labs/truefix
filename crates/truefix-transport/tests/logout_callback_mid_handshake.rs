//! T069/T070 (US2, feature 007): a TCP disconnection during the Logon handshake itself (before
//! login completes) still triggers the application's `on_logout` callback (BUG-93/FR-029) —
//! previously `run_connection`'s shutdown tail only ever called `app.on_logout` when the
//! connection had reached full `logged_on` state, so an initiator whose own Logon was sent (or an
//! acceptor whose inbound Logon was received) but never completed the handshake silently skipped
//! the callback, unlike QFJ (`logonReceived || logonSent`) and QFGo.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::net::TcpListener;
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::connect_initiator;

struct FlagApp {
    logged_on: Arc<AtomicBool>,
    logged_out: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl Application for FlagApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
    async fn on_logout(&self, _s: &SessionId) {
        self.logged_out.store(true, Ordering::SeqCst);
    }
}

fn initiator_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
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
async fn on_logout_fires_when_the_connection_drops_before_the_handshake_ever_completes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let logged_on = Arc::new(AtomicBool::new(false));
    let logged_out = Arc::new(AtomicBool::new(false));
    let handle = connect_initiator(
        addr,
        initiator_cfg(),
        Arc::new(FlagApp {
            logged_on: logged_on.clone(),
            logged_out: logged_out.clone(),
        }),
    )
    .await
    .unwrap();

    // Accept the initiator's connection (it will have sent its own Logon immediately -- logon_sent
    // becomes true in the session's `on_connected` handler), but never reply at all: hold the
    // connection open briefly, then close it without the handshake ever completing.
    let (stream, _peer) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("initiator should connect")
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    drop(stream);

    assert!(
        wait_until(|| logged_out.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "on_logout must fire once the connection drops, even though the handshake never \
         completed (logon_sent was true, even though full logged_on state was never reached)"
    );
    assert!(
        !logged_on.load(Ordering::SeqCst),
        "on_logon must never have fired -- the handshake genuinely never completed"
    );

    handle.abort();
}
