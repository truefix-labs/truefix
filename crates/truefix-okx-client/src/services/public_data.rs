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
    pub async fn delivery_exercise_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/delivery-exercise-history", query)
            .await
    }
    pub async fn open_interest(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/open-interest", query).await
    }
    pub async fn price_limit(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/price-limit", query).await
    }
    pub async fn option_summary(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/opt-summary", query).await
    }
    pub async fn estimated_price(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/estimated-price", query).await
    }
    pub async fn interest_free_quota(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/discount-rate-interest-free-quota", query)
            .await
    }
    pub async fn time(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/time", BTreeMap::new()).await
    }
    pub async fn insurance_fund(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/insurance-fund", query).await
    }
    pub async fn contract_coin(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/convert-contract-coin", query)
            .await
    }
    pub async fn instrument_tick_bands(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/instrument-tick-bands", query)
            .await
    }
    pub async fn option_trades(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/option-trades", query).await
    }
    pub async fn economic_calendar(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/public/economic-calendar", query).await
    }
    pub async fn announcements(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/support/announcements", query).await
    }
    pub async fn platform_status(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/system/status", query).await
    }
}
