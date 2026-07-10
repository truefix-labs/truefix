//! Sub-account operations.
use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
};
use std::collections::BTreeMap;
/// Operations for administrative sub-accounts.
pub struct SubaccountService<'a>(pub(crate) &'a OkxClient);
impl SubaccountService<'_> {
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
    pub async fn list(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/users/subaccount/list", q).await
    }
    pub async fn balances(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/account/subaccount/balances", q).await
    }
    pub async fn bills(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/subaccount/bills", q).await
    }
    pub async fn transfer(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/subaccount/transfer", b).await
    }
    pub async fn set_transfer_out(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/users/subaccount/set-transfer-out", b)
            .await
    }
    pub async fn api_keys(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/users/subaccount/apikey", q).await
    }
    pub async fn create_api_key(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/users/subaccount/apikey", b).await
    }
    pub async fn modify_api_key(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/users/subaccount/modify-apikey", b)
            .await
    }
    pub async fn loaned(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/account/subaccount/loaned", q).await
    }
    pub async fn interest_limits(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/account/subaccount/interest-limits", q)
            .await
    }
}
