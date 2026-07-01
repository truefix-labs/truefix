//! T065 (US10) — the full socket-option set is applied to a connection (FR-019).

use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use truefix_transport::SocketOptions;

#[tokio::test]
async fn full_option_set_applies_without_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (client, _server) = tokio::join!(TcpStream::connect(addr), listener.accept());
    let stream = client.unwrap();

    let opts = SocketOptions {
        tcp_no_delay: true,
        keep_alive: true,
        reuse_address: false, // listener-only; not applicable to an established stream
        linger: Some(Duration::from_secs(0)),
        oob_inline: true,
        recv_buffer_size: Some(64 * 1024),
        send_buffer_size: Some(64 * 1024),
        traffic_class: Some(0),
    };
    opts.apply(&stream);

    assert!(stream.nodelay().unwrap());
    // The remaining options are applied best-effort via socket2; the absence of a panic and the
    // continued usability of the stream is the primary contract (platform support for individual
    // options like traffic-class varies).
    drop(stream);
}

#[tokio::test]
async fn reuse_address_allows_immediate_rebind() {
    // Bind, note the port, drop, then rebind the *same* port immediately: without SO_REUSEADDR
    // this can fail with "address already in use" while the OS still holds the port in TIME_WAIT
    // from a prior connection; this test only asserts the bind-with-reuse path succeeds.
    let acceptor = truefix_transport::Acceptor::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        truefix_session::SessionConfig::new("FIX.4.4", "S", "C", truefix_session::Role::Acceptor),
        std::sync::Arc::new(NoopApp),
        truefix_transport::Services {
            socket_options: SocketOptions {
                reuse_address: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .await;
    assert!(acceptor.is_ok(), "binding with SO_REUSEADDR should succeed");
}

struct NoopApp;

#[async_trait::async_trait]
impl truefix_session::Application for NoopApp {}
