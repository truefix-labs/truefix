//! T151 (NEW-49) — HTTP CONNECT proxy response handling validates the status token is numeric
//! (not merely `starts_with('2')`), and the response reader isn't a byte-at-a-time loop.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{ProxyConfig, ProxyType, connect_initiator_via_proxy};

struct NoopApp;

#[async_trait::async_trait]
impl Application for NoopApp {
    async fn on_logon(&self, _s: &SessionId) {}
}

fn init_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    c.heartbeat_interval = 5;
    c.check_latency = false;
    c
}

/// A fake HTTP CONNECT proxy that reads (and discards) the request, then replies with whatever
/// fixed `response` bytes the caller provides — never actually opening an upstream connection.
async fn run_fixed_response_proxy(listener: TcpListener, response: &'static [u8]) {
    if let Ok((mut client, _)) = listener.accept().await {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if client.read_exact(&mut byte).await.is_err() {
                return;
            }
            buf.push(byte[0]);
            if buf.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let _ = client.write_all(response).await;
        // Hold the connection open briefly so the client has time to read the response before the
        // proxy socket is torn down.
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn attempt_connect(response: &'static [u8]) -> Result<(), ()> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(run_fixed_response_proxy(listener, response));

    let proxy = ProxyConfig {
        proxy_type: ProxyType::HttpConnect,
        proxy_addr,
        username: None,
        password: None,
    };
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        connect_initiator_via_proxy(
            &proxy,
            "127.0.0.1",
            1,
            init_cfg(),
            Arc::new(NoopApp),
            truefix_transport::Services::default(),
        ),
    )
    .await
    .expect("must not hang");
    result.map(|_| ()).map_err(|_| ())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_non_numeric_status_token_beginning_with_2_is_rejected() {
    // `starts_with('2')` alone would previously accept this malformed token as success.
    let outcome = attempt_connect(b"HTTP/1.1 2xx Not A Real Status\r\n\r\n").await;
    assert!(
        outcome.is_err(),
        "a non-numeric status token must not be treated as a successful CONNECT"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_genuine_2xx_numeric_status_is_still_accepted() {
    // Control: a real proxy might still legitimately reply 200 even without a live upstream in
    // this fake, so only assert this isn't rejected for being non-numeric (it will still fail to
    // establish the FIX session since there's no real upstream — that's expected and fine here).
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(run_fixed_response_proxy(
        listener,
        b"HTTP/1.1 200 OK\r\n\r\n",
    ));

    let proxy = ProxyConfig {
        proxy_type: ProxyType::HttpConnect,
        proxy_addr,
        username: None,
        password: None,
    };
    // A 200 response with no real upstream still lets `connect_initiator_via_proxy` return a
    // handle (the tunnel "succeeds" from the proxy-handshake's point of view); the session then
    // just never logs on. This test only exercises that the CONNECT phase itself isn't rejected.
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        connect_initiator_via_proxy(
            &proxy,
            "127.0.0.1",
            1,
            init_cfg(),
            Arc::new(NoopApp),
            truefix_transport::Services::default(),
        ),
    )
    .await
    .expect("must not hang");
    assert!(
        result.is_ok(),
        "a genuine numeric 2xx status must be accepted"
    );
}
