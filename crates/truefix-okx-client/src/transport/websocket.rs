//! Minimal redacted WebSocket transport used by the session layer.

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, client_async_tls_with_config, connect_async,
    tungstenite::{Error as WebSocketError, Message},
};

use crate::{
    error::{OkxError, OkxResult},
    types::websocket::WsHeartbeat,
    ws::session::Session,
};

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

    /// Connects through an HTTP `CONNECT` proxy when one is configured.
    ///
    /// Keeping proxy negotiation in the transport ensures every OKX WebSocket
    /// consumer uses the same connectivity policy as the REST client.
    pub async fn connect_with_proxy(endpoint: &str, proxy: Option<&str>) -> OkxResult<Self> {
        let Some(proxy) = proxy.filter(|value| !value.trim().is_empty()) else {
            return Self::connect(endpoint).await;
        };
        let endpoint_url = url::Url::parse(endpoint).map_err(|error| {
            WebSocketError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                error,
            ))
        })?;
        let endpoint_host = endpoint_url.host_str().ok_or_else(|| {
            WebSocketError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "WebSocket endpoint has no host",
            ))
        })?;
        let endpoint_port = endpoint_url.port_or_known_default().ok_or_else(|| {
            WebSocketError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "WebSocket endpoint has no port",
            ))
        })?;
        let proxy_url = url::Url::parse(proxy).map_err(|error| {
            WebSocketError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                error,
            ))
        })?;
        if proxy_url.scheme() != "http" {
            return Err(WebSocketError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "OKX WebSocket proxy must use the http scheme",
            ))
            .into());
        }
        let proxy_host = proxy_url.host_str().ok_or_else(|| {
            WebSocketError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "proxy URL has no host",
            ))
        })?;
        let proxy_port = proxy_url.port_or_known_default().ok_or_else(|| {
            WebSocketError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "proxy URL has no port",
            ))
        })?;
        let mut stream = tokio::net::TcpStream::connect((proxy_host, proxy_port))
            .await
            .map_err(WebSocketError::Io)?;
        let authority = format!("{endpoint_host}:{endpoint_port}");
        stream
            .write_all(
                format!(
                    "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nProxy-Connection: Keep-Alive\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .map_err(WebSocketError::Io)?;
        let mut response = Vec::with_capacity(512);
        let mut byte = [0u8; 1];
        while response.len() < 16 * 1024 {
            stream
                .read_exact(&mut byte)
                .await
                .map_err(WebSocketError::Io)?;
            response.push(byte[0]);
            if response.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let status = std::str::from_utf8(&response)
            .ok()
            .and_then(|value| value.lines().next())
            .unwrap_or_default();
        if !status.contains(" 200 ") {
            return Err(WebSocketError::Io(std::io::Error::other(format!(
                "proxy CONNECT failed: {status}"
            )))
            .into());
        }
        // The client can be linked into applications that enable more than
        // one rustls backend through unrelated dependencies. In that case
        // rustls cannot infer a process default and panics at the first TLS
        // handshake unless the transport selects one explicitly.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let (socket, _) = client_async_tls_with_config(endpoint, stream, None, None).await?;
        Ok(Self { socket })
    }

    /// Sends one serialized protocol record.
    pub async fn send<T: serde::Serialize>(&mut self, value: &T) -> OkxResult<()> {
        let body = serde_json::to_string(value).map_err(OkxError::Decode)?;
        self.socket.send(Message::Text(body.into())).await?;
        Ok(())
    }

    /// Sends OKX's required literal application heartbeat text frame.
    pub async fn send_heartbeat(&mut self, _: WsHeartbeat) -> OkxResult<()> {
        self.socket.send(Message::Text("ping".into())).await?;
        Ok(())
    }

    /// Sends an OKX application heartbeat when the session's inbound-idle timer expires.
    pub async fn send_heartbeat_if_due(&mut self, session: &mut Session) -> OkxResult<bool> {
        if session.ping_due(std::time::Instant::now()) {
            heartbeat_send_result(session, self.send_heartbeat(WsHeartbeat).await)
        } else {
            Ok(false)
        }
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

    /// Receives one application message while keeping the supplied session's heartbeat state in
    /// sync. A `pong` is consumed after acknowledging the pending heartbeat; other text is
    /// returned to the caller for normal acknowledgement/event handling.
    pub async fn receive_for_session(
        &mut self,
        session: &mut Session,
    ) -> OkxResult<Option<String>> {
        let message = match self.receive().await {
            Ok(message) => message,
            Err(error) => {
                // A close frame, transport failure, and malformed frame all make the
                // connection unusable.  Do not leave callers with an Active session
                // after returning an error.
                session.disconnected();
                return Err(error);
            }
        };
        let now = std::time::Instant::now();
        if message == "pong" {
            session.pong_received();
            Ok(None)
        } else {
            session.message_received(now);
            Ok(Some(message))
        }
    }

    /// Sends a normal close frame.
    pub async fn close(&mut self) -> OkxResult<()> {
        self.socket.close(None).await?;
        Ok(())
    }
}

/// Applies the lifecycle effect of a heartbeat write. Kept separate from the socket I/O so the
/// connection-state invariant remains directly testable.
fn heartbeat_send_result(session: &mut Session, result: OkxResult<()>) -> OkxResult<bool> {
    match result {
        Ok(()) => Ok(true),
        Err(error) => {
            // `ping_due` has already recorded an outstanding pong. A failed write means that
            // pong can never arrive through this transport, so keep the lifecycle gate closed
            // until reconnect recovery completes.
            session.disconnected();
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws::session::SessionState;

    #[test]
    fn failed_heartbeat_write_disconnects_the_session() {
        let mut session = Session::default();
        session.connected();

        assert!(matches!(
            heartbeat_send_result(&mut session, Err(OkxError::UnknownCompletion)),
            Err(OkxError::UnknownCompletion)
        ));
        assert_eq!(session.state(), SessionState::Backoff);
    }
}
