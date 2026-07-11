//! Minimal redacted WebSocket transport used by the session layer.

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

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
