//! Grid, recurring-buy, and copy-trading operations.
use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
};
use std::collections::BTreeMap;
/// Native strategy-product API.
pub struct StrategyService<'a>(pub(crate) &'a OkxClient);
impl StrategyService<'_> {
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
    pub async fn grid_contracts(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/contracts", q).await
    }
    pub async fn grid_min_investment(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/min-investment", q).await
    }
    pub async fn grid_order(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/grid/order-algo", b).await
    }
    pub async fn amend_grid_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/grid/amend-order-algo", b)
            .await
    }
    pub async fn stop_grid_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/grid/stop-order-algo", b)
            .await
    }
    pub async fn grid_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/orders-algo-pending", q)
            .await
    }
    pub async fn grid_order_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/orders-algo-history", q)
            .await
    }
    pub async fn grid_sub_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/sub-orders", q).await
    }
    pub async fn recurring_buy_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/recurring/order-algo", b)
            .await
    }
    pub async fn recurring_buy_amend(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/recurring/amend-order-algo", b)
            .await
    }
    pub async fn recurring_buy_stop(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/recurring/stop-order-algo", b)
            .await
    }
    pub async fn recurring_buy_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/recurring/orders-algo-pending", q)
            .await
    }
    pub async fn copy_traders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/copytrading/public-lead-traders", q).await
    }
    pub async fn copy_lead_positions(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/copytrading/current-subpositions", q)
            .await
    }
    pub async fn copy_settings(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/copytrading/config", q).await
    }
    pub async fn set_copy_settings(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/copytrading/config", b).await
    }
    pub async fn stop_copy(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/copytrading/stop-copy-trading", b).await
    }
}
