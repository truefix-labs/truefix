//! Professional trading, conversion, broker, and status operations.
use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
};
use std::collections::BTreeMap;
/// Native API for RFQ, spread trading, trading data, conversion, brokers, and status.
pub struct ProfessionalService<'a>(pub(crate) &'a OkxClient);
impl ProfessionalService<'_> {
    async fn get(&self, p: &str, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                p,
                q,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    async fn public_get(
        &self,
        p: &str,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                p,
                q,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    async fn write(&self, p: &str, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::POST,
                p,
                BTreeMap::new(),
                Some(b),
                RetrySafety::NeverReplay,
                true,
            )?)
            .await
    }
    /// Creates an RFQ.
    pub async fn create_rfq(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/create-rfq", b).await
    }
    /// Executes the `cancel_rfq` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_rfq(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/cancel-rfq", b).await
    }
    /// Executes the `rfqs` OKX V5 operation with its classified auth and replay policy.
    pub async fn rfqs(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rfq/rfqs", q).await
    }
    /// Executes the `create_quote` OKX V5 operation with its classified auth and replay policy.
    pub async fn create_quote(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/create-quote", b).await
    }
    /// Executes the `cancel_quote` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_quote(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/cancel-quote", b).await
    }
    /// Executes the `quotes` OKX V5 operation with its classified auth and replay policy.
    pub async fn quotes(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rfq/quotes", q).await
    }
    /// Executes the `execute_quote` OKX V5 operation with its classified auth and replay policy.
    pub async fn execute_quote(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/execute-quote", b).await
    }
    /// Executes the `rfq_trades` OKX V5 operation with its classified auth and replay policy.
    pub async fn rfq_trades(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rfq/trades", q).await
    }
    /// Executes the `reset_mmp` OKX V5 operation with its classified auth and replay policy.
    pub async fn reset_mmp(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/mmp-reset", b).await
    }
    /// Executes the `place_spread_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn place_spread_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/sprd/order", b).await
    }
    /// Executes the `cancel_spread_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_spread_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/sprd/cancel-order", b).await
    }
    /// Executes the `spread_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn spread_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/sprd/orders-pending", q).await
    }
    /// Returns historical spread orders.
    pub async fn spread_order_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/sprd/orders-history", q).await
    }
    /// Executes the `spread_trades` OKX V5 operation with its classified auth and replay policy.
    pub async fn spread_trades(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/sprd/trades", q).await
    }
    /// Executes the `support_coin` OKX V5 operation with its classified auth and replay policy.
    pub async fn support_coin(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get(
            "/api/v5/rubik/stat/trading-data/support-coin",
            BTreeMap::new(),
        )
        .await
    }
    /// Executes the `taker_volume` OKX V5 operation with its classified auth and replay policy.
    pub async fn taker_volume(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/taker-volume", q).await
    }
    /// Executes the `long_short_ratio` OKX V5 operation with its classified auth and replay policy.
    pub async fn long_short_ratio(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/contracts/long-short-account-ratio", q)
            .await
    }
    /// Executes the `convert_currencies` OKX V5 operation with its classified auth and replay policy.
    pub async fn convert_currencies(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/convert/currencies", BTreeMap::new())
            .await
    }
    /// Executes the `convert_quote` OKX V5 operation with its classified auth and replay policy.
    pub async fn convert_quote(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/convert/estimate-quote", b).await
    }
    /// Executes the `convert_trade` OKX V5 operation with its classified auth and replay policy.
    pub async fn convert_trade(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/convert/trade", b).await
    }
    /// Executes the `convert_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn convert_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/convert/history", q).await
    }
    /// Executes the `broker_rebate_per_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn broker_rebate_per_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/broker/fd/rebate-per-orders", q).await
    }
    /// Generates a rebate-details download link.
    pub async fn generate_rebate_details_download_link(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/broker/fd/rebate-per-orders", b).await
    }
    /// Returns exchange/platform status; this endpoint is public.
    pub async fn status(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/system/status",
                q,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                false,
            )?)
            .await
    }
    /// Executes the `counterparties` OKX V5 operation with its classified auth and replay policy.
    pub async fn counterparties(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rfq/counterparties", BTreeMap::new())
            .await
    }
    /// Executes the `cancel_batch_rfqs` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_batch_rfqs(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/cancel-batch-rfqs", b).await
    }
    /// Executes the `cancel_all_rfqs` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_all_rfqs(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/cancel-all-rfqs", b).await
    }
    /// Executes the `cancel_batch_quotes` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_batch_quotes(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/cancel-batch-quotes", b).await
    }
    /// Executes the `cancel_all_quotes` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_all_quotes(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/cancel-all-quotes", b).await
    }
    /// Executes the `public_rfq_trades` OKX V5 operation with its classified auth and replay policy.
    pub async fn public_rfq_trades(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rfq/public-trades", q).await
    }
    /// Executes the `maker_instrument_settings` OKX V5 operation with its classified auth and replay policy.
    pub async fn maker_instrument_settings(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rfq/maker-instrument-settings", q).await
    }
    /// Executes the `set_maker_instrument_settings` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_maker_instrument_settings(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/maker-instrument-settings", b).await
    }
    /// Executes the `cancel_all_spread_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_all_spread_orders(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/sprd/mass-cancel", b).await
    }
    /// Executes the `spread_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn spread_order(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/sprd/order", q).await
    }
    /// Executes the `spreads` OKX V5 operation with its classified auth and replay policy.
    pub async fn spreads(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/sprd/spreads", q).await
    }
    /// Executes the `spread_books` OKX V5 operation with its classified auth and replay policy.
    pub async fn spread_books(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/sprd/books", q).await
    }
    /// Executes the `spread_ticker` OKX V5 operation with its classified auth and replay policy.
    pub async fn spread_ticker(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/sprd/ticker", q).await
    }
    /// Executes the `spread_public_trades` OKX V5 operation with its classified auth and replay policy.
    pub async fn spread_public_trades(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/sprd/public-trades", q).await
    }
    /// Executes the `margin_lending_ratio` OKX V5 operation with its classified auth and replay policy.
    pub async fn margin_lending_ratio(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/margin/loan-ratio", q)
            .await
    }
    /// Executes the `contracts_interest_volume` OKX V5 operation with its classified auth and replay policy.
    pub async fn contracts_interest_volume(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/contracts/open-interest-volume", q)
            .await
    }
    /// Executes the `options_interest_volume` OKX V5 operation with its classified auth and replay policy.
    pub async fn options_interest_volume(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/option/open-interest-volume", q)
            .await
    }
    /// Executes the `put_call_ratio` OKX V5 operation with its classified auth and replay policy.
    pub async fn put_call_ratio(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/option/open-interest-volume-ratio", q)
            .await
    }
    /// Executes the `interest_volume_expiry` OKX V5 operation with its classified auth and replay policy.
    pub async fn interest_volume_expiry(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/option/open-interest-volume-expiry", q)
            .await
    }
    /// Executes the `interest_volume_strike` OKX V5 operation with its classified auth and replay policy.
    pub async fn interest_volume_strike(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/option/open-interest-volume-strike", q)
            .await
    }
    /// Executes the `taker_block_volume` OKX V5 operation with its classified auth and replay policy.
    pub async fn taker_block_volume(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/option/taker-block-volume", q)
            .await
    }
    /// Executes the `open_interest_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn open_interest_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.public_get("/api/v5/rubik/stat/contracts/open-interest-history", q)
            .await
    }
    /// Executes the `convert_currency_pair` OKX V5 operation with its classified auth and replay policy.
    pub async fn convert_currency_pair(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/convert/currency-pair", q).await
    }
}
