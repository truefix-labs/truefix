use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use bytes::Bytes;
use prost::Message;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::warn;

use crate::codec::frame::FrameHeader;
use crate::error::{FutuError, FutuResult};
use crate::pb;
use crate::proto_id;
use crate::push::{Push, decode_push};
use crate::transport::{FrameReader, FrameWriter};

pub(crate) enum Command {
    Request {
        proto_id: u32,
        serial_no: Option<u32>,
        body: Bytes,
        reply: oneshot::Sender<FutuResult<(FrameHeader, Bytes)>>,
    },
    Shutdown,
}

pub(crate) struct ConnectionActor {
    pub(crate) reader: FrameReader,
    pub(crate) writer: FrameWriter,
    pub(crate) cmd_rx: mpsc::Receiver<Command>,
    pub(crate) pending: HashMap<u32, oneshot::Sender<FutuResult<(FrameHeader, Bytes)>>>,
    pub(crate) push_tx: broadcast::Sender<Push>,
    pub(crate) serial: Arc<AtomicU32>,
    pub(crate) keepalive: tokio::time::Interval,
    pub(crate) disconnect_tx: mpsc::UnboundedSender<()>,
}

impl ConnectionActor {
    pub(crate) async fn run(mut self) {
        let mut should_reconnect = false;
        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(Command::Request { proto_id, serial_no, body, reply }) => {
                            if let Err(err) = self.on_command(proto_id, serial_no, body, reply).await {
                                warn!(%err, "connection actor request failed");
                                should_reconnect = true;
                                break;
                            }
                        }
                        Some(Command::Shutdown) | None => break,
                    }
                }
                frame = self.reader.recv() => {
                    match frame {
                        Ok((header, body)) => {
                            if let Err(err) = self.on_frame(header, body).await {
                                warn!(%err, "connection actor frame handling failed");
                                should_reconnect = true;
                                break;
                            }
                        }
                        Err(err) => {
                            warn!(%err, "connection actor recv failed");
                            should_reconnect = true;
                            break;
                        }
                    }
                }
                _ = self.keepalive.tick() => {
                    if let Err(err) = self.on_keepalive().await {
                        warn!(%err, "connection actor keepalive failed");
                        should_reconnect = true;
                        break;
                    }
                }
            }
        }
        self.fail_pending(FutuError::ActorGone);
        if should_reconnect {
            let _ = self.disconnect_tx.send(());
        }
    }

    async fn on_command(
        &mut self,
        proto_id: u32,
        serial_no: Option<u32>,
        body: Bytes,
        reply: oneshot::Sender<FutuResult<(FrameHeader, Bytes)>>,
    ) -> FutuResult<()> {
        let serial = match serial_no {
            Some(serial) => serial,
            None => next_serial(&self.serial)?,
        };
        self.pending.insert(serial, reply);
        if let Err(err) = self.writer.send(proto_id, serial, &body).await {
            if let Some(reply) = self.pending.remove(&serial) {
                let _ = reply.send(Err(err));
            }
            return Err(FutuError::ActorGone);
        }
        Ok(())
    }

    async fn on_frame(&mut self, header: FrameHeader, body: Bytes) -> FutuResult<()> {
        if header.proto_id == proto_id::KEEP_ALIVE {
            return Ok(());
        }
        if proto_id::is_push(header.proto_id) {
            let push = decode_push(header.proto_id, &body)?;
            let _ = self.push_tx.send(push);
            return Ok(());
        }
        if let Some(reply) = self.pending.remove(&header.serial_no) {
            let _ = reply.send(Ok((header, body)));
        }
        Ok(())
    }

    async fn on_keepalive(&mut self) -> FutuResult<()> {
        let request = pb::keep_alive::Request {
            c2s: pb::keep_alive::C2s {
                time: unix_time_secs(),
            },
        };
        let body = request.encode_to_vec();
        let serial = next_serial(&self.serial)?;
        self.writer.send(proto_id::KEEP_ALIVE, serial, &body).await?;
        Ok(())
    }

    fn fail_pending(&mut self, err: FutuError) {
        for (_, reply) in self.pending.drain() {
            let _ = reply.send(Err(FutuError::ActorGone));
        }
        let _ = err;
    }
}

fn next_serial(counter: &Arc<AtomicU32>) -> FutuResult<u32> {
    let current = counter.fetch_add(1, Ordering::Relaxed);
    current.checked_add(1).ok_or(FutuError::SerialOverflow)
}

fn unix_time_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}
