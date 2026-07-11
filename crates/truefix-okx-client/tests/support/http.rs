//! Minimal local HTTP fixture used to verify the bytes emitted by the SDK.

use std::collections::BTreeMap;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

/// An HTTP request captured by a one-shot local fixture.
#[derive(Debug)]
pub struct CapturedRequest {
    /// Request method.
    pub method: String,
    /// Target path and query.
    pub target: String,
    /// Lowercase header names and their values.
    pub headers: BTreeMap<String, String>,
    /// Exact body bytes.
    pub body: Vec<u8>,
}

/// Starts a one-shot HTTP/1.1 server returning an OKX envelope and its captured request receiver.
pub async fn start(response_body: &'static str) -> (String, oneshot::Receiver<CapturedRequest>) {
    start_with_headers(response_body, &[]).await
}

/// Starts a one-shot HTTP/1.1 server with additional response headers.
pub async fn start_with_headers(
    response_body: &'static str,
    response_headers: &[(&str, &str)],
) -> (String, oneshot::Receiver<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = oneshot::channel();
    let response_headers = response_headers
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let count = stream.read(&mut buffer).await.unwrap();
            if count == 0 {
                return;
            }
            bytes.extend_from_slice(&buffer[..count]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap()
            + 4;
        let headers_text = std::str::from_utf8(&bytes[..header_end]).unwrap();
        let mut lines = headers_text.split("\r\n");
        let request_line = lines.next().unwrap();
        let mut request_words = request_line.split_whitespace();
        let method = request_words.next().unwrap().to_owned();
        let target = request_words.next().unwrap().to_owned();
        let mut headers = BTreeMap::new();
        for line in lines.filter(|line| !line.is_empty()) {
            let (name, value) = line.split_once(':').unwrap();
            headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
        }
        let content_length = headers
            .get("content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        while bytes.len() - header_end < content_length {
            let count = stream.read(&mut buffer).await.unwrap();
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        let body = bytes[header_end..header_end + content_length].to_vec();
        let _ = sender.send(CapturedRequest {
            method,
            target,
            headers,
            body,
        });
        let extra_headers = response_headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n{extra_headers}content-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    (format!("http://{address}"), receiver)
}

/// Starts a fixture which returns each supplied status/body pair to consecutive requests.
pub async fn start_sequence(
    responses: Vec<(u16, &'static str, Option<&'static str>)>,
) -> (String, oneshot::Receiver<Vec<CapturedRequest>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let mut captured = Vec::with_capacity(responses.len());
        for (status, body, retry_after) in responses {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            captured.push(request);
            let reason = if status == 200 {
                "OK"
            } else {
                "Too Many Requests"
            };
            let retry_header = retry_after
                .map(|value| format!("retry-after: {value}\r\n"))
                .unwrap_or_default();
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\n{retry_header}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
        let _ = sender.send(captured);
    });
    (format!("http://{address}"), receiver)
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> CapturedRequest {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer).await.unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + 4;
    let headers_text = std::str::from_utf8(&bytes[..header_end]).unwrap();
    let mut lines = headers_text.split("\r\n");
    let mut request_words = lines.next().unwrap().split_whitespace();
    let method = request_words.next().unwrap().to_owned();
    let target = request_words.next().unwrap().to_owned();
    let mut headers = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').unwrap();
        headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
    }
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    while bytes.len() - header_end < content_length {
        let count = stream.read(&mut buffer).await.unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    CapturedRequest {
        method,
        target,
        headers,
        body: bytes[header_end..header_end + content_length].to_vec(),
    }
}
