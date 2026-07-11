use crate::{
    error::{OkxError, OkxResult},
    types::websocket::{SubscriptionArg, WsCommand, WsHeartbeat},
    ws::session::Session,
};

/// Public session entrypoint. It becomes active without credentials after connection setup.
#[derive(Debug, Default)]
pub struct PublicSession(pub Session);
impl PublicSession {
    pub fn connected(&mut self) {
        self.0.connected();
    }
    pub fn subscribe(
        &mut self,
        args: Vec<SubscriptionArg>,
    ) -> OkxResult<WsCommand<Vec<SubscriptionArg>>> {
        self.active()?;
        Ok(WsCommand::new(
            "subscribe",
            Some(self.0.next_request_id()),
            args,
        ))
    }
    pub fn subscribe_raw(
        &mut self,
        args: Vec<serde_json::Value>,
    ) -> OkxResult<WsCommand<Vec<serde_json::Value>>> {
        self.active()?;
        Ok(WsCommand::new(
            "subscribe",
            Some(self.0.next_request_id()),
            args,
        ))
    }
    pub fn unsubscribe(
        &mut self,
        args: Vec<SubscriptionArg>,
    ) -> OkxResult<WsCommand<Vec<SubscriptionArg>>> {
        self.active()?;
        Ok(WsCommand::new(
            "unsubscribe",
            Some(self.0.next_request_id()),
            args,
        ))
    }
    pub fn unsubscribe_raw(
        &mut self,
        args: Vec<serde_json::Value>,
    ) -> OkxResult<WsCommand<Vec<serde_json::Value>>> {
        self.active()?;
        Ok(WsCommand::new(
            "unsubscribe",
            Some(self.0.next_request_id()),
            args,
        ))
    }
    pub fn ping(&mut self) -> WsHeartbeat {
        WsHeartbeat
    }
    pub fn send(
        &mut self,
        op: impl Into<String>,
        args: Vec<serde_json::Value>,
    ) -> OkxResult<WsCommand<Vec<serde_json::Value>>> {
        self.active()?;
        Ok(WsCommand::new(op, Some(self.0.next_request_id()), args))
    }
    fn active(&self) -> OkxResult<()> {
        if self.0.is_active() {
            Ok(())
        } else {
            Err(OkxError::UnknownCompletion)
        }
    }
}
