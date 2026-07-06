//! T039/T040 (US1, feature 009, `NEW-12`, `NEW-13`): `peek_proxy_header` peeked into a fixed
//! 4096-byte buffer (too small for the PROXY v2 spec's 64 KiB maximum) and gave up after a fixed
//! 10-iteration/~50ms budget (far short of the outer 5s `PROXY_HEADER_TIMEOUT`) -- either gap
//! leaves the remaining PROXY-protocol bytes to be misread as the start of the FIX stream.

use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use truefix_core::{Field, Message, decode};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::AcceptorBuilder;

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

fn logon_bytes(sender: &str, target: &str) -> Vec<u8> {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, sender));
    m.header.set(Field::string(56, target));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m.encode()
}

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 5;
    c.check_latency = false;
    c
}

/// A PROXY v2 header exceeding the old 4096-byte peek buffer must still parse correctly, not be
/// truncated in a way that leaves the remaining PROXY-protocol bytes to corrupt FIX framing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_proxy_v2_header_exceeding_4096_bytes_is_not_truncated() {
    use ppp::v2::{Builder, Command, IPv4, Protocol, Type, Version};

    let acc_flag = Arc::new(AtomicBool::new(false));
    let declared_client_ip: IpAddr = "203.0.113.5".parse().unwrap();

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            on: acc_flag.clone(),
        }),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor
        .with_session(acc_cfg())
        .allow_remotes(vec![declared_client_ip])
        .trust_proxy_from(vec!["127.0.0.1".parse().unwrap()])
        .serve();

    let addresses: ppp::v2::Addresses =
        IPv4::new([203, 0, 113, 5], [127, 0, 0, 1], 12345, addr.port()).into();
    // Pad well past the old 4096-byte buffer with NoOp TLVs (each capped at 255 bytes of value).
    let mut builder =
        Builder::with_addresses(Version::Two | Command::Proxy, Protocol::Stream, addresses);
    for _ in 0..20 {
        builder = builder.write_tlv(Type::NoOp, vec![0u8; 250].as_slice()).unwrap();
    }
    let header = builder.build().unwrap();
    assert!(
        header.len() > 4096,
        "test setup: header must exceed the old 4096-byte buffer to be a meaningful regression \
         check (got {} bytes)",
        header.len()
    );

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(&header).await.unwrap();
    stream
        .write_all(&logon_bytes("CLIENT", "SERVER"))
        .await
        .unwrap();

    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    let resp = decode(&buf[..n]).expect(
        "a valid FIX frame -- if the header were truncated past the old 4096-byte cap, the \
         leftover PROXY bytes would corrupt framing here instead",
    );
    assert_eq!(resp.msg_type(), Some("A"));
    assert!(
        wait_until(|| acc_flag.load(Ordering::SeqCst), Duration::from_secs(3)).await,
        "session should have logged on"
    );
}

/// A PROXY header delivered slowly (across multiple segments spanning well over the old
/// ~50ms/10-iteration budget, but comfortably within the 5s outer timeout) must still be captured
/// in full, not abandoned early leaving a partial header to corrupt FIX framing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slowly_arriving_proxy_header_is_still_captured_within_the_outer_timeout() {
    let acc_flag = Arc::new(AtomicBool::new(false));
    let declared_client_ip: IpAddr = "203.0.113.5".parse().unwrap();

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            on: acc_flag.clone(),
        }),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor
        .with_session(acc_cfg())
        .allow_remotes(vec![declared_client_ip])
        .trust_proxy_from(vec!["127.0.0.1".parse().unwrap()])
        .serve();

    let header = format!("PROXY TCP4 203.0.113.5 127.0.0.1 12345 {}\r\n", addr.port());
    let header_bytes = header.as_bytes();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    // Trickle the header one byte at a time, well past the old ~50ms budget (10 iterations *
    // 5ms sleep) but well within the 5s outer PROXY_HEADER_TIMEOUT.
    for &b in header_bytes {
        stream.write_all(&[b]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    stream
        .write_all(&logon_bytes("CLIENT", "SERVER"))
        .await
        .unwrap();

    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(4), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    let resp = decode(&buf[..n]).expect(
        "a valid FIX frame -- if the slow-arriving header were abandoned early, the leftover \
         PROXY bytes would corrupt framing here instead",
    );
    assert_eq!(resp.msg_type(), Some("A"));
    assert!(
        wait_until(|| acc_flag.load(Ordering::SeqCst), Duration::from_secs(3)).await,
        "session should have logged on"
    );
}
