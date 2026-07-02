//! T072 (US14) — a saturated bounded application channel (`in_chan_capacity`) blocks the reader
//! without dropping messages, while concurrent administrative traffic (a heartbeat reply) is
//! still processed promptly (FR-019, Acceptance Scenario 1; "must not starve admin" Clarification).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use truefix_core::{decode, BusinessReject, Field, Message};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::Acceptor;

/// An `Application` whose `from_app` deliberately takes a while — simulating a slow downstream
/// consumer (e.g. persisting to a database, running risk checks) — so a burst of application
/// messages backs up the bounded channel long enough to observe backpressure.
struct SlowApp {
    processed: Arc<AtomicU32>,
    logged_on: Arc<std::sync::atomic::AtomicBool>,
}

#[async_trait::async_trait]
impl Application for SlowApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
    async fn from_app(&self, _m: &Message, _s: &SessionId) -> Result<(), BusinessReject> {
        tokio::time::sleep(Duration::from_millis(250)).await;
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

fn test_request(seq: i64, id: &str) -> Message {
    let mut m = Message::new();
    header("1", seq, &mut m);
    m.body.set(Field::string(112, id));
    m
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saturated_application_channel_blocks_without_dropping_and_admin_traffic_is_unaffected() {
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    acc_cfg.check_latency = false;
    acc_cfg.in_chan_capacity = Some(2); // small: a handful of NewOrderSingles saturates it fast

    let processed = Arc::new(AtomicU32::new(0));
    let logged_on = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
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
        .expect("no timeout")
        .expect("read ok");
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("A"));
    assert!(wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(3)).await);

    // Flood 8 application messages (well past the bound of 2 — each takes 250ms to process), then
    // immediately send a TestRequest (administrative). If admin traffic were stuck behind the
    // saturated application queue, the Heartbeat reply would take seconds (8 * 250ms); if it's
    // correctly prioritized, it arrives promptly regardless of the backlog.
    for i in 0..8 {
        let seq = 2 + i;
        stream
            .write_all(&new_order_single(seq, &format!("ORD-{i}")).encode())
            .await
            .unwrap();
    }
    stream
        .write_all(&test_request(10, "PING").encode())
        .await
        .unwrap();

    let start = std::time::Instant::now();
    let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    let elapsed = start.elapsed();
    let resp = decode(&buf[..n]).unwrap();
    // The TestRequest is admin traffic, so it's read/classified/dispatched promptly regardless of
    // the saturated application queue (that's what's under test here) — but *which* response it
    // draws depends on session sequencing state: since admin messages are drained with priority
    // (a deliberate, documented trade-off — see `AppSender`'s doc), a TestRequest that arrives
    // ahead of still-queued lower-sequence application messages looks like a sequence gap to the
    // session, drawing a ResendRequest (35=2) rather than a plain Heartbeat (35=0) reply. Either
    // is valid proof the reader wasn't stuck: a genuinely blocked reader wouldn't produce *any*
    // response this quickly, since it couldn't have read the TestRequest's bytes off the wire at
    // all while still blocked mid-send on an earlier application message.
    assert!(
        matches!(resp.msg_type(), Some("0") | Some("2")),
        "expected a Heartbeat or ResendRequest reply to the TestRequest, got {:?}",
        resp.msg_type()
    );
    assert!(
        elapsed < Duration::from_millis(900),
        "admin traffic should not be starved behind the saturated application channel \
         (reply took {elapsed:?}, which is close to or exceeds the full backlog's own \
         8*250ms=2s processing time)"
    );

    // No message is dropped — all 8 eventually get processed once the backlog drains.
    assert!(
        wait_until(
            || processed.load(Ordering::SeqCst) >= 8,
            Duration::from_secs(5),
        )
        .await,
        "all 8 application messages should eventually be processed, got {}",
        processed.load(Ordering::SeqCst)
    );
}

/// `in_chan_capacity: None` (default) is unaffected — behavior identical to no backpressure at
/// all (Acceptance Scenario 2).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unbounded_default_is_unaffected() {
    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    acc_cfg.check_latency = false;
    assert_eq!(acc_cfg.in_chan_capacity, None);

    let processed = Arc::new(AtomicU32::new(0));
    let logged_on = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg,
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
        .expect("no timeout")
        .expect("read ok");
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("A"));
    assert!(wait_until(|| logged_on.load(Ordering::SeqCst), Duration::from_secs(3)).await);

    stream
        .write_all(&new_order_single(2, "ORD-ONLY").encode())
        .await
        .unwrap();
    assert!(
        wait_until(
            || processed.load(Ordering::SeqCst) >= 1,
            Duration::from_secs(3),
        )
        .await
    );
}
