use crate::{
    client::OkxClient,
    error::OkxResult,
    request::{CanonicalRequest, RetrySafety},
    types::{
        common::ExpirationTime,
        order::{
            AmendOrder, BatchResult, Fill, Order, OrderAck, OrderReference, PlaceOrder,
            require_successful_acknowledgements,
        },
    },
};
use std::collections::BTreeMap;
pub struct TradeService<'a>(pub(crate) &'a OkxClient);
impl TradeService<'_> {
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
    async fn write_order_json<T: serde::Serialize + ?Sized>(
        &self,
        path: &'static str,
        request: &T,
        expiration_time: ExpirationTime,
    ) -> OkxResult<Vec<OrderAck>> {
        let acknowledgements = self
            .0
            .execute(
                CanonicalRequest::new(
                    reqwest::Method::POST,
                    path,
                    BTreeMap::new(),
                    Some(request),
                    RetrySafety::NeverReplay,
                    true,
                )?
                .with_expiration_time(expiration_time),
            )
            .await?;
        require_successful_acknowledgements(acknowledgements)
    }

    async fn write_order<T: serde::Serialize + ?Sized>(
        &self,
        path: &'static str,
        request: &T,
    ) -> OkxResult<Vec<OrderAck>> {
        let acknowledgements = self
            .0
            .execute(CanonicalRequest::new(
                reqwest::Method::POST,
                path,
                BTreeMap::new(),
                Some(request),
                RetrySafety::NeverReplay,
                true,
            )?)
            .await?;
        require_successful_acknowledgements(acknowledgements)
    }
    /// Executes the `place_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn place_order(&self, order: &PlaceOrder) -> OkxResult<Vec<OrderAck>> {
        self.write_order("/api/v5/trade/order", order).await
    }
    /// Places an order which OKX must reject if it has not begun processing by `expiration_time`.
    pub async fn place_order_with_expiration(
        &self,
        order: &PlaceOrder,
        expiration_time: ExpirationTime,
    ) -> OkxResult<Vec<OrderAck>> {
        self.write_order_json("/api/v5/trade/order", order, expiration_time)
            .await
    }
    /// Places a batch of orders; each acknowledgement retains its own exchange code.
    pub async fn place_batch(&self, orders: &[PlaceOrder]) -> OkxResult<BatchResult> {
        self.write_order("/api/v5/trade/batch-orders", orders).await
    }
    /// Places a batch with one shared OKX processing deadline.
    pub async fn place_batch_with_expiration(
        &self,
        orders: &[PlaceOrder],
        expiration_time: ExpirationTime,
    ) -> OkxResult<BatchResult> {
        self.write_order_json("/api/v5/trade/batch-orders", orders, expiration_time)
            .await
    }
    /// Cancels one order.
    pub async fn cancel_order(&self, order: &OrderReference) -> OkxResult<Vec<OrderAck>> {
        self.write_order("/api/v5/trade/cancel-order", order).await
    }
    /// Cancels a batch of orders without replaying the write.
    pub async fn cancel_batch(&self, orders: &[OrderReference]) -> OkxResult<BatchResult> {
        self.write_order("/api/v5/trade/cancel-batch-orders", orders)
            .await
    }
    /// Amends one order.
    pub async fn amend_order(&self, order: &AmendOrder) -> OkxResult<Vec<OrderAck>> {
        self.write_order("/api/v5/trade/amend-order", order).await
    }
    /// Amends an order with an OKX processing deadline.
    pub async fn amend_order_with_expiration(
        &self,
        order: &AmendOrder,
        expiration_time: ExpirationTime,
    ) -> OkxResult<Vec<OrderAck>> {
        self.write_order_json("/api/v5/trade/amend-order", order, expiration_time)
            .await
    }
    /// Amends a batch of orders without replaying the write.
    pub async fn amend_batch(&self, orders: &[AmendOrder]) -> OkxResult<BatchResult> {
        self.write_order("/api/v5/trade/amend-batch-orders", orders)
            .await
    }
    /// Amends a batch with one shared OKX processing deadline.
    pub async fn amend_batch_with_expiration(
        &self,
        orders: &[AmendOrder],
        expiration_time: ExpirationTime,
    ) -> OkxResult<BatchResult> {
        self.write_order_json("/api/v5/trade/amend-batch-orders", orders, expiration_time)
            .await
    }
    /// Closes positions according to the supplied native request.
    pub async fn close_positions(&self, request: &serde_json::Value) -> OkxResult<Vec<OrderAck>> {
        self.write_order("/api/v5/trade/close-position", request)
            .await
    }
    /// Gets a single order by exchange or client identifier.
    pub async fn order(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Order>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/trade/order",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Lists orders and preserves caller-supplied cursor filters.
    pub async fn orders(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Order>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/trade/orders-pending",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Lists completed order history while preserving server cursor filters.
    pub async fn order_history(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Order>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/trade/orders-history",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Lists fills and preserves exact decimal execution data.
    pub async fn fills(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Fill>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/trade/fills",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }

    /// Places an algo order. Asset-changing commands are not automatically replayed.
    pub async fn place_algo_order(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/trade/order-algo", request).await
    }
    /// Executes the `cancel_algo_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn cancel_algo_order(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/trade/cancel-algos", request).await
    }
    /// Executes the `amend_algo_order` OKX V5 operation with its classified auth and replay policy.
    pub async fn amend_algo_order(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/trade/amend-algos", request).await
    }
    /// Executes the `algo_orders_pending` OKX V5 operation with its classified auth and replay policy.
    pub async fn algo_orders_pending(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/trade/orders-algo-pending", query)
            .await
    }
    /// Executes the `algo_orders_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn algo_orders_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/trade/orders-algo-history", query)
            .await
    }
    /// Gets the conversion currencies available to the account.
    pub async fn easy_convert_currency_list(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/trade/easy-convert-currency-list", query)
            .await
    }
    /// Executes the `easy_convert` OKX V5 operation with its classified auth and replay policy.
    pub async fn easy_convert(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/trade/easy-convert", request).await
    }
    /// Executes the `easy_convert_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn easy_convert_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/trade/easy-convert-history", query)
            .await
    }
    /// Executes the `one_click_repay_currency_list` OKX V5 operation with its classified auth and replay policy.
    pub async fn one_click_repay_currency_list(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/trade/one-click-repay-currency-list", query)
            .await
    }
    /// Executes the `one_click_repay` OKX V5 operation with its classified auth and replay policy.
    pub async fn one_click_repay(
        &self,
        request: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/trade/one-click-repay", request)
            .await
    }
    /// Executes the `one_click_repay_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn one_click_repay_history(
        &self,
        query: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/trade/one-click-repay-history", query)
            .await
    }
    /// Executes the `fills_history` OKX V5 operation with its classified auth and replay policy.
    pub async fn fills_history(&self, query: BTreeMap<String, String>) -> OkxResult<Vec<Fill>> {
        self.0
            .execute(CanonicalRequest::new(
                reqwest::Method::GET,
                "/api/v5/trade/fills-history",
                query,
                None::<&serde_json::Value>,
                RetrySafety::ReadOnly,
                true,
            )?)
            .await
    }
    /// Executes the `orders_history_archive` OKX V5 operation with its classified auth and replay policy.
    pub async fn orders_history_archive(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/trade/orders-history-archive", q)
            .await
    }
    /// Executes the `algo_order_details` OKX V5 operation with its classified auth and replay policy.
    pub async fn algo_order_details(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/trade/order-algo", q).await
    }
    /// Executes the `one_click_repay_currency_list_v2` OKX V5 operation with its classified auth and replay policy.
    pub async fn one_click_repay_currency_list_v2(&self) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json(
            "/api/v5/trade/one-click-repay-currency-list-v2",
            BTreeMap::new(),
        )
        .await
    }
    /// Executes the `one_click_repay_v2` OKX V5 operation with its classified auth and replay policy.
    pub async fn one_click_repay_v2(
        &self,
        b: &serde_json::Value,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.write_json("/api/v5/trade/one-click-repay-v2", b).await
    }
    /// Executes the `one_click_repay_history_v2` OKX V5 operation with its classified auth and replay policy.
    pub async fn one_click_repay_history_v2(
        &self,
        q: BTreeMap<String, String>,
    ) -> OkxResult<Vec<serde_json::Value>> {
        self.read_json("/api/v5/trade/one-click-repay-history-v2", q)
            .await
    }
}
