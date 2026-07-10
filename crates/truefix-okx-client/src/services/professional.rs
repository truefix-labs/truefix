//! Professional trading, conversion, broker, and platform-status operations.
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
    pub async fn cancel_rfq(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/cancel-rfq", b).await
    }
    pub async fn rfqs(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rfq/rfqs", q).await
    }
    pub async fn create_quote(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/create-quote", b).await
    }
    pub async fn cancel_quote(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/cancel-quote", b).await
    }
    pub async fn quotes(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rfq/quotes", q).await
    }
    pub async fn execute_quote(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/execute-quote", b).await
    }
    pub async fn rfq_trades(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rfq/trades", q).await
    }
    pub async fn mmp_config(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rfq/mmp-config", q).await
    }
    pub async fn set_mmp_config(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/mmp-config", b).await
    }
    pub async fn reset_mmp(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/rfq/mmp-reset", b).await
    }
    pub async fn spread_instruments(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/sprd/instruments", q).await
    }
    pub async fn place_spread_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/sprd/order", b).await
    }
    pub async fn cancel_spread_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/sprd/cancel-order", b).await
    }
    pub async fn amend_spread_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/sprd/amend-order", b).await
    }
    pub async fn spread_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/sprd/orders-pending", q).await
    }
    pub async fn spread_order_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/sprd/orders-history", q).await
    }
    pub async fn spread_trades(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/sprd/trades", q).await
    }
    pub async fn support_coin(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get(
            "/api/v5/rubik/stat/trading-data/support-coin",
            BTreeMap::new(),
        )
        .await
    }
    pub async fn taker_volume(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rubik/stat/taker-volume", q).await
    }
    pub async fn long_short_ratio(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/rubik/stat/contracts/long-short-account-ratio", q)
            .await
    }
    pub async fn convert_currencies(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/convert/currencies", BTreeMap::new())
            .await
    }
    pub async fn convert_quote(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/convert/estimate-quote", b).await
    }
    pub async fn convert_trade(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/convert/trade", b).await
    }
    pub async fn convert_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/convert/history", q).await
    }
    pub async fn broker_rebate_per_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/broker/nd/rebate-per-orders", q).await
    }
    pub async fn broker_rebate_daily(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/broker/nd/rebate-daily", q).await
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
}
