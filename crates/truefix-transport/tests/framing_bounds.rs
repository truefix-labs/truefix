//! T052/T053 (US6, feature 006): bounded frame size (BUG-13/FR-024) and precise malformed-prefix
//! recovery (B14/FR-028).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use truefix_core::Field;
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::Acceptor;

struct FlagApp {
    on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for FlagApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.on.store(true, Ordering::SeqCst);
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

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 5;
    c.check_latency = false;
    c
}

fn logon_bytes() -> Vec<u8> {
    let mut m = truefix_core::Message::new();
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_connection_declaring_an_oversized_body_length_is_closed() {
    let flag = Arc::new(AtomicBool::new(false));
    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg(),
        Arc::new(FlagApp { on: flag.clone() }),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    // Declare a BodyLength far beyond MAX_BODY_LEN (16 MiB), then trickle no further bytes --
    // before the fix, the read loop would keep buffering indefinitely waiting for that much data.
    stream
        .write_all(b"8=FIX.4.4\x019=999999999\x01")
        .await
        .unwrap();

    let mut buf = [0u8; 16];
    let result = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf)).await;
    match result {
        Ok(Ok(0)) => {} // connection closed (EOF) -- expected
        Ok(Ok(n)) => panic!("expected the connection to close, got {n} more bytes"),
        Ok(Err(_)) => {} // connection reset -- also an acceptable "closed" signal
        Err(_) => panic!(
            "connection was not closed within 5s -- an oversized declared BodyLength must not \
             leave the connection open indefinitely"
        ),
    }
    assert!(!flag.load(Ordering::SeqCst), "must never reach a logon");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_legitimate_message_following_a_malformed_prefix_is_preserved() {
    let flag = Arc::new(AtomicBool::new(false));
    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg(),
        Arc::new(FlagApp { on: flag.clone() }),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    // A handful of non-FIX bytes (not starting with "8="), SOH-terminated, followed immediately
    // by a real, well-formed Logon. Before the fix, `classify_buffered` cleared the *entire*
    // buffer on the framing error the garbage bytes trigger, discarding the legitimate Logon
    // right along with them.
    let mut buf = b"XXX\x01".to_vec();
    buf.extend_from_slice(&logon_bytes());
    stream.write_all(&buf).await.unwrap();

    assert!(
        wait_until(|| flag.load(Ordering::SeqCst), Duration::from_secs(5)).await,
        "the legitimate Logon that followed a malformed prefix must still be processed, not \
         discarded along with the prefix"
    );
}
