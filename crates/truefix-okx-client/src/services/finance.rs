//! Savings, staking, loans, and investment-product operations.
use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
};
use std::collections::BTreeMap;
/// Native finance-product API grouped by product family.
pub struct FinanceService<'a>(pub(crate) &'a OkxClient);
impl FinanceService<'_> {
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
    /// Gets Simple Earn savings products.
    pub async fn savings_products(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/savings/lending-rate-summary", q)
            .await
    }
    pub async fn savings_purchase(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/savings/purchase-redempt", b)
            .await
    }
    pub async fn savings_balance(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/savings/balance", q).await
    }
    pub async fn savings_purchase_redempt_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/savings/purchase-redempt-history", q)
            .await
    }
    pub async fn savings_interest_accrued(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/savings/interest-accrued", q)
            .await
    }
    pub async fn eth_products(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/eth/product-info", q)
            .await
    }
    pub async fn eth_purchase(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/eth/purchase", b)
            .await
    }
    pub async fn eth_redeem(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/eth/redeem", b)
            .await
    }
    pub async fn sol_products(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/sol/product-info", q)
            .await
    }
    pub async fn sol_purchase(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/sol/purchase", b)
            .await
    }
    pub async fn defi_products(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/defi/offer-list", q)
            .await
    }
    pub async fn defi_purchase(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/defi/purchase", b)
            .await
    }
    pub async fn flexible_loan_currencies(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get(
            "/api/v5/finance/flexible-loan/loan-currency",
            BTreeMap::new(),
        )
        .await
    }
    pub async fn flexible_loan_borrow(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/flexible-loan/borrow", b).await
    }
    pub async fn flexible_loan_repay(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/flexible-loan/repay", b).await
    }
    pub async fn flexible_loan_positions(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/flexible-loan/position-list", q)
            .await
    }
    pub async fn dual_investments(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/dual-investment/products", q)
            .await
    }
    pub async fn dual_invest(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/dual-investment/purchase", b)
            .await
    }
    pub async fn dual_invest_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/dual-investment/orders", q)
            .await
    }
}
