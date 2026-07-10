use prost::Message;
use rust_decimal::Decimal;

use crate::error::{TwsApiError, TwsApiResult};
use crate::events::{Event, ScannerDataRow};
use crate::message::{Incoming, PROTOBUF_MSG_ID};
use crate::protobuf;
use crate::types::{
    BarData, ComboLeg, CommissionAndFeesReport, Contract, ContractDescription, ContractDetails,
    DeltaNeutralContract, Execution, HistoricalSession, HistoricalTick, HistoricalTickBidAsk,
    HistoricalTickLast, IneligibilityReason, LegOpenClose, Order, OrderAllocation, OrderCondition,
    OrderState, Origin, SmartComponent, SoftDollarTier, TagValue, TickAttribBidAsk, TickAttribLast,
    TickByTick,
};

mod order;
mod order_proto;

/// Decodes a field-based incoming payload into an [`Event`].
pub fn decode_payload(server_uses_raw_msg_id: bool, payload: &[u8]) -> TwsApiResult<Event> {
    let (msg_id, body) = split_message_id(server_uses_raw_msg_id, payload)?;
    if msg_id > PROTOBUF_MSG_ID {
        return decode_protobuf_event(msg_id - PROTOBUF_MSG_ID, body);
    }

    let fields = crate::comm::read_fields(body)
        .into_iter()
        .map(|field| String::from_utf8_lossy(field).into_owned())
        .collect::<Vec<_>>();

    match Incoming::try_from(msg_id) {
        Ok(Incoming::NextValidId) => {
            let order_id = parse_i32(fields.first())?;
            Ok(Event::NextValidId { order_id })
        }
        Ok(Incoming::CurrentTime) => {
            let time = parse_i64(fields.first())?;
            Ok(Event::CurrentTime { time })
        }
        Ok(Incoming::CurrentTimeInMillis) => {
            let time_in_millis = parse_i64(fields.first())?;
            Ok(Event::CurrentTimeInMillis { time_in_millis })
        }
        Ok(Incoming::ErrorMessage) => {
            let req_id = parse_i32(fields.first())?;
            let code = parse_i32(fields.get(1))?;
            let message = fields.get(2).cloned().unwrap_or_default();
            let advanced_order_reject_json = fields.get(3).cloned().unwrap_or_default();
            Ok(Event::Error {
                req_id,
                time: 0,
                code,
                message,
                advanced_order_reject_json,
            })
        }
        Ok(Incoming::ManagedAccounts) => Ok(Event::ManagedAccounts {
            accounts: fields.first().cloned().unwrap_or_default(),
        }),
        Ok(Incoming::AccountValue) => Ok(Event::AccountValue {
            key: fields.first().cloned().unwrap_or_default(),
            value: fields.get(1).cloned().unwrap_or_default(),
            currency: fields.get(2).cloned().unwrap_or_default(),
            account_name: fields.get(3).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::AccountUpdateTime) => Ok(Event::AccountUpdateTime {
            timestamp: fields.first().cloned().unwrap_or_default(),
        }),
        Ok(Incoming::AccountDownloadEnd) => Ok(Event::AccountDownloadEnd {
            account_name: fields.first().cloned().unwrap_or_default(),
        }),
        Ok(Incoming::NewsBulletins) => Ok(Event::NewsBulletin {
            news_msg_id: parse_i32(fields.first())?,
            news_msg_type: parse_i32(fields.get(1))?,
            news_message: fields.get(2).cloned().unwrap_or_default(),
            originating_exch: fields.get(3).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::ReceiveFa) => Ok(Event::ReceiveFa {
            fa_data_type: parse_i32(fields.first())?,
            xml: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::ScannerParameters) => Ok(Event::ScannerParameters {
            xml: fields.first().cloned().unwrap_or_default(),
        }),
        Ok(Incoming::ScannerData) => decode_scanner_data_fields(&fields),
        Ok(Incoming::SoftDollarTiers) => decode_soft_dollar_tiers_fields(&fields),
        Ok(Incoming::FamilyCodes) => decode_family_codes_fields(&fields),
        Ok(Incoming::SymbolSamples) => decode_symbol_samples_fields(&fields),
        Ok(Incoming::SmartComponents) => decode_smart_components_fields(&fields),
        Ok(Incoming::MarketDepthExchanges) => decode_market_depth_exchanges_fields(&fields),
        Ok(Incoming::HeadTimestamp) => Ok(Event::HeadTimestamp {
            req_id: parse_i32(fields.first())?,
            head_timestamp: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::HistogramData) => decode_histogram_data_fields(&fields),
        Ok(Incoming::RerouteMarketDataRequest) => Ok(Event::RerouteMarketDataRequest {
            req_id: parse_i32(fields.first())?,
            con_id: parse_i32(fields.get(1))?,
            exchange: fields.get(2).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::RerouteMarketDepthRequest) => Ok(Event::RerouteMarketDepthRequest {
            req_id: parse_i32(fields.first())?,
            con_id: parse_i32(fields.get(1))?,
            exchange: fields.get(2).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::MarketRule) => decode_market_rule_fields(&fields),
        Ok(Incoming::DisplayGroupList) => Ok(Event::DisplayGroupList {
            req_id: parse_i32(fields.first())?,
            groups: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::DisplayGroupUpdated) => Ok(Event::DisplayGroupUpdated {
            req_id: parse_i32(fields.first())?,
            contract_info: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::MarketDataType) => Ok(Event::MarketDataType {
            req_id: parse_i32(fields.first())?,
            market_data_type: parse_i32(fields.get(1))?,
        }),
        Ok(Incoming::TickPrice) => Ok(Event::TickPrice {
            req_id: parse_i32(fields.first())?,
            tick_type: parse_i32(fields.get(1))?,
            price: parse_f64(fields.get(2))?,
            attrib: parse_i32(fields.get(3)).unwrap_or_default(),
        }),
        Ok(Incoming::TickSize) => Ok(Event::TickSize {
            req_id: parse_i32(fields.first())?,
            tick_type: parse_i32(fields.get(1))?,
            size: parse_decimal(fields.get(2))?,
        }),
        Ok(Incoming::TickGeneric) => Ok(Event::TickGeneric {
            req_id: parse_i32(fields.first())?,
            tick_type: parse_i32(fields.get(1))?,
            value: parse_f64(fields.get(2))?,
        }),
        Ok(Incoming::TickString) => Ok(Event::TickString {
            req_id: parse_i32(fields.first())?,
            tick_type: parse_i32(fields.get(1))?,
            value: fields.get(2).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::TickOptionComputation) => decode_tick_option_computation_fields(&fields),
        Ok(Incoming::TickEfp) => Ok(Event::TickEfp {
            req_id: parse_i32(fields.first())?,
            tick_type: parse_i32(fields.get(1))?,
            basis_points: parse_f64(fields.get(2))?,
            formatted_basis_points: fields.get(3).cloned().unwrap_or_default(),
            total_dividends: parse_f64(fields.get(4))?,
            hold_days: parse_i32(fields.get(5))?,
            future_last_trade_date: fields.get(6).cloned().unwrap_or_default(),
            dividend_impact: parse_f64(fields.get(7))?,
            dividends_to_last_trade_date: parse_f64(fields.get(8))?,
        }),
        Ok(Incoming::TickRequestParameters) => Ok(Event::TickReqParams {
            req_id: parse_i32(fields.first())?,
            min_tick: fields.get(1).cloned().unwrap_or_default(),
            bbo_exchange: fields.get(2).cloned().unwrap_or_default(),
            snapshot_permissions: parse_i32(fields.get(3))?,
            last_price_precision: String::new(),
            last_size_precision: String::new(),
        }),
        Ok(Incoming::TickSnapshotEnd) => Ok(Event::TickSnapshotEnd {
            req_id: parse_i32(fields.first())?,
        }),
        Ok(Incoming::MarketDepth) => decode_market_depth_fields(&fields, false),
        Ok(Incoming::MarketDepthL2) => decode_market_depth_fields(&fields, true),
        Ok(Incoming::OrderStatus) => {
            let offset = if server_uses_raw_msg_id { 0 } else { 1 };
            Ok(Event::OrderStatus {
                order_id: parse_i32(fields.get(offset))?,
                status: fields.get(offset + 1).cloned().unwrap_or_default(),
                filled: parse_decimal(fields.get(offset + 2))?,
                remaining: parse_decimal(fields.get(offset + 3))?,
                avg_fill_price: parse_f64(fields.get(offset + 4))?,
                perm_id: parse_i64(fields.get(offset + 5))?,
                parent_id: parse_i32(fields.get(offset + 6))?,
                last_fill_price: parse_f64(fields.get(offset + 7))?,
                client_id: parse_i32(fields.get(offset + 8))?,
                why_held: fields.get(offset + 9).cloned().unwrap_or_default(),
                market_cap_price: parse_f64(fields.get(offset + 10)).unwrap_or_default(),
            })
        }
        Ok(Incoming::DeltaNeutralValidation) => Ok(Event::DeltaNeutralValidation {
            req_id: parse_i32(fields.get(1))?,
            delta_neutral_contract: DeltaNeutralContract {
                con_id: parse_i32(fields.get(2))?,
                delta: parse_f64(fields.get(3))?,
                price: parse_f64(fields.get(4))?,
            },
        }),
        Ok(Incoming::CommissionAndFeesReport) => Ok(Event::CommissionAndFeesReport {
            report: CommissionAndFeesReport {
                exec_id: fields.get(1).cloned().unwrap_or_default(),
                commission_and_fees: parse_f64(fields.get(2))?,
                currency: fields.get(3).cloned().unwrap_or_default(),
                realized_pnl: parse_f64(fields.get(4))?,
                bond_yield: parse_f64(fields.get(5))?,
                yield_redemption_date: fields.get(6).cloned().unwrap_or_default(),
            },
        }),
        Ok(Incoming::PortfolioValue) => decode_portfolio_value_fields(&fields),
        Ok(Incoming::OpenOrder) => order::decode_open_order_fields(&fields),
        Ok(Incoming::OpenOrderEnd) => Ok(Event::OpenOrderEnd),
        Ok(Incoming::ContractData) => decode_contract_data_fields(&fields),
        Ok(Incoming::BondContractData) => decode_bond_contract_data_fields(&fields),
        Ok(Incoming::ContractDataEnd) => Ok(Event::ContractDetailsEnd {
            req_id: parse_i32(fields.first())?,
        }),
        Ok(Incoming::ExecutionData) => decode_execution_data_fields(&fields),
        Ok(Incoming::ExecutionDataEnd) => Ok(Event::ExecutionDetailsEnd {
            req_id: parse_i32(fields.first())?,
        }),
        Ok(Incoming::PositionData) => decode_position_fields(&fields),
        Ok(Incoming::PositionEnd) => Ok(Event::PositionEnd),
        Ok(Incoming::PositionMulti) => decode_position_multi_fields(&fields),
        Ok(Incoming::PositionMultiEnd) => Ok(Event::PositionMultiEnd {
            req_id: parse_i32(fields.first())?,
        }),
        Ok(Incoming::AccountSummary) => Ok(Event::AccountSummary {
            req_id: parse_i32(fields.first())?,
            account: fields.get(1).cloned().unwrap_or_default(),
            tag: fields.get(2).cloned().unwrap_or_default(),
            value: fields.get(3).cloned().unwrap_or_default(),
            currency: fields.get(4).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::AccountSummaryEnd) => Ok(Event::AccountSummaryEnd {
            req_id: parse_i32(fields.first())?,
        }),
        Ok(Incoming::AccountUpdateMulti) => Ok(Event::AccountUpdateMulti {
            req_id: parse_i32(fields.first())?,
            account: fields.get(1).cloned().unwrap_or_default(),
            model_code: fields.get(2).cloned().unwrap_or_default(),
            key: fields.get(3).cloned().unwrap_or_default(),
            value: fields.get(4).cloned().unwrap_or_default(),
            currency: fields.get(5).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::AccountUpdateMultiEnd) => Ok(Event::AccountUpdateMultiEnd {
            req_id: parse_i32(fields.first())?,
        }),
        Ok(Incoming::VerifyMessageApi) => Ok(Event::VerifyMessageApi {
            api_data: fields.first().cloned().unwrap_or_default(),
        }),
        Ok(Incoming::VerifyCompleted) => Ok(Event::VerifyCompleted {
            is_successful: parse_bool(fields.first()),
            error_text: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::VerifyAndAuthMessageApi) => Ok(Event::VerifyAndAuthMessageApi {
            api_data: fields.first().cloned().unwrap_or_default(),
            challenge: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::VerifyAndAuthCompleted) => Ok(Event::VerifyAndAuthCompleted {
            is_successful: parse_bool(fields.first()),
            error_text: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::SecurityDefinitionOptionParameter) => {
            decode_security_definition_option_parameter_fields(&fields)
        }
        Ok(Incoming::SecurityDefinitionOptionParameterEnd) => {
            Ok(Event::SecurityDefinitionOptionParameterEnd {
                req_id: parse_i32(fields.first())?,
            })
        }
        Ok(Incoming::HistoricalData) => decode_historical_data_fields(&fields),
        Ok(Incoming::HistoricalDataUpdate) => decode_historical_data_update_fields(&fields),
        Ok(Incoming::HistoricalTicks) => decode_historical_ticks_fields(&fields),
        Ok(Incoming::HistoricalTicksBidAsk) => decode_historical_ticks_bid_ask_fields(&fields),
        Ok(Incoming::HistoricalTicksLast) => decode_historical_ticks_last_fields(&fields),
        Ok(Incoming::TickByTick) => decode_tick_by_tick_fields(&fields),
        Ok(Incoming::RealTimeBars) => decode_real_time_bar_fields(&fields),
        Ok(Incoming::HistoricalSchedule) => decode_historical_schedule_fields(&fields),
        Ok(Incoming::Pnl) => decode_pnl_fields(&fields),
        Ok(Incoming::PnlSingle) => decode_pnl_single_fields(&fields),
        Ok(Incoming::NewsArticle) => Ok(Event::NewsArticle {
            req_id: parse_i32(fields.first())?,
            article_type: parse_i32(fields.get(1))?,
            article_text: fields.get(2).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::NewsProviders) => decode_news_providers_fields(&fields),
        Ok(Incoming::HistoricalNews) => Ok(Event::HistoricalNews {
            req_id: parse_i32(fields.first())?,
            time: fields.get(1).cloned().unwrap_or_default(),
            provider_code: fields.get(2).cloned().unwrap_or_default(),
            article_id: fields.get(3).cloned().unwrap_or_default(),
            headline: fields.get(4).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::HistoricalNewsEnd) => Ok(Event::HistoricalNewsEnd {
            req_id: parse_i32(fields.first())?,
            has_more: parse_bool(fields.get(1)),
        }),
        Ok(Incoming::TickNews) => Ok(Event::TickNews {
            req_id: parse_i32(fields.first())?,
            timestamp: parse_i64(fields.get(1))?,
            provider_code: fields.get(2).cloned().unwrap_or_default(),
            article_id: fields.get(3).cloned().unwrap_or_default(),
            headline: fields.get(4).cloned().unwrap_or_default(),
            extra_data: fields.get(5).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::HistoricalDataEnd) => Ok(Event::HistoricalDataEnd {
            req_id: parse_i32(fields.first())?,
            start: fields.get(1).cloned().unwrap_or_default(),
            end: fields.get(2).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::ReplaceFaEnd) => Ok(Event::ReplaceFaEnd {
            req_id: parse_i32(fields.first())?,
            text: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::WshMetaData) => Ok(Event::WshMetaData {
            req_id: parse_i32(fields.first())?,
            data_json: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::WshEventData) => Ok(Event::WshEventData {
            req_id: parse_i32(fields.first())?,
            data_json: fields.get(1).cloned().unwrap_or_default(),
        }),
        Ok(Incoming::CompletedOrdersEnd) => Ok(Event::CompletedOrdersEnd),
        Ok(Incoming::CompletedOrder) => order::decode_completed_order_fields(&fields),
        Ok(Incoming::OrderBound) => Ok(Event::OrderBound {
            perm_id: parse_i64(fields.first())?,
            client_id: parse_i32(fields.get(1))?,
            order_id: parse_i32(fields.get(2))?,
        }),
        Ok(Incoming::UserInfo) => Ok(Event::UserInfo {
            req_id: parse_i32(fields.first())?,
            white_branding_id: fields.get(1).cloned().unwrap_or_default(),
        }),
        _ => Ok(Event::Raw { msg_id, fields }),
    }
}

/// Decodes a protobuf incoming payload into an [`Event`].
pub fn decode_protobuf_event(msg_id: i32, payload: &[u8]) -> TwsApiResult<Event> {
    match Incoming::try_from(msg_id) {
        Ok(Incoming::NextValidId) => {
            let msg = protobuf::NextValidId::decode(payload)?;
            Ok(Event::NextValidId {
                order_id: msg.order_id.unwrap_or_default(),
            })
        }
        Ok(Incoming::CurrentTime) => {
            let msg = protobuf::CurrentTime::decode(payload)?;
            Ok(Event::CurrentTime {
                time: msg.current_time.unwrap_or_default(),
            })
        }
        Ok(Incoming::CurrentTimeInMillis) => {
            let msg = protobuf::CurrentTimeInMillis::decode(payload)?;
            Ok(Event::CurrentTimeInMillis {
                time_in_millis: msg.current_time_in_millis.unwrap_or_default(),
            })
        }
        Ok(Incoming::ErrorMessage) => {
            let msg = protobuf::ErrorMessage::decode(payload)?;
            Ok(Event::Error {
                req_id: msg.id.unwrap_or_default(),
                time: msg.error_time.unwrap_or_default(),
                code: msg.error_code.unwrap_or_default(),
                message: msg.error_msg.unwrap_or_default(),
                advanced_order_reject_json: msg.advanced_order_reject_json.unwrap_or_default(),
            })
        }
        Ok(Incoming::ManagedAccounts) => {
            let msg = protobuf::ManagedAccounts::decode(payload)?;
            Ok(Event::ManagedAccounts {
                accounts: msg.accounts_list.unwrap_or_default(),
            })
        }
        Ok(Incoming::OrderStatus) => {
            let msg = protobuf::OrderStatus::decode(payload)?;
            Ok(Event::OrderStatus {
                order_id: msg.order_id.unwrap_or_default(),
                status: msg.status.unwrap_or_default(),
                filled: parse_decimal_string(msg.filled.as_deref())?,
                remaining: parse_decimal_string(msg.remaining.as_deref())?,
                avg_fill_price: msg.avg_fill_price.unwrap_or_default(),
                perm_id: msg.perm_id.unwrap_or_default(),
                parent_id: msg.parent_id.unwrap_or_default(),
                last_fill_price: msg.last_fill_price.unwrap_or_default(),
                client_id: msg.client_id.unwrap_or_default(),
                why_held: msg.why_held.unwrap_or_default(),
                market_cap_price: msg.mkt_cap_price.unwrap_or_default(),
            })
        }
        Ok(Incoming::TickString) => {
            let msg = protobuf::TickString::decode(payload)?;
            Ok(Event::TickString {
                req_id: msg.req_id.unwrap_or_default(),
                tick_type: msg.tick_type.unwrap_or_default(),
                value: msg.value.unwrap_or_default(),
            })
        }
        Ok(Incoming::TickPrice) => {
            let msg = protobuf::TickPrice::decode(payload)?;
            Ok(Event::TickPrice {
                req_id: msg.req_id.unwrap_or_default(),
                tick_type: msg.tick_type.unwrap_or_default(),
                price: msg.price.unwrap_or_default(),
                attrib: msg.attr_mask.unwrap_or_default(),
            })
        }
        Ok(Incoming::TickSize) => {
            let msg = protobuf::TickSize::decode(payload)?;
            Ok(Event::TickSize {
                req_id: msg.req_id.unwrap_or_default(),
                tick_type: msg.tick_type.unwrap_or_default(),
                size: parse_decimal_string(msg.size.as_deref())?,
            })
        }
        Ok(Incoming::TickGeneric) => {
            let msg = protobuf::TickGeneric::decode(payload)?;
            Ok(Event::TickGeneric {
                req_id: msg.req_id.unwrap_or_default(),
                tick_type: msg.tick_type.unwrap_or_default(),
                value: msg.value.unwrap_or_default(),
            })
        }
        Ok(Incoming::TickOptionComputation) => {
            let msg = protobuf::TickOptionComputation::decode(payload)?;
            Ok(Event::TickOptionComputation {
                req_id: msg.req_id.unwrap_or_default(),
                tick_type: msg.tick_type.unwrap_or_default(),
                tick_attrib: msg.tick_attrib.unwrap_or_default(),
                implied_vol: msg.implied_vol.unwrap_or_default(),
                delta: msg.delta.unwrap_or_default(),
                opt_price: msg.opt_price.unwrap_or_default(),
                pv_dividend: msg.pv_dividend.unwrap_or_default(),
                gamma: msg.gamma.unwrap_or_default(),
                vega: msg.vega.unwrap_or_default(),
                theta: msg.theta.unwrap_or_default(),
                und_price: msg.und_price.unwrap_or_default(),
            })
        }
        Ok(Incoming::TickRequestParameters) => {
            let msg = protobuf::TickReqParams::decode(payload)?;
            Ok(Event::TickReqParams {
                req_id: msg.req_id.unwrap_or_default(),
                min_tick: msg.min_tick.unwrap_or_default(),
                bbo_exchange: msg.bbo_exchange.unwrap_or_default(),
                snapshot_permissions: msg.snapshot_permissions.unwrap_or_default(),
                last_price_precision: msg.last_price_precision.unwrap_or_default(),
                last_size_precision: msg.last_size_precision.unwrap_or_default(),
            })
        }
        Ok(Incoming::CommissionAndFeesReport) => {
            let msg = protobuf::CommissionAndFeesReport::decode(payload)?;
            Ok(Event::CommissionAndFeesReport {
                report: CommissionAndFeesReport {
                    exec_id: msg.exec_id.unwrap_or_default(),
                    commission_and_fees: msg.commission_and_fees.unwrap_or_default(),
                    currency: msg.currency.unwrap_or_default(),
                    realized_pnl: msg.realized_pnl.unwrap_or_default(),
                    bond_yield: msg.bond_yield.unwrap_or_default(),
                    yield_redemption_date: msg.yield_redemption_date.unwrap_or_default(),
                },
            })
        }
        Ok(Incoming::TickSnapshotEnd) => {
            let msg = protobuf::TickSnapshotEnd::decode(payload)?;
            Ok(Event::TickSnapshotEnd {
                req_id: msg.req_id.unwrap_or_default(),
            })
        }
        Ok(Incoming::MarketDataType) => {
            let msg = protobuf::MarketDataType::decode(payload)?;
            Ok(Event::MarketDataType {
                req_id: msg.req_id.unwrap_or_default(),
                market_data_type: msg.market_data_type.unwrap_or_default(),
            })
        }
        Ok(Incoming::OpenOrderEnd) => Ok(Event::OpenOrderEnd),
        Ok(Incoming::AccountValue) => {
            let msg = protobuf::AccountValue::decode(payload)?;
            Ok(Event::AccountValue {
                key: msg.key.unwrap_or_default(),
                value: msg.value.unwrap_or_default(),
                currency: msg.currency.unwrap_or_default(),
                account_name: msg.account_name.unwrap_or_default(),
            })
        }
        Ok(Incoming::PortfolioValue) => {
            let msg = protobuf::PortfolioValue::decode(payload)?;
            Ok(Event::PortfolioValue {
                contract: Box::new(proto_contract_to_contract(msg.contract)),
                position: parse_decimal_string(msg.position.as_deref())?,
                market_price: msg.market_price.unwrap_or_default(),
                market_value: msg.market_value.unwrap_or_default(),
                average_cost: msg.average_cost.unwrap_or_default(),
                unrealized_pnl: msg.unrealized_pnl.unwrap_or_default(),
                realized_pnl: msg.realized_pnl.unwrap_or_default(),
                account_name: msg.account_name.unwrap_or_default(),
            })
        }
        Ok(Incoming::AccountUpdateTime) => {
            let msg = protobuf::AccountUpdateTime::decode(payload)?;
            Ok(Event::AccountUpdateTime {
                timestamp: msg.time_stamp.unwrap_or_default(),
            })
        }
        Ok(Incoming::AccountDownloadEnd) => {
            let msg = protobuf::AccountDataEnd::decode(payload)?;
            Ok(Event::AccountDownloadEnd {
                account_name: msg.account_name.unwrap_or_default(),
            })
        }
        Ok(Incoming::MarketDepth) => {
            let msg = protobuf::MarketDepth::decode(payload)?;
            proto_market_depth_to_event(msg.req_id, msg.market_depth_data)
        }
        Ok(Incoming::MarketDepthL2) => {
            let msg = protobuf::MarketDepthL2::decode(payload)?;
            proto_market_depth_to_event(msg.req_id, msg.market_depth_data)
        }
        Ok(Incoming::MarketDepthExchanges) => {
            let msg = protobuf::MarketDepthExchanges::decode(payload)?;
            Ok(Event::MarketDepthExchanges {
                descriptions: msg
                    .depth_market_data_descriptions
                    .into_iter()
                    .map(|description| format!("{description:?}"))
                    .collect(),
            })
        }
        Ok(Incoming::SmartComponents) => {
            let msg = protobuf::SmartComponents::decode(payload)?;
            Ok(Event::SmartComponents {
                req_id: msg.req_id.unwrap_or_default(),
                components: msg
                    .smart_components
                    .into_iter()
                    .map(proto_smart_component)
                    .collect(),
            })
        }
        Ok(Incoming::RerouteMarketDataRequest) => {
            let msg = protobuf::RerouteMarketDataRequest::decode(payload)?;
            Ok(Event::RerouteMarketDataRequest {
                req_id: msg.req_id.unwrap_or_default(),
                con_id: msg.con_id.unwrap_or_default(),
                exchange: msg.exchange.unwrap_or_default(),
            })
        }
        Ok(Incoming::RerouteMarketDepthRequest) => {
            let msg = protobuf::RerouteMarketDepthRequest::decode(payload)?;
            Ok(Event::RerouteMarketDepthRequest {
                req_id: msg.req_id.unwrap_or_default(),
                con_id: msg.con_id.unwrap_or_default(),
                exchange: msg.exchange.unwrap_or_default(),
            })
        }
        Ok(Incoming::OpenOrder) => {
            let msg = protobuf::OpenOrder::decode(payload)?;
            Ok(Event::OpenOrder {
                order_id: msg.order_id.unwrap_or_default(),
                contract: Box::new(proto_contract_to_contract(msg.contract)),
                order: Box::new(order_proto::proto_order_to_order(msg.order)?),
                order_state: Box::new(order_proto::proto_order_state_to_order_state(
                    msg.order_state,
                )),
            })
        }
        Ok(Incoming::ContractData) => {
            let msg = protobuf::ContractData::decode(payload)?;
            Ok(Event::ContractDetails {
                req_id: msg.req_id.unwrap_or_default(),
                details: Box::new(proto_contract_details_to_contract_details(
                    msg.contract,
                    msg.contract_details,
                )?),
            })
        }
        Ok(Incoming::BondContractData) => {
            let msg = protobuf::ContractData::decode(payload)?;
            Ok(Event::BondContractDetails {
                req_id: msg.req_id.unwrap_or_default(),
                details: Box::new(proto_contract_details_to_contract_details(
                    msg.contract,
                    msg.contract_details,
                )?),
            })
        }
        Ok(Incoming::ContractDataEnd) => {
            let msg = protobuf::ContractDataEnd::decode(payload)?;
            Ok(Event::ContractDetailsEnd {
                req_id: msg.req_id.unwrap_or_default(),
            })
        }
        Ok(Incoming::ExecutionData) => {
            let msg = protobuf::ExecutionDetails::decode(payload)?;
            Ok(Event::ExecutionDetails {
                req_id: msg.req_id.unwrap_or_default(),
                contract: Box::new(proto_contract_to_contract(msg.contract)),
                execution: proto_execution_to_execution(msg.execution)?,
            })
        }
        Ok(Incoming::ExecutionDataEnd) => {
            let msg = protobuf::ExecutionDetailsEnd::decode(payload)?;
            Ok(Event::ExecutionDetailsEnd {
                req_id: msg.req_id.unwrap_or_default(),
            })
        }
        Ok(Incoming::HistoricalData) => {
            let msg = protobuf::HistoricalData::decode(payload)?;
            Ok(Event::HistoricalDataBars {
                req_id: msg.req_id.unwrap_or_default(),
                bars: msg
                    .historical_data_bars
                    .into_iter()
                    .map(proto_bar_to_bar)
                    .collect::<TwsApiResult<Vec<_>>>()?,
            })
        }
        Ok(Incoming::HistoricalDataUpdate) => {
            let msg = protobuf::HistoricalDataUpdate::decode(payload)?;
            Ok(Event::HistoricalDataUpdate {
                req_id: msg.req_id.unwrap_or_default(),
                bar: proto_bar_to_bar(msg.historical_data_bar.unwrap_or_default())?,
            })
        }
        Ok(Incoming::RealTimeBars) => {
            let msg = protobuf::RealTimeBarTick::decode(payload)?;
            Ok(Event::RealTimeBar {
                req_id: msg.req_id.unwrap_or_default(),
                time: msg.time.unwrap_or_default(),
                bar: BarData {
                    date: msg.time.unwrap_or_default().to_string(),
                    open: msg.open.unwrap_or_default(),
                    high: msg.high.unwrap_or_default(),
                    low: msg.low.unwrap_or_default(),
                    close: msg.close.unwrap_or_default(),
                    volume: parse_decimal_string(msg.volume.as_deref())?,
                    wap: parse_decimal_string(msg.wap.as_deref())?,
                    bar_count: msg.count.unwrap_or_default(),
                },
            })
        }
        Ok(Incoming::HeadTimestamp) => {
            let msg = protobuf::HeadTimestamp::decode(payload)?;
            Ok(Event::HeadTimestamp {
                req_id: msg.req_id.unwrap_or_default(),
                head_timestamp: msg.head_timestamp.unwrap_or_default(),
            })
        }
        Ok(Incoming::HistogramData) => {
            let msg = protobuf::HistogramData::decode(payload)?;
            Ok(Event::HistogramData {
                req_id: msg.req_id.unwrap_or_default(),
                items: msg
                    .histogram_data_entries
                    .into_iter()
                    .map(|entry| {
                        Ok((
                            entry.price.unwrap_or_default(),
                            parse_decimal_string(entry.size.as_deref())?,
                        ))
                    })
                    .collect::<TwsApiResult<Vec<_>>>()?,
            })
        }
        Ok(Incoming::ScannerParameters) => {
            let msg = protobuf::ScannerParameters::decode(payload)?;
            Ok(Event::ScannerParameters {
                xml: msg.xml.unwrap_or_default(),
            })
        }
        Ok(Incoming::ScannerData) => {
            let msg = protobuf::ScannerData::decode(payload)?;
            Ok(Event::ScannerData {
                req_id: msg.req_id.unwrap_or_default(),
                rows: msg
                    .scanner_data_element
                    .into_iter()
                    .map(|row| ScannerDataRow {
                        rank: row.rank.unwrap_or_default(),
                        contract: Box::new(proto_contract_to_contract(row.contract)),
                        market_name: row.market_name.unwrap_or_default(),
                        distance: row.distance.unwrap_or_default(),
                        benchmark: row.benchmark.unwrap_or_default(),
                        projection: row.projection.unwrap_or_default(),
                        combo_key: row.combo_key.unwrap_or_default(),
                    })
                    .collect(),
            })
        }
        Ok(Incoming::SoftDollarTiers) => {
            let msg = protobuf::SoftDollarTiers::decode(payload)?;
            Ok(Event::SoftDollarTiers {
                req_id: msg.req_id.unwrap_or_default(),
                tiers: msg
                    .soft_dollar_tiers
                    .into_iter()
                    .map(|tier| SoftDollarTier {
                        name: tier.name.unwrap_or_default(),
                        value: tier.value.unwrap_or_default(),
                        display_name: tier.display_name.unwrap_or_default(),
                    })
                    .collect(),
            })
        }
        Ok(Incoming::FamilyCodes) => {
            let msg = protobuf::FamilyCodes::decode(payload)?;
            Ok(Event::FamilyCodes {
                family_codes: msg
                    .family_codes
                    .into_iter()
                    .map(|code| {
                        (
                            code.account_id.unwrap_or_default(),
                            code.family_code.unwrap_or_default(),
                        )
                    })
                    .collect(),
            })
        }
        Ok(Incoming::SymbolSamples) => {
            let msg = protobuf::SymbolSamples::decode(payload)?;
            Ok(Event::SymbolSamples {
                req_id: msg.req_id.unwrap_or_default(),
                descriptions: msg
                    .contract_descriptions
                    .into_iter()
                    .map(proto_contract_description)
                    .collect(),
            })
        }
        Ok(Incoming::MarketRule) => {
            let msg = protobuf::MarketRule::decode(payload)?;
            Ok(Event::MarketRule {
                market_rule_id: msg.market_rule_id.unwrap_or_default(),
                price_increments: msg
                    .price_increments
                    .into_iter()
                    .map(|increment| {
                        (
                            increment.low_edge.unwrap_or_default(),
                            increment.increment.unwrap_or_default(),
                        )
                    })
                    .collect(),
            })
        }
        Ok(Incoming::PositionData) => {
            let msg = protobuf::Position::decode(payload)?;
            Ok(Event::Position {
                account: msg.account.unwrap_or_default(),
                contract: Box::new(proto_contract_to_contract(msg.contract)),
                position: parse_decimal_string(msg.position.as_deref())?,
                avg_cost: msg.avg_cost.unwrap_or_default(),
            })
        }
        Ok(Incoming::PositionEnd) => Ok(Event::PositionEnd),
        Ok(Incoming::PositionMulti) => {
            let msg = protobuf::PositionMulti::decode(payload)?;
            Ok(Event::PositionMulti {
                req_id: msg.req_id.unwrap_or_default(),
                account: msg.account.unwrap_or_default(),
                model_code: msg.model_code.unwrap_or_default(),
                contract: Box::new(proto_contract_to_contract(msg.contract)),
                position: parse_decimal_string(msg.position.as_deref())?,
                avg_cost: msg.avg_cost.unwrap_or_default(),
            })
        }
        Ok(Incoming::PositionMultiEnd) => {
            let msg = protobuf::PositionMultiEnd::decode(payload)?;
            Ok(Event::PositionMultiEnd {
                req_id: msg.req_id.unwrap_or_default(),
            })
        }
        Ok(Incoming::AccountSummary) => {
            let msg = protobuf::AccountSummary::decode(payload)?;
            Ok(Event::AccountSummary {
                req_id: msg.req_id.unwrap_or_default(),
                account: msg.account.unwrap_or_default(),
                tag: msg.tag.unwrap_or_default(),
                value: msg.value.unwrap_or_default(),
                currency: msg.currency.unwrap_or_default(),
            })
        }
        Ok(Incoming::AccountSummaryEnd) => {
            let msg = protobuf::AccountSummaryEnd::decode(payload)?;
            Ok(Event::AccountSummaryEnd {
                req_id: msg.req_id.unwrap_or_default(),
            })
        }
        Ok(Incoming::AccountUpdateMulti) => {
            let msg = protobuf::AccountUpdateMulti::decode(payload)?;
            Ok(Event::AccountUpdateMulti {
                req_id: msg.req_id.unwrap_or_default(),
                account: msg.account.unwrap_or_default(),
                model_code: msg.model_code.unwrap_or_default(),
                key: msg.key.unwrap_or_default(),
                value: msg.value.unwrap_or_default(),
                currency: msg.currency.unwrap_or_default(),
            })
        }
        Ok(Incoming::AccountUpdateMultiEnd) => {
            let msg = protobuf::AccountUpdateMultiEnd::decode(payload)?;
            Ok(Event::AccountUpdateMultiEnd {
                req_id: msg.req_id.unwrap_or_default(),
            })
        }
        Ok(Incoming::HistoricalDataEnd) => {
            let msg = protobuf::HistoricalDataEnd::decode(payload)?;
            Ok(Event::HistoricalDataEnd {
                req_id: msg.req_id.unwrap_or_default(),
                start: msg.start_date_str.unwrap_or_default(),
                end: msg.end_date_str.unwrap_or_default(),
            })
        }
        Ok(Incoming::HistoricalTicks) => {
            let msg = protobuf::HistoricalTicks::decode(payload)?;
            Ok(Event::HistoricalTicks {
                req_id: msg.req_id.unwrap_or_default(),
                ticks: msg
                    .historical_ticks
                    .into_iter()
                    .map(proto_historical_tick)
                    .collect::<TwsApiResult<Vec<_>>>()?,
                done: msg.is_done.unwrap_or_default(),
            })
        }
        Ok(Incoming::HistoricalTicksBidAsk) => {
            let msg = protobuf::HistoricalTicksBidAsk::decode(payload)?;
            Ok(Event::HistoricalTicksBidAsk {
                req_id: msg.req_id.unwrap_or_default(),
                ticks: msg
                    .historical_ticks_bid_ask
                    .into_iter()
                    .map(proto_historical_tick_bid_ask)
                    .collect::<TwsApiResult<Vec<_>>>()?,
                done: msg.is_done.unwrap_or_default(),
            })
        }
        Ok(Incoming::HistoricalTicksLast) => {
            let msg = protobuf::HistoricalTicksLast::decode(payload)?;
            Ok(Event::HistoricalTicksLast {
                req_id: msg.req_id.unwrap_or_default(),
                ticks: msg
                    .historical_ticks_last
                    .into_iter()
                    .map(proto_historical_tick_last)
                    .collect::<TwsApiResult<Vec<_>>>()?,
                done: msg.is_done.unwrap_or_default(),
            })
        }
        Ok(Incoming::TickByTick) => {
            let msg = protobuf::TickByTickData::decode(payload)?;
            Ok(Event::TickByTick {
                req_id: msg.req_id.unwrap_or_default(),
                tick_type: msg.tick_type.unwrap_or_default(),
                tick: proto_tick_by_tick(msg.tick)?,
            })
        }
        Ok(Incoming::HistoricalSchedule) => {
            let msg = protobuf::HistoricalSchedule::decode(payload)?;
            Ok(Event::HistoricalSchedule {
                req_id: msg.req_id.unwrap_or_default(),
                start_date_time: msg.start_date_time.unwrap_or_default(),
                end_date_time: msg.end_date_time.unwrap_or_default(),
                time_zone: msg.time_zone.unwrap_or_default(),
                sessions: msg
                    .historical_sessions
                    .into_iter()
                    .map(proto_historical_session)
                    .collect(),
            })
        }
        Ok(Incoming::Pnl) => {
            let msg = protobuf::PnL::decode(payload)?;
            Ok(Event::Pnl {
                req_id: msg.req_id.unwrap_or_default(),
                daily_pnl: msg.daily_pn_l.unwrap_or_default(),
                unrealized_pnl: msg.unrealized_pn_l.unwrap_or_default(),
                realized_pnl: msg.realized_pn_l.unwrap_or_default(),
            })
        }
        Ok(Incoming::PnlSingle) => {
            let msg = protobuf::PnLSingle::decode(payload)?;
            Ok(Event::PnlSingle {
                req_id: msg.req_id.unwrap_or_default(),
                position: parse_decimal_string(msg.position.as_deref())?,
                daily_pnl: msg.daily_pn_l.unwrap_or_default(),
                unrealized_pnl: msg.unrealized_pn_l.unwrap_or_default(),
                realized_pnl: msg.realized_pn_l.unwrap_or_default(),
                value: msg.value.unwrap_or_default(),
            })
        }
        Ok(Incoming::NewsArticle) => {
            let msg = protobuf::NewsArticle::decode(payload)?;
            Ok(Event::NewsArticle {
                req_id: msg.req_id.unwrap_or_default(),
                article_type: msg.article_type.unwrap_or_default(),
                article_text: msg.article_text.unwrap_or_default(),
            })
        }
        Ok(Incoming::NewsBulletins) => {
            let msg = protobuf::NewsBulletin::decode(payload)?;
            Ok(Event::NewsBulletin {
                news_msg_id: msg.news_msg_id.unwrap_or_default(),
                news_msg_type: msg.news_msg_type.unwrap_or_default(),
                news_message: msg.news_message.unwrap_or_default(),
                originating_exch: msg.originating_exch.unwrap_or_default(),
            })
        }
        Ok(Incoming::NewsProviders) => {
            let msg = protobuf::NewsProviders::decode(payload)?;
            Ok(Event::NewsProviders {
                providers: msg
                    .news_providers
                    .into_iter()
                    .map(|provider| {
                        (
                            provider.provider_code.unwrap_or_default(),
                            provider.provider_name.unwrap_or_default(),
                        )
                    })
                    .collect(),
            })
        }
        Ok(Incoming::HistoricalNews) => {
            let msg = protobuf::HistoricalNews::decode(payload)?;
            Ok(Event::HistoricalNews {
                req_id: msg.req_id.unwrap_or_default(),
                time: msg.time.unwrap_or_default(),
                provider_code: msg.provider_code.unwrap_or_default(),
                article_id: msg.article_id.unwrap_or_default(),
                headline: msg.headline.unwrap_or_default(),
            })
        }
        Ok(Incoming::HistoricalNewsEnd) => {
            let msg = protobuf::HistoricalNewsEnd::decode(payload)?;
            Ok(Event::HistoricalNewsEnd {
                req_id: msg.req_id.unwrap_or_default(),
                has_more: msg.has_more.unwrap_or_default(),
            })
        }
        Ok(Incoming::TickNews) => {
            let msg = protobuf::TickNews::decode(payload)?;
            Ok(Event::TickNews {
                req_id: msg.req_id.unwrap_or_default(),
                timestamp: msg.timestamp.unwrap_or_default(),
                provider_code: msg.provider_code.unwrap_or_default(),
                article_id: msg.article_id.unwrap_or_default(),
                headline: msg.headline.unwrap_or_default(),
                extra_data: msg.extra_data.unwrap_or_default(),
            })
        }
        Ok(Incoming::WshMetaData) => {
            let msg = protobuf::WshMetaData::decode(payload)?;
            Ok(Event::WshMetaData {
                req_id: msg.req_id.unwrap_or_default(),
                data_json: msg.data_json.unwrap_or_default(),
            })
        }
        Ok(Incoming::WshEventData) => {
            let msg = protobuf::WshEventData::decode(payload)?;
            Ok(Event::WshEventData {
                req_id: msg.req_id.unwrap_or_default(),
                data_json: msg.data_json.unwrap_or_default(),
            })
        }
        Ok(Incoming::ReceiveFa) => {
            let msg = protobuf::ReceiveFa::decode(payload)?;
            Ok(Event::ReceiveFa {
                fa_data_type: msg.fa_data_type.unwrap_or_default(),
                xml: msg.xml.unwrap_or_default(),
            })
        }
        Ok(Incoming::ReplaceFaEnd) => {
            let msg = protobuf::ReplaceFaEnd::decode(payload)?;
            Ok(Event::ReplaceFaEnd {
                req_id: msg.req_id.unwrap_or_default(),
                text: msg.text.unwrap_or_default(),
            })
        }
        Ok(Incoming::DisplayGroupList) => {
            let msg = protobuf::DisplayGroupList::decode(payload)?;
            Ok(Event::DisplayGroupList {
                req_id: msg.req_id.unwrap_or_default(),
                groups: msg.groups.unwrap_or_default(),
            })
        }
        Ok(Incoming::DisplayGroupUpdated) => {
            let msg = protobuf::DisplayGroupUpdated::decode(payload)?;
            Ok(Event::DisplayGroupUpdated {
                req_id: msg.req_id.unwrap_or_default(),
                contract_info: msg.contract_info.unwrap_or_default(),
            })
        }
        Ok(Incoming::VerifyMessageApi) => {
            let msg = protobuf::VerifyMessageApi::decode(payload)?;
            Ok(Event::VerifyMessageApi {
                api_data: msg.api_data.unwrap_or_default(),
            })
        }
        Ok(Incoming::VerifyCompleted) => {
            let msg = protobuf::VerifyCompleted::decode(payload)?;
            Ok(Event::VerifyCompleted {
                is_successful: msg.is_successful.unwrap_or_default(),
                error_text: msg.error_text.unwrap_or_default(),
            })
        }
        Ok(Incoming::OrderBound) => {
            let msg = protobuf::OrderBound::decode(payload)?;
            Ok(Event::OrderBound {
                perm_id: msg.perm_id.unwrap_or_default(),
                client_id: msg.client_id.unwrap_or_default(),
                order_id: msg.order_id.unwrap_or_default(),
            })
        }
        Ok(Incoming::CompletedOrder) => {
            let msg = protobuf::CompletedOrder::decode(payload)?;
            Ok(Event::CompletedOrder {
                contract: Box::new(proto_contract_to_contract(msg.contract)),
                order: Box::new(order_proto::proto_order_to_order(msg.order)?),
                order_state: Box::new(order_proto::proto_order_state_to_order_state(
                    msg.order_state,
                )),
            })
        }
        Ok(Incoming::CompletedOrdersEnd) => Ok(Event::CompletedOrdersEnd),
        Ok(Incoming::UserInfo) => {
            let msg = protobuf::UserInfo::decode(payload)?;
            Ok(Event::UserInfo {
                req_id: msg.req_id.unwrap_or_default(),
                white_branding_id: msg.white_branding_id.unwrap_or_default(),
            })
        }
        Ok(Incoming::ConfigResponse) => {
            let msg = protobuf::ConfigResponse::decode(payload)?;
            Ok(Event::ConfigResponse {
                req_id: msg.req_id.unwrap_or_default(),
                status: String::new(),
                message: format!(
                    "lock_and_exit={}, messages={}, api={}, orders={}",
                    msg.lock_and_exit.is_some(),
                    msg.messages.len(),
                    msg.api.is_some(),
                    msg.orders.is_some()
                ),
            })
        }
        Ok(Incoming::UpdateConfigResponse) => {
            let msg = protobuf::UpdateConfigResponse::decode(payload)?;
            Ok(Event::UpdateConfigResponse {
                req_id: msg.req_id.unwrap_or_default(),
                status: msg.status.unwrap_or_default(),
                message: msg.message.unwrap_or_default(),
                changed_fields: msg.changed_fields,
                errors: msg.errors,
            })
        }
        _ => Ok(Event::RawProtobuf {
            msg_id,
            payload: payload.to_vec(),
        }),
    }
}

fn proto_market_depth_to_event(
    req_id: Option<i32>,
    data: Option<protobuf::MarketDepthData>,
) -> TwsApiResult<Event> {
    let data = data.unwrap_or_default();
    Ok(Event::MarketDepth {
        req_id: req_id.unwrap_or_default(),
        position: data.position.unwrap_or_default(),
        operation: data.operation.unwrap_or_default(),
        side: data.side.unwrap_or_default(),
        price: data.price.unwrap_or_default(),
        size: parse_decimal_string(data.size.as_deref())?,
        market_maker: data.market_maker.unwrap_or_default(),
        is_smart_depth: data.is_smart_depth.unwrap_or_default(),
    })
}

fn proto_bar_to_bar(bar: protobuf::HistoricalDataBar) -> TwsApiResult<BarData> {
    Ok(BarData {
        date: bar.date.unwrap_or_default(),
        open: bar.open.unwrap_or_default(),
        high: bar.high.unwrap_or_default(),
        low: bar.low.unwrap_or_default(),
        close: bar.close.unwrap_or_default(),
        volume: parse_decimal_string(bar.volume.as_deref())?,
        wap: parse_decimal_string(bar.wap.as_deref())?,
        bar_count: bar.bar_count.unwrap_or_default(),
    })
}

fn proto_historical_tick(tick: protobuf::HistoricalTick) -> TwsApiResult<HistoricalTick> {
    Ok(HistoricalTick {
        time: tick.time.unwrap_or_default(),
        price: tick.price.unwrap_or_default(),
        size: parse_decimal_string(tick.size.as_deref())?,
    })
}

fn proto_tick_attrib_bid_ask(attrib: Option<protobuf::TickAttribBidAsk>) -> TickAttribBidAsk {
    let Some(attrib) = attrib else {
        return TickAttribBidAsk::default();
    };

    TickAttribBidAsk {
        bid_past_low: attrib.bid_past_low.unwrap_or_default(),
        ask_past_high: attrib.ask_past_high.unwrap_or_default(),
    }
}

fn proto_tick_attrib_last(attrib: Option<protobuf::TickAttribLast>) -> TickAttribLast {
    let Some(attrib) = attrib else {
        return TickAttribLast::default();
    };

    TickAttribLast {
        past_limit: attrib.past_limit.unwrap_or_default(),
        unreported: attrib.unreported.unwrap_or_default(),
    }
}

fn proto_historical_tick_bid_ask(
    tick: protobuf::HistoricalTickBidAsk,
) -> TwsApiResult<HistoricalTickBidAsk> {
    Ok(HistoricalTickBidAsk {
        time: tick.time.unwrap_or_default(),
        tick_attrib_bid_ask: proto_tick_attrib_bid_ask(tick.tick_attrib_bid_ask),
        price_bid: tick.price_bid.unwrap_or_default(),
        price_ask: tick.price_ask.unwrap_or_default(),
        size_bid: parse_decimal_string(tick.size_bid.as_deref())?,
        size_ask: parse_decimal_string(tick.size_ask.as_deref())?,
    })
}

fn proto_historical_tick_last(
    tick: protobuf::HistoricalTickLast,
) -> TwsApiResult<HistoricalTickLast> {
    Ok(HistoricalTickLast {
        time: tick.time.unwrap_or_default(),
        tick_attrib_last: proto_tick_attrib_last(tick.tick_attrib_last),
        price: tick.price.unwrap_or_default(),
        size: parse_decimal_string(tick.size.as_deref())?,
        exchange: tick.exchange.unwrap_or_default(),
        special_conditions: tick.special_conditions.unwrap_or_default(),
    })
}

fn proto_tick_by_tick(
    tick: Option<protobuf::tick_by_tick_data::Tick>,
) -> TwsApiResult<Option<TickByTick>> {
    let Some(tick) = tick else {
        return Ok(None);
    };

    let tick = match tick {
        protobuf::tick_by_tick_data::Tick::HistoricalTickLast(tick) => {
            TickByTick::Last(proto_historical_tick_last(tick)?)
        }
        protobuf::tick_by_tick_data::Tick::HistoricalTickBidAsk(tick) => {
            TickByTick::BidAsk(proto_historical_tick_bid_ask(tick)?)
        }
        protobuf::tick_by_tick_data::Tick::HistoricalTickMidPoint(tick) => {
            TickByTick::MidPoint(proto_historical_tick(tick)?)
        }
    };
    Ok(Some(tick))
}

fn proto_historical_session(session: protobuf::HistoricalSession) -> HistoricalSession {
    HistoricalSession {
        start_date_time: session.start_date_time.unwrap_or_default(),
        end_date_time: session.end_date_time.unwrap_or_default(),
        ref_date: session.ref_date.unwrap_or_default(),
    }
}

fn proto_smart_component(component: protobuf::SmartComponent) -> SmartComponent {
    SmartComponent {
        bit_number: component.bit_number.unwrap_or_default(),
        exchange: component.exchange.unwrap_or_default(),
        exchange_letter: component.exchange_letter.unwrap_or_default(),
    }
}

fn proto_contract_description(description: protobuf::ContractDescription) -> ContractDescription {
    ContractDescription {
        contract: proto_contract_to_contract(description.contract),
        derivative_sec_types: description.derivative_sec_types,
    }
}

fn proto_execution_to_execution(execution: Option<protobuf::Execution>) -> TwsApiResult<Execution> {
    let Some(execution) = execution else {
        return Ok(Execution::default());
    };

    Ok(Execution {
        order_id: execution.order_id.unwrap_or_default(),
        exec_id: execution.exec_id.unwrap_or_default(),
        time: execution.time.unwrap_or_default(),
        acct_number: execution.acct_number.unwrap_or_default(),
        exchange: execution.exchange.unwrap_or_default(),
        side: execution.side.unwrap_or_default(),
        shares: parse_decimal_string(execution.shares.as_deref())?,
        price: execution.price.unwrap_or_default(),
        perm_id: execution.perm_id.unwrap_or_default(),
        client_id: execution.client_id.unwrap_or_default(),
        liquidation: i32::from(execution.is_liquidation.unwrap_or_default()),
        cum_qty: parse_decimal_string(execution.cum_qty.as_deref())?,
        avg_price: execution.avg_price.unwrap_or_default(),
        order_ref: execution.order_ref.unwrap_or_default(),
        ev_rule: execution.ev_rule.unwrap_or_default(),
        ev_multiplier: execution.ev_multiplier.unwrap_or_default(),
        model_code: execution.model_code.unwrap_or_default(),
        last_liquidity: execution.last_liquidity.unwrap_or_default(),
        pending_price_revision: execution.is_price_revision_pending.unwrap_or_default(),
        submitter: execution.submitter.unwrap_or_default(),
        opt_exercise_or_lapse_type: execution.opt_exercise_or_lapse_type.unwrap_or_default(),
    })
}

fn proto_contract_details_to_contract_details(
    contract: Option<protobuf::Contract>,
    details: Option<protobuf::ContractDetails>,
) -> TwsApiResult<ContractDetails> {
    let Some(details) = details else {
        return Ok(ContractDetails {
            contract: proto_contract_to_contract(contract),
            ..ContractDetails::default()
        });
    };

    Ok(ContractDetails {
        contract: proto_contract_to_contract(contract),
        market_name: details.market_name.unwrap_or_default(),
        min_tick: details
            .min_tick
            .as_deref()
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or_default(),
        order_types: details.order_types.unwrap_or_default(),
        valid_exchanges: details.valid_exchanges.unwrap_or_default(),
        price_magnifier: details.price_magnifier.unwrap_or_default(),
        under_con_id: details.under_con_id.unwrap_or_default(),
        long_name: details.long_name.unwrap_or_default(),
        contract_month: details.contract_month.unwrap_or_default(),
        industry: details.industry.unwrap_or_default(),
        category: details.category.unwrap_or_default(),
        subcategory: details.subcategory.unwrap_or_default(),
        time_zone_id: details.time_zone_id.unwrap_or_default(),
        trading_hours: details.trading_hours.unwrap_or_default(),
        liquid_hours: details.liquid_hours.unwrap_or_default(),
        market_rule_ids: details.market_rule_ids.unwrap_or_default(),
        cusip: details.cusip.unwrap_or_default(),
        issue_date: details.issue_date.unwrap_or_default(),
        ratings: details.ratings.unwrap_or_default(),
        bond_type: details.bond_type.unwrap_or_default(),
        coupon: details.coupon.unwrap_or_default(),
        coupon_type: details.coupon_type.unwrap_or_default(),
        convertible: details.convertible.unwrap_or_default(),
        callable: details.callable.unwrap_or_default(),
        puttable: details.puttable.unwrap_or_default(),
        desc_append: details.desc_append.unwrap_or_default(),
        next_option_date: details.next_option_date.unwrap_or_default(),
        next_option_type: details.next_option_type.unwrap_or_default(),
        next_option_partial: details.next_option_partial.unwrap_or_default(),
        bond_notes: details.bond_notes.unwrap_or_default(),
        real_expiration_date: details.real_expiration_date.unwrap_or_default(),
        stock_type: details.stock_type.unwrap_or_default(),
        min_size: parse_decimal_string(details.min_size.as_deref())?,
        size_increment: parse_decimal_string(details.size_increment.as_deref())?,
        suggested_size_increment: parse_decimal_string(
            details.suggested_size_increment.as_deref(),
        )?,
        fund_name: details.fund_name.unwrap_or_default(),
        fund_family: details.fund_family.unwrap_or_default(),
        fund_type: details.fund_type.unwrap_or_default(),
        fund_front_load: details.fund_front_load.unwrap_or_default(),
        fund_back_load: details.fund_back_load.unwrap_or_default(),
        fund_back_load_time_interval: details.fund_back_load_time_interval.unwrap_or_default(),
        fund_management_fee: details.fund_management_fee.unwrap_or_default(),
        fund_closed: details.fund_closed.unwrap_or_default(),
        fund_closed_for_new_investors: details.fund_closed_for_new_investors.unwrap_or_default(),
        fund_closed_for_new_money: details.fund_closed_for_new_money.unwrap_or_default(),
        fund_notify_amount: details.fund_notify_amount.unwrap_or_default(),
        fund_minimum_initial_purchase: details.fund_minimum_initial_purchase.unwrap_or_default(),
        fund_minimum_subsequent_purchase: details
            .fund_minimum_subsequent_purchase
            .unwrap_or_default(),
        fund_blue_sky_states: details.fund_blue_sky_states.unwrap_or_default(),
        fund_blue_sky_territories: details.fund_blue_sky_territories.unwrap_or_default(),
        fund_distribution_policy_indicator: details
            .fund_distribution_policy_indicator
            .unwrap_or_default(),
        fund_asset_type: details.fund_asset_type.unwrap_or_default(),
        ineligibility_reason_list: details
            .ineligibility_reason_list
            .into_iter()
            .map(|reason| IneligibilityReason {
                id: reason.id.unwrap_or_default(),
                description: reason.description.unwrap_or_default(),
            })
            .collect(),
        event_contract1: details.event_contract1.unwrap_or_default(),
        event_contract_description1: details.event_contract_description1.unwrap_or_default(),
        event_contract_description2: details.event_contract_description2.unwrap_or_default(),
        min_algo_size: parse_decimal_string(details.min_algo_size.as_deref())?,
        last_price_precision: parse_decimal_string(details.last_price_precision.as_deref())?,
        last_size_precision: parse_decimal_string(details.last_size_precision.as_deref())?,
    })
}

fn proto_contract_to_contract(contract: Option<protobuf::Contract>) -> Contract {
    let Some(contract) = contract else {
        return Contract::default();
    };

    Contract {
        con_id: contract.con_id.unwrap_or_default(),
        symbol: contract.symbol.unwrap_or_default(),
        sec_type: contract.sec_type.unwrap_or_default(),
        last_trade_date_or_contract_month: contract
            .last_trade_date_or_contract_month
            .unwrap_or_default(),
        last_trade_date: contract.last_trade_date.unwrap_or_default(),
        strike: contract.strike.unwrap_or_default(),
        right: contract.right.unwrap_or_default(),
        multiplier: contract
            .multiplier
            .map(|value| value.to_string())
            .unwrap_or_default(),
        exchange: contract.exchange.unwrap_or_default(),
        primary_exchange: contract.primary_exch.unwrap_or_default(),
        currency: contract.currency.unwrap_or_default(),
        local_symbol: contract.local_symbol.unwrap_or_default(),
        trading_class: contract.trading_class.unwrap_or_default(),
        include_expired: contract.include_expired.unwrap_or_default(),
        sec_id_type: contract.sec_id_type.unwrap_or_default(),
        sec_id: contract.sec_id.unwrap_or_default(),
        description: contract.description.unwrap_or_default(),
        issuer_id: contract.issuer_id.unwrap_or_default(),
        combo_legs_description: contract.combo_legs_descrip.unwrap_or_default(),
        combo_legs: contract
            .combo_legs
            .into_iter()
            .map(proto_combo_leg)
            .collect(),
        delta_neutral_contract: contract
            .delta_neutral_contract
            .map(|delta| DeltaNeutralContract {
                con_id: delta.con_id.unwrap_or_default(),
                delta: delta.delta.unwrap_or_default(),
                price: delta.price.unwrap_or_default(),
            }),
    }
}

fn proto_combo_leg(leg: protobuf::ComboLeg) -> ComboLeg {
    ComboLeg {
        con_id: leg.con_id.unwrap_or_default(),
        ratio: leg.ratio.unwrap_or_default(),
        action: leg.action.unwrap_or_default(),
        exchange: leg.exchange.unwrap_or_default(),
        open_close: match leg.open_close.unwrap_or_default() {
            1 => LegOpenClose::OpenPosition,
            2 => LegOpenClose::ClosePosition,
            3 => LegOpenClose::Unknown,
            _ => LegOpenClose::SamePosition,
        },
        short_sale_slot: leg.short_sales_slot.unwrap_or_default(),
        designated_location: leg.designated_location.unwrap_or_default(),
        exempt_code: leg.exempt_code.unwrap_or(-1),
    }
}

fn split_message_id(raw: bool, payload: &[u8]) -> TwsApiResult<(i32, &[u8])> {
    if raw {
        let (prefix, body) = payload
            .split_at_checked(4)
            .ok_or(TwsApiError::IncompleteFrame {
                needed: 4,
                available: payload.len(),
            })?;
        let prefix: [u8; 4] = prefix
            .try_into()
            .map_err(|_| TwsApiError::IncompleteFrame {
                needed: 4,
                available: payload.len(),
            })?;
        return Ok((i32::from_be_bytes(prefix), body));
    }

    let nul = payload
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(TwsApiError::MalformedHandshake)?;
    let (id, rest) = payload.split_at(nul);
    let rest = rest.get(1..).ok_or(TwsApiError::IncompleteFrame {
        needed: nul + 1,
        available: payload.len(),
    })?;
    let id = String::from_utf8_lossy(id).into_owned();
    let msg_id = id
        .parse::<i32>()
        .map_err(|source| TwsApiError::InvalidInteger { field: id, source })?;
    Ok((msg_id, rest))
}

fn decode_portfolio_value_fields(fields: &[String]) -> TwsApiResult<Event> {
    let version = parse_i32(fields.first())?;
    let mut index = 1;
    let mut contract = Contract {
        con_id: next_i32(fields, &mut index)?,
        symbol: next_string(fields, &mut index),
        sec_type: next_string(fields, &mut index),
        last_trade_date_or_contract_month: next_string(fields, &mut index),
        strike: next_f64(fields, &mut index)?,
        right: next_string(fields, &mut index),
        ..Contract::default()
    };

    if version >= 7 {
        contract.multiplier = next_string(fields, &mut index);
        contract.primary_exchange = next_string(fields, &mut index);
    }

    contract.currency = next_string(fields, &mut index);
    contract.local_symbol = next_string(fields, &mut index);
    if version >= 8 {
        contract.trading_class = next_string(fields, &mut index);
    }

    Ok(Event::PortfolioValue {
        contract: Box::new(contract),
        position: next_decimal(fields, &mut index)?,
        market_price: next_f64(fields, &mut index)?,
        market_value: next_f64(fields, &mut index)?,
        average_cost: next_f64(fields, &mut index)?,
        unrealized_pnl: next_f64(fields, &mut index)?,
        realized_pnl: next_f64(fields, &mut index)?,
        account_name: next_string(fields, &mut index),
    })
}

fn decode_market_depth_fields(fields: &[String], is_l2: bool) -> TwsApiResult<Event> {
    let mut index = 1;
    let req_id = next_i32(fields, &mut index)?;
    let position = next_i32(fields, &mut index)?;
    let market_maker = if is_l2 {
        next_string(fields, &mut index)
    } else {
        String::new()
    };
    let operation = next_i32(fields, &mut index)?;
    let side = next_i32(fields, &mut index)?;
    let price = next_f64(fields, &mut index)?;
    let size = next_decimal(fields, &mut index)?;
    let is_smart_depth = is_l2 && parse_bool(fields.get(index));

    Ok(Event::MarketDepth {
        req_id,
        position,
        operation,
        side,
        price,
        size,
        market_maker,
        is_smart_depth,
    })
}

fn decode_tick_option_computation_fields(fields: &[String]) -> TwsApiResult<Event> {
    let has_version = fields.len() > 11;
    let mut index = usize::from(has_version);
    let req_id = next_i32(fields, &mut index)?;
    let tick_type = next_i32(fields, &mut index)?;
    let tick_attrib = if has_version && fields.len() == 10 {
        0
    } else {
        next_i32(fields, &mut index).unwrap_or_default()
    };

    Ok(Event::TickOptionComputation {
        req_id,
        tick_type,
        tick_attrib,
        implied_vol: next_f64(fields, &mut index)?,
        delta: next_f64(fields, &mut index)?,
        opt_price: if index < fields.len() {
            next_f64(fields, &mut index)?
        } else {
            0.0
        },
        pv_dividend: if index < fields.len() {
            next_f64(fields, &mut index)?
        } else {
            0.0
        },
        gamma: if index < fields.len() {
            next_f64(fields, &mut index)?
        } else {
            0.0
        },
        vega: if index < fields.len() {
            next_f64(fields, &mut index)?
        } else {
            0.0
        },
        theta: if index < fields.len() {
            next_f64(fields, &mut index)?
        } else {
            0.0
        },
        und_price: if index < fields.len() {
            next_f64(fields, &mut index)?
        } else {
            0.0
        },
    })
}

fn decode_scanner_data_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 1;
    let req_id = next_i32(fields, &mut index)?;
    let row_count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut rows = Vec::with_capacity(row_count);

    for _ in 0..row_count {
        let rank = next_i32(fields, &mut index)?;
        let contract = Contract {
            con_id: next_i32(fields, &mut index)?,
            symbol: next_string(fields, &mut index),
            sec_type: next_string(fields, &mut index),
            last_trade_date_or_contract_month: next_string(fields, &mut index),
            strike: next_f64(fields, &mut index)?,
            right: next_string(fields, &mut index),
            exchange: next_string(fields, &mut index),
            currency: next_string(fields, &mut index),
            local_symbol: next_string(fields, &mut index),
            ..Contract::default()
        };
        let market_name = next_string(fields, &mut index);
        let mut contract = contract;
        contract.trading_class = next_string(fields, &mut index);
        rows.push(ScannerDataRow {
            rank,
            contract: Box::new(contract),
            market_name,
            distance: next_string(fields, &mut index),
            benchmark: next_string(fields, &mut index),
            projection: next_string(fields, &mut index),
            combo_key: next_string(fields, &mut index),
        });
    }

    Ok(Event::ScannerData { req_id, rows })
}

fn decode_soft_dollar_tiers_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut tiers = Vec::with_capacity(count);
    for _ in 0..count {
        tiers.push(SoftDollarTier {
            name: next_string(fields, &mut index),
            value: next_string(fields, &mut index),
            display_name: next_string(fields, &mut index),
        });
    }
    Ok(Event::SoftDollarTiers { req_id, tiers })
}

fn decode_family_codes_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut family_codes = Vec::with_capacity(count);
    for _ in 0..count {
        family_codes.push((
            next_string(fields, &mut index),
            next_string(fields, &mut index),
        ));
    }
    Ok(Event::FamilyCodes { family_codes })
}

fn decode_symbol_samples_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut descriptions = Vec::with_capacity(count);
    for _ in 0..count {
        let mut contract = Contract {
            con_id: next_i32(fields, &mut index)?,
            symbol: next_string(fields, &mut index),
            sec_type: next_string(fields, &mut index),
            primary_exchange: next_string(fields, &mut index),
            currency: next_string(fields, &mut index),
            ..Contract::default()
        };
        let derivative_count = next_i32(fields, &mut index)?.max(0) as usize;
        let mut derivative_sec_types = Vec::with_capacity(derivative_count);
        for _ in 0..derivative_count {
            derivative_sec_types.push(next_string(fields, &mut index));
        }
        if index + 1 < fields.len() {
            contract.description = next_string(fields, &mut index);
            contract.issuer_id = next_string(fields, &mut index);
        }
        descriptions.push(ContractDescription {
            contract,
            derivative_sec_types,
        });
    }
    Ok(Event::SymbolSamples {
        req_id,
        descriptions,
    })
}

fn decode_smart_components_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut components = Vec::with_capacity(count);
    for _ in 0..count {
        components.push(SmartComponent {
            bit_number: next_i32(fields, &mut index)?,
            exchange: next_string(fields, &mut index),
            exchange_letter: next_string(fields, &mut index),
        });
    }
    Ok(Event::SmartComponents { req_id, components })
}

fn decode_market_depth_exchanges_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut descriptions = Vec::with_capacity(count);
    for _ in 0..count {
        let exchange = next_string(fields, &mut index);
        let sec_type = next_string(fields, &mut index);
        let listing_exch = next_string(fields, &mut index);
        let service_data_type = next_string(fields, &mut index);
        let agg_group = next_i32(fields, &mut index).unwrap_or_default();
        descriptions.push(format!(
            "{exchange}:{sec_type}:{listing_exch}:{service_data_type}:{agg_group}"
        ));
    }
    Ok(Event::MarketDepthExchanges { descriptions })
}

fn decode_histogram_data_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        items.push((
            next_f64(fields, &mut index)?,
            next_decimal(fields, &mut index)?,
        ));
    }
    Ok(Event::HistogramData { req_id, items })
}

fn decode_market_rule_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let market_rule_id = next_i32(fields, &mut index)?;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut price_increments = Vec::with_capacity(count);
    for _ in 0..count {
        price_increments.push((next_f64(fields, &mut index)?, next_f64(fields, &mut index)?));
    }
    Ok(Event::MarketRule {
        market_rule_id,
        price_increments,
    })
}

fn decode_pnl_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    Ok(Event::Pnl {
        req_id: next_i32(fields, &mut index)?,
        daily_pnl: next_f64(fields, &mut index)?,
        unrealized_pnl: if index < fields.len() {
            next_f64(fields, &mut index)?
        } else {
            0.0
        },
        realized_pnl: if index < fields.len() {
            next_f64(fields, &mut index)?
        } else {
            0.0
        },
    })
}

fn decode_pnl_single_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let position = next_decimal(fields, &mut index)?;
    let daily_pnl = next_f64(fields, &mut index)?;
    let remaining = fields.len().saturating_sub(index);
    let unrealized_pnl = if remaining >= 3 {
        next_f64(fields, &mut index)?
    } else {
        0.0
    };
    let realized_pnl = if remaining >= 3 {
        next_f64(fields, &mut index)?
    } else {
        0.0
    };
    let value = if index < fields.len() {
        next_f64(fields, &mut index)?
    } else {
        0.0
    };

    Ok(Event::PnlSingle {
        req_id,
        position,
        daily_pnl,
        unrealized_pnl,
        realized_pnl,
        value,
    })
}

fn decode_news_providers_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut providers = Vec::with_capacity(count);
    for _ in 0..count {
        providers.push((
            next_string(fields, &mut index),
            next_string(fields, &mut index),
        ));
    }
    Ok(Event::NewsProviders { providers })
}

fn decode_historical_schedule_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let start_date_time = next_string(fields, &mut index);
    let end_date_time = next_string(fields, &mut index);
    let time_zone = next_string(fields, &mut index);
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut sessions = Vec::with_capacity(count);
    for _ in 0..count {
        sessions.push(HistoricalSession {
            start_date_time: next_string(fields, &mut index),
            end_date_time: next_string(fields, &mut index),
            ref_date: next_string(fields, &mut index),
        });
    }
    Ok(Event::HistoricalSchedule {
        req_id,
        start_date_time,
        end_date_time,
        time_zone,
        sessions,
    })
}

fn decode_execution_data_fields(fields: &[String]) -> TwsApiResult<Event> {
    let has_version = fields
        .get(3)
        .is_some_and(|field| field.parse::<i32>().is_ok());
    let mut index = 0;
    let version = if has_version {
        next_i32(fields, &mut index)?
    } else {
        i32::MAX
    };
    let req_id = if version >= 7 {
        next_i32(fields, &mut index)?
    } else {
        -1
    };
    let order_id = next_i32(fields, &mut index)?;

    let mut contract = Contract {
        con_id: next_i32(fields, &mut index)?,
        symbol: next_string(fields, &mut index),
        sec_type: next_string(fields, &mut index),
        last_trade_date_or_contract_month: next_string(fields, &mut index),
        strike: next_f64(fields, &mut index)?,
        right: next_string(fields, &mut index),
        ..Contract::default()
    };
    if version >= 9 {
        contract.multiplier = next_string(fields, &mut index);
    }
    contract.exchange = next_string(fields, &mut index);
    contract.currency = next_string(fields, &mut index);
    contract.local_symbol = next_string(fields, &mut index);
    if version >= 10 {
        contract.trading_class = next_string(fields, &mut index);
    }

    let mut execution = Execution {
        order_id,
        exec_id: next_string(fields, &mut index),
        time: next_string(fields, &mut index),
        acct_number: next_string(fields, &mut index),
        exchange: next_string(fields, &mut index),
        side: next_string(fields, &mut index),
        shares: next_decimal(fields, &mut index)?,
        price: next_f64(fields, &mut index)?,
        perm_id: i64::from(next_i32(fields, &mut index)?),
        client_id: next_i32(fields, &mut index)?,
        liquidation: next_i32(fields, &mut index)?,
        ..Execution::default()
    };

    if version >= 6 {
        execution.cum_qty = next_decimal(fields, &mut index)?;
        execution.avg_price = next_f64(fields, &mut index)?;
    }
    if version >= 8 {
        execution.order_ref = next_string(fields, &mut index);
    }
    if version >= 9 {
        execution.ev_rule = next_string(fields, &mut index);
        execution.ev_multiplier = next_f64(fields, &mut index)?;
    }
    if index < fields.len() {
        execution.model_code = next_string(fields, &mut index);
    }
    if index < fields.len() {
        execution.last_liquidity = next_i32(fields, &mut index)?;
    }
    if index < fields.len() {
        execution.pending_price_revision = parse_bool(fields.get(index));
        index += 1;
    }
    if index < fields.len() {
        execution.submitter = next_string(fields, &mut index);
    }

    Ok(Event::ExecutionDetails {
        req_id,
        contract: Box::new(contract),
        execution,
    })
}

fn decode_contract_data_fields(fields: &[String]) -> TwsApiResult<Event> {
    let has_version = fields
        .get(1)
        .is_some_and(|field| field.parse::<i32>().is_ok());
    let mut index = 0;
    let version = if has_version {
        next_i32(fields, &mut index)?
    } else {
        8
    };
    let req_id = if version >= 3 {
        next_i32(fields, &mut index)?
    } else {
        -1
    };

    let mut details = ContractDetails::default();
    details.contract.symbol = next_string(fields, &mut index);
    details.contract.sec_type = next_string(fields, &mut index);
    details.contract.last_trade_date_or_contract_month = next_string(fields, &mut index);

    if !has_version
        || fields
            .get(index)
            .is_none_or(|field| field.parse::<f64>().is_err())
    {
        details.contract.last_trade_date = next_string(fields, &mut index);
    }

    details.contract.strike = next_f64(fields, &mut index)?;
    details.contract.right = next_string(fields, &mut index);
    details.contract.exchange = next_string(fields, &mut index);
    details.contract.currency = next_string(fields, &mut index);
    details.contract.local_symbol = next_string(fields, &mut index);
    details.market_name = next_string(fields, &mut index);
    details.contract.trading_class = next_string(fields, &mut index);
    details.contract.con_id = next_i32(fields, &mut index)?;
    details.min_tick = next_f64(fields, &mut index)?;
    details.contract.multiplier = next_string(fields, &mut index);
    details.order_types = next_string(fields, &mut index);
    details.valid_exchanges = next_string(fields, &mut index);
    details.price_magnifier = next_i32(fields, &mut index)?;

    if version >= 4 && index < fields.len() {
        details.under_con_id = next_i32(fields, &mut index).unwrap_or_default();
    }
    if version >= 5 && index + 1 < fields.len() {
        details.long_name = next_string(fields, &mut index);
        details.contract.primary_exchange = next_string(fields, &mut index);
    }
    if version >= 6 && index + 6 < fields.len() {
        details.contract_month = next_string(fields, &mut index);
        details.industry = next_string(fields, &mut index);
        details.category = next_string(fields, &mut index);
        details.subcategory = next_string(fields, &mut index);
        details.time_zone_id = next_string(fields, &mut index);
        details.trading_hours = next_string(fields, &mut index);
        details.liquid_hours = next_string(fields, &mut index);
    }
    if version >= 8 && index + 1 < fields.len() {
        let _ev_rule = next_string(fields, &mut index);
        let _ev_multiplier = next_f64(fields, &mut index).unwrap_or_default();
    }
    if version >= 7 && index < fields.len() {
        let sec_id_count = next_i32(fields, &mut index).unwrap_or_default().max(0) as usize;
        for _ in 0..sec_id_count {
            let _tag = next_string(fields, &mut index);
            let _value = next_string(fields, &mut index);
        }
    }
    if index < fields.len() {
        let _agg_group = next_i32(fields, &mut index).unwrap_or_default();
    }
    if index + 1 < fields.len() {
        let _under_symbol = next_string(fields, &mut index);
        let _under_sec_type = next_string(fields, &mut index);
    }
    if index < fields.len() {
        details.market_rule_ids = next_string(fields, &mut index);
    }

    if index < fields.len() {
        details.real_expiration_date = next_string(fields, &mut index);
    }
    if index < fields.len() {
        details.stock_type = next_string(fields, &mut index);
    }
    if index < fields.len() {
        details.min_size = next_decimal(fields, &mut index).unwrap_or_default();
    }
    if index < fields.len() {
        details.size_increment = next_decimal(fields, &mut index).unwrap_or_default();
    }
    if index < fields.len() {
        details.suggested_size_increment = next_decimal(fields, &mut index).unwrap_or_default();
    }

    if details.contract.sec_type == "FUND" && index + 16 < fields.len() {
        details.fund_name = next_string(fields, &mut index);
        details.fund_family = next_string(fields, &mut index);
        details.fund_type = next_string(fields, &mut index);
        details.fund_front_load = next_string(fields, &mut index);
        details.fund_back_load = next_string(fields, &mut index);
        details.fund_back_load_time_interval = next_string(fields, &mut index);
        details.fund_management_fee = next_string(fields, &mut index);
        details.fund_closed = next_bool(fields, &mut index);
        details.fund_closed_for_new_investors = next_bool(fields, &mut index);
        details.fund_closed_for_new_money = next_bool(fields, &mut index);
        details.fund_notify_amount = next_string(fields, &mut index);
        details.fund_minimum_initial_purchase = next_string(fields, &mut index);
        details.fund_minimum_subsequent_purchase = next_string(fields, &mut index);
        details.fund_blue_sky_states = next_string(fields, &mut index);
        details.fund_blue_sky_territories = next_string(fields, &mut index);
        details.fund_distribution_policy_indicator = next_string(fields, &mut index);
        details.fund_asset_type = next_string(fields, &mut index);
    }

    if index < fields.len() {
        let count = next_i32(fields, &mut index).unwrap_or_default().max(0) as usize;
        for _ in 0..count {
            if index + 1 >= fields.len() {
                break;
            }
            details.ineligibility_reason_list.push(IneligibilityReason {
                id: next_string(fields, &mut index),
                description: next_string(fields, &mut index),
            });
        }
    }

    Ok(Event::ContractDetails {
        req_id,
        details: Box::new(details),
    })
}

fn decode_bond_contract_data_fields(fields: &[String]) -> TwsApiResult<Event> {
    let has_version = fields
        .get(1)
        .is_some_and(|field| field.parse::<i32>().is_ok());
    let mut index = 0;
    let version = if has_version {
        next_i32(fields, &mut index)?
    } else {
        6
    };
    let req_id = if version >= 3 {
        next_i32(fields, &mut index)?
    } else {
        -1
    };

    let mut details = ContractDetails::default();
    details.contract.symbol = next_string(fields, &mut index);
    details.contract.sec_type = next_string(fields, &mut index);
    details.cusip = next_string(fields, &mut index);
    details.coupon = next_f64(fields, &mut index)?;
    details.contract.last_trade_date_or_contract_month = next_string(fields, &mut index);
    details.issue_date = next_string(fields, &mut index);
    details.ratings = next_string(fields, &mut index);
    details.bond_type = next_string(fields, &mut index);
    details.coupon_type = next_string(fields, &mut index);
    details.convertible = next_bool(fields, &mut index);
    details.callable = next_bool(fields, &mut index);
    details.puttable = next_bool(fields, &mut index);
    details.desc_append = next_string(fields, &mut index);
    details.contract.exchange = next_string(fields, &mut index);
    details.contract.currency = next_string(fields, &mut index);
    details.market_name = next_string(fields, &mut index);
    details.contract.trading_class = next_string(fields, &mut index);
    details.contract.con_id = next_i32(fields, &mut index)?;
    details.min_tick = next_f64(fields, &mut index)?;
    details.order_types = next_string(fields, &mut index);
    details.valid_exchanges = next_string(fields, &mut index);
    details.next_option_date = next_string(fields, &mut index);
    details.next_option_type = next_string(fields, &mut index);
    details.next_option_partial = next_bool(fields, &mut index);
    details.bond_notes = next_string(fields, &mut index);

    if version >= 4 && index < fields.len() {
        details.long_name = next_string(fields, &mut index);
    }
    if index + 2 < fields.len() {
        details.time_zone_id = next_string(fields, &mut index);
        details.trading_hours = next_string(fields, &mut index);
        details.liquid_hours = next_string(fields, &mut index);
    }
    if version >= 6 && index + 1 < fields.len() {
        let _ev_rule = next_string(fields, &mut index);
        let _ev_multiplier = next_f64(fields, &mut index).unwrap_or_default();
    }
    if version >= 5 && index < fields.len() {
        let sec_id_count = next_i32(fields, &mut index).unwrap_or_default().max(0) as usize;
        for _ in 0..sec_id_count {
            let _tag = next_string(fields, &mut index);
            let _value = next_string(fields, &mut index);
        }
    }
    if index < fields.len() {
        let _agg_group = next_i32(fields, &mut index).unwrap_or_default();
    }
    if index < fields.len() {
        details.market_rule_ids = next_string(fields, &mut index);
    }

    Ok(Event::ContractDetails {
        req_id,
        details: Box::new(details),
    })
}

fn decode_historical_data_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let first = next_i32(fields, &mut index)?;
    let req_id = if fields
        .get(index)
        .and_then(|field| field.parse::<usize>().ok())
        .is_some_and(|count| fields.len() == index + 1 + count * 8)
    {
        first
    } else {
        next_i32(fields, &mut index)?
    };

    let bar_count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut bars = Vec::with_capacity(bar_count);
    for _ in 0..bar_count {
        bars.push(BarData {
            date: next_string(fields, &mut index),
            open: next_f64(fields, &mut index)?,
            high: next_f64(fields, &mut index)?,
            low: next_f64(fields, &mut index)?,
            close: next_f64(fields, &mut index)?,
            volume: next_decimal(fields, &mut index)?,
            wap: next_decimal(fields, &mut index)?,
            bar_count: next_i32(fields, &mut index)?,
        });
    }

    Ok(Event::HistoricalDataBars { req_id, bars })
}

fn decode_historical_data_update_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let bar_count = next_i32(fields, &mut index)?;
    let bar = BarData {
        date: next_string(fields, &mut index),
        open: next_f64(fields, &mut index)?,
        close: next_f64(fields, &mut index)?,
        high: next_f64(fields, &mut index)?,
        low: next_f64(fields, &mut index)?,
        wap: next_decimal(fields, &mut index)?,
        volume: next_decimal(fields, &mut index)?,
        bar_count,
    };

    Ok(Event::HistoricalDataUpdate { req_id, bar })
}

fn decode_historical_ticks_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut ticks = Vec::with_capacity(count);
    for _ in 0..count {
        let time = i64::from(next_i32(fields, &mut index)?);
        let _unused = next_string(fields, &mut index);
        ticks.push(HistoricalTick {
            time,
            price: next_f64(fields, &mut index)?,
            size: next_decimal(fields, &mut index)?,
        });
    }
    let done = next_bool(fields, &mut index);
    Ok(Event::HistoricalTicks {
        req_id,
        ticks,
        done,
    })
}

fn decode_historical_ticks_bid_ask_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut ticks = Vec::with_capacity(count);
    for _ in 0..count {
        let time = i64::from(next_i32(fields, &mut index)?);
        let mask = next_i32(fields, &mut index)?;
        ticks.push(HistoricalTickBidAsk {
            time,
            tick_attrib_bid_ask: TickAttribBidAsk {
                ask_past_high: mask & 1 != 0,
                bid_past_low: mask & 2 != 0,
            },
            price_bid: next_f64(fields, &mut index)?,
            price_ask: next_f64(fields, &mut index)?,
            size_bid: next_decimal(fields, &mut index)?,
            size_ask: next_decimal(fields, &mut index)?,
        });
    }
    let done = next_bool(fields, &mut index);
    Ok(Event::HistoricalTicksBidAsk {
        req_id,
        ticks,
        done,
    })
}

fn decode_historical_ticks_last_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let count = next_i32(fields, &mut index)?.max(0) as usize;
    let mut ticks = Vec::with_capacity(count);
    for _ in 0..count {
        let time = i64::from(next_i32(fields, &mut index)?);
        let mask = next_i32(fields, &mut index)?;
        ticks.push(HistoricalTickLast {
            time,
            tick_attrib_last: TickAttribLast {
                past_limit: mask & 1 != 0,
                unreported: mask & 2 != 0,
            },
            price: next_f64(fields, &mut index)?,
            size: next_decimal(fields, &mut index)?,
            exchange: next_string(fields, &mut index),
            special_conditions: next_string(fields, &mut index),
        });
    }
    let done = next_bool(fields, &mut index);
    Ok(Event::HistoricalTicksLast {
        req_id,
        ticks,
        done,
    })
}

fn decode_tick_by_tick_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 0;
    let req_id = next_i32(fields, &mut index)?;
    let tick_type = next_i32(fields, &mut index)?;
    let time = i64::from(next_i32(fields, &mut index)?);
    let tick = match tick_type {
        1 | 2 => {
            let price = next_f64(fields, &mut index)?;
            let size = next_decimal(fields, &mut index)?;
            let mask = next_i32(fields, &mut index)?;
            Some(TickByTick::Last(HistoricalTickLast {
                time,
                tick_attrib_last: TickAttribLast {
                    past_limit: mask & 1 != 0,
                    unreported: mask & 2 != 0,
                },
                price,
                size,
                exchange: next_string(fields, &mut index),
                special_conditions: next_string(fields, &mut index),
            }))
        }
        3 => {
            let price_bid = next_f64(fields, &mut index)?;
            let price_ask = next_f64(fields, &mut index)?;
            let size_bid = next_decimal(fields, &mut index)?;
            let size_ask = next_decimal(fields, &mut index)?;
            let mask = next_i32(fields, &mut index)?;
            Some(TickByTick::BidAsk(HistoricalTickBidAsk {
                time,
                tick_attrib_bid_ask: TickAttribBidAsk {
                    bid_past_low: mask & 1 != 0,
                    ask_past_high: mask & 2 != 0,
                },
                price_bid,
                price_ask,
                size_bid,
                size_ask,
            }))
        }
        4 => Some(TickByTick::MidPoint(HistoricalTick {
            time,
            price: next_f64(fields, &mut index)?,
            size: Decimal::default(),
        })),
        _ => None,
    };

    Ok(Event::TickByTick {
        req_id,
        tick_type,
        tick,
    })
}

fn decode_real_time_bar_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 1;
    let req_id = next_i32(fields, &mut index)?;
    let bar = BarData {
        date: next_string(fields, &mut index),
        open: next_f64(fields, &mut index)?,
        high: next_f64(fields, &mut index)?,
        low: next_f64(fields, &mut index)?,
        close: next_f64(fields, &mut index)?,
        volume: next_decimal(fields, &mut index)?,
        wap: next_decimal(fields, &mut index)?,
        bar_count: next_i32(fields, &mut index)?,
    };

    Ok(Event::RealTimeBar {
        req_id,
        time: bar.date.parse::<i64>().unwrap_or_default(),
        bar,
    })
}

