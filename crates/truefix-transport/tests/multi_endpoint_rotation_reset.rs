//! T075/T076 (US2, feature 007): the multi-endpoint reconnecting initiator prefers the primary
//! endpoint again after any successful connection, not continuing to rotate forward (BUG-51/
//! FR-042) — QFJ resets `nextSocketAddressIndex = 0` on a successful connect; TrueFix's
//! `next_endpoint` was only ever incremented, never reset, so after a connection to a
//! non-primary endpoint dropped, the next reconnect attempt continued rotating forward instead of
//! preferring the primary.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::net::TcpListener;
use truefix_session::{Application, Role, SessionConfig};
use truefix_transport::{Services, connect_initiator_reconnecting_multi};

struct NoopApp;
#[async_trait::async_trait]
impl Application for NoopApp {}

fn initiator_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    c.check_latency = false;
    c.reconnect_interval = 1; // fast retries -- this test waits through a few connect cycles
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
async fn a_dropped_non_primary_connection_reconnects_via_the_primary_again_not_the_next_endpoint() {
    // Endpoint A (primary): a port that's freed immediately, so connecting to it always fails
    // fast (connection refused) -- A is never actually reachable in this test.
    let raw_a = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr_a = raw_a.local_addr().unwrap();
    drop(raw_a);

    // Endpoint B: a real, reachable listener -- the initiator should land here on its second
    // attempt (A refused, B accepted).
    let listener_b = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr_b = listener_b.local_addr().unwrap();

    // Endpoint C: must NEVER be contacted if the fix works -- after B's connection drops, the
    // *correct* next attempt targets A again (refused) then B again, never rotating forward to C.
    let listener_c = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr_c = listener_c.local_addr().unwrap();
    let c_accept_count = Arc::new(AtomicUsize::new(0));
    {
        let count = c_accept_count.clone();
        tokio::spawn(async move {
            loop {
                if listener_c.accept().await.is_ok() {
                    count.fetch_add(1, Ordering::SeqCst);
                } else {
                    break;
                }
            }
        });
    }

    let handle = connect_initiator_reconnecting_multi(
        vec![addr_a, addr_b, addr_c],
        initiator_cfg(),
        Arc::new(NoopApp),
        Services::default(),
    );

    // First connection: must land on B (A refused first).
    let (stream1, _) = tokio::time::timeout(Duration::from_secs(5), listener_b.accept())
        .await
        .expect("initiator should reach B on its second attempt")
        .unwrap();
    // Force a drop from B's side, without ever completing the FIX handshake.
    drop(stream1);

    // After B drops, the initiator must retry A (refused) then land on B again -- never C.
    let (stream2, _) = tokio::time::timeout(Duration::from_secs(5), listener_b.accept())
        .await
        .expect(
            "after B drops, the initiator must reconnect via the primary (A, refused) and \
                 then B again, not rotate forward to C",
        )
        .unwrap();
    drop(stream2);

    assert!(
        wait_until(
            || c_accept_count.load(Ordering::SeqCst) == 0,
            Duration::from_millis(300)
        )
        .await,
        "endpoint C must never have been contacted"
    );

    handle.stop();
    handle.join().await;
}
