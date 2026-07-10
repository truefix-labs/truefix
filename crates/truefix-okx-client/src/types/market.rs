use crate::types::{
    common::DecimalValue,
    gateway::{GatewayProject, GatewayTicker, NativeFields},
};
/// Last-price ticker.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Ticker {
    #[serde(rename = "instId")]
    pub instrument_id: String,
    #[serde(rename = "last")]
    pub last: DecimalValue,
    #[serde(rename = "vol24h")]
    pub volume_24h: Option<DecimalValue>,
    /// OKX fields outside the core ticker representation.
    #[serde(flatten)]
    pub native_fields: NativeFields,
}

impl GatewayProject for Ticker {
    type Projection = GatewayTicker;

    fn project_gateway(&self) -> Self::Projection {
        GatewayTicker {
            instrument_id: self.instrument_id.clone(),
            last: self.last,
            volume_24h: self.volume_24h,
            native_fields: self.native_fields.clone(),
        }
    }
}
/// Order book level.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BookLevel(
    pub DecimalValue,
    pub DecimalValue,
    pub DecimalValue,
    pub DecimalValue,
);
/// Market order book snapshot.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OrderBook {
    #[serde(rename = "instId")]
    pub instrument_id: String,
    pub asks: Vec<BookLevel>,
    pub bids: Vec<BookLevel>,
    #[serde(rename = "ts")]
    pub timestamp: String,
}
/// Candle represented with exact values and exchange timestamp.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Candle(
    pub String,
    pub DecimalValue,
    pub DecimalValue,
    pub DecimalValue,
    pub DecimalValue,
    pub DecimalValue,
);
/// Public trade record.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MarketTrade {
    #[serde(rename = "tradeId")]
    pub trade_id: String,
    #[serde(rename = "instId")]
    pub instrument_id: String,
    pub px: DecimalValue,
    pub sz: DecimalValue,
    pub side: String,
    pub ts: String,
}
/// Index or mark-price ticker.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PriceIndex {
    #[serde(rename = "instId")]
    pub instrument_id: String,
    #[serde(rename = "idxPx")]
    pub index_price: Option<DecimalValue>,
    #[serde(rename = "markPx")]
    pub mark_price: Option<DecimalValue>,
}
/// Current or historical funding rate.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FundingRate {
    #[serde(rename = "instId")]
    pub instrument_id: String,
    #[serde(rename = "fundingRate")]
    pub rate: DecimalValue,
    #[serde(rename = "fundingTime")]
    pub funding_time: String,
}
