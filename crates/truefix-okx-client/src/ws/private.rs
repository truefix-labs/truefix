use crate::{
    error::{OkxError, OkxResult},
    types::websocket::{
        SubscriptionArg, WsAmendOrder, WsCommand, WsMassCancel, WsOrder, WsOrderReference,
    },
    ws::session::Session,
};

/// Authenticated private session entrypoint.
#[derive(Debug, Default)]
pub struct PrivateSession(pub Session);
impl PrivateSession {
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
    /// Rejects writes until authentication and subscription recovery complete.
    pub fn write_allowed(&self) -> OkxResult<()> {
        if self.0.can_write() {
            Ok(())
        } else {
            Err(OkxError::UnknownCompletion)
        }
    }
    pub fn login(&mut self, args: Vec<serde_json::Value>) -> WsCommand<Vec<serde_json::Value>> {
        self.0.login_required();
        WsCommand {
            op: "login".to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        }
    }
    pub fn subscribe(&mut self, args: Vec<SubscriptionArg>) -> WsCommand<Vec<SubscriptionArg>> {
        WsCommand {
            op: "subscribe".to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        }
    }
    pub fn place_order(&mut self, order: WsOrder) -> OkxResult<WsCommand<Vec<WsOrder>>> {
        self.write("order", vec![order])
    }
    pub fn batch_orders(&mut self, orders: Vec<WsOrder>) -> OkxResult<WsCommand<Vec<WsOrder>>> {
        self.write("batch-orders", orders)
    }
    pub fn cancel_order(
        &mut self,
        order: WsOrderReference,
    ) -> OkxResult<WsCommand<Vec<WsOrderReference>>> {
        self.write("cancel-order", vec![order])
    }
    pub fn batch_cancel_orders(
        &mut self,
        orders: Vec<WsOrderReference>,
    ) -> OkxResult<WsCommand<Vec<WsOrderReference>>> {
        self.write("batch-cancel-orders", orders)
    }
    pub fn amend_order(&mut self, order: WsAmendOrder) -> OkxResult<WsCommand<Vec<WsAmendOrder>>> {
        self.write("amend-order", vec![order])
    }
    pub fn batch_amend_orders(
        &mut self,
        orders: Vec<WsAmendOrder>,
    ) -> OkxResult<WsCommand<Vec<WsAmendOrder>>> {
        self.write("batch-amend-orders", orders)
    }
    pub fn mass_cancel(
        &mut self,
        request: WsMassCancel,
    ) -> OkxResult<WsCommand<Vec<WsMassCancel>>> {
        self.write("mass-cancel", vec![request])
    }
    fn write<T>(&mut self, op: &str, args: T) -> OkxResult<WsCommand<T>> {
        self.write_allowed()?;
        Ok(WsCommand {
            op: op.to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        })
    }
}
