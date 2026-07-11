use crate::types::{
    common::DecimalValue,
    gateway::{GatewayExecution, GatewayOrder, GatewayProject, NativeFields},
};
/// Stable caller-provided order identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ClientOrderId(pub String);
/// Completion state of an asset-changing command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionState {
    Confirmed,
    Rejected,
    Unknown,
}
/// Ordinary order submission.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaceOrder {
    #[serde(rename = "instId")]
    pub instrument_id: String,
    #[serde(rename = "tdMode")]
    pub trade_mode: String,
    pub side: String,
    #[serde(rename = "ordType")]
    pub order_type: String,
    #[serde(rename = "sz")]
    pub size: DecimalValue,
    #[serde(rename = "px", skip_serializing_if = "Option::is_none")]
    pub price: Option<DecimalValue>,
    #[serde(rename = "clOrdId", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<ClientOrderId>,
    #[serde(rename = "ccy", skip_serializing_if = "Option::is_none")]
    pub margin_currency: Option<String>,
    #[serde(rename = "tag", skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(rename = "posSide", skip_serializing_if = "Option::is_none")]
    pub position_side: Option<String>,
    #[serde(rename = "reduceOnly", skip_serializing_if = "Option::is_none")]
    pub reduce_only: Option<bool>,
    #[serde(rename = "tgtCcy", skip_serializing_if = "Option::is_none")]
    pub target_currency: Option<String>,
    #[serde(rename = "stpMode", skip_serializing_if = "Option::is_none")]
    pub self_trade_prevention_mode: Option<String>,
    #[serde(rename = "attachAlgoOrds", skip_serializing_if = "Option::is_none")]
    pub attached_algo_orders: Option<serde_json::Value>,
    #[serde(rename = "pxUsd", skip_serializing_if = "Option::is_none")]
    pub price_usd: Option<DecimalValue>,
    #[serde(rename = "pxVol", skip_serializing_if = "Option::is_none")]
    pub price_volatility: Option<DecimalValue>,
    #[serde(rename = "banAmend", skip_serializing_if = "Option::is_none")]
    pub ban_amend: Option<bool>,
    #[serde(rename = "tradeQuoteCcy", skip_serializing_if = "Option::is_none")]
    pub trade_quote_currency: Option<String>,
    #[serde(rename = "pxAmendType", skip_serializing_if = "Option::is_none")]
    pub price_amend_type: Option<String>,
    #[serde(rename = "isElpTakerAccess", skip_serializing_if = "Option::is_none")]
    pub elp_taker_access: Option<bool>,
    #[serde(rename = "instIdCode", skip_serializing_if = "Option::is_none")]
    pub instrument_id_code: Option<String>,
}
impl PlaceOrder {
    /// Creates a minimal order; advanced OKX fields default to omitted.
    pub fn new(
        instrument_id: impl Into<String>,
        trade_mode: impl Into<String>,
        side: impl Into<String>,
        order_type: impl Into<String>,
        size: DecimalValue,
    ) -> Self {
        Self {
            instrument_id: instrument_id.into(),
            trade_mode: trade_mode.into(),
            side: side.into(),
            order_type: order_type.into(),
            size,
            price: None,
            client_order_id: None,
            margin_currency: None,
            tag: None,
            position_side: None,
            reduce_only: None,
            target_currency: None,
            self_trade_prevention_mode: None,
            attached_algo_orders: None,
            price_usd: None,
            price_volatility: None,
            ban_amend: None,
            trade_quote_currency: None,
            price_amend_type: None,
            elp_taker_access: None,
            instrument_id_code: None,
        }
    }
}
/// An order reference used for cancellation, amendment and lookup.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OrderReference {
    #[serde(rename = "instId")]
    pub instrument_id: String,
    #[serde(rename = "ordId", skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(rename = "clOrdId", skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<ClientOrderId>,
}
/// Amendment request preserving the idempotency request identifier.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AmendOrder {
    #[serde(flatten)]
    pub order: OrderReference,
    #[serde(rename = "newSz", skip_serializing_if = "Option::is_none")]
    pub new_size: Option<DecimalValue>,
    #[serde(rename = "newPx", skip_serializing_if = "Option::is_none")]
    pub new_price: Option<DecimalValue>,
    #[serde(rename = "reqId", skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(rename = "newTpTriggerPx", skip_serializing_if = "Option::is_none")]
    pub new_take_profit_trigger_price: Option<DecimalValue>,
    #[serde(rename = "newTpOrdPx", skip_serializing_if = "Option::is_none")]
    pub new_take_profit_order_price: Option<DecimalValue>,
    #[serde(rename = "newSlTriggerPx", skip_serializing_if = "Option::is_none")]
    pub new_stop_loss_trigger_price: Option<DecimalValue>,
    #[serde(rename = "newSlOrdPx", skip_serializing_if = "Option::is_none")]
    pub new_stop_loss_order_price: Option<DecimalValue>,
    #[serde(rename = "newTpTriggerPxType", skip_serializing_if = "Option::is_none")]
    pub new_take_profit_trigger_price_type: Option<String>,
    #[serde(rename = "newSlTriggerPxType", skip_serializing_if = "Option::is_none")]
    pub new_stop_loss_trigger_price_type: Option<String>,
    #[serde(rename = "attachAlgoOrds", skip_serializing_if = "Option::is_none")]
    pub attached_algo_orders: Option<serde_json::Value>,
    #[serde(rename = "newTriggerPx", skip_serializing_if = "Option::is_none")]
    pub new_trigger_price: Option<DecimalValue>,
    #[serde(rename = "newOrdPx", skip_serializing_if = "Option::is_none")]
    pub new_order_price: Option<DecimalValue>,
    #[serde(rename = "pxAmendType", skip_serializing_if = "Option::is_none")]
    pub price_amend_type: Option<String>,
    #[serde(rename = "newTpTriggerRatio", skip_serializing_if = "Option::is_none")]
    pub new_take_profit_trigger_ratio: Option<DecimalValue>,
    #[serde(rename = "newSlTriggerRatio", skip_serializing_if = "Option::is_none")]
    pub new_stop_loss_trigger_ratio: Option<DecimalValue>,
    #[serde(rename = "cxlOnFail", skip_serializing_if = "Option::is_none")]
    pub cancel_on_fail: Option<bool>,
}
/// Exchange order record.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Order {
    #[serde(rename = "ordId")]
    pub order_id: String,
    #[serde(rename = "clOrdId")]
    pub client_order_id: Option<ClientOrderId>,
    #[serde(rename = "instId")]
    pub instrument_id: String,
    pub state: String,
    #[serde(rename = "accFillSz")]
    pub filled_size: Option<DecimalValue>,
    /// OKX fields outside the core order representation.
    #[serde(flatten)]
    pub native_fields: NativeFields,
}
/// Execution/fill record.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Fill {
    #[serde(rename = "fillId")]
    pub fill_id: String,
    #[serde(rename = "ordId")]
    pub order_id: String,
    #[serde(rename = "fillPx")]
    pub price: DecimalValue,
    #[serde(rename = "fillSz")]
    pub size: DecimalValue,
    /// OKX fields outside the core execution representation.
    #[serde(flatten)]
    pub native_fields: NativeFields,
}

