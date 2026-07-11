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
    /// Executes the `balances` OKX V5 operation with its classified auth and replay policy.
    pub async fn balances(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/balances", q).await
    }
    /// Executes the `deposit_addresses` OKX V5 operation with its classified auth and replay policy.
    pub async fn deposit_addresses(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/deposit-address", q).await
    }
    /// Executes the `deposits` OKX V5 operation with its classified auth and replay policy.
    pub async fn deposits(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/deposit-history", q).await
    }
    /// Executes the `withdraw` OKX V5 operation with its classified auth and replay policy.
    pub async fn withdraw(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/withdrawal", b).await
    }
    /// Executes the `cancel_withdrawal` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_withdrawal(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/cancel-withdrawal", b).await
    }
    /// Executes the `withdrawals` OKX V5 operation with its classified auth and replay policy.
    pub async fn withdrawals(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/withdrawal-history", q).await
    }
    /// Executes the `transfer` OKX V5 operation with its classified auth and replay policy.
    pub async fn transfer(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/transfer", b).await
    }
    /// Executes the `transfer_state` OKX V5 operation with its classified auth and replay policy.
    pub async fn transfer_state(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/transfer-state", q).await
    }
    /// Executes the `lightning` OKX V5 operation with its classified auth and replay policy.
    pub async fn lightning(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/withdrawal-lightning", b).await
    }
    /// Executes the `convert_dust` OKX V5 operation with its classified auth and replay policy.
    pub async fn convert_dust(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/convert-dust-assets", b).await
    }
    /// Executes the `asset_valuation` OKX V5 operation with its classified auth and replay policy.
    pub async fn asset_valuation(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/asset-valuation", q).await
    }
    /// Executes the `bills` OKX V5 operation with its classified auth and replay policy.
    pub async fn bills(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/bills", q).await
    }
    /// Executes the `non_tradable_assets` OKX V5 operation with its classified auth and replay policy.
    pub async fn non_tradable_assets(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/non-tradable-assets", q).await
    }
    /// Executes the `currencies` OKX V5 operation with its classified auth and replay policy.
    pub async fn currencies(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/currencies", q).await
    }
    /// Executes the `purchase_redempt` OKX V5 operation with its classified auth and replay policy.
    pub async fn purchase_redempt(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/purchase_redempt", b).await
    }
    /// Executes the `deposit_lightning` OKX V5 operation with its classified auth and replay policy.
    pub async fn deposit_lightning(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/deposit-lightning", q).await
    }
    /// Executes the `withdrawal_lightning` OKX V5 operation with its classified auth and replay policy.
    pub async fn withdrawal_lightning(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/withdrawal-lightning", b).await
    }
    /// Executes the `deposit_withdraw_status` OKX V5 operation with its classified auth and replay policy.
    pub async fn deposit_withdraw_status(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/deposit-withdraw-status", q).await
    }
}
