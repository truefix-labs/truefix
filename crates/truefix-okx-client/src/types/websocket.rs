//! Typed OKX V5 WebSocket protocol records.

/// Request identifier used to correlate acknowledgements and commands.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct RequestId(pub String);

/// OKX WebSocket application-level heartbeat.
///
/// This is deliberately not serializable: OKX requires the literal text frame `ping`,
/// rather than a JSON command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WsHeartbeat;

/// A channel subscription argument.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SubscriptionArg {
    /// OKX channel name.
    pub channel: String,
    /// Optional instrument identifier.
    #[serde(rename = "instId", skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<String>,
    /// Channel-specific parameters such as `instType`, `uly`, and `instFamily`.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, String>,
}

impl SubscriptionArg {
    /// Creates an argument with the fields common to most market channels.
    pub fn new(channel: impl Into<String>, instrument_id: Option<String>) -> Self {
        Self {
            channel: channel.into(),
            instrument_id,
            extra: std::collections::BTreeMap::new(),
        }
    }

    /// Adds a channel-specific OKX subscription parameter.
    pub fn with_parameter(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(name.into(), value.into());
        self
    }
}

/// WebSocket command sent to OKX.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WsCommand<T> {
    /// `login`, `subscribe`, `unsubscribe`, or a trading operation.
    pub op: String,
    /// Correlation identifier.
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    /// Optional Unix-millisecond deadline for order and amendment commands.
    #[serde(rename = "expTime", skip_serializing_if = "Option::is_none")]
    expiration_time: Option<crate::types::common::ExpirationTime>,
    /// Command arguments.
    pub args: T,
}

impl<T> WsCommand<T> {
    /// Builds a WebSocket command without a processing deadline.
    ///
    /// OKX only accepts `expTime` for private order and amendment operations. Those
    /// operations are exposed through the dedicated `PrivateSession` helpers rather
    /// than this general-purpose constructor.
    pub fn new(op: impl Into<String>, id: Option<RequestId>, args: T) -> Self {
        Self {
            op: op.into(),
            id,
            expiration_time: None,
            args,
        }
    }

    /// Returns the optional processing deadline selected by a supported private
    /// order or amendment helper.
    pub fn expiration_time(&self) -> Option<crate::types::common::ExpirationTime> {
        self.expiration_time
    }

    /// Adds an OKX command-level processing deadline for supported order and amendment
    /// operations. Public session methods enforce that scope.
    pub(crate) fn with_expiration_time(
        mut self,
        expiration_time: crate::types::common::ExpirationTime,
    ) -> Self {
        self.expiration_time = Some(expiration_time);
        self
    }
}

/// Server acknowledgement or error.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WsAcknowledgement {
    /// Acknowledged operation.
    ///
    /// OKX names this field `event` (for example `login`, `subscribe`,
    /// `unsubscribe`, `error`, or `notice`); it is distinct from the `op`
    /// field used in client commands.
    pub event: Option<String>,
    /// Server correlation identifier.
    #[serde(rename = "id")]
    pub request_id: Option<RequestId>,
    /// OKX status code.
    pub code: Option<String>,
    /// Error text when non-successful.
    pub msg: Option<String>,
}

/// A routed real-time event preserving the source channel and raw native payload.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WsEvent {
    /// Event argument including the source channel.
    pub arg: SubscriptionArg,
    /// Native exchange records.
    pub data: Vec<serde_json::Value>,
}

/// Typed arguments accepted by the real-time order command.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WsOrder {
    #[serde(rename = "instId")]
    pub instrument_id: String,
    #[serde(rename = "tdMode")]
    pub trade_mode: String,
    pub side: String,
    #[serde(rename = "ordType")]
    pub order_type: String,
    #[serde(rename = "sz")]
    pub size: String,
    #[serde(rename = "px", skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    #[serde(rename = "clOrdId", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

/// Typed order identity used by cancel and amend commands.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WsOrderReference {
    #[serde(rename = "instId", skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<String>,
    #[serde(rename = "ordId", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(rename = "clOrdId", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

/// Typed arguments accepted by the real-time amend command.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WsAmendOrder {
    #[serde(flatten)]
    pub order: WsOrderReference,
    #[serde(rename = "newSz", skip_serializing_if = "Option::is_none")]
    pub new_size: Option<String>,
    #[serde(rename = "newPx", skip_serializing_if = "Option::is_none")]
    pub new_price: Option<String>,
}

/// Typed arguments accepted by the real-time mass-cancel command.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WsMassCancel {
    #[serde(rename = "instType")]
    pub instrument_type: String,
    #[serde(rename = "uly", skip_serializing_if = "Option::is_none")]
    pub underlying: Option<String>,
    #[serde(rename = "instFamily", skip_serializing_if = "Option::is_none")]
    pub instrument_family: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_order_uses_okx_size_field_name() {
        let order = WsOrder {
            instrument_id: "BTC-USDT".to_owned(),
            trade_mode: "cash".to_owned(),
            side: "buy".to_owned(),
            order_type: "market".to_owned(),
            size: "1".to_owned(),
            price: None,
            client_order_id: None,
        };
        let value = serde_json::to_value(order).unwrap();
        assert_eq!(
            value.get("sz").and_then(serde_json::Value::as_str),
            Some("1")
        );
        assert!(value.get("size").is_none());
    }

    #[test]
    fn websocket_command_expiry_is_encoded_at_the_top_level() {
        let order = WsOrder {
            instrument_id: "BTC-USDT".to_owned(),
            trade_mode: "cash".to_owned(),
            side: "buy".to_owned(),
            order_type: "market".to_owned(),
            size: "1".to_owned(),
            price: None,
            client_order_id: None,
        };
        let command = WsCommand::new("order", Some(RequestId("1".to_owned())), vec![order])
            .with_expiration_time(
                crate::types::common::ExpirationTime::new(1_704_067_200_123).unwrap(),
            );
        assert_eq!(
            serde_json::to_value(command).unwrap()["expTime"],
            "1704067200123"
        );
    }

    #[test]
    fn subscription_arguments_preserve_channel_specific_fields() {
        let argument = SubscriptionArg::new("orders", None)
            .with_parameter("instType", "FUTURES")
            .with_parameter("instFamily", "BTC-USD");
        assert_eq!(
            serde_json::to_value(argument).unwrap(),
            serde_json::json!({
                "channel": "orders",
                "instType": "FUTURES",
                "instFamily": "BTC-USD",
            })
        );
    }

    #[test]
    fn acknowledgements_use_the_okx_event_field() {
        for event in ["login", "subscribe", "unsubscribe", "error", "notice"] {
            let acknowledgement: WsAcknowledgement = serde_json::from_value(serde_json::json!({
                "event": event,
                "code": "0",
            }))
            .unwrap();
            assert_eq!(acknowledgement.event.as_deref(), Some(event));
        }
    }
}
