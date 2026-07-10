use crate::types::{
    common::DecimalValue,
    gateway::{GatewayBalance, GatewayPosition, GatewayProject, NativeFields},
};

/// Account balance with native availability and equity fields.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Balance {
    #[serde(rename = "ccy")]
    pub currency: String,
    #[serde(rename = "availBal")]
    pub available: Option<DecimalValue>,
    #[serde(rename = "eq")]
    pub equity: Option<DecimalValue>,
    #[serde(rename = "frozenBal")]
    pub frozen: Option<DecimalValue>,
    /// OKX fields outside the core balance representation.
    #[serde(flatten)]
    pub native_fields: NativeFields,
}
/// Open position returned by the account service.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Position {
    #[serde(rename = "instId")]
    pub instrument_id: String,
    #[serde(rename = "pos")]
    pub quantity: DecimalValue,
    #[serde(rename = "avgPx")]
    pub average_price: Option<DecimalValue>,
    #[serde(rename = "upl")]
    pub unrealized_pnl: Option<DecimalValue>,
    /// OKX fields outside the core position representation.
    #[serde(flatten)]
    pub native_fields: NativeFields,
}

impl GatewayProject for Balance {
    type Projection = GatewayBalance;

    fn project_gateway(&self) -> Self::Projection {
        GatewayBalance {
            currency: self.currency.clone(),
            available: self.available,
            equity: self.equity,
            native_fields: self.native_fields.clone(),
        }
    }
}

impl GatewayProject for Position {
    type Projection = GatewayPosition;

    fn project_gateway(&self) -> Self::Projection {
        GatewayPosition {
            instrument_id: self.instrument_id.clone(),
            quantity: self.quantity,
            average_price: self.average_price,
            unrealized_pnl: self.unrealized_pnl,
            native_fields: self.native_fields.clone(),
        }
    }
}
/// Account bill preserving the exchange's accounting classification.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Bill {
    #[serde(rename = "billId")]
    pub bill_id: String,
    #[serde(rename = "instId")]
    pub instrument_id: Option<String>,
    #[serde(rename = "balChg")]
    pub balance_change: Option<DecimalValue>,
    #[serde(rename = "ts")]
    pub timestamp: String,
}
/// Current leverage for an instrument or currency scope.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Leverage {
    pub lever: DecimalValue,
    #[serde(rename = "mgnMode")]
    pub margin_mode: String,
    #[serde(rename = "instId")]
    pub instrument_id: Option<String>,
}
/// Margin adjustment acknowledgement.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MarginAdjustment {
    #[serde(rename = "amt")]
    pub amount: DecimalValue,
    #[serde(rename = "instId")]
    pub instrument_id: String,
}
/// Position-risk record supplied by OKX.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PositionRisk {
    #[serde(rename = "instId")]
    pub instrument_id: Option<String>,
    #[serde(rename = "liquidationPx")]
    pub liquidation_price: Option<DecimalValue>,
    #[serde(rename = "mgnRatio")]
    pub margin_ratio: Option<DecimalValue>,
}
/// Fee schedule for a product and account level.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FeeRate {
    #[serde(rename = "instType")]
    pub instrument_type: String,
    #[serde(rename = "maker")]
    pub maker: DecimalValue,
    #[serde(rename = "taker")]
    pub taker: DecimalValue,
}