fn decode_position_fields(fields: &[String]) -> TwsApiResult<Event> {
    let version = parse_i32(fields.first())?;
    let mut index = 1;
    let account = next_string(fields, &mut index);
    let contract = decode_position_contract_fields(fields, &mut index, version >= 2)?;
    let position = next_decimal(fields, &mut index)?;
    let avg_cost = if version >= 3 {
        next_f64(fields, &mut index)?
    } else {
        0.0
    };

    Ok(Event::Position {
        account,
        contract: Box::new(contract),
        position,
        avg_cost,
    })
}

fn decode_position_multi_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut index = 1;
    let req_id = next_i32(fields, &mut index)?;
    let account = next_string(fields, &mut index);
    let contract = decode_position_contract_fields(fields, &mut index, true)?;
    let position = next_decimal(fields, &mut index)?;
    let avg_cost = next_f64(fields, &mut index)?;
    let model_code = next_string(fields, &mut index);

    Ok(Event::PositionMulti {
        req_id,
        account,
        model_code,
        contract: Box::new(contract),
        position,
        avg_cost,
    })
}

fn decode_position_contract_fields(
    fields: &[String],
    index: &mut usize,
    include_trading_class: bool,
) -> TwsApiResult<Contract> {
    let mut contract = Contract {
        con_id: next_i32(fields, index)?,
        symbol: next_string(fields, index),
        sec_type: next_string(fields, index),
        last_trade_date_or_contract_month: next_string(fields, index),
        strike: next_f64(fields, index)?,
        right: next_string(fields, index),
        multiplier: next_string(fields, index),
        exchange: next_string(fields, index),
        currency: next_string(fields, index),
        local_symbol: next_string(fields, index),
        ..Contract::default()
    };

    if include_trading_class {
        contract.trading_class = next_string(fields, index);
    }

    Ok(contract)
}

