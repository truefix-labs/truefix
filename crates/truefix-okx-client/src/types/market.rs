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
    /// The REST books endpoint omits `instId`; the market service restores the
    /// requested identity when the provider does not include it in the payload.
    #[serde(rename = "instId", default)]
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
    pub DecimalValue,
    pub DecimalValue,
    pub String,
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

#[cfg(test)]
mod tests {
    use super::{Candle, OrderBook};

    #[test]
    fn candle_deserializes_the_complete_okx_wire_tuple() {
        let candle: Candle = serde_json::from_str(
            r#"["1697025780000","27000.5","27001.0","26999.8","27000.2","15","405003","405003","1"]"#,
        )
        .unwrap();

        assert_eq!(candle.0, "1697025780000");
        assert_eq!(candle.5.to_string(), "15");
        assert_eq!(candle.6.to_string(), "405003");
        assert_eq!(candle.7.to_string(), "405003");
        assert_eq!(candle.8, "1");
    }

    #[test]
    fn order_book_accepts_the_rest_payload_without_an_instrument_id() {
        let book: OrderBook = serde_json::from_str(
            r#"{"asks":[["2","1","0","1"]],"bids":[["1","1","0","1"]],"ts":"1"}"#,
        )
        .unwrap();
        assert!(book.instrument_id.is_empty());
    }
}
