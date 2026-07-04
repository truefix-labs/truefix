//! T081/T082 (US2, feature 007): the transport's internal admin-message channel applies bounded
//! backpressure rather than growing without limit (BUG-53/FR-045) -- previously `read_loop`'s
//! admin channel was `mpsc::unbounded_channel()`, so a peer that keeps sending admin-classified
//! traffic (heartbeats, TestRequests, garbled frames) faster than the processing loop can drain it
//! could grow this connection's memory usage without bound, a DoS vector. With a bounded channel,
//! once it fills, the reader task's own send blocks, which in turn stops it from draining the
//! socket -- backpressure genuinely reaches the TCP connection instead of buffering forever in
//! process memory.
//!
//! Mirrors `sync_writes.rs`'s outbound-backpressure test (a stalled peer's *write* blocking past a
//! socket-buffer limit), but for the inbound direction: here it's *our own* processing loop that's
//! deliberately stalled (an `Application::from_admin` that never returns), and what's under test is
//! whether the *test client's* writes eventually block -- proof that this side stopped reading.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use truefix_core::{Field, Message, Reject};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{Acceptor, Services, SocketOptions};

/// An `Application` that logs on normally, but then blocks forever (never returns) on every
/// subsequent inbound admin message -- simulating a processing loop that's stuck (e.g. behind a
/// slow integration hook), so the admin channel behind it has no chance to drain.
struct WedgedApp {
    logged_on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for WedgedApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
    async fn from_admin(&self, message: &Message, _s: &SessionId) -> Result<(), Reject> {
        if message.msg_type() == Some("A") {
            return Ok(()); // let the Logon itself through normally
        }
        std::future::pending::<()>().await; // wedge forever on anything after the Logon
        Ok(())
    }
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

fn logon_bytes() -> Vec<u8> {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m.encode()
}

/// A Heartbeat, padded with an oversized Text field so a handful of these overflow the (small,
/// test-configured) socket receive buffer in far fewer iterations -- same trick as
/// `sync_writes.rs`'s `order()` helper, applied to an admin (rather than application) message.
fn padded_heartbeat(seq: i64) -> Vec<u8> {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(58, &"X".repeat(4000)));
    m.encode()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_flood_of_admin_traffic_against_a_wedged_processing_loop_eventually_blocks_the_writer() {
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    acc_cfg.check_latency = false;

    let logged_on = Arc::new(AtomicBool::new(false));

    let services = Services {
        socket_options: SocketOptions {
            recv_buffer_size: Some(2048), // small, so the backlog fills quickly
            ..SocketOptions::default()
        },
        ..Services::default()
    };

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(WedgedApp {
            logged_on: logged_on.clone(),
        }),
        services,
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    // Shrink the *receiver's* own window too (from the client's perspective, the acceptor's
    // recv_buffer_size above already does this on its side; this mirrors sync_writes.rs's
    // belt-and-suspenders approach for reliability across platforms/loopback tuning).
    let _ = socket2::SockRef::from(&stream).set_send_buffer_size(2048);

    stream.write_all(&logon_bytes()).await.unwrap();

    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    assert!(n > 0);
    assert!(
        wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(3)).await,
        "should log on before the admin channel is flooded"
    );

    // Flood padded Heartbeats (admin traffic). The acceptor's `from_admin` is wedged forever on
    // the first one it dequeues, so nothing ever drains the (now-bounded) admin channel. Before
    // the fix (unbounded channel): the reader keeps consuming bytes off the wire and queueing them
    // in memory indefinitely, so every write below completes promptly, no matter how many
    // iterations. After the fix: once the channel and this connection's slice of the (small)
    // socket receive buffer fill up, the reader stops reading, and a subsequent write blocks past
    // a short timeout.
    let mut saw_a_blocked_write = false;
    for i in 0..500 {
        let write = tokio::time::timeout(
            Duration::from_millis(300),
            stream.write_all(&padded_heartbeat(2 + i)),
        )
        .await;
        if write.is_err() {
            saw_a_blocked_write = true;
            break;
        }
    }

    assert!(
        saw_a_blocked_write,
        "expected a bounded admin channel to eventually apply backpressure all the way to the \
         TCP connection (a blocked write), but 500 padded Heartbeats (~4KB each) all wrote \
         through instantly -- the admin channel appears to still be unbounded"
    );
}