impl GatewayProject for Order {
    type Projection = GatewayOrder;

    fn project_gateway(&self) -> Self::Projection {
        GatewayOrder {
            venue_order_id: self.order_id.clone(),
            client_order_id: self.client_order_id.clone(),
            instrument_id: self.instrument_id.clone(),
            state: self.state.clone(),
            filled_size: self.filled_size,
            native_fields: self.native_fields.clone(),
        }
    }
}

impl GatewayProject for Fill {
    type Projection = GatewayExecution;

    fn project_gateway(&self) -> Self::Projection {
        GatewayExecution {
            venue_execution_id: self.fill_id.clone(),
            venue_order_id: self.order_id.clone(),
            price: self.price,
            size: self.size,
            native_fields: self.native_fields.clone(),
        }
    }
}
/// Algorithmic order record.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AlgorithmicOrder {
    #[serde(rename = "algoId")]
    pub algorithm_id: String,
    #[serde(rename = "instId")]
    pub instrument_id: String,
    pub state: String,
}
/// Per-item result returned by batch commands.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OrderAck {
    #[serde(rename = "ordId")]
    pub order_id: String,
    #[serde(rename = "sCode")]
    pub code: String,
    #[serde(rename = "sMsg")]
    pub message: String,
    /// Exchange fields such as clOrdId, tag, and reqId retained for reconciliation.
    #[serde(flatten)]
    pub native_fields: NativeFields,
}
/// Batch result preserving successful and failed item acknowledgements.
pub type BatchResult = Vec<OrderAck>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_order_serializes_elp_taker_access_with_the_okx_field_name() {
        let mut order = PlaceOrder::new(
            "BTC-USDT",
            "cash",
            "buy",
            "market",
            "1".parse::<DecimalValue>().unwrap(),
        );
        order.elp_taker_access = Some(true);

        let rendered = serde_json::to_value(order).unwrap();
        assert_eq!(rendered["isElpTakerAccess"], true);
        assert!(!rendered.as_object().unwrap().contains_key("isElpTaker"));
    }
}
