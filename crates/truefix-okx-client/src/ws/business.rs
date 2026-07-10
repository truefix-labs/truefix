use crate::{
    error::{OkxError, OkxResult},
    types::websocket::{SubscriptionArg, WsCommand, WsMassCancel},
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
    pub fn login_acknowledged(&mut self) {
        self.0.login_acknowledged();
    }
    pub fn subscriptions_replayed(&mut self) {
        self.0.subscriptions_replayed();
    }
    pub fn subscribe(&mut self, args: Vec<SubscriptionArg>) -> WsCommand<Vec<SubscriptionArg>> {
        WsCommand {
            op: "subscribe".to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        }
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
        Ok(WsCommand {
            op: "mass-cancel".to_owned(),
            id: Some(self.0.next_request_id()),
            args: vec![request],
        })
    }
}
