use rust_decimal::Decimal;

use crate::types::{
    BarData, CommissionAndFeesReport, Contract, ContractDescription, ContractDetails,
    DeltaNeutralContract, Execution, HistoricalSession, HistoricalTick, HistoricalTickBidAsk,
    HistoricalTickLast, Order, OrderState, SmartComponent, SoftDollarTier, TickByTick,
};

/// Scanner row data.
#[derive(Debug, Clone, PartialEq)]
pub struct ScannerDataRow {
    /// Rank.
    pub rank: i32,
    /// Contract.
    pub contract: Box<Contract>,
    /// Market name.
    pub market_name: String,
    /// Distance.
    pub distance: String,
    /// Benchmark.
    pub benchmark: String,
    /// Projection.
    pub projection: String,
    /// Combo key.
    pub combo_key: String,
}

/// A normalized TWS callback event.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Connection completed.
    ConnectAck,
    /// Connection closed.
    ConnectionClosed,
    /// Error callback.
    Error {
        /// Request id.
        req_id: i32,
        /// Error time in milliseconds or server-provided timestamp.
        time: i64,
        /// Error code.
        code: i32,
        /// Error message.
        message: String,
        /// Advanced order reject payload.
        advanced_order_reject_json: String,
    },
    /// Next valid order id.
    NextValidId {
        /// Order id.
        order_id: i32,
    },
    /// Current time.
    CurrentTime {
        /// Unix timestamp seconds.
        time: i64,
    },
    /// Current time in milliseconds.
    CurrentTimeInMillis {
        /// Unix timestamp milliseconds.
        time_in_millis: i64,
    },
    /// Market data type.
    MarketDataType {
        /// Request id.
        req_id: i32,
        /// Market data type.
        market_data_type: i32,
    },
    /// Tick price.
    TickPrice {
        /// Request id.
        req_id: i32,
        /// Tick type.
        tick_type: i32,
        /// Price.
        price: f64,
        /// Attributes as raw bitset.
        attrib: i32,
    },
    /// Tick size.
    TickSize {
        /// Request id.
        req_id: i32,
        /// Tick type.
        tick_type: i32,
        /// Size.
        size: Decimal,
    },
    /// Generic tick value.
    TickGeneric {
        /// Request id.
        req_id: i32,
        /// Tick type.
        tick_type: i32,
        /// Value.
        value: f64,
    },
    /// Tick string.
    TickString {
        /// Request id.
        req_id: i32,
        /// Tick type.
        tick_type: i32,
        /// Value.
        value: String,
    },
    /// Exchange-for-physical tick.
    TickEfp {
        /// Request id.
        req_id: i32,
        /// Tick type.
        tick_type: i32,
        /// Basis points.
        basis_points: f64,
        /// Formatted basis points.
        formatted_basis_points: String,
        /// Total dividends.
        total_dividends: f64,
        /// Hold days.
        hold_days: i32,
        /// Future last trade date.
        future_last_trade_date: String,
        /// Dividend impact.
        dividend_impact: f64,
        /// Dividends to last trade date.
        dividends_to_last_trade_date: f64,
    },
    /// Tick snapshot end.
    TickSnapshotEnd {
        /// Request id.
        req_id: i32,
    },
    /// Option computation tick.
    TickOptionComputation {
        /// Request id.
        req_id: i32,
        /// Tick type.
        tick_type: i32,
        /// Attribute bitset.
        tick_attrib: i32,
        /// Implied volatility.
        implied_vol: f64,
        /// Delta.
        delta: f64,
        /// Option price.
        opt_price: f64,
        /// Present value dividend.
        pv_dividend: f64,
        /// Gamma.
        gamma: f64,
        /// Vega.
        vega: f64,
        /// Theta.
        theta: f64,
        /// Underlying price.
        und_price: f64,
    },
    /// Tick request parameters.
    TickReqParams {
        /// Request id.
        req_id: i32,
        /// Minimum tick.
        min_tick: String,
        /// BBO exchange.
        bbo_exchange: String,
        /// Snapshot permissions.
        snapshot_permissions: i32,
        /// Last price precision.
        last_price_precision: String,
        /// Last size precision.
        last_size_precision: String,
    },
    /// Commission and fees report callback.
    CommissionAndFeesReport {
        /// Report.
        report: CommissionAndFeesReport,
    },
    /// Delta-neutral validation callback.
    DeltaNeutralValidation {
        /// Request id.
        req_id: i32,
        /// Delta-neutral contract.
        delta_neutral_contract: DeltaNeutralContract,
    },
    /// Order status.
    OrderStatus {
        /// Order id.
        order_id: i32,
        /// Status text.
        status: String,
        /// Filled quantity.
        filled: Decimal,
        /// Remaining quantity.
        remaining: Decimal,
        /// Average fill price.
        avg_fill_price: f64,
        /// Permanent id.
        perm_id: i64,
        /// Parent id.
        parent_id: i32,
        /// Last fill price.
        last_fill_price: f64,
        /// Client id.
        client_id: i32,
        /// Why held.
        why_held: String,
        /// Market cap price.
        market_cap_price: f64,
    },
    /// Open order callback.
    OpenOrder {
        /// Order id.
        order_id: i32,
        /// Contract.
        contract: Box<Contract>,
        /// Order.
        order: Box<Order>,
        /// Order state.
        order_state: Box<OrderState>,
    },
    /// Open order end.
    OpenOrderEnd,
    /// Contract details.
    ContractDetails {
        /// Request id.
        req_id: i32,
        /// Details.
        details: Box<ContractDetails>,
    },
    /// Bond contract details.
    BondContractDetails {
        /// Request id.
        req_id: i32,
        /// Details.
        details: Box<ContractDetails>,
    },
    /// Contract details end.
    ContractDetailsEnd {
        /// Request id.
        req_id: i32,
    },
    /// Execution details.
    ExecutionDetails {
        /// Request id.
        req_id: i32,
        /// Contract.
        contract: Box<Contract>,
        /// Execution details.
        execution: Execution,
    },
    /// Execution details end.
    ExecutionDetailsEnd {
        /// Request id.
        req_id: i32,
    },
    /// Market depth update.
    MarketDepth {
        /// Request id.
        req_id: i32,
        /// Position.
        position: i32,
        /// Operation.
        operation: i32,
        /// Side.
        side: i32,
        /// Price.
        price: f64,
        /// Size.
        size: Decimal,
        /// Market maker.
        market_maker: String,
        /// Smart depth flag.
        is_smart_depth: bool,
    },
    /// Market depth exchanges.
    MarketDepthExchanges {
        /// Raw exchange descriptions as debug strings.
        descriptions: Vec<String>,
    },
    /// Smart components callback.
    SmartComponents {
        /// Request id.
        req_id: i32,
        /// Components.
        components: Vec<SmartComponent>,
    },
    /// Reroute market-data request.
    RerouteMarketDataRequest {
        /// Request id.
        req_id: i32,
        /// Contract id.
        con_id: i32,
        /// Exchange.
        exchange: String,
    },
    /// Reroute market-depth request.
    RerouteMarketDepthRequest {
        /// Request id.
        req_id: i32,
        /// Contract id.
        con_id: i32,
        /// Exchange.
        exchange: String,
    },
    /// Historical data bar.
    HistoricalData {
        /// Request id.
        req_id: i32,
        /// Bar.
        bar: BarData,
    },
    /// Historical data batch.
    HistoricalDataBars {
        /// Request id.
        req_id: i32,
        /// Bars.
        bars: Vec<BarData>,
    },
    /// Historical data update.
    HistoricalDataUpdate {
        /// Request id.
        req_id: i32,
        /// Bar.
        bar: BarData,
    },
    /// Historical data end.
    HistoricalDataEnd {
        /// Request id.
        req_id: i32,
        /// Start marker.
        start: String,
        /// End marker.
        end: String,
    },
    /// Historical midpoint/trade ticks callback.
    HistoricalTicks {
        /// Request id.
        req_id: i32,
        /// Ticks.
        ticks: Vec<HistoricalTick>,
        /// Done marker.
        done: bool,
    },
    /// Historical bid/ask ticks callback.
    HistoricalTicksBidAsk {
        /// Request id.
        req_id: i32,
        /// Ticks.
        ticks: Vec<HistoricalTickBidAsk>,
        /// Done marker.
        done: bool,
    },
    /// Historical last trade ticks callback.
    HistoricalTicksLast {
        /// Request id.
        req_id: i32,
        /// Ticks.
        ticks: Vec<HistoricalTickLast>,
        /// Done marker.
        done: bool,
    },
    /// Tick-by-tick callback.
    TickByTick {
        /// Request id.
        req_id: i32,
        /// Tick type.
        tick_type: i32,
        /// Tick payload.
        tick: Option<TickByTick>,
    },
    /// Historical schedule callback.
    HistoricalSchedule {
        /// Request id.
        req_id: i32,
        /// Schedule start.
        start_date_time: String,
        /// Schedule end.
        end_date_time: String,
        /// Time zone.
        time_zone: String,
        /// Sessions.
        sessions: Vec<HistoricalSession>,
    },
    /// Position callback.
    Position {
        /// Account.
        account: String,
        /// Contract.
        contract: Box<Contract>,
        /// Position.
        position: Decimal,
        /// Average cost.
        avg_cost: f64,
    },
    /// Position end.
    PositionEnd,
    /// Position multi callback.
    PositionMulti {
        /// Request id.
        req_id: i32,
        /// Account.
        account: String,
        /// Model code.
        model_code: String,
        /// Contract.
        contract: Box<Contract>,
        /// Position.
        position: Decimal,
        /// Average cost.
        avg_cost: f64,
    },
    /// Position multi end.
    PositionMultiEnd {
        /// Request id.
        req_id: i32,
    },
    /// Account value update.
    AccountValue {
        /// Key.
        key: String,
        /// Value.
        value: String,
        /// Currency.
        currency: String,
        /// Account name.
        account_name: String,
    },
    /// Portfolio value update.
    PortfolioValue {
        /// Contract.
        contract: Box<Contract>,
        /// Position.
        position: Decimal,
        /// Market price.
        market_price: f64,
        /// Market value.
        market_value: f64,
        /// Average cost.
        average_cost: f64,
        /// Unrealized PnL.
        unrealized_pnl: f64,
        /// Realized PnL.
        realized_pnl: f64,
        /// Account name.
        account_name: String,
    },
    /// Account update time.
    AccountUpdateTime {
        /// Timestamp.
        timestamp: String,
    },
    /// Account download end.
    AccountDownloadEnd {
        /// Account name.
        account_name: String,
    },
    /// Account summary.
    AccountSummary {
        /// Request id.
        req_id: i32,
        /// Account.
        account: String,
        /// Tag.
        tag: String,
        /// Value.
        value: String,
        /// Currency.
        currency: String,
    },
    /// Account summary end.
    AccountSummaryEnd {
        /// Request id.
        req_id: i32,
    },
    /// Account update multi.
    AccountUpdateMulti {
        /// Request id.
        req_id: i32,
        /// Account.
        account: String,
        /// Model code.
        model_code: String,
        /// Key.
        key: String,
        /// Value.
        value: String,
        /// Currency.
        currency: String,
    },
    /// Account update multi end.
    AccountUpdateMultiEnd {
        /// Request id.
        req_id: i32,
    },
    /// Real-time bar tick.
    RealTimeBar {
        /// Request id.
        req_id: i32,
        /// Unix timestamp seconds.
        time: i64,
        /// Bar.
        bar: BarData,
    },
    /// Head timestamp.
    HeadTimestamp {
        /// Request id.
        req_id: i32,
        /// Head timestamp.
        head_timestamp: String,
    },
    /// Histogram data.
    HistogramData {
        /// Request id.
        req_id: i32,
        /// Price/size entries.
        items: Vec<(f64, Decimal)>,
    },
    /// Scanner parameters XML.
    ScannerParameters {
        /// XML payload.
        xml: String,
    },
    /// Scanner data.
    ScannerData {
        /// Request id.
        req_id: i32,
        /// Rows.
        rows: Vec<ScannerDataRow>,
    },
    /// Scanner data end.
    ScannerDataEnd {
        /// Request id.
        req_id: i32,
    },
    /// Soft-dollar tiers.
    SoftDollarTiers {
        /// Request id.
        req_id: i32,
        /// Tiers.
        tiers: Vec<SoftDollarTier>,
    },
    /// Family codes.
    FamilyCodes {
        /// `(account_id, family_code)` pairs.
        family_codes: Vec<(String, String)>,
    },
    /// Symbol samples callback.
    SymbolSamples {
        /// Request id.
        req_id: i32,
        /// Contract descriptions.
        descriptions: Vec<ContractDescription>,
    },
    /// Security definition option parameter callback.
    SecurityDefinitionOptionParameter {
        /// Request id.
        req_id: i32,
        /// Exchange.
        exchange: String,
        /// Underlying contract id.
        underlying_con_id: i32,
        /// Trading class.
        trading_class: String,
        /// Multiplier.
        multiplier: String,
        /// Expirations.
        expirations: Vec<String>,
        /// Strikes.
        strikes: Vec<f64>,
    },
    /// Security definition option parameter end.
    SecurityDefinitionOptionParameterEnd {
        /// Request id.
        req_id: i32,
    },
    /// Market rule.
    MarketRule {
        /// Market rule id.
        market_rule_id: i32,
        /// `(low_edge, increment)` entries.
        price_increments: Vec<(f64, f64)>,
    },
    /// PnL update.
    Pnl {
        /// Request id.
        req_id: i32,
        /// Daily PnL.
        daily_pnl: f64,
        /// Unrealized PnL.
        unrealized_pnl: f64,
        /// Realized PnL.
        realized_pnl: f64,
    },
    /// Single-contract PnL update.
    PnlSingle {
        /// Request id.
        req_id: i32,
        /// Position.
        position: Decimal,
        /// Daily PnL.
        daily_pnl: f64,
        /// Unrealized PnL.
        unrealized_pnl: f64,
        /// Realized PnL.
        realized_pnl: f64,
        /// Value.
        value: f64,
    },
    /// News article body.
    NewsArticle {
        /// Request id.
        req_id: i32,
        /// Article type.
        article_type: i32,
        /// Article text.
        article_text: String,
    },
    /// News bulletin.
    NewsBulletin {
        /// Message id.
        news_msg_id: i32,
        /// Message type.
        news_msg_type: i32,
        /// Message text.
        news_message: String,
        /// Originating exchange.
        originating_exch: String,
    },
    /// News providers.
    NewsProviders {
        /// `(provider_code, provider_name)` pairs.
        providers: Vec<(String, String)>,
    },
    /// Historical news item.
    HistoricalNews {
        /// Request id.
        req_id: i32,
        /// Time.
        time: String,
        /// Provider code.
        provider_code: String,
        /// Article id.
        article_id: String,
        /// Headline.
        headline: String,
    },
    /// Historical news end.
    HistoricalNewsEnd {
        /// Request id.
        req_id: i32,
        /// Whether more data is available.
        has_more: bool,
    },
    /// Tick news item.
    TickNews {
        /// Request id.
        req_id: i32,
        /// Timestamp.
        timestamp: i64,
        /// Provider code.
        provider_code: String,
        /// Article id.
        article_id: String,
        /// Headline.
        headline: String,
        /// Extra data.
        extra_data: String,
    },
    /// WSH metadata.
    WshMetaData {
        /// Request id.
        req_id: i32,
        /// JSON payload.
        data_json: String,
    },
    /// WSH event data.
    WshEventData {
        /// Request id.
        req_id: i32,
        /// JSON payload.
        data_json: String,
    },
    /// Receive FA data.
    ReceiveFa {
        /// FA data type.
        fa_data_type: i32,
        /// XML payload.
        xml: String,
    },
    /// Replace FA end.
    ReplaceFaEnd {
        /// Request id.
        req_id: i32,
        /// Response text.
        text: String,
    },
    /// Display group list.
    DisplayGroupList {
        /// Request id.
        req_id: i32,
        /// Groups.
        groups: String,
    },
    /// Display group updated.
    DisplayGroupUpdated {
        /// Request id.
        req_id: i32,
        /// Contract info.
        contract_info: String,
    },
    /// Verify message API.
    VerifyMessageApi {
        /// API data.
        api_data: String,
    },
    /// Verify completed.
    VerifyCompleted {
        /// Success flag.
        is_successful: bool,
        /// Error text.
        error_text: String,
    },
    /// Verify-and-auth message API.
    VerifyAndAuthMessageApi {
        /// API data.
        api_data: String,
        /// Challenge payload.
        challenge: String,
    },
    /// Verify-and-auth completed.
    VerifyAndAuthCompleted {
        /// Success flag.
        is_successful: bool,
        /// Error text.
        error_text: String,
    },
    /// Config response.
    ConfigResponse {
        /// Request id.
        req_id: i32,
        /// Status.
        status: String,
        /// Message.
        message: String,
    },
    /// Update config response.
    UpdateConfigResponse {
        /// Request id.
        req_id: i32,
        /// Status.
        status: String,
        /// Message.
        message: String,
        /// Changed fields.
        changed_fields: Vec<String>,
        /// Errors.
        errors: Vec<String>,
    },
    /// Order bound callback.
    OrderBound {
        /// Permanent id.
        perm_id: i64,
        /// Client id.
        client_id: i32,
        /// Order id.
        order_id: i32,
    },
    /// Completed order callback.
    CompletedOrder {
        /// Contract.
        contract: Box<Contract>,
        /// Order.
        order: Box<Order>,
        /// Order state.
        order_state: Box<OrderState>,
    },
    /// Completed orders end.
    CompletedOrdersEnd,
    /// User info callback.
    UserInfo {
        /// Request id.
        req_id: i32,
        /// White-branding id.
        white_branding_id: String,
    },
    /// Managed account list.
    ManagedAccounts {
        /// Comma-separated accounts.
        accounts: String,
    },
    /// Raw callback not decoded into a typed variant yet.
    Raw {
        /// Message id.
        msg_id: i32,
        /// Raw fields.
        fields: Vec<String>,
    },
    /// Raw protobuf callback not decoded by this crate.
    RawProtobuf {
        /// Message id without protobuf offset.
        msg_id: i32,
        /// Raw payload.
        payload: Vec<u8>,
    },
}

/// Callback sink for TWS events.
///
/// Implementors can override either [`Wrapper::on_event`] or individual typed methods.
pub trait Wrapper: Send {
    /// Receives every event.
    fn on_event(&mut self, _event: Event) {}

    /// Connection completed.
    fn connect_ack(&mut self) {
        self.on_event(Event::ConnectAck);
    }

    /// Connection closed.
    fn connection_closed(&mut self) {
        self.on_event(Event::ConnectionClosed);
    }

    /// Error callback.
    fn error(&mut self, req_id: i32, time: i64, code: i32, message: String) {
        self.on_event(Event::Error {
            req_id,
            time,
            code,
            message,
            advanced_order_reject_json: String::new(),
        });
    }

    /// Next valid id.
    fn next_valid_id(&mut self, order_id: i32) {
        self.on_event(Event::NextValidId { order_id });
    }

    /// Current time.
    fn current_time(&mut self, time: i64) {
        self.on_event(Event::CurrentTime { time });
    }

    /// Current time in milliseconds.
    fn current_time_in_millis(&mut self, time_in_millis: i64) {
        self.on_event(Event::CurrentTimeInMillis { time_in_millis });
    }
}
