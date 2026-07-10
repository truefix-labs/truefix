//! Minimal redacted WebSocket transport used by the session layer.

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use crate::error::{OkxError, OkxResult};

/// Connected WebSocket transport. Credentials are never retained or logged here.
pub struct WebSocketTransport {
    socket: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
}

impl WebSocketTransport {
    /// Connects to an explicitly selected OKX endpoint.
    pub async fn connect(endpoint: &str) -> OkxResult<Self> {
        let (socket, _) = connect_async(endpoint).await?;
        Ok(Self { socket })
    }

    /// Sends one serialized protocol record.
    pub async fn send<T: serde::Serialize>(&mut self, value: &T) -> OkxResult<()> {
        let body = serde_json::to_string(value).map_err(OkxError::Decode)?;
        self.socket.send(Message::Text(body.into())).await?;
        Ok(())
    }

    /// Receives one text event; close before acknowledgement is an unknown completion.
    pub async fn receive(&mut self) -> OkxResult<String> {
        match self.socket.next().await {
            Some(Ok(Message::Text(text))) => Ok(text.to_string()),
            Some(Ok(Message::Close(_))) | None => Err(OkxError::UnknownCompletion),
            Some(Ok(_)) => Err(OkxError::Decode(serde_json::Error::io(
                std::io::Error::other("unexpected non-text WebSocket message"),
            ))),
            Some(Err(error)) => Err(error.into()),
        }
    }

    /// Sends a normal close frame.
    pub async fn close(&mut self) -> OkxResult<()> {
        self.socket.close(None).await?;
        Ok(())
    }
}
