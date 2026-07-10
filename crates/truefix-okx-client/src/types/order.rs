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
}
/// Batch result preserving successful and failed item acknowledgements.
pub type BatchResult = Vec<OrderAck>;
