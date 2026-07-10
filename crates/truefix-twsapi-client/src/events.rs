use rust_decimal::Decimal;

use crate::enums::{FaDataType, MarketDataType, TickType};
use crate::types::{
    BarData, CommissionAndFeesReport, Contract, ContractDescription, ContractDetails,
    DeltaNeutralContract, Execution, HistoricalSession, HistoricalTick, HistoricalTickBidAsk,
    HistoricalTickLast, Order, OrderState, SmartComponent, SoftDollarTier, TickByTick,
};
use crate::types::{
    DepthMarketDataDescription, FamilyCode, NewsProvider, PriceIncrement, RealTimeBar, WshEventData,
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
#[allow(clippy::too_many_arguments)]
pub trait Wrapper: Send {
    /// Receives every event.
    fn on_event(&mut self, _event: Event) {}

    /// Connection completed.
    fn connect_ack(&mut self) {}

    /// Connection closed.
    fn connection_closed(&mut self) {}

    /// Error callback.
    fn error(&mut self, _req_id: i32, _time: i64, _code: i32, _message: String) {}

    /// Next valid id.
    fn next_valid_id(&mut self, _order_id: i32) {}

    /// Current time.
    fn current_time(&mut self, _time: i64) {}

    /// Current time in milliseconds.
    fn current_time_in_millis(&mut self, _time_in_millis: i64) {}

    /// Dispatches an event to its typed callback.
    fn dispatch(&mut self, event: Event) {
        self.on_event(event.clone());
        match event {
            Event::ConnectAck => self.connect_ack(),
            Event::ConnectionClosed => self.connection_closed(),
            Event::MarketDataType {
                req_id,
                market_data_type,
            } => {
                self.market_data_type(req_id, MarketDataType::from_i32(market_data_type));
            }
            Event::TickPrice {
                req_id,
                tick_type,
                price,
                attrib,
            } => {
                self.tick_price(req_id, TickType::from_i32(tick_type), price, attrib);
            }
            Event::TickSize {
                req_id,
                tick_type,
                size,
            } => {
                self.tick_size(req_id, TickType::from_i32(tick_type), size);
            }
            Event::TickGeneric {
                req_id,
                tick_type,
                value,
            } => {
                self.tick_generic(req_id, TickType::from_i32(tick_type), value);
            }
            Event::TickString {
                req_id,
                tick_type,
                value,
            } => {
                self.tick_string(req_id, TickType::from_i32(tick_type), value);
            }
            Event::TickEfp {
                req_id,
                tick_type,
                basis_points,
                formatted_basis_points,
                total_dividends,
                hold_days,
                future_last_trade_date,
                dividend_impact,
                dividends_to_last_trade_date,
            } => self.tick_efp(
                req_id,
                TickType::from_i32(tick_type),
                basis_points,
                formatted_basis_points,
                total_dividends,
                hold_days,
                future_last_trade_date,
                dividend_impact,
                dividends_to_last_trade_date,
            ),
            Event::TickSnapshotEnd { req_id } => self.tick_snapshot_end(req_id),
            Event::TickOptionComputation {
                req_id,
                tick_type,
                tick_attrib,
                implied_vol,
                delta,
                opt_price,
                pv_dividend,
                gamma,
                vega,
                theta,
                und_price,
            } => self.tick_option_computation(
                req_id,
                TickType::from_i32(tick_type),
                tick_attrib,
                implied_vol,
                delta,
                opt_price,
                pv_dividend,
                gamma,
                vega,
                theta,
                und_price,
            ),
            Event::TickReqParams {
                req_id,
                min_tick,
                bbo_exchange,
                snapshot_permissions,
                last_price_precision,
                last_size_precision,
            } => self.tick_req_params(
                req_id,
                min_tick,
                bbo_exchange,
                snapshot_permissions,
                last_price_precision,
                last_size_precision,
            ),
            Event::CommissionAndFeesReport { report } => self.commission_and_fees_report(report),
            Event::DeltaNeutralValidation {
                req_id,
                delta_neutral_contract,
            } => self.delta_neutral_validation(req_id, delta_neutral_contract),
            Event::NextValidId { order_id } => self.next_valid_id(order_id),
            Event::CurrentTime { time } => self.current_time(time),
            Event::CurrentTimeInMillis { time_in_millis } => {
                self.current_time_in_millis(time_in_millis)
            }
            Event::Error {
                req_id,
                time,
                code,
                message,
                advanced_order_reject_json,
            } => {
                self.error_with_advanced(req_id, time, code, message, advanced_order_reject_json);
            }
            Event::OrderStatus {
                order_id,
                status,
                filled,
                remaining,
                avg_fill_price,
                perm_id,
                parent_id,
                last_fill_price,
                client_id,
                why_held,
                market_cap_price,
            } => {
                self.order_status(
                    order_id,
                    status,
                    filled,
                    remaining,
                    avg_fill_price,
                    perm_id,
                    parent_id,
                    last_fill_price,
                    client_id,
                    why_held,
                    market_cap_price,
                );
            }
            Event::OpenOrder {
                order_id,
                contract,
                order,
                order_state,
            } => {
                self.open_order(order_id, *contract, *order, *order_state);
            }
            Event::ContractDetails { req_id, details } => self.contract_details(req_id, *details),
            Event::BondContractDetails { req_id, details } => {
                self.bond_contract_details(req_id, *details)
            }
            Event::OpenOrderEnd => self.open_order_end(),
            Event::ContractDetailsEnd { req_id } => self.contract_details_end(req_id),
            Event::ExecutionDetails {
                req_id,
                contract,
                execution,
            } => self.execution_details(req_id, *contract, execution),
            Event::ExecutionDetailsEnd { req_id } => self.execution_details_end(req_id),
            Event::MarketDepth {
                req_id,
                position,
                operation,
                side,
                price,
                size,
                market_maker,
                is_smart_depth,
            } => self.market_depth(
                req_id,
                position,
                operation,
                side,
                price,
                size,
                market_maker,
                is_smart_depth,
            ),
            Event::SmartComponents { req_id, components } => {
                self.smart_components(req_id, components)
            }
            Event::RerouteMarketDataRequest {
                req_id,
                con_id,
                exchange,
            } => self.reroute_market_data(req_id, con_id, exchange),
            Event::RerouteMarketDepthRequest {
                req_id,
                con_id,
                exchange,
            } => self.reroute_market_depth(req_id, con_id, exchange),
            Event::HistoricalData { req_id, bar } => self.historical_data(req_id, bar),
            Event::HistoricalDataBars { req_id, bars } => self.historical_data_bars(req_id, bars),
            Event::HistoricalDataUpdate { req_id, bar } => self.historical_data_update(req_id, bar),
            Event::HistoricalDataEnd { req_id, start, end } => {
                self.historical_data_end(req_id, start, end)
            }
            Event::HistoricalTicks {
                req_id,
                ticks,
                done,
            } => self.historical_ticks(req_id, ticks, done),
            Event::HistoricalTicksBidAsk {
                req_id,
                ticks,
                done,
            } => self.historical_ticks_bid_ask(req_id, ticks, done),
            Event::HistoricalTicksLast {
                req_id,
                ticks,
                done,
            } => self.historical_ticks_last(req_id, ticks, done),
            Event::TickByTick {
                req_id,
                tick_type,
                tick,
            } => self.tick_by_tick(req_id, TickType::from_i32(tick_type), tick),
            Event::HistoricalSchedule {
                req_id,
                start_date_time,
                end_date_time,
                time_zone,
                sessions,
            } => self.historical_schedule(
                req_id,
                start_date_time,
                end_date_time,
                time_zone,
                sessions,
            ),
            Event::RealTimeBar { req_id, time, bar } => self.real_time_bar(
                req_id,
                RealTimeBar {
                    time,
                    end_time: time + 5,
                    open: bar.open,
                    high: bar.high,
                    low: bar.low,
                    close: bar.close,
                    volume: bar.volume,
                    wap: bar.wap,
                    count: bar.bar_count,
                },
            ),
            Event::Position {
                account,
                contract,
                position,
                avg_cost,
            } => self.position(account, *contract, position, avg_cost),
            Event::PositionEnd => self.position_end(),
            Event::PositionMulti {
                req_id,
                account,
                model_code,
                contract,
                position,
                avg_cost,
            } => self.position_multi(req_id, account, model_code, *contract, position, avg_cost),
            Event::PositionMultiEnd { req_id } => self.position_multi_end(req_id),
            Event::AccountValue {
                key,
                value,
                currency,
                account_name,
            } => self.account_value(key, value, currency, account_name),
            Event::AccountSummary {
                req_id,
                account,
                tag,
                value,
                currency,
            } => self.account_summary(req_id, account, tag, value, currency),
            Event::PortfolioValue {
                contract,
                position,
                market_price,
                market_value,
                average_cost,
                unrealized_pnl,
                realized_pnl,
                account_name,
            } => self.portfolio_value(
                *contract,
                position,
                market_price,
                market_value,
                average_cost,
                unrealized_pnl,
                realized_pnl,
                account_name,
            ),
            Event::AccountUpdateTime { timestamp } => self.account_update_time(timestamp),
            Event::AccountDownloadEnd { account_name } => self.account_download_end(account_name),
            Event::AccountSummaryEnd { req_id } => self.account_summary_end(req_id),
            Event::AccountUpdateMulti {
                req_id,
                account,
                model_code,
                key,
                value,
                currency,
            } => self.account_update_multi(req_id, account, model_code, key, value, currency),
            Event::AccountUpdateMultiEnd { req_id } => self.account_update_multi_end(req_id),
            Event::FamilyCodes { family_codes } => self.family_codes(
                family_codes
                    .into_iter()
                    .map(|(account_id, family_code)| FamilyCode {
                        account_id,
                        family_code,
                    })
                    .collect(),
            ),
            Event::NewsProviders { providers } => self.news_providers(
                providers
                    .into_iter()
                    .map(|(code, name)| NewsProvider { code, name })
                    .collect(),
            ),
            Event::MarketDepthExchanges { descriptions } => self.market_depth_exchanges(
                descriptions
                    .into_iter()
                    .filter_map(|value| parse_depth_description(&value))
                    .collect(),
            ),
            Event::MarketRule {
                market_rule_id,
                price_increments,
            } => self.market_rule(
                market_rule_id,
                price_increments
                    .into_iter()
                    .map(|(low_edge, increment)| PriceIncrement {
                        low_edge,
                        increment,
                    })
                    .collect(),
            ),
            Event::ReceiveFa { fa_data_type, xml } => {
                self.receive_fa(FaDataType::from_i32(fa_data_type), xml)
            }
            Event::WshEventData { req_id, data_json } => {
                self.wsh_event_data(WshEventData::from_json(req_id, data_json))
            }
            Event::HeadTimestamp {
                req_id,
                head_timestamp,
            } => self.head_timestamp(req_id, head_timestamp),
            Event::HistogramData { req_id, items } => self.histogram_data(req_id, items),
            Event::ScannerParameters { xml } => self.scanner_parameters(xml),
            Event::ScannerData { req_id, rows } => self.scanner_data(req_id, rows),
            Event::ScannerDataEnd { req_id } => self.scanner_data_end(req_id),
            Event::SoftDollarTiers { req_id, tiers } => self.soft_dollar_tiers(req_id, tiers),
            Event::SymbolSamples {
                req_id,
                descriptions,
            } => self.symbol_samples(req_id, descriptions),
            Event::SecurityDefinitionOptionParameter {
                req_id,
                exchange,
                underlying_con_id,
                trading_class,
                multiplier,
                expirations,
                strikes,
            } => self.security_definition_option_parameter(
                req_id,
                exchange,
                underlying_con_id,
                trading_class,
                multiplier,
                expirations,
                strikes,
            ),
            Event::SecurityDefinitionOptionParameterEnd { req_id } => {
                self.security_definition_option_parameter_end(req_id)
            }
            Event::Pnl {
                req_id,
                daily_pnl,
                unrealized_pnl,
                realized_pnl,
            } => self.pnl(req_id, daily_pnl, unrealized_pnl, realized_pnl),
            Event::PnlSingle {
                req_id,
                position,
                daily_pnl,
                unrealized_pnl,
                realized_pnl,
                value,
            } => self.pnl_single(
                req_id,
                position,
                daily_pnl,
                unrealized_pnl,
                realized_pnl,
                value,
            ),
            Event::NewsArticle {
                req_id,
                article_type,
                article_text,
            } => self.news_article(req_id, article_type, article_text),
            Event::NewsBulletin {
                news_msg_id,
                news_msg_type,
                news_message,
                originating_exch,
            } => self.news_bulletin(news_msg_id, news_msg_type, news_message, originating_exch),
            Event::HistoricalNews {
                req_id,
                time,
                provider_code,
                article_id,
                headline,
            } => self.historical_news(req_id, time, provider_code, article_id, headline),
            Event::HistoricalNewsEnd { req_id, has_more } => {
                self.historical_news_end(req_id, has_more)
            }
            Event::TickNews {
                req_id,
                timestamp,
                provider_code,
                article_id,
                headline,
                extra_data,
            } => self.tick_news(
                req_id,
                timestamp,
                provider_code,
                article_id,
                headline,
                extra_data,
            ),
            Event::WshMetaData { req_id, data_json } => self.wsh_metadata(req_id, data_json),
            Event::ReplaceFaEnd { req_id, text } => self.replace_fa_end(req_id, text),
            Event::DisplayGroupList { req_id, groups } => self.display_group_list(req_id, groups),
            Event::DisplayGroupUpdated {
                req_id,
                contract_info,
            } => self.display_group_updated(req_id, contract_info),
            Event::ManagedAccounts { accounts } => self.managed_accounts(accounts),
            Event::VerifyMessageApi { api_data } => self.verify_message_api(api_data),
            Event::VerifyCompleted {
                is_successful,
                error_text,
            } => self.verify_completed(is_successful, error_text),
            Event::VerifyAndAuthMessageApi {
                api_data,
                challenge,
            } => self.verify_and_auth_message_api(api_data, challenge),
            Event::VerifyAndAuthCompleted {
                is_successful,
                error_text,
            } => self.verify_and_auth_completed(is_successful, error_text),
            Event::ConfigResponse {
                req_id,
                status,
                message,
            } => self.config_response(req_id, status, message),
            Event::UpdateConfigResponse {
                req_id,
                status,
                message,
                changed_fields,
                errors,
            } => self.update_config_response(req_id, status, message, changed_fields, errors),
            Event::OrderBound {
                perm_id,
                client_id,
                order_id,
            } => self.order_bound(perm_id, client_id, order_id),
            Event::CompletedOrder {
                contract,
                order,
                order_state,
            } => self.completed_order(*contract, *order, *order_state),
            Event::CompletedOrdersEnd => self.completed_orders_end(),
            Event::UserInfo {
                req_id,
                white_branding_id,
            } => self.user_info(req_id, white_branding_id),
            Event::Raw { msg_id, fields } => self.raw(msg_id, fields),
            Event::RawProtobuf { msg_id, payload } => self.raw_protobuf(msg_id, payload),
        }
    }

    /// Typed market-data mode callback.
    fn market_data_type(&mut self, _req_id: i32, _data_type: MarketDataType) {}
    /// Typed price tick callback.
    fn tick_price(&mut self, _req_id: i32, _tick_type: TickType, _price: f64, _attrib: i32) {}
    /// Typed size tick callback.
    fn tick_size(&mut self, _req_id: i32, _tick_type: TickType, _size: Decimal) {}
    /// Typed generic tick callback.
    fn tick_generic(&mut self, _req_id: i32, _tick_type: TickType, _value: f64) {}
    /// Typed string tick callback.
    fn tick_string(&mut self, _req_id: i32, _tick_type: TickType, _value: String) {}
    /// Error callback including advanced reject details.
    fn error_with_advanced(
        &mut self,
        req_id: i32,
        time: i64,
        code: i32,
        message: String,
        _advanced: String,
    ) {
        self.error(req_id, time, code, message);
    }
    fn order_status(
        &mut self,
        _order_id: i32,
        _status: String,
        _filled: Decimal,
        _remaining: Decimal,
        _avg_fill_price: f64,
        _perm_id: i64,
        _parent_id: i32,
        _last_fill_price: f64,
        _client_id: i32,
        _why_held: String,
        _market_cap_price: f64,
    ) {
    }
    fn open_order(
        &mut self,
        _order_id: i32,
        _contract: Contract,
        _order: Order,
        _order_state: OrderState,
    ) {
    }
    fn contract_details(&mut self, _req_id: i32, _details: ContractDetails) {}
    fn bond_contract_details(&mut self, _req_id: i32, _details: ContractDetails) {}
    fn historical_data(&mut self, _req_id: i32, _bar: BarData) {}
    fn historical_data_bars(&mut self, _req_id: i32, _bars: Vec<BarData>) {}
    fn historical_data_update(&mut self, _req_id: i32, _bar: BarData) {}
    fn real_time_bar(&mut self, _req_id: i32, _bar: RealTimeBar) {}
    fn position(
        &mut self,
        _account: String,
        _contract: Contract,
        _position: Decimal,
        _avg_cost: f64,
    ) {
    }
    fn position_end(&mut self) {}
    fn account_value(
        &mut self,
        _key: String,
        _value: String,
        _currency: String,
        _account_name: String,
    ) {
    }
    fn account_summary(
        &mut self,
        _req_id: i32,
        _account: String,
        _tag: String,
        _value: String,
        _currency: String,
    ) {
    }
    fn family_codes(&mut self, _codes: Vec<FamilyCode>) {}
    fn news_providers(&mut self, _providers: Vec<NewsProvider>) {}
    fn market_depth_exchanges(&mut self, _descriptions: Vec<DepthMarketDataDescription>) {}
    fn market_rule(&mut self, _market_rule_id: i32, _increments: Vec<PriceIncrement>) {}
    fn receive_fa(&mut self, _data_type: FaDataType, _xml: String) {}
    fn wsh_event_data(&mut self, _data: WshEventData) {}
    fn tick_efp(
        &mut self,
        _req_id: i32,
        _tick_type: TickType,
        _basis_points: f64,
        _formatted_basis_points: String,
        _total_dividends: f64,
        _hold_days: i32,
        _future_last_trade_date: String,
        _dividend_impact: f64,
        _dividends_to_last_trade_date: f64,
    ) {
    }
    fn tick_snapshot_end(&mut self, _req_id: i32) {}
    fn tick_option_computation(
        &mut self,
        _req_id: i32,
        _tick_type: TickType,
        _tick_attrib: i32,
        _implied_vol: f64,
        _delta: f64,
        _opt_price: f64,
        _pv_dividend: f64,
        _gamma: f64,
        _vega: f64,
        _theta: f64,
        _und_price: f64,
    ) {
    }
    fn tick_req_params(
        &mut self,
        _req_id: i32,
        _min_tick: String,
        _bbo_exchange: String,
        _snapshot_permissions: i32,
        _last_price_precision: String,
        _last_size_precision: String,
    ) {
    }
    fn commission_and_fees_report(&mut self, _report: CommissionAndFeesReport) {}
    fn delta_neutral_validation(&mut self, _req_id: i32, _contract: DeltaNeutralContract) {}
    fn open_order_end(&mut self) {}
    fn contract_details_end(&mut self, _req_id: i32) {}
    fn execution_details(&mut self, _req_id: i32, _contract: Contract, _execution: Execution) {}
    fn execution_details_end(&mut self, _req_id: i32) {}
    fn market_depth(
        &mut self,
        _req_id: i32,
        _position: i32,
        _operation: i32,
        _side: i32,
        _price: f64,
        _size: Decimal,
        _market_maker: String,
        _is_smart_depth: bool,
    ) {
    }
    fn smart_components(&mut self, _req_id: i32, _components: Vec<SmartComponent>) {}
    fn reroute_market_data(&mut self, _req_id: i32, _con_id: i32, _exchange: String) {}
    fn reroute_market_depth(&mut self, _req_id: i32, _con_id: i32, _exchange: String) {}
    fn historical_data_end(&mut self, _req_id: i32, _start: String, _end: String) {}
    fn historical_ticks(&mut self, _req_id: i32, _ticks: Vec<HistoricalTick>, _done: bool) {}
    fn historical_ticks_bid_ask(
        &mut self,
        _req_id: i32,
        _ticks: Vec<HistoricalTickBidAsk>,
        _done: bool,
    ) {
    }
    fn historical_ticks_last(
        &mut self,
        _req_id: i32,
        _ticks: Vec<HistoricalTickLast>,
        _done: bool,
    ) {
    }
    fn tick_by_tick(&mut self, _req_id: i32, _tick_type: TickType, _tick: Option<TickByTick>) {}
    fn historical_schedule(
        &mut self,
        _req_id: i32,
        _start: String,
        _end: String,
        _time_zone: String,
        _sessions: Vec<HistoricalSession>,
    ) {
    }
    fn position_multi(
        &mut self,
        _req_id: i32,
        _account: String,
        _model_code: String,
        _contract: Contract,
        _position: Decimal,
        _avg_cost: f64,
    ) {
    }
    fn position_multi_end(&mut self, _req_id: i32) {}
    fn portfolio_value(
        &mut self,
        _contract: Contract,
        _position: Decimal,
        _market_price: f64,
        _market_value: f64,
        _average_cost: f64,
        _unrealized_pnl: f64,
        _realized_pnl: f64,
        _account_name: String,
    ) {
    }
    fn account_update_time(&mut self, _timestamp: String) {}
    fn account_download_end(&mut self, _account_name: String) {}
    fn account_summary_end(&mut self, _req_id: i32) {}
    fn account_update_multi(
        &mut self,
        _req_id: i32,
        _account: String,
        _model_code: String,
        _key: String,
        _value: String,
        _currency: String,
    ) {
    }
    fn account_update_multi_end(&mut self, _req_id: i32) {}
    fn head_timestamp(&mut self, _req_id: i32, _timestamp: String) {}
    fn histogram_data(&mut self, _req_id: i32, _items: Vec<(f64, Decimal)>) {}
    fn scanner_parameters(&mut self, _xml: String) {}
    fn scanner_data(&mut self, _req_id: i32, _rows: Vec<ScannerDataRow>) {}
    fn scanner_data_end(&mut self, _req_id: i32) {}
    fn soft_dollar_tiers(&mut self, _req_id: i32, _tiers: Vec<SoftDollarTier>) {}
    fn symbol_samples(&mut self, _req_id: i32, _descriptions: Vec<ContractDescription>) {}
    fn security_definition_option_parameter(
        &mut self,
        _req_id: i32,
        _exchange: String,
        _underlying_con_id: i32,
        _trading_class: String,
        _multiplier: String,
        _expirations: Vec<String>,
        _strikes: Vec<f64>,
    ) {
    }
    fn security_definition_option_parameter_end(&mut self, _req_id: i32) {}
    fn pnl(&mut self, _req_id: i32, _daily_pnl: f64, _unrealized_pnl: f64, _realized_pnl: f64) {}
    fn pnl_single(
        &mut self,
        _req_id: i32,
        _position: Decimal,
        _daily_pnl: f64,
        _unrealized_pnl: f64,
        _realized_pnl: f64,
        _value: f64,
    ) {
    }
    fn news_article(&mut self, _req_id: i32, _article_type: i32, _article_text: String) {}
    fn news_bulletin(&mut self, _msg_id: i32, _msg_type: i32, _message: String, _exchange: String) {
    }
    fn historical_news(
        &mut self,
        _req_id: i32,
        _time: String,
        _provider_code: String,
        _article_id: String,
        _headline: String,
    ) {
    }
    fn historical_news_end(&mut self, _req_id: i32, _has_more: bool) {}
    fn tick_news(
        &mut self,
        _req_id: i32,
        _timestamp: i64,
        _provider_code: String,
        _article_id: String,
        _headline: String,
        _extra_data: String,
    ) {
    }
    fn wsh_metadata(&mut self, _req_id: i32, _data_json: String) {}
    fn replace_fa_end(&mut self, _req_id: i32, _text: String) {}
    fn display_group_list(&mut self, _req_id: i32, _groups: String) {}
    fn display_group_updated(&mut self, _req_id: i32, _contract_info: String) {}
    fn managed_accounts(&mut self, _accounts: String) {}
    fn verify_message_api(&mut self, _api_data: String) {}
    fn verify_completed(&mut self, _is_successful: bool, _error_text: String) {}
    fn verify_and_auth_message_api(&mut self, _api_data: String, _challenge: String) {}
    fn verify_and_auth_completed(&mut self, _is_successful: bool, _error_text: String) {}
    fn config_response(&mut self, _req_id: i32, _status: String, _message: String) {}
    fn update_config_response(
        &mut self,
        _req_id: i32,
        _status: String,
        _message: String,
        _changed_fields: Vec<String>,
        _errors: Vec<String>,
    ) {
    }
    fn order_bound(&mut self, _perm_id: i64, _client_id: i32, _order_id: i32) {}
    fn completed_order(&mut self, _contract: Contract, _order: Order, _order_state: OrderState) {}
    fn completed_orders_end(&mut self) {}
    fn user_info(&mut self, _req_id: i32, _white_branding_id: String) {}
    fn raw(&mut self, _msg_id: i32, _fields: Vec<String>) {}
    fn raw_protobuf(&mut self, _msg_id: i32, _payload: Vec<u8>) {}
}

fn parse_depth_description(value: &str) -> Option<DepthMarketDataDescription> {
    let mut parts = value.split(':');
    Some(DepthMarketDataDescription {
        exchange: parts.next()?.to_owned(),
        security_type: parts.next()?.to_owned(),
        listing_exchange: parts.next()?.to_owned(),
        service_data_type: parts.next()?.to_owned(),
        aggregate_group: parts.next()?.parse().ok()?,
    })
}

impl Event {
    /// Returns a semantic tick type for tick events.
    pub fn tick_type(&self) -> Option<TickType> {
        match self {
            Self::TickPrice { tick_type, .. }
            | Self::TickSize { tick_type, .. }
            | Self::TickGeneric { tick_type, .. }
            | Self::TickString { tick_type, .. }
            | Self::TickOptionComputation { tick_type, .. }
            | Self::TickEfp { tick_type, .. }
            | Self::TickByTick { tick_type, .. } => Some(TickType::from_i32(*tick_type)),
            _ => None,
        }
    }

    /// Returns a semantic market-data type for a market-data-mode event.
    pub fn market_data_type(&self) -> Option<MarketDataType> {
        match self {
            Self::MarketDataType {
                market_data_type, ..
            } => Some(MarketDataType::from_i32(*market_data_type)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestWrapper {
        tick: Option<TickType>,
        wsh_con_id: i32,
    }

    impl Wrapper for TestWrapper {
        fn tick_price(&mut self, _req_id: i32, tick_type: TickType, _price: f64, _attrib: i32) {
            self.tick = Some(tick_type);
        }

        fn wsh_event_data(&mut self, data: WshEventData) {
            self.wsh_con_id = data.con_id;
        }
    }

    #[test]
    fn dispatches_typed_tick_and_wsh_callbacks() {
        let mut wrapper = TestWrapper::default();
        wrapper.dispatch(Event::TickPrice {
            req_id: 1,
            tick_type: 1,
            price: 100.0,
            attrib: 0,
        });
        wrapper.dispatch(Event::WshEventData {
            req_id: 2,
            data_json: r#"{"conId":42,"filter":"e"}"#.to_owned(),
        });
        assert_eq!(wrapper.tick, Some(TickType::Bid));
        assert_eq!(wrapper.wsh_con_id, 42);
    }
}
