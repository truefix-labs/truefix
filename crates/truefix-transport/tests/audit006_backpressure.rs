//! Audit 006 backpressure coverage for NEW-149/NEW-150.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use truefix_core::{BusinessReject, Field, Message, decode};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::Acceptor;

struct SlowApp {
    processed: Arc<AtomicU32>,
    logged_on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for SlowApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }

    async fn from_app(&self, _m: &Message, _s: &SessionId) -> Result<(), BusinessReject> {
        tokio::time::sleep(Duration::from_millis(150)).await;
        self.processed.fetch_add(1, Ordering::SeqCst);
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

fn header(msg_type: &str, seq: i64, m: &mut Message) {
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
}

fn logon(seq: i64) -> Message {
    let mut m = Message::new();
    header("A", seq, &mut m);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn order(seq: i64) -> Message {
    let mut m = Message::new();
    header("D", seq, &mut m);
    m.body.set(Field::string(11, &format!("ORD-{seq}")));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::int(54, 1));
    m.body.set(Field::int(38, 100));
    m.body.set(Field::string(40, "2"));
    m
}

fn test_request(seq: i64) -> Message {
    let mut m = Message::new();
    header("1", seq, &mut m);
    m.body.set(Field::string(112, "AUDIT006"));
    m
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_application_staging_still_allows_admin_progress() {
    let mut cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    cfg.heartbeat_interval = 30;
    cfg.check_latency = false;
    cfg.in_chan_capacity = Some(2);

    let processed = Arc::new(AtomicU32::new(0));
    let logged_on = Arc::new(AtomicBool::new(false));
    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        cfg,
        Arc::new(SlowApp {
            processed: processed.clone(),
            logged_on: logged_on.clone(),
        }),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(&logon(1).encode()).await.unwrap();

    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("logon response timeout")
        .expect("read logon response");
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("A"));
    assert!(wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(3)).await);

    for seq in 2..=7 {
        stream.write_all(&order(seq).encode()).await.unwrap();
    }
    stream.write_all(&test_request(8).encode()).await.unwrap();

    let start = std::time::Instant::now();
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("admin response timeout")
        .expect("read admin response");
    let elapsed = start.elapsed();
    let response = decode(&buf[..n]).unwrap();
    assert!(matches!(response.msg_type(), Some("0") | Some("2")));
    assert!(
        elapsed < Duration::from_millis(900),
        "admin response took too long behind bounded app staging: {elapsed:?}"
    );
    assert!(
        wait_until(
            || processed.load(Ordering::SeqCst) >= 6,
            Duration::from_secs(5),
        )
        .await,
        "all staged app messages should eventually process"
    );
}
