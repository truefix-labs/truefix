//! T079/T080 (US2, feature 007): an acceptor refuses a connection whose first inbound message is
//! not a Logon (BUG-52/FR-044) -- QuickFIX/J and QuickFIX/Go both close the socket immediately if
//! the first message received isn't a Logon, rather than attempting to route it. Previously
//! `route_and_run` accepted *any* decodable first message, extracted routing fields from it (even
//! though only a Logon carries the right semantics), and tried to resolve a session -- a
//! non-Logon first message would either silently fail routing or (worse) be misinterpreted.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use truefix_core::message::Message;
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::AcceptorBuilder;

struct CountApp {
    logons: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Application for CountApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logons.fetch_add(1, Ordering::SeqCst);
    }
}

fn acc(sender: &str, target: &str) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", sender, target, Role::Acceptor);
    c.heartbeat_interval = 30;
    c
}

/// Build the raw wire bytes of a standalone Heartbeat (MsgType "0") -- a validly-framed,
/// validly-checksummed message that simply isn't a Logon.
fn raw_heartbeat_bytes(sender: &str, target: &str) -> Vec<u8> {
    let mut msg = Message::new();
    msg.header
        .set(truefix_core::field::Field::new(8, "FIX.4.4"));
    msg.header.set(truefix_core::field::Field::new(35, "0"));
    msg.header.set(truefix_core::field::Field::new(49, sender));
    msg.header.set(truefix_core::field::Field::new(56, target));
    msg.header.set(truefix_core::field::Field::new(34, "1"));
    msg.header
        .set(truefix_core::field::Field::new(52, "20260101-00:00:00"));
    msg.encode()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_connection_whose_first_message_is_not_a_logon_is_refused() {
    let acc_logons = Arc::new(AtomicUsize::new(0));
    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(CountApp {
            logons: acc_logons.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc("SERVER", "CLIENT"));
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut raw = TcpStream::connect(addr).await.unwrap();
    let heartbeat = raw_heartbeat_bytes("CLIENT", "SERVER");
    raw.write_all(&heartbeat).await.unwrap();
    raw.flush().await.unwrap();

    // Before the fix: `route_and_run` would decode this Heartbeat, extract (bogus) routing
    // fields from it, and either wedge trying to resolve a session or otherwise mishandle it --
    // in no case is the socket promptly closed. After the fix: the acceptor recognizes the first
    // message isn't a Logon and closes the connection outright.
    let mut buf = [0u8; 16];
    let result = tokio::time::timeout(Duration::from_secs(5), raw.read(&mut buf)).await;
    match result {
        Ok(Ok(0)) => {}  // clean EOF -- expected
        Ok(Err(_)) => {} // connection reset -- also an acceptable "closed" signal
        Ok(Ok(n)) => panic!("expected the connection to close, got {n} more bytes"),
        Err(_) => panic!(
            "an acceptor must refuse a connection whose first message isn't a Logon, not leave \
             it open"
        ),
    }
    assert_eq!(
        acc_logons.load(Ordering::SeqCst),
        0,
        "a non-Logon first message must never be treated as a completed logon"
    );
}
