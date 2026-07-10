use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
    types::account::{Balance, Bill, FeeRate, Leverage, MarginAdjustment, Position, PositionRisk},
};
use std::collections::BTreeMap;
pub struct AccountService<'a>(pub(crate) &'a OkxClient);
impl AccountService<'_> {
    async fn read_json(
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
                true,
            )?)
            .await
    }
    async fn write_json(
        &self,
        path: &'static str,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::POST,
                path,
                BTreeMap::new(),
                Some(request),
                RetrySafety::NeverReplay,
                true,
            )?)
            .await
    }
    pub async fn balances(&self) -> OkxResult<Vec<Balance>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/account/balance",
                BTreeMap::new(),
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    pub async fn positions(&self) -> OkxResult<Vec<Position>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/account/positions",
                BTreeMap::new(),
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Gets account bills using the server's cursor pagination parameters.
    pub async fn bills(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Bill>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/account/bills",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Gets immutable account configuration records.
    pub async fn configuration(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/account/config",
                BTreeMap::new(),
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Gets leverage for the requested margin scope.
    pub async fn leverage(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Leverage>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/account/leverage-info",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Sets leverage; writes are never replayed automatically.
    pub async fn set_leverage(&self, request: &serde_json::Value) -> OkxResult<Vec<Leverage>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::POST,
                "/api/v5/account/set-leverage",
                BTreeMap::new(),
                Some(request),
                RetrySafety::NeverReplay,
                true,
            )?)
            .await
    }
    /// Adjusts isolated margin and returns the acknowledged amount.
    pub async fn adjust_margin(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<MarginAdjustment>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::POST,
                "/api/v5/account/position/margin-balance",
                BTreeMap::new(),
                Some(request),
                RetrySafety::NeverReplay,
                true,
            )?)
            .await
    }
    /// Gets account position-risk records.
    pub async fn position_risk(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<PositionRisk>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/account/account-position-risk",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Gets the fee schedule for a product scope.
    pub async fn fee_rates(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<FeeRate>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/account/trade-fee",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }

    /// Borrows or repays a margin currency. This command is never replayed.
    pub async fn borrow_repay(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/borrow-repay", request)
            .await
    }
    pub async fn borrow_repay_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/borrow-repay-history", query)
            .await
    }
    pub async fn interest_accrued(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/interest-accrued", query)
            .await
    }
    pub async fn interest_rate(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/interest-rate", query).await
    }
    pub async fn max_loan(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/max-loan", query).await
    }
    pub async fn quick_margin_borrow_repay(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/quick-margin-borrow-repay", request)
            .await
    }
    pub async fn quick_margin_borrow_repay_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/quick-margin-borrow-repay-history", query)
            .await
    }

    pub async fn fixed_loan_borrowing_orders(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/fixed-loan/borrowing-order-list", query)
            .await
    }
    pub async fn fixed_loan_borrow(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/fixed-loan/borrowing-order", request)
            .await
    }
    pub async fn fixed_loan_amend(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/fixed-loan/amend-borrowing-order", request)
            .await
    }
    pub async fn fixed_loan_reborrow(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/fixed-loan/manual-reborrow", request)
            .await
    }
    pub async fn fixed_loan_repay(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/fixed-loan/repay-borrowing-order", request)
            .await
    }
    pub async fn fixed_loan_repayments(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json(
            "/api/v5/account/fixed-loan/repay-borrowing-order-list",
            query,
        )
        .await
    }
    pub async fn fixed_loan_account(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/fixed-loan/account", query)
            .await
    }
    pub async fn fixed_loan_interest_accrued(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/fixed-loan/interest-accrued", query)
            .await
    }

    pub async fn vip_loan_borrow(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/vip-loan/borrow-order", request)
            .await
    }
    pub async fn vip_loan_repay(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/vip-loan/repay-loan", request)
            .await
    }
    pub async fn vip_loan_amend(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/vip-loan/amend-loan", request)
            .await
    }
    pub async fn vip_loan_orders(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/vip-loan/loan-order-list", query)
            .await
    }
    pub async fn vip_loan_order(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/vip-loan/loan-order-detail", query)
            .await
    }
    pub async fn vip_loan_repayment_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/vip-loan/repay-history", query)
            .await
    }
    pub async fn vip_loan_account(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/vip-loan/account", query)
            .await
    }
    pub async fn vip_loan_interest_accrued(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/vip-loan/interest-accrued", query)
            .await
    }

    pub async fn greeks(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/greeks", query).await
    }
    pub async fn set_greeks(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-greeks", request).await
    }
    pub async fn set_position_mode(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-position-mode", request)
            .await
    }
    pub async fn set_account_level(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-account-level", request)
            .await
    }
    pub async fn set_auto_loan(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-auto-loan", request)
            .await
    }
    pub async fn set_risk_offset_type(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-risk-offset-type", request)
            .await
    }
    pub async fn activate_option(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/activate-option", request)
            .await
    }
    pub async fn set_spot_manual_borrowing(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-spot-manual-borrow-repay", request)
            .await
    }
    pub async fn risk_state(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/risk-state", BTreeMap::new())
            .await
    }
}
