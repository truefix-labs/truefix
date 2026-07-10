use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
    types::market::{Candle, FundingRate, MarketTrade, OrderBook, PriceIndex, Ticker},
};
use std::collections::BTreeMap;
pub struct MarketService<'a>(pub(crate) &'a OkxClient);
impl MarketService<'_> {
    pub async fn ticker(&self, instrument_id: &str) -> OkxResult<Vec<Ticker>> {
        let mut q = BTreeMap::new();
        q.insert("instId".into(), instrument_id.into());
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/market/ticker",
                q,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Lists tickers for an instrument type.
    pub async fn tickers(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Ticker>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/market/tickers",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Gets an order-book snapshot.
    pub async fn books(&self, instrument_id: &str, size: Option<u16>) -> OkxResult<Vec<OrderBook>> {
        let mut query = BTreeMap::new();
        query.insert("instId".to_owned(), instrument_id.to_owned());
        if let Some(size) = size {
            query.insert("sz".to_owned(), size.to_string());
        }
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/market/books",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Lists candles with opaque server cursor filters.
    pub async fn candles(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Candle>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/market/candles",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Lists historical candles with opaque server cursor filters.
    pub async fn history_candles(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Candle>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/market/history-candles",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Lists recent public trades.
    pub async fn trades(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<MarketTrade>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/market/trades",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Gets index ticker values.
    pub async fn index_tickers(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<PriceIndex>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/market/index-tickers",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Gets mark-price values.
    pub async fn mark_price(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<PriceIndex>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/public/mark-price",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Gets current funding rates.
    pub async fn funding_rate(&self, instrument_id: &str) -> OkxResult<Vec<FundingRate>> {
        let mut query = BTreeMap::new();
        query.insert("instId".to_owned(), instrument_id.to_owned());
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/public/funding-rate",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Lists funding-rate history with opaque server cursor filters.
    pub async fn funding_rate_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<FundingRate>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/public/funding-rate-history",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
}
