use crate::{
    auth::{Clock, sign_websocket_login},
    config::Credentials,
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
    pub fn subscribe(
        &mut self,
        args: Vec<SubscriptionArg>,
    ) -> OkxResult<WsCommand<Vec<SubscriptionArg>>> {
        self.active()?;
        Ok(WsCommand {
            op: "subscribe".to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        })
    }
    pub fn subscribe_raw(
        &mut self,
        args: Vec<serde_json::Value>,
    ) -> OkxResult<WsCommand<Vec<serde_json::Value>>> {
        self.active()?;
        Ok(WsCommand {
            op: "subscribe".to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        })
    }
    pub fn unsubscribe(
        &mut self,
        args: Vec<SubscriptionArg>,
    ) -> OkxResult<WsCommand<Vec<SubscriptionArg>>> {
        self.active()?;
        Ok(WsCommand {
            op: "unsubscribe".to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        })
    }
    pub fn unsubscribe_raw(
        &mut self,
        args: Vec<serde_json::Value>,
    ) -> OkxResult<WsCommand<Vec<serde_json::Value>>> {
        self.active()?;
        Ok(WsCommand {
            op: "unsubscribe".to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        })
    }
    pub fn ping(&mut self) -> WsCommand<Vec<()>> {
        WsCommand {
            op: "ping".to_owned(),
            id: Some(self.0.next_request_id()),
            args: vec![],
        }
    }
    pub fn send(
        &mut self,
        op: impl Into<String>,
        args: Vec<serde_json::Value>,
    ) -> WsCommand<Vec<serde_json::Value>> {
        WsCommand {
            op: op.into(),
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
    fn active(&self) -> OkxResult<()> {
        if self.0.is_active() {
            Ok(())
        } else {
            Err(OkxError::UnknownCompletion)
        }
    }
}
