//! T077/T078 (US2, feature 007): `ReconnectHandle::stop()` tears down a currently-active
//! connection, not only future reconnect attempts (BUG-50/FR-043) — previously `stop()` only set
//! an `AtomicBool`, checked at the top of the reconnect loop and after `run_connection` returns;
//! while a connection was actively live, the loop was blocked awaiting it, so `stop()` had no
//! effect until that connection ended on its own. The initiator kept running (and the underlying
//! TCP connection stayed open) indefinitely after `stop()` was called.

use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use truefix_session::{Application, Role, SessionConfig};
use truefix_transport::connect_initiator_reconnecting_multi;
use truefix_transport::Services;

struct NoopApp;
#[async_trait::async_trait]
impl Application for NoopApp {}

fn initiator_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    c.check_latency = false;
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_tears_down_a_currently_active_connection_not_just_future_attempts() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = connect_initiator_reconnecting_multi(
        vec![addr],
        initiator_cfg(),
        std::sync::Arc::new(NoopApp),
        Services::default(),
    );

    // Accept the initiator's connection -- it never replies, so the connection stays open and
    // "active" (the reconnect loop is blocked awaiting it) until something intervenes.
    let (mut stream, _peer) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("initiator should connect")
        .unwrap();

    // Drain the initiator's own outbound Logon bytes first (already buffered by the time we get
    // here) so the read below observes the connection's actual close, not just leftover already-
    // sent handshake data.
    let mut drain_buf = [0u8; 4096];
    let _ = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut drain_buf)).await;

    handle.stop();

    // Before the fix: stop() only set a flag the blocked reconnect loop never got to check again
    // until this connection ended on its own -- which, since neither side ever initiates a
    // graceful close here, would be never. After the fix: stop() also aborts the currently-active
    // connection outright, so the peer observes the socket close promptly.
    let mut buf = [0u8; 16];
    let result = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf)).await;
    match result {
        Ok(Ok(0)) => {}  // clean EOF -- expected
        Ok(Err(_)) => {} // connection reset -- also an acceptable "closed" signal
        Ok(Ok(n)) => panic!("expected the connection to close, got {n} more bytes"),
        Err(_) => panic!(
            "stop() must tear down a currently-active connection promptly, not leave it running \
             until it happens to end on its own"
        ),
    }
}
