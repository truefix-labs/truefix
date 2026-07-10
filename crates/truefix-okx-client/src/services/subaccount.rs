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
    /// Executes the `list` OKX V5 operation with its classified auth and replay policy.
    pub async fn list(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/users/subaccount/list", q).await
    }
    /// Executes the `balances` OKX V5 operation with its classified auth and replay policy.
    pub async fn balances(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/account/subaccount/balances", q).await
    }
    /// Executes the `bills` OKX V5 operation with its classified auth and replay policy.
    pub async fn bills(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/subaccount/bills", q).await
    }
    /// Executes the `transfer` OKX V5 operation with its classified auth and replay policy.
    pub async fn transfer(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/asset/subaccount/transfer", b).await
    }
    /// Executes the `set_transfer_out` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_transfer_out(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/users/subaccount/set-transfer-out", b)
            .await
    }
    /// Executes the `api_keys` OKX V5 operation with its classified auth and replay policy.
    pub async fn api_keys(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/users/subaccount/apikey", q).await
    }
    /// Executes the `create_api_key` OKX V5 operation with its classified auth and replay policy.
    pub async fn create_api_key(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/users/subaccount/apikey", b).await
    }
    /// Executes the `modify_api_key` OKX V5 operation with its classified auth and replay policy.
    pub async fn modify_api_key(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/users/subaccount/modify-apikey", b)
            .await
    }
    /// Executes the `loaned` OKX V5 operation with its classified auth and replay policy.
    pub async fn loaned(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/account/subaccount/loaned", q).await
    }
    /// Executes the `interest_limits` OKX V5 operation with its classified auth and replay policy.
    pub async fn interest_limits(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/account/subaccount/interest-limits", q)
            .await
    }
    /// Executes the `funding_balance` OKX V5 operation with its classified auth and replay policy.
    pub async fn funding_balance(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/asset/subaccount/balances", q).await
    }
    /// Executes the `entrust_list` OKX V5 operation with its classified auth and replay policy.
    pub async fn entrust_list(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/users/entrust-subaccount-list", q).await
    }
    /// Executes the `set_loan_allocation` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_loan_allocation(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/account/subaccount/set-loan-allocation", b)
            .await
    }
}
