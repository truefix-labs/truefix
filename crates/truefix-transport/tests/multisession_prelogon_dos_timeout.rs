//! T005/T006 (US1, feature 009, `NEW-93`): a multi-session `AcceptorBuilder` connection's
//! pre-Logon read loop (`route_and_run`) had no timeout and no buffer cap -- a peer that opens a
//! connection and sends nothing, or sends bytes that never include an SOH byte, made the loop
//! `continue` forever: `frame_length` returns `Ok(None)` immediately whenever no SOH is present,
//! never reaching the `MAX_BODY_LEN` check, and nothing here was bounded by `logon_timeout` (that
//! only starts once a `Session` object exists, which happens after this loop already returns a
//! decoded Logon). Classic slowloris-shaped pre-auth DoS: cheap for an attacker, unbounded for the
//! acceptor. Contrast the single-session `Acceptor`, whose `Session`/ticker/`logon_timeout` are
//! live from the instant a connection is accepted -- only the multi-session routing-by-Logon-
//! content path was exposed.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use truefix_session::{Application, Role, SessionConfig};
use truefix_transport::AcceptorBuilder;

struct NoopApp;

#[async_trait::async_trait]
impl Application for NoopApp {}

fn acc(sender: &str, target: &str, logon_timeout: u32) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", sender, target, Role::Acceptor);
    c.heartbeat_interval = 30;
    c.logon_timeout = logon_timeout;
    c
}

/// A silent connection (sends nothing at all) must be disconnected within a bounded time derived
/// from the registered sessions' `logon_timeout` -- not left open indefinitely.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_silent_pre_logon_connection_is_disconnected_within_bounded_time() {
    let acceptor =
        AcceptorBuilder::bind("127.0.0.1:0".parse().unwrap(), std::sync::Arc::new(NoopApp))
            .await
            .unwrap()
            .with_session(acc("SERVER", "CLIENT", 1));
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut raw = TcpStream::connect(addr).await.unwrap();
    // Send nothing at all.
    let mut buf = [0u8; 16];
    let result = tokio::time::timeout(Duration::from_secs(5), raw.read(&mut buf)).await;
    match result {
        Ok(Ok(0)) => {}  // clean EOF -- expected
        Ok(Err(_)) => {} // reset -- also acceptable
        Ok(Ok(n)) => panic!("expected the connection to close, got {n} more bytes"),
        Err(_) => panic!(
            "a silent pre-Logon connection to a multi-session acceptor must be disconnected \
             within a bounded time (derived from logon_timeout), not held open indefinitely -- \
             this is NEW-93's unauthenticated pre-Logon DoS vector"
        ),
    }
}

/// A connection that trickles bytes that never include an SOH (so `frame_length` never leaves
/// `Ok(None)`) must still be bounded by the same timeout, not merely by EOF/error detection.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_connection_trickling_non_soh_bytes_is_disconnected_within_bounded_time() {
    let acceptor =
        AcceptorBuilder::bind("127.0.0.1:0".parse().unwrap(), std::sync::Arc::new(NoopApp))
            .await
            .unwrap()
            .with_session(acc("SERVER", "CLIENT", 1));
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut raw = TcpStream::connect(addr).await.unwrap();
    // Race: keep writing non-SOH garbage while waiting for the connection to close. If the
    // timeout fires as intended, the read side observes EOF/reset well before we'd ever finish
    // writing an unbounded amount of garbage.
    let write_task = tokio::spawn(async move {
        for _ in 0..50u32 {
            if raw.write_all(&[b'X'; 256]).await.is_err() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
    let result = tokio::time::timeout(Duration::from_secs(5), raw_read_until_closed(addr)).await;
    write_task.abort();
    assert!(
        result.is_ok(),
        "a pre-Logon connection trickling non-SOH bytes must still be disconnected within a \
         bounded time, not held open as long as the peer keeps sending garbage -- this is \
         NEW-93's unauthenticated pre-Logon DoS vector"
    );
}

/// Helper: open a fresh connection, send non-SOH garbage, and wait for the acceptor to close it.
async fn raw_read_until_closed(addr: std::net::SocketAddr) {
    let mut raw = TcpStream::connect(addr).await.unwrap();
    raw.write_all(&[b'Y'; 64]).await.unwrap();
    let mut buf = [0u8; 16];
    loop {
        match raw.read(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(_) => continue,
        }
    }
}
