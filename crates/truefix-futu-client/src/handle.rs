use std::time::Duration;

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

use crate::actor::Command;
use crate::codec::frame::FrameHeader;
use crate::error::{FutuError, FutuResult};

#[derive(Clone)]
pub(crate) struct ActorHandle {
    pub(crate) cmd_tx: mpsc::Sender<Command>,
    pub(crate) request_timeout_ms: u64,
}

impl ActorHandle {
    pub async fn request_with_serial(
        &self,
        proto_id: u32,
        serial_no: Option<u32>,
        body: Bytes,
    ) -> FutuResult<(FrameHeader, Bytes)> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Request {
                proto_id,
                serial_no,
                body,
                reply: reply_tx,
            })
            .await
            .map_err(|_| FutuError::ActorGone)?;
        match tokio::time::timeout(Duration::from_millis(self.request_timeout_ms), reply_rx).await {
            Ok(result) => result.map_err(|_| FutuError::ActorGone)?,
            Err(_) => Err(FutuError::Timeout {
                timeout_ms: self.request_timeout_ms,
            }),
        }
    }

    pub async fn shutdown(&self) -> FutuResult<()> {
        self.cmd_tx
            .send(Command::Shutdown)
            .await
            .map_err(|_| FutuError::ActorGone)
    }
}
