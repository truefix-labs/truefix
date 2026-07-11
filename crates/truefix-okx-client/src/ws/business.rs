use crate::{
    auth::{Clock, sign_websocket_login},
    config::Credentials,
    error::{OkxError, OkxResult},
    types::websocket::{SubscriptionArg, WsCommand, WsHeartbeat, WsMassCancel},
    ws::session::Session,
};

/// Business session supports public feeds and becomes authenticated only after an explicit login.
#[derive(Debug, Default)]
pub struct BusinessSession(pub Session);
impl BusinessSession {
    pub fn connected(&mut self) {
        self.0.connected();
    }
    pub fn login_required(&mut self) {
        self.0.login_required();
    }
    /// Records whether OKX accepted the optional business login command.
    pub fn login_acknowledged(&mut self, success: bool) {
        self.0.login_acknowledged(success);
    }
    pub fn subscriptions_replayed(&mut self) {
        self.0.subscriptions_replayed();
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
    pub fn login(&mut self, args: Vec<serde_json::Value>) -> WsCommand<Vec<serde_json::Value>> {
        self.0.login_required();
        WsCommand::new("login", None, args)
    }
    pub fn signed_login(
        &mut self,
        credentials: &Credentials,
        clock: &dyn Clock,
    ) -> OkxResult<WsCommand<Vec<serde_json::Value>>> {
        let timestamp = clock.now().unix_timestamp();
        let sign = sign_websocket_login(credentials, timestamp)?;
        Ok(self.login(vec![serde_json::json!({
            "apiKey": credentials.api_key(),
            "passphrase": credentials.passphrase(),
            "timestamp": timestamp.to_string(),
            "sign": sign,
        })]))
    }
    pub fn send(
        &mut self,
        op: impl Into<String>,
        args: Vec<serde_json::Value>,
    ) -> WsCommand<Vec<serde_json::Value>> {
        WsCommand::new(op, Some(self.0.next_request_id()), args)
    }
    /// Business mass cancel has its own server rate-limit class; callers can reserve that
    /// limiter before sending this command. It is never queued or replayed by a session.
    pub fn mass_cancel(
        &mut self,
        request: WsMassCancel,
    ) -> OkxResult<WsCommand<Vec<WsMassCancel>>> {
        if !self.0.can_write() {
            return Err(OkxError::UnknownCompletion);
        }
        Ok(WsCommand::new(
            "mass-cancel",
            Some(self.0.next_request_id()),
            vec![request],
        ))
    }
    fn active(&self) -> OkxResult<()> {
        if self.0.is_active() {
            Ok(())
        } else {
            Err(OkxError::UnknownCompletion)
        }
    }
}
