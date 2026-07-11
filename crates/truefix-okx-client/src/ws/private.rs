use crate::{
    auth::{Clock, sign_websocket_login},
    config::Credentials,
    error::{OkxError, OkxResult},
    types::websocket::{
        SubscriptionArg, WsAmendOrder, WsCommand, WsHeartbeat, WsMassCancel, WsOrder,
        WsOrderReference,
    },
    ws::session::Session,
};

/// Authenticated private session entrypoint.
#[derive(Debug, Default)]
pub struct PrivateSession(pub Session);
impl PrivateSession {
    pub fn connected(&mut self) {
        // Private endpoints always require a login, including on the initial
        // connection. Mark that requirement before Session::connected so it
        // cannot incorrectly enter Active as a public session would.
        self.0.login_required();
        self.0.connected();
    }
    pub fn login_required(&mut self) {
        self.0.login_required();
    }
    /// Records whether OKX accepted the login command.
    pub fn login_acknowledged(&mut self, success: bool) {
        self.0.login_acknowledged(success);
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
    ) -> WsCommand<Vec<serde_json::Value>> {
        WsCommand::new(op, Some(self.0.next_request_id()), args)
    }
    pub fn place_order(&mut self, order: WsOrder) -> OkxResult<WsCommand<Vec<WsOrder>>> {
        self.write("order", vec![order])
    }
    /// Places one order with an OKX processing deadline.
    pub fn place_order_with_expiration(
        &mut self,
        order: WsOrder,
        expiration_time: crate::types::common::ExpirationTime,
    ) -> OkxResult<WsCommand<Vec<WsOrder>>> {
        Ok(self
            .write("order", vec![order])?
            .with_expiration_time(expiration_time))
    }
    pub fn batch_orders(&mut self, orders: Vec<WsOrder>) -> OkxResult<WsCommand<Vec<WsOrder>>> {
        self.write("batch-orders", orders)
    }
    /// Places a batch of orders with one shared OKX processing deadline.
    pub fn batch_orders_with_expiration(
        &mut self,
        orders: Vec<WsOrder>,
        expiration_time: crate::types::common::ExpirationTime,
    ) -> OkxResult<WsCommand<Vec<WsOrder>>> {
        Ok(self
            .write("batch-orders", orders)?
            .with_expiration_time(expiration_time))
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
    /// Amends one order with an OKX processing deadline.
    pub fn amend_order_with_expiration(
        &mut self,
        order: WsAmendOrder,
        expiration_time: crate::types::common::ExpirationTime,
    ) -> OkxResult<WsCommand<Vec<WsAmendOrder>>> {
        Ok(self
            .write("amend-order", vec![order])?
            .with_expiration_time(expiration_time))
    }
    pub fn batch_amend_orders(
        &mut self,
        orders: Vec<WsAmendOrder>,
    ) -> OkxResult<WsCommand<Vec<WsAmendOrder>>> {
        self.write("batch-amend-orders", orders)
    }
    /// Amends a batch with one shared OKX processing deadline.
    pub fn batch_amend_orders_with_expiration(
        &mut self,
        orders: Vec<WsAmendOrder>,
        expiration_time: crate::types::common::ExpirationTime,
    ) -> OkxResult<WsCommand<Vec<WsAmendOrder>>> {
        Ok(self
            .write("batch-amend-orders", orders)?
            .with_expiration_time(expiration_time))
    }
    pub fn mass_cancel(
        &mut self,
        request: WsMassCancel,
    ) -> OkxResult<WsCommand<Vec<WsMassCancel>>> {
        self.write("mass-cancel", vec![request])
    }
    fn write<T>(&mut self, op: &str, args: T) -> OkxResult<WsCommand<T>> {
        self.write_allowed()?;
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
