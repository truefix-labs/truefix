//! T064 (US12) — `SocketSynchronousWrites` with a configured timeout surfaces a typed timeout
//! (logged distinctly, connection torn down) on a write that would otherwise block indefinitely
//! (FR-017, Acceptance Scenario 5).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use truefix_core::{Field, Message};
use truefix_log::Log;
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{Acceptor, Monitor, Services};

struct FlagApp {
    on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for FlagApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.on.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct CaptureLog {
    events: Mutex<Vec<String>>,
}

impl Log for CaptureLog {
    fn on_incoming(&self, _message: &str) {}
    fn on_outgoing(&self, _message: &str) {}
    fn on_event(&self, text: &str) {
        if let Ok(mut events) = self.events.lock() {
            events.push(text.to_owned());
        }
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

fn order(clordid: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "D"));
    m.body.set(Field::string(11, clordid));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::int(54, 1));
    m.body.set(Field::int(38, 100));
    m.body.set(Field::string(40, "2"));
    m.body.set(Field::string(44, "150.25"));
    // A large Text field pads each message so the backlog fills the (deliberately tiny) socket
    // buffers in far fewer iterations.
    m.body.set(Field::string(58, &"X".repeat(4000)));
    m
}

/// A write exceeding `SocketSynchronousWriteTimeout` tears the connection down and logs a
/// distinct timeout event, rather than blocking indefinitely: a raw client completes Logon, then
/// stops reading entirely (simulating a stalled peer) while the acceptor keeps sending — with a
/// small `send_buffer_size` and a short write timeout, the outbound backlog exceeds the socket
/// buffer and a subsequent write blocks past the timeout.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stalled_peer_times_out_a_synchronous_write() {
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    acc_cfg.check_latency = false;

    let acc_flag = Arc::new(AtomicBool::new(false));
    let log = Arc::new(CaptureLog::default());
    let monitor = Monitor::new();

    let socket_options = truefix_transport::SocketOptions {
        send_buffer_size: Some(2048), // small, so the backlog fills quickly
        ..truefix_transport::SocketOptions::default()
    };

    let services = Services {
        monitor: Some(monitor.clone()),
        log: Some(log.clone() as Arc<dyn Log>),
        socket_options,
        sync_write_timeout: Some(Duration::from_millis(150)),
        ..Services::default()
    };

    let acceptor = Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(FlagApp {
            on: acc_flag.clone(),
        }),
        services,
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    // Shrink the *receiver's* own window too — the sender's SO_SNDBUF alone often isn't enough to
    // force a block on loopback, since the kernel keeps accepting bytes into a comfortably large
    // default receive buffer regardless of whether the application ever calls read().
    let _ = socket2::SockRef::from(&stream).set_recv_buffer_size(2048);
    tokio::io::AsyncWriteExt::write_all(&mut stream, &logon_bytes())
        .await
        .unwrap();

    // Read exactly the Logon response, then stop reading — simulating a stalled peer.
    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    assert!(n > 0);
    assert!(
        wait_until(|| acc_flag.load(Ordering::SeqCst), Duration::from_secs(3)).await,
        "session should have logged on"
    );

    let id = monitor.sessions().first().cloned().expect("one session");

    // Flood outbound application messages without the peer ever reading again, until the socket
    // buffer backs up and a write blocks past the configured timeout.
    for i in 0..500 {
        monitor.send_app(&id, order(&format!("ORD-{i}"))).await;
        if !monitor.is_connected(&id) {
            break;
        }
    }

    assert!(
        wait_until(|| !monitor.is_connected(&id), Duration::from_secs(10)).await,
        "the connection should be torn down after the write times out"
    );
    let events = log.events.lock().unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.contains("synchronous write timed out")),
        "expected a distinct timeout event, got: {events:?}"
    );

    // Keep the stalled client's stream alive for the duration of the test (never read again).
    let _ = stream;
}
