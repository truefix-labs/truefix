//! Scripted local WebSocket fixtures used by session integration tests.

use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpListener, sync::oneshot};
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// Starts a server which captures one command and sends a subscribe acknowledgement and event.
pub async fn start() -> (String, oneshot::Receiver<String>) {
    start_scripted(
        &[
            r#"{"event":"subscribe","code":"0"}"#,
            r#"{"arg":{"channel":"tickers","instId":"BTC-USDT"},"data":[]}"#,
        ],
        false,
    )
    .await
}

/// Starts a server that captures one client command, then emits the supplied text frames.
///
/// A final close frame is useful for asserting that an unacknowledged command has unknown
/// completion.  The helper deliberately keeps credentials out of its return value.
pub async fn start_scripted(
    frames: &[&str],
    close_after_frames: bool,
) -> (String, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let script = frames
        .iter()
        .map(|frame| (*frame).to_owned())
        .collect::<Vec<_>>();
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        if let Some(Ok(Message::Text(command))) = socket.next().await {
            let _ = sender.send(command.to_string());
            for frame in script {
                socket.send(Message::Text(frame.into())).await.unwrap();
            }
            if close_after_frames {
                socket.send(Message::Close(None)).await.unwrap();
            }
        }
    });
    (format!("ws://{address}"), receiver)
}

/// Script a successful login acknowledgement followed by a liveness ping and disconnect.
pub async fn start_login_ping_disconnect() -> (String, oneshot::Receiver<String>) {
    start_scripted(&[r#"{"event":"login","code":"0"}"#, "ping"], true).await
}

/// Sends a binary frame after capturing one command, to exercise protocol-frame failures.
pub async fn start_binary_frame() -> (String, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        if let Some(Ok(Message::Text(command))) = socket.next().await {
            let _ = sender.send(command.to_string());
            socket.send(Message::Binary(vec![1].into())).await.unwrap();
        }
    });
    (format!("ws://{address}"), receiver)
}

/// Script an exchange error acknowledgement for correlation/error-path tests.
pub async fn start_error(code: &str, request_id: &str) -> (String, oneshot::Receiver<String>) {
    let error =
        format!(r#"{{"event":"error","id":"{request_id}","code":"{code}","msg":"fixture error"}}"#);
    start_scripted(&[&error], false).await
}
