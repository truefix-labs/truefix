//! T062 (US12) — an initiator connects through a configured forward proxy (SOCKS4, SOCKS5 with
//! optional username/password auth, HTTP CONNECT; FR-016). Each test hand-rolls a minimal local
//! proxy server that speaks just enough of the real wire protocol to complete the handshake, then
//! relays bytes to/from a real `Acceptor` FIX session — proving both the handshake succeeds *and*
//! the tunneled stream carries the actual FIX traffic end-to-end.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{Acceptor, ProxyConfig, ProxyType, connect_initiator_via_proxy};

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

async fn relay(mut a: TcpStream, mut b: TcpStream) {
    let _ = tokio::io::copy_bidirectional(&mut a, &mut b).await;
}

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 5;
    c.check_latency = false;
    c
}

fn init_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    c.heartbeat_interval = 5;
    c.check_latency = false;
    c
}

/// Start a real FIX acceptor to be the ultimate target of every proxied connection in this file.
async fn start_acceptor() -> (SocketAddr, Arc<AtomicBool>) {
    let flag = Arc::new(AtomicBool::new(false));
    let acceptor = Acceptor::bind(
        "127.0.0.1:0".parse().unwrap(),
        acc_cfg(),
        Arc::new(FlagApp { on: flag.clone() }),
    )
    .await
    .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();
    (addr, flag)
}

/// A minimal SOCKS4 proxy: reads `VER(1)=4 CMD(1)=1 DSTPORT(2) DSTIP(4) USERID(null-terminated)`,
/// replies `VER(1)=0 CD(1)=0x5A DSTPORT(2) DSTIP(4)`, then relays to the requested target.
async fn run_socks4_proxy(listener: TcpListener) {
    if let Ok((mut client, _)) = listener.accept().await {
        let mut header = [0u8; 8];
        if client.read_exact(&mut header).await.is_err() {
            return;
        }
        let port = u16::from_be_bytes([header[2], header[3]]);
        let ip = std::net::Ipv4Addr::new(header[4], header[5], header[6], header[7]);
        // Drain the null-terminated USERID.
        let mut byte = [0u8; 1];
        loop {
            if client.read_exact(&mut byte).await.is_err() || byte[0] == 0 {
                break;
            }
        }
        let target = SocketAddr::from((ip, port));
        let Ok(upstream) = TcpStream::connect(target).await else {
            return;
        };
        let reply = [
            0u8, 0x5A, header[2], header[3], header[4], header[5], header[6], header[7],
        ];
        if client.write_all(&reply).await.is_err() {
            return;
        }
        relay(client, upstream).await;
    }
}

