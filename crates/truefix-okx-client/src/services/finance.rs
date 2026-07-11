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
    /// Gets the public Simple Earn lending-rate summary.
    pub async fn savings_public_borrow_info(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/savings/lending-rate-summary", q)
            .await
    }
    /// Executes the `savings_purchase` OKX V5 operation with its classified auth and replay policy.
    pub async fn savings_purchase(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/savings/purchase-redempt", b)
            .await
    }
    /// Executes the `savings_balance` OKX V5 operation with its classified auth and replay policy.
    pub async fn savings_balance(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/savings/balance", q).await
    }
    /// Executes the `eth_products` OKX V5 operation with its classified auth and replay policy.
    pub async fn eth_products(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/eth/product-info", q)
            .await
    }
    /// Executes the `eth_purchase` OKX V5 operation with its classified auth and replay policy.
    pub async fn eth_purchase(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/eth/purchase", b)
            .await
    }
    /// Executes the `eth_redeem` OKX V5 operation with its classified auth and replay policy.
    pub async fn eth_redeem(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/eth/redeem", b)
            .await
    }
    /// Executes the `sol_products` OKX V5 operation with its classified auth and replay policy.
    pub async fn sol_products(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/sol/product-info", q)
            .await
    }
    /// Executes the `sol_purchase` OKX V5 operation with its classified auth and replay policy.
    pub async fn sol_purchase(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/sol/purchase", b)
            .await
    }
    /// Executes the `defi_products` OKX V5 operation with its classified auth and replay policy.
    pub async fn defi_products(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/offers", q).await
    }
    /// Executes the `defi_purchase` OKX V5 operation with its classified auth and replay policy.
    pub async fn defi_purchase(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/purchase", b).await
    }
    /// Executes the `flexible_loan_currencies` OKX V5 operation with its classified auth and replay policy.
    pub async fn flexible_loan_currencies(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.get(
            "/api/v5/finance/flexible-loan/borrow-currencies",
            BTreeMap::new(),
        )
        .await
    }
    /// Returns assets accepted as flexible-loan collateral.
    pub async fn flexible_loan_collateral_assets(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/flexible-loan/collateral-assets", q)
            .await
    }
    /// Calculates the maximum flexible-loan amount for the supplied collateral.
    pub async fn flexible_loan_max_loan(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/flexible-loan/max-loan", b)
            .await
    }
    /// Returns the maximum collateral amount that can be redeemed.
    pub async fn flexible_loan_max_collateral_redeem(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get(
            "/api/v5/finance/flexible-loan/max-collateral-redeem-amount",
            q,
        )
        .await
    }
    /// Adjusts collateral for a flexible loan without replaying the write.
    pub async fn flexible_loan_adjust_collateral(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/flexible-loan/adjust-collateral", b)
            .await
    }
    /// Executes the `flexible_loan_positions` OKX V5 operation with its classified auth and replay policy.
    pub async fn flexible_loan_positions(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/flexible-loan/loan-info", q).await
    }
    /// Returns flexible-loan history.
    pub async fn flexible_loan_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/flexible-loan/loan-history", q)
            .await
    }
    /// Returns flexible-loan accrued interest.
    pub async fn flexible_loan_interest_accrued(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/flexible-loan/interest-accrued", q)
            .await
    }
    /// Executes the `dual_investments` OKX V5 operation with its classified auth and replay policy.
    pub async fn dual_investments(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/sfp/dcd/products", q).await
    }
    /// Executes the `dual_invest` OKX V5 operation with its classified auth and replay policy.
    pub async fn dual_invest(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/sfp/dcd/trade", b).await
    }
    /// Executes the `dual_invest_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn dual_invest_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/sfp/dcd/order-history", q).await
    }
    /// Executes the `eth_cancel_redeem` OKX V5 operation with its classified auth and replay policy.
    pub async fn eth_cancel_redeem(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/eth/cancel-redeem", b)
            .await
    }
    /// Executes the `eth_balance` OKX V5 operation with its classified auth and replay policy.
    pub async fn eth_balance(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/eth/balance", q)
            .await
    }
    /// Executes the `eth_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn eth_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get(
            "/api/v5/finance/staking-defi/eth/purchase-redeem-history",
            q,
        )
        .await
    }
    /// Executes the `eth_apy_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn eth_apy_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/eth/apy-history", q)
            .await
    }
    /// Executes the `sol_redeem` OKX V5 operation with its classified auth and replay policy.
    pub async fn sol_redeem(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/sol/redeem", b)
            .await
    }
    /// Executes the `sol_balance` OKX V5 operation with its classified auth and replay policy.
    pub async fn sol_balance(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/sol/balance", q)
            .await
    }
    /// Executes the `sol_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn sol_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get(
            "/api/v5/finance/staking-defi/sol/purchase-redeem-history",
            q,
        )
        .await
    }
    /// Executes the `sol_apy_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn sol_apy_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/sol/apy-history", q)
            .await
    }
    /// Executes the `defi_redeem` OKX V5 operation with its classified auth and replay policy.
    pub async fn defi_redeem(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/redeem", b).await
    }
    /// Executes the `defi_cancel` OKX V5 operation with its classified auth and replay policy.
    pub async fn defi_cancel(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/staking-defi/cancel", b).await
    }
    /// Executes the `defi_active_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn defi_active_orders(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/orders-active", q)
            .await
    }
    /// Executes the `defi_order_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn defi_order_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/staking-defi/orders-history", q)
            .await
    }
    /// Executes the `savings_set_lending_rate` OKX V5 operation with its classified auth and replay policy.
    pub async fn savings_set_lending_rate(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/savings/set-lending-rate", b)
            .await
    }
    /// Executes the `savings_lending_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn savings_lending_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/savings/lending-history", q).await
    }
    /// Executes the `savings_public_borrow_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn savings_public_borrow_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/savings/lending-rate-history", q)
            .await
    }
    /// Executes the `dual_currency_pairs` OKX V5 operation with its classified auth and replay policy.
    pub async fn dual_currency_pairs(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/sfp/dcd/currency-pair", q).await
    }
    /// Executes the `dual_quote` OKX V5 operation with its classified auth and replay policy.
    pub async fn dual_quote(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/sfp/dcd/quote", b).await
    }
    /// Executes the `dual_redeem_quote` OKX V5 operation with its classified auth and replay policy.
    pub async fn dual_redeem_quote(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/sfp/dcd/redeem-quote", b).await
    }
    /// Executes the `dual_redeem` OKX V5 operation with its classified auth and replay policy.
    pub async fn dual_redeem(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write("/api/v5/finance/sfp/dcd/redeem", b).await
    }
    /// Executes the `dual_order_status` OKX V5 operation with its classified auth and replay policy.
    pub async fn dual_order_status(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.get("/api/v5/finance/sfp/dcd/order-status", q).await
    }
}