fn decode_security_definition_option_parameter_fields(fields: &[String]) -> TwsApiResult<Event> {
    let req_id = parse_i32(fields.first())?;
    let exchange = fields.get(1).cloned().unwrap_or_default();
    let underlying_con_id = parse_i32(fields.get(2))?;
    let trading_class = fields.get(3).cloned().unwrap_or_default();
    let multiplier = fields.get(4).cloned().unwrap_or_default();

    let mut index = 5;
    let expiration_count = parse_i32(fields.get(index)).unwrap_or_default().max(0) as usize;
    index += 1;

    let mut expirations = Vec::with_capacity(expiration_count);
    for _ in 0..expiration_count {
        expirations.push(fields.get(index).cloned().unwrap_or_default());
        index += 1;
    }

    let strike_count = parse_i32(fields.get(index)).unwrap_or_default().max(0) as usize;
    index += 1;

    let mut strikes = Vec::with_capacity(strike_count);
    for _ in 0..strike_count {
        strikes.push(parse_f64(fields.get(index))?);
        index += 1;
    }

    Ok(Event::SecurityDefinitionOptionParameter {
        req_id,
        exchange,
        underlying_con_id,
        trading_class,
        multiplier,
        expirations,
        strikes,
    })
}

fn next_string(fields: &[String], index: &mut usize) -> String {
    let value = fields.get(*index).cloned().unwrap_or_default();
    *index += 1;
    value
}

