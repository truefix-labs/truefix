//! Funding-account and asset-movement operations.
use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
};
use std::collections::BTreeMap;
/// Operations for the OKX funding account.
pub struct FundingService<'a>(pub(crate) &'a OkxClient);
impl FundingService<'_> {
    async fn get(
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
                true,
            )?)
            .await
    }
    async fn write(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::POST,
                path,
                BTreeMap::new(),
                Some(body),
                RetrySafety::NeverReplay,
                true,
            )?)
            .await
    }
    pub async fn balances(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/balances", q).await
    }
    pub async fn deposit_addresses(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/deposit-address", q).await
    }
    pub async fn deposit_currencies(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/deposit/currencies", q).await
    }
    pub async fn deposits(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/deposit-history", q).await
    }
    pub async fn withdraw(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/withdrawal", b).await
    }
    pub async fn cancel_withdrawal(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/cancel-withdrawal", b).await
    }
    pub async fn withdrawals(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/withdrawal-history", q).await
    }
    pub async fn transfer(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/transfer", b).await
    }
    pub async fn transfer_state(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/transfer-state", q).await
    }
    pub async fn lightning(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/lightning", b).await
    }
    pub async fn convert_dust(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/convert-dust-assets", b).await
    }
    pub async fn dust_assets(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/convert-dust-assets", BTreeMap::new())
            .await
    }
    pub async fn asset_valuation(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/asset-valuation", q).await
    }
    pub async fn bills(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/bills", q).await
    }
}
