//! Public reference-data endpoints. These calls never require credentials.

use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
};
use std::collections::BTreeMap;

/// Access to OKX public instruments, reference data, and platform status.
pub struct PublicDataService<'a>(pub(crate) &'a OkxClient);

impl PublicDataService<'_> {
    async fn get(
        &self,
        path: &'static str,
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

    /// Lists instruments for an `instType` and optional filters.
    pub async fn instruments(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/instruments", query).await
    }
    /// Executes the `delivery_exercise_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn delivery_exercise_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/delivery-exercise-history", query)
            .await
    }
    /// Executes the `open_interest` OKX V5 operation with its classified auth and replay policy.
    pub async fn open_interest(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/open-interest", query).await
    }
    /// Executes the `price_limit` OKX V5 operation with its classified auth and replay policy.
    pub async fn price_limit(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/price-limit", query).await
    }
    /// Executes the `option_summary` OKX V5 operation with its classified auth and replay policy.
    pub async fn option_summary(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/opt-summary", query).await
    }
    /// Executes the `estimated_price` OKX V5 operation with its classified auth and replay policy.
    pub async fn estimated_price(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/estimated-price", query).await
    }
    /// Executes the `interest_free_quota` OKX V5 operation with its classified auth and replay policy.
    pub async fn interest_free_quota(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/discount-rate-interest-free-quota", query)
            .await
    }
    /// Executes the `time` OKX V5 operation with its classified auth and replay policy.
    pub async fn time(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/time", BTreeMap::new()).await
    }
    /// Executes the `insurance_fund` OKX V5 operation with its classified auth and replay policy.
    pub async fn insurance_fund(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/insurance-fund", query).await
    }
    /// Executes the `contract_coin` OKX V5 operation with its classified auth and replay policy.
    pub async fn contract_coin(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/convert-contract-coin", query)
            .await
    }
    /// Executes the `instrument_tick_bands` OKX V5 operation with its classified auth and replay policy.
    pub async fn instrument_tick_bands(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/instrument-tick-bands", query)
            .await
    }
    /// Executes the `option_trades` OKX V5 operation with its classified auth and replay policy.
    pub async fn option_trades(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/option-trades", query).await
    }
    /// Executes the `announcements` OKX V5 operation with its classified auth and replay policy.
    pub async fn announcements(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/support/announcements", query).await
    }
    /// Executes the `platform_status` OKX V5 operation with its classified auth and replay policy.
    pub async fn platform_status(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/system/status", query).await
    }
    /// Executes the `position_tiers` OKX V5 operation with its classified auth and replay policy.
    pub async fn position_tiers(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/position-tiers", q).await
    }
    /// Executes the `interest_rate_loan_quota` OKX V5 operation with its classified auth and replay policy.
    pub async fn interest_rate_loan_quota(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/interest-rate-loan-quota", BTreeMap::new())
            .await
    }
    /// Executes the `vip_interest_rate_loan_quota` OKX V5 operation with its classified auth and replay policy.
    pub async fn vip_interest_rate_loan_quota(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get(
            "/api/v5/public/vip-interest-rate-loan-quota",
            BTreeMap::new(),
        )
        .await
    }
    /// Executes the `underlying` OKX V5 operation with its classified auth and replay policy.
    pub async fn underlying(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/underlying", q).await
    }
    /// Executes the `market_data_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn market_data_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/market-data-history", q).await
    }
    /// Executes the `liquidation_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn liquidation_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/liquidation-orders", q).await
    }
}