/// A minimal SOCKS5 proxy (no-auth or username/password), CONNECT command, IPv4 addressing only.
async fn run_socks5_proxy(
    listener: TcpListener,
    require_auth: Option<(&'static str, &'static str)>,
) {
    if let Ok((mut client, _)) = listener.accept().await {
        let mut greeting = [0u8; 2];
        if client.read_exact(&mut greeting).await.is_err() {
            return;
        }
        let nmethods = greeting[1] as usize;
        let mut methods = vec![0u8; nmethods];
        if client.read_exact(&mut methods).await.is_err() {
            return;
        }
        let chosen: u8 = if require_auth.is_some() { 0x02 } else { 0x00 };
        if client.write_all(&[0x05, chosen]).await.is_err() {
            return;
        }
        if let Some((user, pass)) = require_auth {
            let mut auth_header = [0u8; 2];
            if client.read_exact(&mut auth_header).await.is_err() {
                return;
            }
            let mut uname = vec![0u8; auth_header[1] as usize];
            if client.read_exact(&mut uname).await.is_err() {
                return;
            }
            let mut plen_buf = [0u8; 1];
            if client.read_exact(&mut plen_buf).await.is_err() {
                return;
            }
            let mut pass_buf = vec![0u8; plen_buf[0] as usize];
            if client.read_exact(&mut pass_buf).await.is_err() {
                return;
            }
            let ok = uname == user.as_bytes() && pass_buf == pass.as_bytes();
            let _ = client.write_all(&[0x01, u8::from(!ok)]).await;
            if !ok {
                return;
            }
        }
        let mut req = [0u8; 4];
        if client.read_exact(&mut req).await.is_err() {
            return;
        }
        // ATYP == 1 (IPv4) is all this test proxy supports.
        let mut addr_buf = [0u8; 4];
        if client.read_exact(&mut addr_buf).await.is_err() {
            return;
        }
        let mut port_buf = [0u8; 2];
        if client.read_exact(&mut port_buf).await.is_err() {
            return;
        }
        let target = SocketAddr::from((
            std::net::Ipv4Addr::from(addr_buf),
            u16::from_be_bytes(port_buf),
        ));
        let Ok(upstream) = TcpStream::connect(target).await else {
            return;
        };
        let reply = [0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
        if client.write_all(&reply).await.is_err() {
            return;
        }
        relay(client, upstream).await;
    }
}

/// A minimal HTTP CONNECT proxy: reads the request line + headers up to the blank line, replies
/// `200`, then relays.
async fn run_http_connect_proxy(listener: TcpListener) {
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
        let request = String::from_utf8_lossy(&buf);
        let first_line = request.lines().next().unwrap_or("");
        let target_str = first_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .to_owned();
        let Ok(target) = target_str.parse::<SocketAddr>() else {
            let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
            return;
        };
        let Ok(upstream) = TcpStream::connect(target).await else {
            let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
            return;
        };
        if client
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await
            .is_err()
        {
            return;
        }
        relay(client, upstream).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_connects_through_a_socks4_proxy() {
    let (target, acc_flag) = start_acceptor().await;
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    tokio::spawn(run_socks4_proxy(proxy_listener));

    let init_flag = Arc::new(AtomicBool::new(false));
    let proxy = ProxyConfig {
        proxy_type: ProxyType::Socks4,
        proxy_addr,
        username: None,
        password: None,
    };
    let _handle = connect_initiator_via_proxy(
        &proxy,
        &target.ip().to_string(),
        target.port(),
        init_cfg(),
        Arc::new(FlagApp {
            on: init_flag.clone(),
        }),
        truefix_transport::Services::default(),
    )
    .await
    .expect("SOCKS4-proxied connect");

    assert!(
        wait_until(
            || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on through the SOCKS4 proxy"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_connects_through_a_socks5_proxy_without_auth() {
    let (target, acc_flag) = start_acceptor().await;
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    tokio::spawn(run_socks5_proxy(proxy_listener, None));

    let init_flag = Arc::new(AtomicBool::new(false));
    let proxy = ProxyConfig {
        proxy_type: ProxyType::Socks5,
        proxy_addr,
        username: None,
        password: None,
    };
    let _handle = connect_initiator_via_proxy(
        &proxy,
        &target.ip().to_string(),
        target.port(),
        init_cfg(),
        Arc::new(FlagApp {
            on: init_flag.clone(),
        }),
        truefix_transport::Services::default(),
    )
    .await
    .expect("SOCKS5-proxied connect");

    assert!(
        wait_until(
            || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on through the SOCKS5 proxy"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_connects_through_a_socks5_proxy_with_password_auth() {
    let (target, acc_flag) = start_acceptor().await;
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    tokio::spawn(run_socks5_proxy(proxy_listener, Some(("alice", "s3cret"))));

    let init_flag = Arc::new(AtomicBool::new(false));
    let proxy = ProxyConfig {
        proxy_type: ProxyType::Socks5,
        proxy_addr,
        username: Some("alice".to_owned()),
        password: Some("s3cret".to_owned()),
    };
    let _handle = connect_initiator_via_proxy(
        &proxy,
        &target.ip().to_string(),
        target.port(),
        init_cfg(),
        Arc::new(FlagApp {
            on: init_flag.clone(),
        }),
        truefix_transport::Services::default(),
    )
    .await
    .expect("SOCKS5-proxied connect with password auth");

    assert!(
        wait_until(
            || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on through the authenticated SOCKS5 proxy"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initiator_connects_through_an_http_connect_proxy() {
    let (target, acc_flag) = start_acceptor().await;
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    tokio::spawn(run_http_connect_proxy(proxy_listener));

    let init_flag = Arc::new(AtomicBool::new(false));
    let proxy = ProxyConfig {
        proxy_type: ProxyType::HttpConnect,
        proxy_addr,
        username: None,
        password: None,
    };
    let _handle = connect_initiator_via_proxy(
        &proxy,
        &target.ip().to_string(),
        target.port(),
        init_cfg(),
        Arc::new(FlagApp {
            on: init_flag.clone(),
        }),
        truefix_transport::Services::default(),
    )
    .await
    .expect("HTTP CONNECT-proxied connect");

    assert!(
        wait_until(
            || acc_flag.load(Ordering::SeqCst) && init_flag.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on through the HTTP CONNECT proxy"
    );
}

// --- T054 (US6, feature 006): proxy connect is bounded by connect_timeout (BUG-19) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_proxy_that_accepts_but_never_completes_the_handshake_times_out() {
    // A "proxy" that accepts the TCP connection and then sends nothing at all -- before the fix,
    // connect_initiator_via_proxy had no timeout of its own and would hang indefinitely waiting
    // for a SOCKS/CONNECT reply that will never arrive.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            // Hold the connection open, reply with nothing, forever.
            std::mem::forget(stream);
        }
    });

    let proxy = ProxyConfig {
        proxy_type: ProxyType::Socks5,
        proxy_addr,
        username: None,
        password: None,
    };
    let mut cfg = init_cfg();
    cfg.connect_timeout = Some(Duration::from_millis(500));

    let flag = Arc::new(AtomicBool::new(false));
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        connect_initiator_via_proxy(
            &proxy,
            "127.0.0.1",
            1, // never reached -- the proxy handshake itself never completes
            cfg,
            Arc::new(FlagApp { on: flag.clone() }),
            truefix_transport::Services::default(),
        ),
    )
    .await;

    match result {
        Err(_) => panic!(
            "connect_initiator_via_proxy did not respect connect_timeout -- it hung past this \
             test's own 5s outer bound instead of failing within the configured 500ms"
        ),
        Ok(Err(_)) => {} // expected: the proxy connect attempt itself timed out and errored
        Ok(Ok(_)) => panic!("expected the proxy connect attempt to fail (no handshake reply)"),
    }
}
