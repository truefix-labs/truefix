//! T113 — complete PROXY headers without an IP address are consumed before FIX framing starts.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use ppp::v2::{Addresses, Builder, Command, Protocol, Unix, Version};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use truefix_core::{Field, Message, decode};
use truefix_session::{Application, Role, SessionConfig};
use truefix_transport::AcceptorBuilder;

struct NoopApp;

#[async_trait::async_trait]
impl Application for NoopApp {}

fn logon_bytes() -> Vec<u8> {
    let mut message = Message::new();
    message.header.set(Field::string(8, "FIX.4.4"));
    message.header.set(Field::string(35, "A"));
    message.header.set(Field::int(34, 1));
    message.header.set(Field::string(49, "CLIENT"));
    message.header.set(Field::string(56, "SERVER"));
    message.header.set(Field::string(52, "20240101-00:00:00"));
    message.body.set(Field::int(98, 0));
    message.body.set(Field::int(108, 30));
    message.encode()
}

async fn assert_header_is_consumed(header: &[u8]) {
    let mut config = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    config.check_latency = false;

    let acceptor = AcceptorBuilder::bind("127.0.0.1:0".parse().unwrap(), Arc::new(NoopApp))
        .await
        .unwrap();
    let address = acceptor.local_addr().unwrap();
    acceptor
        .with_session(config)
        .trust_proxy_from(vec!["127.0.0.1".parse::<IpAddr>().unwrap()])
        .serve();

    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(header).await.unwrap();
    stream.write_all(&logon_bytes()).await.unwrap();

    let mut response = [0u8; 512];
    let read = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut response))
        .await
        .expect("a complete PROXY header must not wait for the 5-second partial-header timeout")
        .expect("read Logon response");
    let message = decode(&response[..read])
        .expect("PROXY bytes must be consumed rather than passed into FIX framing");
    assert_eq!(message.msg_type(), Some("A"));
}

#[tokio::test]
async fn proxy_v1_unknown_header_is_consumed() {
    assert_header_is_consumed(b"PROXY UNKNOWN\r\n").await;
}

#[tokio::test]
async fn proxy_v2_unspecified_header_is_consumed() {
    let header = Builder::with_addresses(
        Version::Two | Command::Proxy,
        Protocol::Unspecified,
        Addresses::Unspecified,
    )
    .build()
    .unwrap();

    assert_header_is_consumed(&header).await;
}

#[tokio::test]
async fn proxy_v2_unix_header_is_consumed() {
    let addresses = Unix::new([b'a'; 108], [b'b'; 108]);
    let header =
        Builder::with_addresses(Version::Two | Command::Proxy, Protocol::Stream, addresses)
            .build()
            .unwrap();

    assert_header_is_consumed(&header).await;
}
