//! Network hardening (US12; FR-015/FR-016):
//! - PROXY protocol (v1/v2, via `ppp`) parsing for acceptors, gated on a trusted-upstream source
//!   IP set — used to recover the real client address behind a load balancer/proxy.
//! - A forward-proxy client for initiators (SOCKS4/SOCKS5 with optional username/password auth via
//!   `tokio-socks`; HTTP CONNECT hand-rolled — a single request line plus a header block, not worth
//!   a full HTTP client dependency).

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use ppp::{HeaderResult, v1, v2};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Which forward-proxy protocol to use (`ProxyType`; FR-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyType {
    /// SOCKS4 (optionally with a user ID).
    Socks4,
    /// SOCKS5 (optionally with username/password authentication).
    Socks5,
    /// HTTP CONNECT.
    HttpConnect,
}

/// Forward-proxy configuration for an initiator connection (FR-016).
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Which proxy protocol to speak.
    pub proxy_type: ProxyType,
    /// The proxy server's address.
    pub proxy_addr: SocketAddr,
    /// Optional username (SOCKS5 password auth, or SOCKS4 user ID).
    pub username: Option<String>,
    /// Optional password (SOCKS5 password auth only).
    pub password: Option<String>,
}

/// An error establishing a forward-proxy connection.
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    /// A SOCKS4/SOCKS5 handshake error.
    #[error("SOCKS proxy error: {0}")]
    Socks(#[from] tokio_socks::Error),
    /// An HTTP CONNECT handshake error.
    #[error("HTTP CONNECT proxy error: {0}")]
    HttpConnect(String),
    /// An underlying I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Connect to `target_host:target_port` through the configured forward proxy, returning the
/// tunneled TCP stream ready for the FIX (or TLS-over-FIX) handshake.
pub(crate) async fn connect_through_proxy(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream, ProxyError> {
    match proxy.proxy_type {
        ProxyType::Socks4 => {
            let stream = match &proxy.username {
                Some(user_id) => {
                    tokio_socks::tcp::Socks4Stream::connect_with_userid(
                        proxy.proxy_addr,
                        (target_host, target_port),
                        user_id.as_str(),
                    )
                    .await?
                }
                None => {
                    tokio_socks::tcp::Socks4Stream::connect(
                        proxy.proxy_addr,
                        (target_host, target_port),
                    )
                    .await?
                }
            };
            Ok(stream.into_inner())
        }
        ProxyType::Socks5 => {
            let stream = match (&proxy.username, &proxy.password) {
                (Some(user), Some(pass)) => {
                    tokio_socks::tcp::Socks5Stream::connect_with_password(
                        proxy.proxy_addr,
                        (target_host, target_port),
                        user.as_str(),
                        pass.as_str(),
                    )
                    .await?
                }
                _ => {
                    tokio_socks::tcp::Socks5Stream::connect(
                        proxy.proxy_addr,
                        (target_host, target_port),
                    )
                    .await?
                }
            };
            Ok(stream.into_inner())
        }
        ProxyType::HttpConnect => connect_http(proxy.proxy_addr, target_host, target_port).await,
    }
}

/// Hand-rolled HTTP CONNECT: a request line + a `Host` header, terminated by a blank line;
/// success is any `2xx` status on the response's status line.
async fn connect_http(
    proxy_addr: SocketAddr,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream, ProxyError> {
    let mut stream = TcpStream::connect(proxy_addr).await?;
    let request = format!(
        "CONNECT {target_host}:{target_port} HTTP/1.1\r\nHost: {target_host}:{target_port}\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await?;

    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).await?;
        if n == 0 {
            return Err(ProxyError::HttpConnect(
                "proxy closed the connection before completing the CONNECT response".to_owned(),
            ));
        }
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
        if buf.len() > 8192 {
            return Err(ProxyError::HttpConnect(
                "CONNECT response headers exceeded 8KiB".to_owned(),
            ));
        }
    }
    let response = String::from_utf8_lossy(&buf);
    let status_line = response.lines().next().unwrap_or("");
    let status_ok = status_line
        .split_whitespace()
        .nth(1)
        .is_some_and(|code| code.starts_with('2'));
    if !status_ok {
        return Err(ProxyError::HttpConnect(format!(
            "CONNECT failed: {status_line:?}"
        )));
    }
    Ok(stream)
}

/// Parse a PROXY protocol (v1 or v2) header at the start of `buf`, returning the original client
/// address it declares and the number of bytes the header occupied — or `None` if `buf` doesn't
/// start with a complete, valid header.
fn parse_proxy_header(buf: &[u8]) -> Option<(SocketAddr, usize)> {
    match HeaderResult::parse(buf) {
        HeaderResult::V1(Ok(header)) => {
            let len = header.header.len();
            let addr = match header.addresses {
                v1::Addresses::Tcp4(a) => {
                    SocketAddr::new(IpAddr::V4(a.source_address), a.source_port)
                }
                v1::Addresses::Tcp6(a) => {
                    SocketAddr::new(IpAddr::V6(a.source_address), a.source_port)
                }
                v1::Addresses::Unknown => return None,
            };
            Some((addr, len))
        }
        HeaderResult::V2(Ok(header)) => {
            let len = header.len();
            let addr = match header.addresses {
                v2::Addresses::IPv4(a) => {
                    SocketAddr::new(IpAddr::V4(a.source_address), a.source_port)
                }
                v2::Addresses::IPv6(a) => {
                    SocketAddr::new(IpAddr::V6(a.source_address), a.source_port)
                }
                v2::Addresses::Unspecified | v2::Addresses::Unix(_) => return None,
            };
            Some((addr, len))
        }
        _ => None,
    }
}

/// If `peer`'s IP is in `trusted`, peek the start of `stream` for a PROXY (v1/v2) header; if one
/// is present, consume exactly its bytes and return the original client address it declares.
/// Otherwise (untrusted source, or no valid header present) the stream is left completely
/// untouched and `peer` is returned unchanged — an untrusted source's header is never trusted,
/// matching the spec's Clarifications (rejected in favor of the physical source address).
/// GAP-54 (feature 006): the overall bound on how long a trusted-source connection may take to
/// deliver a complete PROXY header before it's treated as if none was sent — `TcpStream::peek`
/// awaits until at least one byte is available, so with no timeout at all a connection that opens
/// but sends nothing would hang this task indefinitely.
const PROXY_HEADER_TIMEOUT: Duration = Duration::from_secs(5);

/// GAP-53/B15 (feature 006, per docs/todo/003.md's audit numbering — the PROXY v2 buffer-sizing
/// item): large enough to comfortably hold a realistic PROXY v2 header's TLV set (spec maximum is
/// 64 KiB, but that's far beyond anything this codebase's own trusted-proxy use actually attaches)
/// without truncating it mid-TLV, which previously could cause the remaining PROXY-protocol bytes
/// to be misread as FIX message data.
const PROXY_HEADER_PEEK_BUF: usize = 4096;

pub(crate) async fn strip_trusted_proxy_header(
    stream: &mut TcpStream,
    peer: SocketAddr,
    trusted: &[IpAddr],
) -> SocketAddr {
    if trusted.is_empty() || !trusted.contains(&peer.ip()) {
        return peer;
    }
    match tokio::time::timeout(PROXY_HEADER_TIMEOUT, peek_proxy_header(stream, peer)).await {
        Ok(addr) => addr,
        Err(_) => peer, // timed out waiting for a complete header -- treat as untrusted/absent
    }
}

async fn peek_proxy_header(stream: &mut TcpStream, peer: SocketAddr) -> SocketAddr {
    let mut buf = vec![0u8; PROXY_HEADER_PEEK_BUF];
    for _ in 0..10 {
        let n = match stream.peek(&mut buf).await {
            Ok(n) => n,
            Err(_) => return peer,
        };
        let Some(peeked) = buf.get(..n) else {
            return peer;
        };
        match parse_proxy_header(peeked) {
            Some((addr, len)) => {
                let mut discard = vec![0u8; len];
                if stream.read_exact(&mut discard).await.is_err() {
                    return peer;
                }
                return addr;
            }
            None => {
                if n >= buf.len() {
                    return peer; // a full peek's worth of bytes, still no valid header
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }
    peer
}
