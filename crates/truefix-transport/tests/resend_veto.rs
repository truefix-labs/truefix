//! T020 (US3, feature 005) — an application-vetoed resend is replaced by a `SequenceReset-GapFill`
//! on the wire (GAP-07/FR-007), over a real connection. Extends `role_parity.rs`'s
//! `initiator_serves_a_resend_request_from_its_own_durable_store` pattern (a hand-rolled peer
//! controlling exactly when the `ResendRequest` is sent) with an `Application::to_app` that vetoes
//! one specific stale order.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use truefix_core::{DoNotSend, Field, Message, decode};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{Monitor, Services, connect_initiator_with};

#[derive(Default)]
struct Flags {
    logged_on: AtomicBool,
}

/// Vetoes only the *resend* (PossDupFlag=Y) of an outbound application message whose ClOrdID (11)
/// is `"STALE"` — the original live send (no PossDupFlag yet) still goes out normally, matching
/// how a real application would only judge an order "too stale to replay" once it's actually being
/// replayed, not when it's first sent.
struct VetoingApp {
    f: Arc<Flags>,
}

#[async_trait::async_trait]
impl Application for VetoingApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.f.logged_on.store(true, Ordering::SeqCst);
    }
    async fn to_app(&self, msg: &mut Message, _s: &SessionId) -> Result<(), DoNotSend> {
        let is_stale = msg.body.get(11).and_then(|f| f.as_str().ok()) == Some("STALE");
        let is_resend = msg.header.get(43).and_then(|f| f.as_str().ok()) == Some("Y");
        if is_stale && is_resend {
            Err(DoNotSend)
        } else {
            Ok(())
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

fn new_order(clordid: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "D"));
    m.body.set(Field::string(11, clordid));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::int(54, 1));
    m.body.set(Field::int(38, 100));
    m.body.set(Field::string(40, "2"));
    m
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_vetoed_resend_is_replaced_by_a_gap_fill_on_the_wire() {
    let monitor = Monitor::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 30;
    init_cfg.check_latency = false;
    init_cfg.reset_on_logon = false;

    let flags = Arc::new(Flags::default());
    let _handle = connect_initiator_with(
        addr,
        init_cfg,
        Arc::new(VetoingApp { f: flags.clone() }),
        Services {
            monitor: Some(monitor.clone()),
            ..Services::default()
        },
    )
    .await
    .unwrap();

    let (mut peer, _) = listener.accept().await.unwrap();

    let mut buf = [0u8; 512];
    let n = peer.read(&mut buf).await.unwrap();
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("A"));

    let mut reply_logon = Message::new();
    reply_logon.header.set(Field::string(8, "FIX.4.4"));
    reply_logon.header.set(Field::string(35, "A"));
    reply_logon.header.set(Field::int(34, 1));
    reply_logon.header.set(Field::string(49, "SERVER"));
    reply_logon.header.set(Field::string(56, "CLIENT"));
    reply_logon
        .header
        .set(Field::string(52, "20240101-00:00:00"));
    reply_logon.body.set(Field::int(98, 0));
    reply_logon.body.set(Field::int(108, 30));
    peer.write_all(&reply_logon.encode()).await.unwrap();

    assert!(
        wait_until(
            || flags.logged_on.load(Ordering::SeqCst),
            Duration::from_secs(3)
        )
        .await
    );

    // Send one order (seq 2) — not yet vetoed (only *resends* of it get vetoed, since
    // `to_app` sees the exact same ClOrdID every time it's replayed). Drain the live send.
    let id = SessionId::new("FIX.4.4", "CLIENT", "SERVER");
    assert!(monitor.send_app(&id, new_order("STALE")).await);
    let n = tokio::time::timeout(Duration::from_secs(3), peer.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    assert_eq!(decode(&buf[..n]).unwrap().msg_type(), Some("D"));

    // Request a resend of exactly that message — this time `to_app` vetoes it (ClOrdID=STALE),
    // so the wire must carry a GapFill instead of the resent order.
    let mut resend_request = Message::new();
    resend_request.header.set(Field::string(8, "FIX.4.4"));
    resend_request.header.set(Field::string(35, "2"));
    resend_request.header.set(Field::int(34, 2));
    resend_request.header.set(Field::string(49, "SERVER"));
    resend_request.header.set(Field::string(56, "CLIENT"));
    resend_request
        .header
        .set(Field::string(52, "20240101-00:00:00"));
    resend_request.body.set(Field::int(7, 2));
    resend_request.body.set(Field::int(16, 2));
    peer.write_all(&resend_request.encode()).await.unwrap();

    let n = tokio::time::timeout(Duration::from_secs(5), peer.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    let reply = decode(&buf[..n]).unwrap();
    assert_eq!(
        reply.msg_type(),
        Some("4"),
        "expected a SequenceReset-GapFill in place of the vetoed order, got {:?}",
        reply.msg_type()
    );
    assert_eq!(reply.header.get(34).and_then(|f| f.as_int().ok()), Some(2));
    assert_eq!(
        reply.body.get(123).and_then(|f| f.as_str().ok()),
        Some("Y"),
        "GapFillFlag"
    );
    assert_eq!(
        reply.body.get(36).and_then(|f| f.as_int().ok()),
        Some(3),
        "NewSeqNo = vetoed seq + 1"
    );
}
