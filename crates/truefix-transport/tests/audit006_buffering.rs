//! NEW-133: `classify_buffered` decodes/logs directly against a `&buf[..total]` slice view and
//! drains the consumed prefix in place, instead of first `collect()`ing it into a fresh `Vec<u8>`
//! per message. This is a regression test for that refactor: several FIX messages concatenated
//! into a single TCP write (so one buffered read must yield multiple framed messages from the
//! same growing/draining `Vec<u8>`) must all still decode correctly, in order, with none
//! corrupted, dropped, or duplicated by the in-place slicing/draining.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use truefix_core::{Field, Message, decode};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::Acceptor;

struct RecordingApp {
    logged_on: Arc<std::sync::atomic::AtomicBool>,
    received: Arc<std::sync::Mutex<Vec<String>>>,
    count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Application for RecordingApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
    async fn from_app(
        &self,
        m: &Message,
        _s: &SessionId,
    ) -> Result<(), truefix_core::BusinessReject> {
        if let Some(clordid) = m.body.get(11).and_then(|f| f.as_str().ok()) {
            self.received.lock().unwrap().push(clordid.to_owned());
        }
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

async fn wait_until<F: Fn() -> bool>(cond: F, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
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

fn new_order_single(seq: i64, clordid: &str) -> Message {
    let mut m = Message::new();
    header("D", seq, &mut m);
    m.body.set(Field::string(11, clordid));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::int(54, 1));
    m.body.set(Field::int(38, 100));
    m.body.set(Field::string(40, "2"));
    m
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audit006_multiple_messages_concatenated_in_one_write_all_decode_correctly() {
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    acc_cfg.check_latency = false;

    let logged_on = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let received = Arc::new(std::sync::Mutex::new(Vec::new()));
    let count = Arc::new(AtomicU32::new(0));

    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
        Arc::new(RecordingApp {
            logged_on: logged_on.clone(),
            received: received.clone(),
            count: count.clone(),
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
        .expect("no timeout")
        .expect("read ok");
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("A"));
    assert!(wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(3)).await);

    // Concatenate 20 NewOrderSingles into a single TCP write, forcing one buffered read to
    // contain many complete frames back-to-back.
    let mut batch = Vec::new();
    let expected: Vec<String> = (0..20).map(|i| format!("ORD-{i}")).collect();
    for (i, clordid) in expected.iter().enumerate() {
        batch.extend_from_slice(&new_order_single(2 + i as i64, clordid).encode());
    }
    stream.write_all(&batch).await.unwrap();

    assert!(
        wait_until(
            || count.load(Ordering::SeqCst) >= 20,
            Duration::from_secs(5)
        )
        .await,
        "expected all 20 batched messages to be processed, got {}",
        count.load(Ordering::SeqCst)
    );
    let got = received.lock().unwrap().clone();
    assert_eq!(
        got, expected,
        "messages must decode in order with none corrupted/dropped"
    );
}