fn next_i32(fields: &[String], index: &mut usize) -> TwsApiResult<i32> {
    let value = parse_i32(fields.get(*index));
    *index += 1;
    value
}

fn next_f64(fields: &[String], index: &mut usize) -> TwsApiResult<f64> {
    let value = parse_f64(fields.get(*index));
    *index += 1;
    value
}

fn next_bool(fields: &[String], index: &mut usize) -> bool {
    let value = parse_bool(fields.get(*index));
    *index += 1;
    value
}

fn next_decimal(fields: &[String], index: &mut usize) -> TwsApiResult<Decimal> {
    let value = parse_decimal(fields.get(*index));
    *index += 1;
    value
}

fn parse_i32(value: Option<&String>) -> TwsApiResult<i32> {
    let field = value.cloned().unwrap_or_default();
    field
        .parse::<i32>()
        .map_err(|source| TwsApiError::InvalidInteger { field, source })
}

fn parse_i64(value: Option<&String>) -> TwsApiResult<i64> {
    let field = value.cloned().unwrap_or_default();
    field
        .parse::<i64>()
        .map_err(|source| TwsApiError::InvalidInteger { field, source })
}

fn parse_f64(value: Option<&String>) -> TwsApiResult<f64> {
    Ok(value
        .and_then(|field| field.parse::<f64>().ok())
        .unwrap_or_default())
}

fn parse_bool(value: Option<&String>) -> bool {
    matches!(
        value.map(String::as_str),
        Some("1") | Some("true") | Some("True")
    )
}

fn parse_decimal(value: Option<&String>) -> TwsApiResult<Decimal> {
    parse_decimal_string(value.map(String::as_str))
}

fn parse_decimal_string(value: Option<&str>) -> TwsApiResult<Decimal> {
    let field = value.unwrap_or_default().to_owned();
    if field.is_empty() {
        return Ok(Decimal::default());
    }
    field
        .parse::<Decimal>()
        .map_err(|source| TwsApiError::InvalidDecimal { field, source })
}
