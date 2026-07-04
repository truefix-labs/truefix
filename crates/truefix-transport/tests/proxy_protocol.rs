//! T061 (US12) — PROXY protocol (v1/v2) is trusted only from a configured trusted-upstream
//! source (FR-015). Drives the multi-session `AcceptorBuilder`'s allow-list directly, since that's
//! the only place in this crate an IP-based decision is actually made from the resolved address.

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

fn proxy_v1_header(declared_source: IpAddr, dest_port: u16) -> String {
    format!("PROXY TCP4 {declared_source} 127.0.0.1 12345 {dest_port}\r\n")
}

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 5;
    c.check_latency = false;
    c
}

/// A physically-trusted upstream's PROXY header is parsed, and the *declared* client IP — not the
/// physical peer's — is what the allow-list checks (Acceptance Scenario 1, first half).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trusted_upstream_proxy_header_is_parsed_into_the_effective_client_ip() {
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
        .allow_remotes(vec![declared_client_ip]) // only the *declared* IP is allowed, not 127.0.0.1
        .trust_proxy_from(vec!["127.0.0.1".parse().unwrap()]) // physical test client is trusted
        .serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let header = proxy_v1_header(declared_client_ip, addr.port());
    stream.write_all(header.as_bytes()).await.unwrap();
    stream
        .write_all(&logon_bytes("CLIENT", "SERVER"))
        .await
        .unwrap();

    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf))
        .await
        .expect("no timeout")
        .expect("read ok");
    let resp = decode(&buf[..n]).expect("a valid FIX frame (not the raw PROXY header)");
    assert_eq!(resp.msg_type(), Some("A")); // Logon response: the connection was accepted

    assert!(
        wait_until(|| acc_flag.load(Ordering::SeqCst), Duration::from_secs(3)).await,
        "session should have logged on"
    );
}

/// The same header from a physically-*untrusted* source is never parsed/trusted — the physical
/// peer IP is used for the allow-list instead, and since it isn't on the list, the connection is
/// refused (the raw PROXY header bytes are never mistaken for a valid FIX frame either — the
/// connection is simply dropped before either is read) (Acceptance Scenario 1, second half).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn untrusted_source_proxy_header_is_not_trusted() {
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
        .allow_remotes(vec![declared_client_ip]) // the physical peer (127.0.0.1) is NOT on this list
        .trust_proxy_from(vec!["10.0.0.1".parse().unwrap()]) // does not match the real peer
        .serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let header = proxy_v1_header(declared_client_ip, addr.port());
    // The server refuses (and drops) the connection promptly since the effective (physical) peer
    // IP isn't allow-listed — a write racing that close may itself fail (broken pipe), which is
    // just as valid a sign of refusal as a clean EOF on read, so both writes are best-effort here.
    let _ = stream.write_all(header.as_bytes()).await;
    let _ = stream.write_all(&logon_bytes("CLIENT", "SERVER")).await;

    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf))
        .await
        .expect("should not hang — the connection is dropped promptly")
        .unwrap_or(0);
    assert_eq!(n, 0, "the connection should be closed, not answered");
    assert!(!acc_flag.load(Ordering::SeqCst), "session must not log on");
}

// --- T056 (US6, feature 006): PROXY header peek is bounded by a timeout (GAP-54) ---

/// A trusted-source connection that opens and sends *nothing at all* must not hang the connection
/// task indefinitely -- before the fix, `TcpStream::peek` awaiting with no timeout at all meant
/// such a connection would wait forever inside the PROXY-header peek. This test waits past the
/// internal timeout with the connection silent, then sends a plain FIX Logon (no PROXY header)
/// and confirms it still gets through -- proving the peek loop gave up and moved on, rather than
/// remaining permanently stuck waiting for a header that was never coming.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_trusted_source_sending_nothing_at_all_does_not_hang_the_connection_forever() {
    let acc_flag = Arc::new(AtomicBool::new(false));

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
        .trust_proxy_from(vec!["127.0.0.1".parse().unwrap()]) // physical test client is trusted
        .serve();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    // Send nothing at all for longer than the internal PROXY-header-wait timeout, to prove the
    // peek loop actually gives up rather than blocking on TcpStream::peek forever.
    tokio::time::sleep(Duration::from_secs(6)).await;
    stream
        .write_all(&logon_bytes("CLIENT", "SERVER"))
        .await
        .unwrap();

    let mut buf = [0u8; 512];
    let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf))
        .await
        .expect(
            "the connection must have already given up waiting for a PROXY header by now, not \
             still be blocked on it",
        )
        .expect("read ok");
    let resp = decode(&buf[..n]).expect("a valid FIX frame");
    assert_eq!(resp.msg_type(), Some("A"));
    assert!(
        wait_until(|| acc_flag.load(Ordering::SeqCst), Duration::from_secs(3)).await,
        "session should log on once the header-wait times out and falls back to the physical \
         peer address"
    );
}

// --- T057 (US6, feature 006): PROXY v2 header peek buffer sizing (GAP-53/B15) ---

/// A PROXY v2 header whose TLV set pushes its total size past the *old* 256-byte peek buffer
/// (but comfortably within the new one) must still parse correctly, not be truncated in a way
/// that causes the remaining PROXY-protocol bytes to be misread as FIX message data.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_large_proxy_v2_header_near_the_old_256_byte_buffer_is_not_truncated() {
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
    // Pad well past the old 256-byte buffer with NoOp TLVs (each capped at 255 bytes of value).
    let builder =
        Builder::with_addresses(Version::Two | Command::Proxy, Protocol::Stream, addresses)
            .write_tlv(Type::NoOp, vec![0u8; 250].as_slice())
            .unwrap()
            .write_tlv(Type::NoOp, vec![0u8; 250].as_slice())
            .unwrap();
    let header = builder.build().unwrap();
    assert!(
        header.len() > 256,
        "test setup: header must exceed the old 256-byte buffer to be a meaningful regression \
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
        "a valid FIX frame -- if the header were truncated, the leftover PROXY bytes would \
         corrupt framing here instead",
    );
    assert_eq!(resp.msg_type(), Some("A"));
    assert!(
        wait_until(|| acc_flag.load(Ordering::SeqCst), Duration::from_secs(3)).await,
        "session should have logged on"
    );
}
