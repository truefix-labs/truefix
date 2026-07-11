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
    /// Executes the `grid_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_order(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/grid/order-algo", b).await
    }
    /// Executes the `amend_grid_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn amend_grid_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/grid/amend-order-algo", b)
            .await
    }
    /// Executes the `stop_grid_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn stop_grid_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/grid/stop-order-algo", b)
            .await
    }
    /// Executes the `grid_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/orders-algo-pending", q)
            .await
    }
    /// Executes the `grid_order_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_order_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/orders-algo-history", q)
            .await
    }
    /// Executes the `grid_sub_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_sub_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/sub-orders", q).await
    }
    /// Executes the `recurring_buy_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn recurring_buy_order(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/recurring/order-algo", b)
            .await
    }
    /// Executes the `recurring_buy_amend` OKX V5 operation with its classified auth and replay policy.
    pub async fn recurring_buy_amend(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/recurring/amend-order-algo", b)
            .await
    }
    /// Executes the `recurring_buy_stop` OKX V5 operation with its classified auth and replay policy.
    pub async fn recurring_buy_stop(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/recurring/stop-order-algo", b)
            .await
    }
    /// Executes the `recurring_buy_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn recurring_buy_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/recurring/orders-algo-pending", q)
            .await
    }
    /// Executes the `grid_order_details` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_order_details(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/orders-algo-details", q)
            .await
    }
    /// Executes the `grid_positions` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_positions(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/positions", q).await
    }
    /// Executes the `grid_withdraw_income` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_withdraw_income(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/grid/withdraw-income", b)
            .await
    }
    /// Executes the `grid_compute_margin` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_compute_margin(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/grid/compute-margin-balance", b)
            .await
    }
    /// Executes the `grid_adjust_margin` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_adjust_margin(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/tradingBot/grid/margin-balance", b)
            .await
    }
    /// Executes the `grid_ai_param` OKX V5 operation with its classified auth and replay policy.
    pub async fn grid_ai_param(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/grid/ai-param", q).await
    }
    /// Executes the `recurring_details` OKX V5 operation with its classified auth and replay policy.
    pub async fn recurring_details(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/recurring/orders-algo-details", q)
            .await
    }
    /// Executes the `recurring_sub_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn recurring_sub_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/recurring/sub-orders", q).await
    }
    /// Executes the `recurring_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn recurring_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/tradingBot/recurring/orders-algo-history", q)
            .await
    }
    /// Executes the `leading_positions` OKX V5 operation with its classified auth and replay policy.
    pub async fn leading_positions(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/copytrading/current-subpositions", q)
            .await
    }
    /// Returns historical lead-trading subpositions.
    pub async fn leading_position_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/copytrading/subpositions-history", q)
            .await
    }
    /// Executes the `leading_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn leading_order(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/copytrading/algo-order", b).await
    }
    /// Executes the `close_leading_position` OKX V5 operation with its classified auth and replay policy.
    pub async fn close_leading_position(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/copytrading/close-subposition", b).await
    }
    /// Executes the `leading_instruments` OKX V5 operation with its classified auth and replay policy.
    pub async fn leading_instruments(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/copytrading/instruments", q).await
    }
    /// Executes the `set_leading_instruments` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_leading_instruments(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/copytrading/set-instruments", b).await
    }
    /// Returns realized profit-sharing details for lead trading.
    pub async fn profit_sharing_details(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/copytrading/profit-sharing-details", q)
            .await
    }
    /// Returns total lead-trading profit sharing.
    pub async fn total_profit_sharing(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/copytrading/total-profit-sharing", q)
            .await
    }
    /// Returns unrealized profit-sharing details for lead trading.
    pub async fn unrealized_profit_sharing_details(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/copytrading/unrealized-profit-sharing-details", q)
            .await
    }
}
