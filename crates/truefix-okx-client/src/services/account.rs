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
    /// Executes the `balances` OKX V5 operation with its classified auth and replay policy.
    pub async fn balances(&self) -> OkxResult<Vec<Balance>> {
        self.balances_with_currency(None).await
    }
    /// Executes the `balances` OKX V5 operation with an optional currency filter.
    pub async fn balances_with_currency(&self, ccy: Option<&str>) -> OkxResult<Vec<Balance>> {
        let mut query = BTreeMap::new();
        if let Some(ccy) = ccy.filter(|ccy| !ccy.is_empty()) {
            query.insert("ccy".to_owned(), ccy.to_owned());
        }
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/account/balance",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Executes the `positions` OKX V5 operation with its classified auth and replay policy.
    pub async fn positions(&self) -> OkxResult<Vec<Position>> {
        self.positions_with_filters(None, None, None).await
    }
    /// Executes the `positions` OKX V5 operation with optional OKX filters.
    pub async fn positions_with_filters(
        &self,
        inst_type: Option<&str>,
        inst_id: Option<&str>,
        pos_id: Option<&str>,
    ) -> OkxResult<Vec<Position>> {
        let mut query = BTreeMap::new();
        if let Some(inst_type) = inst_type.filter(|value| !value.is_empty()) {
            query.insert("instType".to_owned(), inst_type.to_owned());
        }
        if let Some(inst_id) = inst_id.filter(|value| !value.is_empty()) {
            query.insert("instId".to_owned(), inst_id.to_owned());
        }
        if let Some(pos_id) = pos_id.filter(|value| !value.is_empty()) {
            query.insert("posId".to_owned(), pos_id.to_owned());
        }
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/account/positions",
                query,
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
    /// Executes the `borrow_repay_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn borrow_repay_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/borrow-repay-history", query)
            .await
    }
    /// Executes the `interest_accrued` OKX V5 operation with its classified auth and replay policy.
    pub async fn interest_accrued(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/interest-accrued", query)
            .await
    }
    /// Executes the `interest_rate` OKX V5 operation with its classified auth and replay policy.
    pub async fn interest_rate(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/interest-rate", query).await
    }
    /// Executes the `max_loan` OKX V5 operation with its classified auth and replay policy.
    pub async fn max_loan(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/max-loan", query).await
    }
    /// Executes the `quick_margin_borrow_repay` OKX V5 operation with its classified auth and replay policy.
    pub async fn quick_margin_borrow_repay(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/quick-margin-borrow-repay", request)
            .await
    }
    /// Executes the `quick_margin_borrow_repay_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn quick_margin_borrow_repay_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/quick-margin-borrow-repay-history", query)
            .await
    }

    /// Executes the `fixed_loan_borrowing_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn fixed_loan_borrowing_orders(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/fixed-loan/borrowing-orders-list", query)
            .await
    }
    /// Executes the `fixed_loan_borrow` OKX V5 operation with its classified auth and replay policy.
    pub async fn fixed_loan_borrow(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/fixed-loan/borrowing-order", request)
            .await
    }
    /// Executes the `fixed_loan_amend` OKX V5 operation with its classified auth and replay policy.
    pub async fn fixed_loan_amend(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/fixed-loan/amend-borrowing-order", request)
            .await
    }
    /// Executes the `fixed_loan_reborrow` OKX V5 operation with its classified auth and replay policy.
    pub async fn fixed_loan_reborrow(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/fixed-loan/manual-reborrow", request)
            .await
    }
    /// Executes the `fixed_loan_repay` OKX V5 operation with its classified auth and replay policy.
    pub async fn fixed_loan_repay(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/fixed-loan/repay-borrowing-order", request)
            .await
    }
    /// Executes the `vip_loan_orders` OKX V5 operation with its classified auth and replay policy.
    pub async fn vip_loan_orders(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/vip-loan-order-list", query)
            .await
    }
    /// Executes the `vip_loan_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn vip_loan_order(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/vip-loan-order-detail", query)
            .await
    }
    /// Executes the `vip_loan_interest_accrued` OKX V5 operation with its classified auth and replay policy.
    pub async fn vip_loan_interest_accrued(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/vip-interest-accrued", query)
            .await
    }

    /// Executes the `greeks` OKX V5 operation with its classified auth and replay policy.
    pub async fn greeks(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/greeks", query).await
    }
    /// Executes the `set_greeks` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_greeks(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-greeks", request).await
    }
    /// Executes the `set_position_mode` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_position_mode(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-position-mode", request)
            .await
    }
    /// Executes the `set_account_level` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_account_level(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-account-level", request)
            .await
    }
    /// Executes the `set_auto_loan` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_auto_loan(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-auto-loan", request)
            .await
    }
    /// Executes the `set_risk_offset_type` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_risk_offset_type(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-riskOffset-type", request)
            .await
    }
    /// Executes the `activate_option` OKX V5 operation with its classified auth and replay policy.
    pub async fn activate_option(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/activate-option", &serde_json::json!({}))
            .await
    }
    /// Executes the `activate_option` OKX V5 operation with an explicit JSON body.
    pub async fn activate_option_request(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/activate-option", request)
            .await
    }
    /// Executes the `set_spot_manual_borrowing` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_spot_manual_borrowing(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/spot-manual-borrow-repay", request)
            .await
    }
    /// Executes the `risk_state` OKX V5 operation with its classified auth and replay policy.
    pub async fn risk_state(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/risk-state", BTreeMap::new())
            .await
    }

    /// Executes the `bills_archive` OKX V5 operation with its classified auth and replay policy.
    pub async fn bills_archive(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/bills-archive", q).await
    }
    /// Executes the `max_order_size` OKX V5 operation with its classified auth and replay policy.
    pub async fn max_order_size(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/max-size", q).await
    }
    /// Executes the `max_available_size` OKX V5 operation with its classified auth and replay policy.
    pub async fn max_available_size(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/max-avail-size", q).await
    }
    /// Executes the `max_withdrawal` OKX V5 operation with its classified auth and replay policy.
    pub async fn max_withdrawal(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/max-withdrawal", q).await
    }
    /// Executes the `interest_limits` OKX V5 operation with its classified auth and replay policy.
    pub async fn interest_limits(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/interest-limits", q).await
    }
    /// Executes the `positions_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn positions_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/positions-history", q).await
    }
    /// Executes the `account_position_tiers` OKX V5 operation with its classified auth and replay policy.
    pub async fn account_position_tiers(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/position-tiers", q).await
    }
    /// Executes the `vip_interest_deducted` OKX V5 operation with its classified auth and replay policy.
    pub async fn vip_interest_deducted(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/vip-interest-deducted", q)
            .await
    }
    /// Executes the `account_instruments` OKX V5 operation with its classified auth and replay policy.
    pub async fn account_instruments(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/instruments", q).await
    }
    /// Executes the `fixed_loan_borrowing_limit` OKX V5 operation with its classified auth and replay policy.
    pub async fn fixed_loan_borrowing_limit(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/fixed-loan/borrowing-limit", q)
            .await
    }
    /// Executes the `fixed_loan_borrowing_quote` OKX V5 operation with its classified auth and replay policy.
    pub async fn fixed_loan_borrowing_quote(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/fixed-loan/borrowing-quote", q)
            .await
    }
    /// Executes the `spot_borrow_repay_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn spot_borrow_repay_history(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/spot-borrow-repay-history", q)
            .await
    }
    /// Executes the `precheck_delta_neutral` OKX V5 operation with its classified auth and replay policy.
    pub async fn precheck_delta_neutral(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/precheck-set-delta-neutral", q)
            .await
    }
    /// Executes the `bill_subtypes` OKX V5 operation with its classified auth and replay policy.
    pub async fn bill_subtypes(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/account/subtypes", q).await
    }
    /// Executes the `simulated_margin` OKX V5 operation with its classified auth and replay policy.
    pub async fn simulated_margin(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/simulated_margin", b).await
    }
    /// Executes the `position_builder` OKX V5 operation with its classified auth and replay policy.
    pub async fn position_builder(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/position-builder", b).await
    }
    /// Executes the `set_isolated_mode` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_isolated_mode(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-isolated-mode", b)
            .await
    }
    /// Executes the `set_auto_repay` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_auto_repay(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-auto-repay", b).await
    }
    /// Executes the `set_auto_earn` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_auto_earn(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-auto-earn", b).await
    }
    /// Executes the `set_trading_config` OKX V5 operation with its classified auth and replay policy.
    pub async fn set_trading_config(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/set-trading-config", b)
            .await
    }
    /// Executes the `archive_bills` OKX V5 operation with its classified auth and replay policy.
    pub async fn archive_bills(&self, b: &serde_json::Value) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/account/bills-history-archive", b)
            .await
    }
}
