use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
    types::market::{Candle, FundingRate, MarketTrade, OrderBook, PriceIndex, Ticker},
};
use std::collections::BTreeMap;
pub struct MarketService<'a>(pub(crate) &'a OkxClient);
impl MarketService<'_> {
    async fn read_json(
        &self,
        path: &str,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                path,
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Executes the `ticker` OKX V5 operation with its classified auth and replay policy.
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
        let mut books: Vec<OrderBook> = self
            .0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/market/books",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await?;
        for book in &mut books {
            if book.instrument_id.is_empty() {
                book.instrument_id = instrument_id.to_owned();
            }
        }
        Ok(books)
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
    /// Executes the `index_candles` OKX V5 operation with its classified auth and replay policy.
    pub async fn index_candles(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/index-candles", q).await
    }
    /// Executes the `mark_price_candles` OKX V5 operation with its classified auth and replay policy.
    pub async fn mark_price_candles(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/mark-price-candles", q).await
    }
    /// Executes the `platform_volume` OKX V5 operation with its classified auth and replay policy.
    pub async fn platform_volume(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/platform-24-volume", BTreeMap::new())
            .await
    }
    /// Executes the `index_components` OKX V5 operation with its classified auth and replay policy.
    pub async fn index_components(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/index-components", q).await
    }
    /// Executes the `exchange_rate` OKX V5 operation with its classified auth and replay policy.
    pub async fn exchange_rate(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/exchange-rate", BTreeMap::new())
            .await
    }
    /// Executes the `history_trades` OKX V5 operation with its classified auth and replay policy.
    pub async fn history_trades(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/history-trades", q).await
    }
    /// Executes the `block_ticker` OKX V5 operation with its classified auth and replay policy.
    pub async fn block_ticker(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/block-ticker", q).await
    }
    /// Executes the `block_tickers` OKX V5 operation with its classified auth and replay policy.
    pub async fn block_tickers(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/block-tickers", q).await
    }
    /// Executes the `block_trades` OKX V5 operation with its classified auth and replay policy.
    pub async fn block_trades(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/block-trades", q).await
    }
    /// Executes the `books_lite` OKX V5 operation with its classified auth and replay policy.
    pub async fn books_lite(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/books-lite", q).await
    }
    /// Executes the `option_family_trades` OKX V5 operation with its classified auth and replay policy.
    pub async fn option_family_trades(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/market/option/instrument-family-trades", q)
            .await
    }
}
