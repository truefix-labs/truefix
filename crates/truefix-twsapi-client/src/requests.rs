use prost::Message;

use crate::comm;
use crate::constants::{UNSET_DOUBLE, UNSET_INTEGER};
use crate::error::{TwsApiError, TwsApiResult};
use crate::message::Outgoing;
use crate::protobuf;
use crate::server_versions::{
    MAX_CLIENT_VER, MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1,
    MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_2, MIN_SERVER_VER_ADVANCED_ORDER_REJECT,
    MIN_SERVER_VER_ALGO_ID, MIN_SERVER_VER_ALGO_ORDERS, MIN_SERVER_VER_ATTACHED_ORDERS,
    MIN_SERVER_VER_AUTO_CANCEL_PARENT, MIN_SERVER_VER_AUTO_PRICE_FOR_HEDGE,
    MIN_SERVER_VER_BOND_ISSUERID, MIN_SERVER_VER_CASH_QTY, MIN_SERVER_VER_CME_TAGGING_FIELDS,
    MIN_SERVER_VER_CONTRACT_DATA_CHAIN, MIN_SERVER_VER_CUSTOMER_ACCOUNT,
    MIN_SERVER_VER_D_PEG_ORDERS, MIN_SERVER_VER_DECISION_MAKER, MIN_SERVER_VER_DELTA_NEUTRAL,
    MIN_SERVER_VER_DELTA_NEUTRAL_CONID, MIN_SERVER_VER_DELTA_NEUTRAL_OPEN_CLOSE,
    MIN_SERVER_VER_DURATION, MIN_SERVER_VER_EXT_OPERATOR, MIN_SERVER_VER_HEDGE_MAX_SIZE,
    MIN_SERVER_VER_HEDGE_ORDERS, MIN_SERVER_VER_HISTORICAL_TICKS, MIN_SERVER_VER_IMBALANCE_ONLY,
    MIN_SERVER_VER_INCLUDE_OVERNIGHT, MIN_SERVER_VER_LINKING, MIN_SERVER_VER_MANUAL_ORDER_TIME,
    MIN_SERVER_VER_MANUAL_ORDER_TIME_EXERCISE_OPTIONS, MIN_SERVER_VER_MIFID_EXECUTION,
    MIN_SERVER_VER_MODELS_SUPPORT, MIN_SERVER_VER_NOT_HELD, MIN_SERVER_VER_OPT_OUT_SMART_ROUTING,
    MIN_SERVER_VER_ORDER_COMBO_LEGS_PRICE, MIN_SERVER_VER_ORDER_CONTAINER,
    MIN_SERVER_VER_ORDER_SOLICITED, MIN_SERVER_VER_PEGGED_TO_BENCHMARK,
    MIN_SERVER_VER_PLACE_ORDER_CONID, MIN_SERVER_VER_POST_TO_ATS, MIN_SERVER_VER_PRICE_MGMT_ALGO,
    MIN_SERVER_VER_PRIMARYEXCH, MIN_SERVER_VER_PROFESSIONAL_CUSTOMER, MIN_SERVER_VER_PROTOBUF,
    MIN_SERVER_VER_PROTOBUF_ACCOUNTS_POSITIONS, MIN_SERVER_VER_PROTOBUF_COMPLETED_ORDER,
    MIN_SERVER_VER_PROTOBUF_CONTRACT_DATA, MIN_SERVER_VER_PROTOBUF_HISTORICAL_DATA,
    MIN_SERVER_VER_PROTOBUF_MARKET_DATA, MIN_SERVER_VER_PROTOBUF_NEWS_DATA,
    MIN_SERVER_VER_PROTOBUF_PLACE_ORDER, MIN_SERVER_VER_PROTOBUF_REST_MESSAGES_1,
    MIN_SERVER_VER_PROTOBUF_REST_MESSAGES_2, MIN_SERVER_VER_PROTOBUF_REST_MESSAGES_3,
    MIN_SERVER_VER_PROTOBUF_SCAN_DATA, MIN_SERVER_VER_PTA_ORDERS,
    MIN_SERVER_VER_RANDOMIZE_SIZE_AND_PRICE, MIN_SERVER_VER_REQ_CALC_IMPLIED_VOLAT,
    MIN_SERVER_VER_REQ_MKT_DATA_CONID, MIN_SERVER_VER_REQ_SMART_COMPONENTS,
    MIN_SERVER_VER_SCALE_ORDERS2, MIN_SERVER_VER_SCALE_ORDERS3, MIN_SERVER_VER_SCALE_TABLE,
    MIN_SERVER_VER_SEC_ID_TYPE, MIN_SERVER_VER_SMART_COMBO_ROUTING_PARAMS,
    MIN_SERVER_VER_SOFT_DOLLAR_TIER, MIN_SERVER_VER_SSHORTX_OLD, MIN_SERVER_VER_SYNT_REALTIME_BARS,
    MIN_SERVER_VER_TICK_BY_TICK, MIN_SERVER_VER_TICK_BY_TICK_IGNORE_SIZE,
    MIN_SERVER_VER_TRADING_CLASS, MIN_SERVER_VER_TRAILING_PERCENT,
};
use crate::types::{
    Contract, ExecutionFilter, Order, OrderCancel, ScannerSubscription, TagValue, TickerId,
};

/// A request that can be encoded as TWS NUL-separated fields.
pub trait EncodableRequest {
    /// Outgoing message id.
    fn message(&self) -> Outgoing;

    /// Appends fields after the message id.
    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()>;

    /// Appends fields for a negotiated server version.
    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        _server_version: i32,
    ) -> TwsApiResult<()> {
        self.encode_fields(fields)
    }

    /// Encodes this request as protobuf when implemented for the request type.
    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(None)
    }
}

/// Encodes a request frame, preferring protobuf when supported by server and request type.
pub fn encode_request_frame<R>(request: &R, server_version: i32) -> TwsApiResult<Vec<u8>>
where
    R: EncodableRequest,
{
    encode_request_frame_with_protobuf(request, server_version, true)
}

/// Encodes a request frame, optionally allowing protobuf when supported.
pub fn encode_request_frame_with_protobuf<R>(
    request: &R,
    server_version: i32,
    prefer_protobuf: bool,
) -> TwsApiResult<Vec<u8>>
where
    R: EncodableRequest,
{
    if prefer_protobuf
        && protobuf_min_server_version(request.message())
            .is_some_and(|min_version| server_version >= min_version)
        && let Some(payload) = request.encode_protobuf()?
    {
        return Ok(comm::make_msg_proto(
            request.message().protobuf_id(),
            &payload,
        ));
    }

    let mut fields = FieldSink::default();
    request.encode_fields_for_server_version(&mut fields, server_version)?;
    comm::make_msg(
        request.message().id(),
        server_version >= MIN_SERVER_VER_PROTOBUF,
        &fields.into_string(),
    )
}

/// Returns the minimum server version for protobuf encoding of an outgoing message.
pub const fn protobuf_min_server_version(message: Outgoing) -> Option<i32> {
    Some(match message {
        Outgoing::ReqExecutions => MIN_SERVER_VER_PROTOBUF,
        Outgoing::PlaceOrder | Outgoing::CancelOrder | Outgoing::ReqGlobalCancel => {
            MIN_SERVER_VER_PROTOBUF_PLACE_ORDER
        }
        Outgoing::ReqAllOpenOrders
        | Outgoing::ReqAutoOpenOrders
        | Outgoing::ReqOpenOrders
        | Outgoing::ReqCompletedOrders => MIN_SERVER_VER_PROTOBUF_COMPLETED_ORDER,
        Outgoing::ReqContractData => MIN_SERVER_VER_PROTOBUF_CONTRACT_DATA,
        Outgoing::ReqMktData
        | Outgoing::CancelMktData
        | Outgoing::ReqMktDepth
        | Outgoing::CancelMktDepth
        | Outgoing::ReqMarketDataType => MIN_SERVER_VER_PROTOBUF_MARKET_DATA,
        Outgoing::ReqAcctData
        | Outgoing::ReqManagedAccounts
        | Outgoing::ReqPositions
        | Outgoing::CancelPositions
        | Outgoing::ReqAccountSummary
        | Outgoing::CancelAccountSummary
        | Outgoing::ReqPositionsMulti
        | Outgoing::CancelPositionsMulti
        | Outgoing::ReqAccountUpdatesMulti
        | Outgoing::CancelAccountUpdatesMulti => MIN_SERVER_VER_PROTOBUF_ACCOUNTS_POSITIONS,
        Outgoing::ReqHistoricalData
        | Outgoing::CancelHistoricalData
        | Outgoing::ReqRealTimeBars
        | Outgoing::CancelRealTimeBars
        | Outgoing::ReqHeadTimestamp
        | Outgoing::CancelHeadTimestamp
        | Outgoing::ReqHistogramData
        | Outgoing::CancelHistogramData
        | Outgoing::ReqHistoricalTicks
        | Outgoing::ReqTickByTickData
        | Outgoing::CancelTickByTickData => MIN_SERVER_VER_PROTOBUF_HISTORICAL_DATA,
        Outgoing::ReqNewsBulletins
        | Outgoing::CancelNewsBulletins
        | Outgoing::ReqNewsArticle
        | Outgoing::ReqNewsProviders
        | Outgoing::ReqHistoricalNews
        | Outgoing::ReqWshMetaData
        | Outgoing::CancelWshMetaData
        | Outgoing::ReqWshEventData
        | Outgoing::CancelWshEventData => MIN_SERVER_VER_PROTOBUF_NEWS_DATA,
        Outgoing::ReqScannerParameters
        | Outgoing::ReqScannerSubscription
        | Outgoing::CancelScannerSubscription
        | Outgoing::ReqPnl
        | Outgoing::CancelPnl
        | Outgoing::ReqPnlSingle
        | Outgoing::CancelPnlSingle => MIN_SERVER_VER_PROTOBUF_SCAN_DATA,
        Outgoing::ReqFa
        | Outgoing::ReplaceFa
        | Outgoing::ExerciseOptions
        | Outgoing::ReqCalcImpliedVolat
        | Outgoing::CancelCalcImpliedVolat
        | Outgoing::ReqCalcOptionPrice
        | Outgoing::CancelCalcOptionPrice => MIN_SERVER_VER_PROTOBUF_REST_MESSAGES_1,
        Outgoing::ReqSecDefOptParams
        | Outgoing::ReqSoftDollarTiers
        | Outgoing::ReqFamilyCodes
        | Outgoing::ReqMatchingSymbols
        | Outgoing::ReqSmartComponents
        | Outgoing::ReqMarketRule
        | Outgoing::ReqUserInfo => MIN_SERVER_VER_PROTOBUF_REST_MESSAGES_2,
        Outgoing::ReqIds
        | Outgoing::ReqCurrentTime
        | Outgoing::ReqCurrentTimeInMillis
        | Outgoing::SetServerLogLevel
        | Outgoing::VerifyRequest
        | Outgoing::VerifyMessage
        | Outgoing::QueryDisplayGroups
        | Outgoing::SubscribeToGroupEvents
        | Outgoing::UpdateDisplayGroup
        | Outgoing::UnsubscribeFromGroupEvents
        | Outgoing::ReqMktDepthExchanges => MIN_SERVER_VER_PROTOBUF_REST_MESSAGES_3,
        Outgoing::VerifyAndAuthRequest
        | Outgoing::VerifyAndAuthMessage
        | Outgoing::StartApi
        | Outgoing::CancelContractData
        | Outgoing::CancelHistoricalTicks
        | Outgoing::ReqConfig
        | Outgoing::UpdateConfig => return None,
    })
}

/// Field encoder with TWS sentinel handling.
#[derive(Debug, Clone, Default)]
pub struct FieldSink {
    fields: String,
}

impl FieldSink {
    /// Appends a regular field.
    pub fn push<T>(&mut self, value: T) -> TwsApiResult<&mut Self>
    where
        T: comm::TwsField,
    {
        self.fields.push_str(&comm::make_field(value)?);
        Ok(self)
    }

    /// Appends a field where TWS unset sentinels are encoded as empty values.
    pub fn push_empty<T>(&mut self, value: T) -> TwsApiResult<&mut Self>
    where
        T: comm::TwsNullableField,
    {
        self.fields.push_str(&comm::make_field_handle_empty(value)?);
        Ok(self)
    }

    /// Appends pre-encoded raw fields.
    pub fn push_raw(&mut self, raw: &str) -> &mut Self {
        self.fields.push_str(raw);
        self
    }

    /// Returns encoded fields.
    pub fn into_string(self) -> String {
        self.fields
    }
}

/// A raw request for migration and protocol coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRequest {
    /// Message id.
    pub message: Outgoing,
    /// Already encoded NUL-separated fields after the message id.
    pub fields: String,
}

/// Result of validating order fields against a negotiated server version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OrderFieldValidation {
    /// Unsupported parameter name.
    pub parameter: &'static str,
    /// Minimum supported server version.
    pub min_version: i32,
}

impl EncodableRequest for RawRequest {
    fn message(&self) -> Outgoing {
        self.message
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push_raw(&self.fields);
        Ok(())
    }
}

/// Request with no field payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyRequest {
    /// Message id.
    pub message: Outgoing,
}

impl EncodableRequest for EmptyRequest {
    fn message(&self) -> Outgoing {
        self.message
    }

    fn encode_fields(&self, _fields: &mut FieldSink) -> TwsApiResult<()> {
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let payload = match self.message {
            Outgoing::ReqMktDepthExchanges => {
                protobuf::MarketDepthExchangesRequest::default().encode_to_vec()
            }
            Outgoing::ReqNewsProviders => protobuf::NewsProvidersRequest::default().encode_to_vec(),
            Outgoing::ReqFamilyCodes => protobuf::FamilyCodesRequest::default().encode_to_vec(),
            Outgoing::ReqCurrentTimeInMillis => {
                protobuf::CurrentTimeInMillisRequest::default().encode_to_vec()
            }
            _ => return Ok(None),
        };
        Ok(Some(payload))
    }
}

/// Request for `startApi`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartApiRequest {
    /// Client id.
    pub client_id: i32,
    /// Optional capabilities.
    pub optional_capabilities: Option<String>,
    /// Whether to encode optional capabilities.
    pub include_optional_capabilities: bool,
}

impl EncodableRequest for StartApiRequest {
    fn message(&self) -> Outgoing {
        Outgoing::StartApi
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(2)?.push(self.client_id)?;
        if self.include_optional_capabilities {
            fields.push(self.optional_capabilities.as_deref().unwrap_or(""))?;
        }
        Ok(())
    }
}

/// Request with only a protocol version field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionedRequest {
    /// Message id.
    pub message: Outgoing,
    /// Protocol version.
    pub version: i32,
}

impl EncodableRequest for VersionedRequest {
    fn message(&self) -> Outgoing {
        self.message
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(self.version)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let payload = match self.message {
            Outgoing::ReqCurrentTime => protobuf::CurrentTimeRequest::default().encode_to_vec(),
            Outgoing::ReqCurrentTimeInMillis => {
                protobuf::CurrentTimeInMillisRequest::default().encode_to_vec()
            }
            Outgoing::ReqPositions => protobuf::PositionsRequest::default().encode_to_vec(),
            Outgoing::CancelPositions => protobuf::CancelPositions::default().encode_to_vec(),
            Outgoing::ReqOpenOrders => protobuf::OpenOrdersRequest::default().encode_to_vec(),
            Outgoing::ReqAllOpenOrders => protobuf::AllOpenOrdersRequest::default().encode_to_vec(),
            Outgoing::ReqManagedAccounts => {
                protobuf::ManagedAccountsRequest::default().encode_to_vec()
            }
            Outgoing::ReqScannerParameters => {
                protobuf::ScannerParametersRequest::default().encode_to_vec()
            }
            Outgoing::CancelNewsBulletins => {
                protobuf::CancelNewsBulletins::default().encode_to_vec()
            }
            _ => return Ok(None),
        };
        Ok(Some(payload))
    }
}

/// Request with only an id field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdRequest {
    /// Message id.
    pub message: Outgoing,
    /// Protocol version.
    pub version: Option<i32>,
    /// Request id.
    pub req_id: i32,
}

/// Server log-level request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetServerLogLevelRequest {
    /// Log level.
    pub log_level: i32,
}

impl EncodableRequest for SetServerLogLevelRequest {
    fn message(&self) -> Outgoing {
        Outgoing::SetServerLogLevel
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(1)?.push(self.log_level)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::SetServerLogLevelRequest {
                log_level: Some(self.log_level),
            }
            .encode_to_vec(),
        ))
    }
}

/// Request a market data type switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketDataTypeRequest {
    /// Market data type.
    pub market_data_type: i32,
}

impl EncodableRequest for MarketDataTypeRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqMarketDataType
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(1)?.push(self.market_data_type)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::MarketDataTypeRequest {
                market_data_type: Some(self.market_data_type),
            }
            .encode_to_vec(),
        ))
    }
}

/// Smart components request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartComponentsRequest {
    /// Request id.
    pub req_id: i32,
    /// BBO exchange.
    pub bbo_exchange: String,
}

impl EncodableRequest for SmartComponentsRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqSmartComponents
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(self.req_id)?.push(&self.bbo_exchange)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::SmartComponentsRequest {
                req_id: Some(self.req_id),
                bbo_exchange: non_empty(self.bbo_exchange.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Cancel market depth request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelMarketDepthRequest {
    /// Request id.
    pub req_id: i32,
    /// Smart depth.
    pub is_smart_depth: bool,
}

impl EncodableRequest for CancelMarketDepthRequest {
    fn message(&self) -> Outgoing {
        Outgoing::CancelMktDepth
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(self.req_id)?
            .push(self.is_smart_depth)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::CancelMarketDepth {
                req_id: Some(self.req_id),
                is_smart_depth: self.is_smart_depth.then_some(true),
            }
            .encode_to_vec(),
        ))
    }
}

/// Auto-open-orders request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoOpenOrdersRequest {
    /// Auto-bind flag.
    pub auto_bind: bool,
}

impl EncodableRequest for AutoOpenOrdersRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqAutoOpenOrders
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(1)?.push(self.auto_bind)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::AutoOpenOrdersRequest {
                auto_bind: Some(self.auto_bind),
            }
            .encode_to_vec(),
        ))
    }
}

/// Global cancel request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalCancelRequest {
    /// Cancel metadata.
    pub order_cancel: OrderCancel,
}

impl EncodableRequest for GlobalCancelRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqGlobalCancel
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(&self.order_cancel.manual_order_cancel_time)?
            .push(&self.order_cancel.ext_operator)?
            .push(self.order_cancel.manual_order_indicator)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::GlobalCancelRequest {
                order_cancel: Some(order_cancel_to_proto(&self.order_cancel)),
            }
            .encode_to_vec(),
        ))
    }
}

/// Account data subscription request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDataRequest {
    /// Subscribe or unsubscribe.
    pub subscribe: bool,
    /// Account code.
    pub account_code: String,
}

impl EncodableRequest for AccountDataRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqAcctData
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(2)?
            .push(self.subscribe)?
            .push(&self.account_code)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::AccountDataRequest {
                subscribe: Some(self.subscribe),
                acct_code: non_empty(self.account_code.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Account summary request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummaryRequest {
    /// Request id.
    pub req_id: i32,
    /// Group name.
    pub group_name: String,
    /// Tags.
    pub tags: String,
}

impl EncodableRequest for AccountSummaryRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqAccountSummary
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(self.req_id)?
            .push(&self.group_name)?
            .push(&self.tags)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::AccountSummaryRequest {
                req_id: Some(self.req_id),
                group: non_empty(self.group_name.clone()),
                tags: non_empty(self.tags.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Positions multi request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionsMultiRequest {
    /// Request id.
    pub req_id: i32,
    /// Account.
    pub account: String,
    /// Model code.
    pub model_code: String,
}

impl EncodableRequest for PositionsMultiRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqPositionsMulti
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(self.req_id)?
            .push(&self.account)?
            .push(&self.model_code)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::PositionsMultiRequest {
                req_id: Some(self.req_id),
                account: non_empty(self.account.clone()),
                model_code: non_empty(self.model_code.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Account updates multi request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountUpdatesMultiRequest {
    /// Request id.
    pub req_id: i32,
    /// Account.
    pub account: String,
    /// Model code.
    pub model_code: String,
    /// Whether to include ledger and NLV.
    pub ledger_and_nlv: bool,
}

impl EncodableRequest for AccountUpdatesMultiRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqAccountUpdatesMulti
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(self.req_id)?
            .push(&self.account)?
            .push(&self.model_code)?
            .push(self.ledger_and_nlv)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::AccountUpdatesMultiRequest {
                req_id: Some(self.req_id),
                account: non_empty(self.account.clone()),
                model_code: non_empty(self.model_code.clone()),
                ledger_and_nlv: Some(self.ledger_and_nlv),
            }
            .encode_to_vec(),
        ))
    }
}

/// PnL request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PnlRequest {
    /// Request id.
    pub req_id: i32,
    /// Account.
    pub account: String,
    /// Model code.
    pub model_code: String,
}

impl EncodableRequest for PnlRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqPnl
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(self.req_id)?
            .push(&self.account)?
            .push(&self.model_code)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::PnLRequest {
                req_id: Some(self.req_id),
                account: non_empty(self.account.clone()),
                model_code: non_empty(self.model_code.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Single-position PnL request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PnlSingleRequest {
    /// Request id.
    pub req_id: i32,
    /// Account.
    pub account: String,
    /// Model code.
    pub model_code: String,
    /// Contract id.
    pub con_id: i32,
}

/// News bulletin subscription request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewsBulletinsRequest {
    /// Include all messages.
    pub all_messages: bool,
}

impl EncodableRequest for NewsBulletinsRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqNewsBulletins
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(1)?.push(self.all_messages)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::NewsBulletinsRequest {
                all_messages: Some(self.all_messages),
            }
            .encode_to_vec(),
        ))
    }
}

/// News article request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewsArticleRequest {
    /// Request id.
    pub req_id: i32,
    /// Provider code.
    pub provider_code: String,
    /// Article id.
    pub article_id: String,
    /// Request options.
    pub options: Vec<TagValue>,
}

impl EncodableRequest for NewsArticleRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqNewsArticle
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(self.req_id)?
            .push(&self.provider_code)?
            .push(&self.article_id)?;
        encode_tag_values(fields, &self.options)
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::NewsArticleRequest {
                req_id: Some(self.req_id),
                provider_code: non_empty(self.provider_code.clone()),
                article_id: non_empty(self.article_id.clone()),
                news_article_options: tag_values_to_map(&self.options),
            }
            .encode_to_vec(),
        ))
    }
}

/// Historical news request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoricalNewsRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract id.
    pub con_id: i32,
    /// Provider codes.
    pub provider_codes: String,
    /// Start date/time.
    pub start_date_time: String,
    /// End date/time.
    pub end_date_time: String,
    /// Total results.
    pub total_results: i32,
    /// Request options.
    pub options: Vec<TagValue>,
}

impl EncodableRequest for HistoricalNewsRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqHistoricalNews
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(self.req_id)?
            .push(self.con_id)?
            .push(&self.provider_codes)?
            .push(&self.start_date_time)?
            .push(&self.end_date_time)?
            .push(self.total_results)?;
        encode_tag_values(fields, &self.options)
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::HistoricalNewsRequest {
                req_id: Some(self.req_id),
                con_id: Some(self.con_id),
                provider_codes: non_empty(self.provider_codes.clone()),
                start_date_time: non_empty(self.start_date_time.clone()),
                end_date_time: non_empty(self.end_date_time.clone()),
                total_results: Some(self.total_results),
                historical_news_options: tag_values_to_map(&self.options),
            }
            .encode_to_vec(),
        ))
    }
}

/// Security-definition option parameters request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecDefOptParamsRequest {
    /// Request id.
    pub req_id: i32,
    /// Underlying symbol.
    pub underlying_symbol: String,
    /// FUT/FOP exchange.
    pub fut_fop_exchange: String,
    /// Underlying security type.
    pub underlying_sec_type: String,
    /// Underlying contract id.
    pub underlying_con_id: i32,
}

impl EncodableRequest for SecDefOptParamsRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqSecDefOptParams
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(self.req_id)?
            .push(&self.underlying_symbol)?
            .push(&self.fut_fop_exchange)?
            .push(&self.underlying_sec_type)?
            .push(self.underlying_con_id)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::SecDefOptParamsRequest {
                req_id: Some(self.req_id),
                underlying_symbol: non_empty(self.underlying_symbol.clone()),
                fut_fop_exchange: non_empty(self.fut_fop_exchange.clone()),
                underlying_sec_type: non_empty(self.underlying_sec_type.clone()),
                underlying_con_id: Some(self.underlying_con_id),
            }
            .encode_to_vec(),
        ))
    }
}

/// Matching-symbols request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchingSymbolsRequest {
    /// Request id.
    pub req_id: i32,
    /// Search pattern.
    pub pattern: String,
}

impl EncodableRequest for MatchingSymbolsRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqMatchingSymbols
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(self.req_id)?.push(&self.pattern)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::MatchingSymbolsRequest {
                req_id: Some(self.req_id),
                pattern: non_empty(self.pattern.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Completed-orders request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletedOrdersRequest {
    /// Include API-only orders.
    pub api_only: bool,
}

impl EncodableRequest for CompletedOrdersRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqCompletedOrders
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(self.api_only)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::CompletedOrdersRequest {
                api_only: Some(self.api_only),
            }
            .encode_to_vec(),
        ))
    }
}

/// Financial-advisor data request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinancialAdvisorRequest {
    /// FA data type.
    pub fa_data_type: i32,
}

impl EncodableRequest for FinancialAdvisorRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqFa
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(1)?.push(self.fa_data_type)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::FaRequest {
                fa_data_type: Some(self.fa_data_type),
            }
            .encode_to_vec(),
        ))
    }
}

/// Financial-advisor replace request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceFinancialAdvisorRequest {
    /// Request id.
    pub req_id: i32,
    /// FA data type.
    pub fa_data_type: i32,
    /// XML payload.
    pub xml: String,
}

impl EncodableRequest for ReplaceFinancialAdvisorRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReplaceFa
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(self.req_id)?
            .push(self.fa_data_type)?
            .push(&self.xml)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::FaReplace {
                req_id: Some(self.req_id),
                fa_data_type: Some(self.fa_data_type),
                xml: non_empty(self.xml.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Head timestamp request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeadTimestampRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract.
    pub contract: Contract,
    /// Use regular trading hours.
    pub use_rth: bool,
    /// Data type.
    pub what_to_show: String,
    /// Format date.
    pub format_date: i32,
}

impl EncodableRequest for HeadTimestampRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqHeadTimestamp
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(self.req_id)?;
        encode_contract_core(fields, &self.contract)?;
        fields
            .push(self.use_rth)?
            .push(&self.what_to_show)?
            .push(self.format_date)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::HeadTimestampRequest {
                req_id: Some(self.req_id),
                contract: Some(contract_to_proto(&self.contract, None)),
                use_rth: Some(self.use_rth),
                what_to_show: non_empty(self.what_to_show.clone()),
                format_date: Some(self.format_date),
            }
            .encode_to_vec(),
        ))
    }
}

/// Histogram data request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistogramDataRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract.
    pub contract: Contract,
    /// Use regular trading hours.
    pub use_rth: bool,
    /// Time period.
    pub time_period: String,
}

impl EncodableRequest for HistogramDataRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqHistogramData
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(self.req_id)?;
        encode_contract_core(fields, &self.contract)?;
        fields.push(self.use_rth)?.push(&self.time_period)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::HistogramDataRequest {
                req_id: Some(self.req_id),
                contract: Some(contract_to_proto(&self.contract, None)),
                use_rth: Some(self.use_rth),
                time_period: non_empty(self.time_period.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Real-time bars request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RealTimeBarsRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract.
    pub contract: Contract,
    /// Bar size.
    pub bar_size: i32,
    /// Data type.
    pub what_to_show: String,
    /// Use regular trading hours.
    pub use_rth: bool,
    /// Request options.
    pub options: Vec<TagValue>,
}

impl EncodableRequest for RealTimeBarsRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqRealTimeBars
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(3)?.push(self.req_id)?;
        encode_contract_core(fields, &self.contract)?;
        fields
            .push(self.bar_size)?
            .push(&self.what_to_show)?
            .push(self.use_rth)?;
        encode_tag_values(fields, &self.options)
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::RealTimeBarsRequest {
                req_id: Some(self.req_id),
                contract: Some(contract_to_proto(&self.contract, None)),
                bar_size: Some(self.bar_size),
                what_to_show: non_empty(self.what_to_show.clone()),
                use_rth: Some(self.use_rth),
                real_time_bars_options: tag_values_to_map(&self.options),
            }
            .encode_to_vec(),
        ))
    }
}

/// Subscribe to display-group events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscribeToGroupEventsRequest {
    /// Request id.
    pub req_id: i32,
    /// Group id.
    pub group_id: i32,
}

impl EncodableRequest for SubscribeToGroupEventsRequest {
    fn message(&self) -> Outgoing {
        Outgoing::SubscribeToGroupEvents
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(1)?.push(self.req_id)?.push(self.group_id)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::SubscribeToGroupEventsRequest {
                req_id: Some(self.req_id),
                group_id: Some(self.group_id),
            }
            .encode_to_vec(),
        ))
    }
}

/// Update display group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateDisplayGroupRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract info.
    pub contract_info: String,
}

impl EncodableRequest for UpdateDisplayGroupRequest {
    fn message(&self) -> Outgoing {
        Outgoing::UpdateDisplayGroup
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(self.req_id)?
            .push(&self.contract_info)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::UpdateDisplayGroupRequest {
                req_id: Some(self.req_id),
                contract_info: non_empty(self.contract_info.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Verify request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyRequest {
    /// API name.
    pub api_name: String,
    /// API version.
    pub api_version: String,
}

impl EncodableRequest for VerifyRequest {
    fn message(&self) -> Outgoing {
        Outgoing::VerifyRequest
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(&self.api_name)?
            .push(&self.api_version)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::VerifyRequest {
                api_name: non_empty(self.api_name.clone()),
                api_version: non_empty(self.api_version.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Verify message request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyMessageRequest {
    /// API data.
    pub api_data: String,
}

impl EncodableRequest for VerifyMessageRequest {
    fn message(&self) -> Outgoing {
        Outgoing::VerifyMessage
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(1)?.push(&self.api_data)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::VerifyMessageRequest {
                api_data: non_empty(self.api_data.clone()),
            }
            .encode_to_vec(),
        ))
    }
}

/// Verify-and-auth request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyAndAuthRequest {
    /// API name.
    pub api_name: String,
    /// API version.
    pub api_version: String,
    /// Opaque ISV key.
    pub opaque_isv_key: String,
}

impl EncodableRequest for VerifyAndAuthRequest {
    fn message(&self) -> Outgoing {
        Outgoing::VerifyAndAuthRequest
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(&self.api_name)?
            .push(&self.api_version)?
            .push(&self.opaque_isv_key)?;
        Ok(())
    }
}

/// Verify-and-auth message request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyAndAuthMessageRequest {
    /// API data.
    pub api_data: String,
    /// Challenge response.
    pub xyz_response: String,
}

impl EncodableRequest for VerifyAndAuthMessageRequest {
    fn message(&self) -> Outgoing {
        Outgoing::VerifyAndAuthMessage
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(&self.api_data)?
            .push(&self.xyz_response)?;
        Ok(())
    }
}

/// WSH event data request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WshEventDataRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract id.
    pub con_id: i32,
    /// Filter JSON.
    pub filter: String,
    /// Fill watchlist flag.
    pub fill_watchlist: bool,
    /// Fill portfolio flag.
    pub fill_portfolio: bool,
    /// Fill competitors flag.
    pub fill_competitors: bool,
    /// Start date.
    pub start_date: String,
    /// End date.
    pub end_date: String,
    /// Total limit.
    pub total_limit: i32,
}

impl EncodableRequest for WshEventDataRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqWshEventData
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(self.req_id)?
            .push(self.con_id)?
            .push(&self.filter)?
            .push(self.fill_watchlist)?
            .push(self.fill_portfolio)?
            .push(self.fill_competitors)?
            .push(&self.start_date)?
            .push(&self.end_date)?
            .push(self.total_limit)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::WshEventDataRequest {
                req_id: Some(self.req_id),
                con_id: valid_i32(self.con_id),
                filter: non_empty(self.filter.clone()),
                fill_watchlist: self.fill_watchlist.then_some(true),
                fill_portfolio: self.fill_portfolio.then_some(true),
                fill_competitors: self.fill_competitors.then_some(true),
                start_date: non_empty(self.start_date.clone()),
                end_date: non_empty(self.end_date.clone()),
                total_limit: valid_i32(self.total_limit),
            }
            .encode_to_vec(),
        ))
    }
}

/// Tick-by-tick data request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TickByTickRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract.
    pub contract: Contract,
    /// Tick type.
    pub tick_type: String,
    /// Number of ticks.
    pub number_of_ticks: i32,
    /// Ignore size flag.
    pub ignore_size: bool,
}

impl EncodableRequest for TickByTickRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqTickByTickData
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        self.encode_fields_for_server_version(fields, MAX_CLIENT_VER)
    }

    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        server_version: i32,
    ) -> TwsApiResult<()> {
        ensure_server_version(server_version, MIN_SERVER_VER_TICK_BY_TICK)?;
        fields
            .push(self.req_id)?
            .push(self.contract.con_id)?
            .push(&self.contract.symbol)?
            .push(&self.contract.sec_type)?
            .push(&self.contract.last_trade_date_or_contract_month)?
            .push_empty(self.contract.strike)?
            .push(&self.contract.right)?
            .push(&self.contract.multiplier)?
            .push(&self.contract.exchange)?
            .push(&self.contract.primary_exchange)?
            .push(&self.contract.currency)?
            .push(&self.contract.local_symbol)?
            .push(&self.contract.trading_class)?
            .push(&self.tick_type)?;
        if server_version >= MIN_SERVER_VER_TICK_BY_TICK_IGNORE_SIZE {
            fields.push(self.number_of_ticks)?.push(self.ignore_size)?;
        }
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::TickByTickRequest {
                req_id: Some(self.req_id),
                contract: Some(contract_to_proto(&self.contract, None)),
                tick_type: non_empty(self.tick_type.clone()),
                number_of_ticks: valid_i32(self.number_of_ticks),
                ignore_size: self.ignore_size.then_some(true),
            }
            .encode_to_vec(),
        ))
    }
}

/// Calculate implied volatility request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CalculateImpliedVolatilityRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract.
    pub contract: Contract,
    /// Option price.
    pub option_price: f64,
    /// Underlying price.
    pub under_price: f64,
    /// Request options.
    pub options: Vec<TagValue>,
}

impl EncodableRequest for CalculateImpliedVolatilityRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqCalcImpliedVolat
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        self.encode_fields_for_server_version(fields, MAX_CLIENT_VER)
    }

    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        server_version: i32,
    ) -> TwsApiResult<()> {
        ensure_server_version(server_version, MIN_SERVER_VER_REQ_CALC_IMPLIED_VOLAT)?;
        fields.push(3)?.push(self.req_id)?;
        encode_contract_for_calculation(fields, &self.contract, server_version)?;
        fields.push(self.option_price)?.push(self.under_price)?;
        if server_version >= MIN_SERVER_VER_LINKING {
            fields.push(tag_values_to_tws_options(&self.options))?;
        }
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::CalculateImpliedVolatilityRequest {
                req_id: Some(self.req_id),
                contract: Some(contract_to_proto(&self.contract, None)),
                option_price: valid_f64(self.option_price),
                under_price: valid_f64(self.under_price),
                implied_volatility_options: tag_values_to_map(&self.options),
            }
            .encode_to_vec(),
        ))
    }
}

/// Calculate option price request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CalculateOptionPriceRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract.
    pub contract: Contract,
    /// Volatility.
    pub volatility: f64,
    /// Underlying price.
    pub under_price: f64,
    /// Request options.
    pub options: Vec<TagValue>,
}

impl EncodableRequest for CalculateOptionPriceRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqCalcOptionPrice
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        self.encode_fields_for_server_version(fields, MAX_CLIENT_VER)
    }

    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        server_version: i32,
    ) -> TwsApiResult<()> {
        ensure_server_version(server_version, MIN_SERVER_VER_REQ_CALC_IMPLIED_VOLAT)?;
        fields.push(3)?.push(self.req_id)?;
        encode_contract_for_calculation(fields, &self.contract, server_version)?;
        fields.push(self.volatility)?.push(self.under_price)?;
        if server_version >= MIN_SERVER_VER_LINKING {
            fields.push(tag_values_to_tws_options(&self.options))?;
        }
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::CalculateOptionPriceRequest {
                req_id: Some(self.req_id),
                contract: Some(contract_to_proto(&self.contract, None)),
                volatility: valid_f64(self.volatility),
                under_price: valid_f64(self.under_price),
                option_price_options: tag_values_to_map(&self.options),
            }
            .encode_to_vec(),
        ))
    }
}

/// Exercise options request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExerciseOptionsRequest {
    /// Order/request id.
    pub order_id: i32,
    /// Contract.
    pub contract: Contract,
    /// Exercise action.
    pub exercise_action: i32,
    /// Exercise quantity.
    pub exercise_quantity: i32,
    /// Account.
    pub account: String,
    /// Override natural action.
    pub override_system_action: bool,
    /// Manual order time.
    pub manual_order_time: String,
    /// Customer account.
    pub customer_account: String,
    /// Professional customer flag.
    pub professional_customer: bool,
}

impl EncodableRequest for ExerciseOptionsRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ExerciseOptions
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        self.encode_fields_for_server_version(fields, MAX_CLIENT_VER)
    }

    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        server_version: i32,
    ) -> TwsApiResult<()> {
        fields.push(2)?.push(self.order_id)?;
        if server_version >= MIN_SERVER_VER_TRADING_CLASS {
            fields.push(self.contract.con_id)?;
        }
        fields
            .push(&self.contract.symbol)?
            .push(&self.contract.sec_type)?
            .push(&self.contract.last_trade_date_or_contract_month)?
            .push_empty(self.contract.strike)?
            .push(&self.contract.right)?
            .push(&self.contract.multiplier)?
            .push(&self.contract.exchange)?
            .push(&self.contract.currency)?
            .push(&self.contract.local_symbol)?;
        if server_version >= MIN_SERVER_VER_TRADING_CLASS {
            fields.push(&self.contract.trading_class)?;
        }
        fields
            .push(self.exercise_action)?
            .push(self.exercise_quantity)?
            .push(&self.account)?
            .push(self.override_system_action)?;
        if server_version >= MIN_SERVER_VER_MANUAL_ORDER_TIME_EXERCISE_OPTIONS {
            fields.push(&self.manual_order_time)?;
        }
        if server_version >= MIN_SERVER_VER_CUSTOMER_ACCOUNT {
            fields.push(&self.customer_account)?;
        }
        if server_version >= MIN_SERVER_VER_PROFESSIONAL_CUSTOMER {
            fields.push(self.professional_customer)?;
        }
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::ExerciseOptionsRequest {
                order_id: Some(self.order_id),
                contract: Some(contract_to_proto(&self.contract, None)),
                exercise_action: Some(self.exercise_action),
                exercise_quantity: Some(self.exercise_quantity),
                account: non_empty(self.account.clone()),
                r#override: Some(self.override_system_action),
                manual_order_time: non_empty(self.manual_order_time.clone()),
                customer_account: non_empty(self.customer_account.clone()),
                professional_customer: self.professional_customer.then_some(true),
            }
            .encode_to_vec(),
        ))
    }
}

/// Historical ticks request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoricalTicksRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract.
    pub contract: Contract,
    /// Start date/time.
    pub start_date_time: String,
    /// End date/time.
    pub end_date_time: String,
    /// Number of ticks.
    pub number_of_ticks: i32,
    /// Data type.
    pub what_to_show: String,
    /// Use regular trading hours.
    pub use_rth: bool,
    /// Ignore size flag.
    pub ignore_size: bool,
    /// Request options.
    pub misc_options: Vec<TagValue>,
}

impl EncodableRequest for HistoricalTicksRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqHistoricalTicks
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        self.encode_fields_for_server_version(fields, MAX_CLIENT_VER)
    }

    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        server_version: i32,
    ) -> TwsApiResult<()> {
        ensure_server_version(server_version, MIN_SERVER_VER_HISTORICAL_TICKS)?;
        fields
            .push(self.req_id)?
            .push(self.contract.con_id)?
            .push(&self.contract.symbol)?
            .push(&self.contract.sec_type)?
            .push(&self.contract.last_trade_date_or_contract_month)?
            .push_empty(self.contract.strike)?
            .push(&self.contract.right)?
            .push(&self.contract.multiplier)?
            .push(&self.contract.exchange)?
            .push(&self.contract.primary_exchange)?
            .push(&self.contract.currency)?
            .push(&self.contract.local_symbol)?
            .push(&self.contract.trading_class)?
            .push(self.contract.include_expired)?
            .push(&self.start_date_time)?
            .push(&self.end_date_time)?
            .push(self.number_of_ticks)?
            .push(&self.what_to_show)?
            .push(self.use_rth)?
            .push(self.ignore_size)?
            .push(tag_values_to_tws_options(&self.misc_options))?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::HistoricalTicksRequest {
                req_id: Some(self.req_id),
                contract: Some(contract_to_proto(&self.contract, None)),
                start_date_time: non_empty(self.start_date_time.clone()),
                end_date_time: non_empty(self.end_date_time.clone()),
                number_of_ticks: Some(self.number_of_ticks),
                what_to_show: non_empty(self.what_to_show.clone()),
                use_rth: Some(self.use_rth),
                ignore_size: self.ignore_size.then_some(true),
                misc_options: tag_values_to_map(&self.misc_options),
            }
            .encode_to_vec(),
        ))
    }
}

impl EncodableRequest for PnlSingleRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqPnlSingle
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(self.req_id)?
            .push(&self.account)?
            .push(&self.model_code)?
            .push(self.con_id)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        Ok(Some(
            protobuf::PnLSingleRequest {
                req_id: Some(self.req_id),
                account: non_empty(self.account.clone()),
                model_code: non_empty(self.model_code.clone()),
                con_id: Some(self.con_id),
            }
            .encode_to_vec(),
        ))
    }
}

impl EncodableRequest for IdRequest {
    fn message(&self) -> Outgoing {
        self.message
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        if let Some(version) = self.version {
            fields.push(version)?;
        }
        fields.push(self.req_id)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let payload = match self.message {
            Outgoing::ReqIds => protobuf::IdsRequest {
                num_ids: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelMktData => protobuf::CancelMarketData {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelMktDepth => protobuf::CancelMarketDepth {
                req_id: Some(self.req_id),
                is_smart_depth: None,
            }
            .encode_to_vec(),
            Outgoing::CancelAccountSummary => protobuf::CancelAccountSummary {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelPositionsMulti => protobuf::CancelPositionsMulti {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelAccountUpdatesMulti => protobuf::CancelAccountUpdatesMulti {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelPnl => protobuf::CancelPnL {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelPnlSingle => protobuf::CancelPnLSingle {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelCalcImpliedVolat => protobuf::CancelCalculateImpliedVolatility {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelCalcOptionPrice => protobuf::CancelCalculateOptionPrice {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelContractData => protobuf::CancelContractData {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelHistoricalData => protobuf::CancelHistoricalData {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelScannerSubscription => protobuf::CancelScannerSubscription {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelTickByTickData => protobuf::CancelTickByTick {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelWshMetaData => protobuf::CancelWshMetaData {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelWshEventData => protobuf::CancelWshEventData {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelHeadTimestamp => protobuf::CancelHeadTimestamp {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelHistogramData => protobuf::CancelHistogramData {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::CancelRealTimeBars => protobuf::CancelRealTimeBars {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::QueryDisplayGroups => protobuf::QueryDisplayGroupsRequest {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::UnsubscribeFromGroupEvents => protobuf::UnsubscribeFromGroupEventsRequest {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::ReqMarketRule => protobuf::MarketRuleRequest {
                market_rule_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::ReqSoftDollarTiers => protobuf::SoftDollarTiersRequest {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::ReqWshMetaData => protobuf::WshMetaDataRequest {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            Outgoing::ReqUserInfo => protobuf::UserInfoRequest {
                req_id: Some(self.req_id),
            }
            .encode_to_vec(),
            _ => return Ok(None),
        };
        Ok(Some(payload))
    }
}

/// Market data request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarketDataRequest {
    /// Request id.
    pub req_id: TickerId,
    /// Contract.
    pub contract: Contract,
    /// Generic tick list.
    pub generic_tick_list: String,
    /// Snapshot flag.
    pub snapshot: bool,
    /// Regulatory snapshot flag.
    pub regulatory_snapshot: bool,
    /// Market data options.
    pub market_data_options: Vec<TagValue>,
}

impl EncodableRequest for MarketDataRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqMktData
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        self.encode_fields_for_server_version(fields, MAX_CLIENT_VER)
    }

    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        server_version: i32,
    ) -> TwsApiResult<()> {
        fields.push(11)?.push(self.req_id)?;
        if server_version >= MIN_SERVER_VER_REQ_MKT_DATA_CONID {
            fields.push(self.contract.con_id)?;
        }
        encode_contract_market_data(fields, &self.contract, server_version)?;

        if self.contract.sec_type == "BAG" {
            fields.push(self.contract.combo_legs.len())?;
            for leg in &self.contract.combo_legs {
                fields
                    .push(leg.con_id)?
                    .push(leg.ratio)?
                    .push(&leg.action)?
                    .push(&leg.exchange)?;
            }
        }

        if server_version >= MIN_SERVER_VER_DELTA_NEUTRAL {
            if let Some(delta_neutral) = &self.contract.delta_neutral_contract {
                fields
                    .push(true)?
                    .push(delta_neutral.con_id)?
                    .push(delta_neutral.delta)?
                    .push(delta_neutral.price)?;
            } else {
                fields.push(false)?;
            }
        }

        fields.push(&self.generic_tick_list)?.push(self.snapshot)?;
        if server_version >= MIN_SERVER_VER_REQ_SMART_COMPONENTS {
            fields.push(self.regulatory_snapshot)?;
        }
        if server_version >= MIN_SERVER_VER_LINKING {
            fields.push(tag_values_to_tws_options(&self.market_data_options))?;
        }
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let mut msg = protobuf::MarketDataRequest {
            req_id: Some(self.req_id),
            contract: Some(contract_to_proto(&self.contract, None)),
            generic_tick_list: non_empty(self.generic_tick_list.clone()),
            snapshot: self.snapshot.then_some(true),
            regulatory_snapshot: self.regulatory_snapshot.then_some(true),
            market_data_options: tag_values_to_map(&self.market_data_options),
        };
        if msg.market_data_options.is_empty() {
            msg.market_data_options = Default::default();
        }
        Ok(Some(msg.encode_to_vec()))
    }
}

/// Market depth request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarketDepthRequest {
    /// Request id.
    pub req_id: TickerId,
    /// Contract.
    pub contract: Contract,
    /// Number of rows.
    pub num_rows: i32,
    /// Smart depth.
    pub is_smart_depth: bool,
    /// Market depth options.
    pub market_depth_options: Vec<TagValue>,
}

impl EncodableRequest for MarketDepthRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqMktDepth
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields.push(5)?.push(self.req_id)?;
        encode_contract_core(fields, &self.contract)?;
        fields.push(self.num_rows)?.push(self.is_smart_depth)?;
        encode_tag_values(fields, &self.market_depth_options)
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let msg = protobuf::MarketDepthRequest {
            req_id: Some(self.req_id),
            contract: Some(contract_to_proto(&self.contract, None)),
            num_rows: Some(self.num_rows),
            is_smart_depth: self.is_smart_depth.then_some(true),
            market_depth_options: tag_values_to_map(&self.market_depth_options),
        };
        Ok(Some(msg.encode_to_vec()))
    }
}

/// Place order request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaceOrderRequest {
    /// Order id.
    pub order_id: i32,
    /// Contract.
    pub contract: Contract,
    /// Order.
    pub order: Order,
    /// Extra raw fields for TWS server-version-specific order tail fields.
    pub extra_fields: String,
}

impl EncodableRequest for PlaceOrderRequest {
    fn message(&self) -> Outgoing {
        Outgoing::PlaceOrder
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        self.encode_fields_for_server_version(fields, MAX_CLIENT_VER)
    }

    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        server_version: i32,
    ) -> TwsApiResult<()> {
        encode_place_order_fields(
            fields,
            server_version,
            self.order_id,
            &self.contract,
            &self.order,
            &self.extra_fields,
        )
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let msg = protobuf::PlaceOrderRequest {
            order_id: Some(self.order_id),
            contract: Some(contract_to_proto(&self.contract, Some(&self.order))),
            order: Some(order_to_proto(&self.order)),
            attached_orders: attached_orders_to_proto(&self.order),
        };
        Ok(Some(msg.encode_to_vec()))
    }
}

/// Cancel order request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CancelOrderRequest {
    /// Order id.
    pub order_id: i32,
    /// Cancel metadata.
    pub order_cancel: OrderCancel,
}

impl EncodableRequest for CancelOrderRequest {
    fn message(&self) -> Outgoing {
        Outgoing::CancelOrder
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(1)?
            .push(self.order_id)?
            .push(&self.order_cancel.manual_order_cancel_time)?
            .push(&self.order_cancel.ext_operator)?
            .push(self.order_cancel.manual_order_indicator)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let msg = protobuf::CancelOrderRequest {
            order_id: Some(self.order_id),
            order_cancel: Some(order_cancel_to_proto(&self.order_cancel)),
        };
        Ok(Some(msg.encode_to_vec()))
    }
}

/// Execution request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionRequest {
    /// Request id.
    pub req_id: i32,
    /// Execution filter.
    pub filter: ExecutionFilter,
}

impl EncodableRequest for ExecutionRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqExecutions
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        fields
            .push(3)?
            .push(self.req_id)?
            .push(self.filter.client_id)?
            .push(&self.filter.acct_code)?
            .push(&self.filter.time)?
            .push(&self.filter.symbol)?
            .push(&self.filter.sec_type)?
            .push(&self.filter.exchange)?
            .push(&self.filter.side)?;
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let msg = protobuf::ExecutionRequest {
            req_id: Some(self.req_id),
            execution_filter: Some(execution_filter_to_proto(&self.filter)),
        };
        Ok(Some(msg.encode_to_vec()))
    }
}

/// Contract details request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContractDetailsRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract.
    pub contract: Contract,
}

impl EncodableRequest for ContractDetailsRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqContractData
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        self.encode_fields_for_server_version(fields, MAX_CLIENT_VER)
    }

    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        server_version: i32,
    ) -> TwsApiResult<()> {
        fields.push(8)?;
        if server_version >= MIN_SERVER_VER_CONTRACT_DATA_CHAIN {
            fields.push(self.req_id)?;
        }

        fields
            .push(self.contract.con_id)?
            .push(&self.contract.symbol)?
            .push(&self.contract.sec_type)?
            .push(&self.contract.last_trade_date_or_contract_month)?
            .push_empty(self.contract.strike)?
            .push(&self.contract.right)?
            .push(&self.contract.multiplier)?;

        if server_version >= MIN_SERVER_VER_PRIMARYEXCH {
            fields
                .push(&self.contract.exchange)?
                .push(&self.contract.primary_exchange)?;
        } else if server_version >= MIN_SERVER_VER_LINKING {
            let exchange = if !self.contract.primary_exchange.is_empty()
                && (self.contract.exchange == "BEST" || self.contract.exchange == "SMART")
            {
                format!(
                    "{}:{}",
                    self.contract.exchange, self.contract.primary_exchange
                )
            } else {
                self.contract.exchange.clone()
            };
            fields.push(exchange)?;
        }

        fields
            .push(&self.contract.currency)?
            .push(&self.contract.local_symbol)?;
        if server_version >= MIN_SERVER_VER_TRADING_CLASS {
            fields.push(&self.contract.trading_class)?;
        }
        fields.push(self.contract.include_expired)?;
        if server_version >= MIN_SERVER_VER_SEC_ID_TYPE {
            fields
                .push(&self.contract.sec_id_type)?
                .push(&self.contract.sec_id)?;
        }
        if server_version >= MIN_SERVER_VER_BOND_ISSUERID {
            fields.push(&self.contract.issuer_id)?;
        }
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let msg = protobuf::ContractDataRequest {
            req_id: Some(self.req_id),
            contract: Some(contract_to_proto(&self.contract, None)),
        };
        Ok(Some(msg.encode_to_vec()))
    }
}

/// Historical data request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoricalDataRequest {
    /// Request id.
    pub req_id: i32,
    /// Contract.
    pub contract: Contract,
    /// End date/time.
    pub end_date_time: String,
    /// Duration string.
    pub duration_str: String,
    /// Bar size setting.
    pub bar_size_setting: String,
    /// What to show.
    pub what_to_show: String,
    /// Use RTH.
    pub use_rth: i32,
    /// Format date.
    pub format_date: i32,
    /// Keep up to date.
    pub keep_up_to_date: bool,
    /// Chart options.
    pub chart_options: Vec<TagValue>,
}

impl EncodableRequest for HistoricalDataRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqHistoricalData
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        self.encode_fields_for_server_version(fields, MAX_CLIENT_VER)
    }

    fn encode_fields_for_server_version(
        &self,
        fields: &mut FieldSink,
        server_version: i32,
    ) -> TwsApiResult<()> {
        if server_version < MIN_SERVER_VER_SYNT_REALTIME_BARS {
            fields.push(6)?;
        }
        fields.push(self.req_id)?;
        if server_version >= MIN_SERVER_VER_TRADING_CLASS {
            fields.push(self.contract.con_id)?;
        }
        encode_contract_historical_data(fields, &self.contract, server_version)?;
        fields
            .push(&self.end_date_time)?
            .push(&self.bar_size_setting)?
            .push(&self.duration_str)?
            .push(self.use_rth)?
            .push(&self.what_to_show)?
            .push(self.format_date)?;

        if self.contract.sec_type == "BAG" {
            fields.push(self.contract.combo_legs.len())?;
            for leg in &self.contract.combo_legs {
                fields
                    .push(leg.con_id)?
                    .push(leg.ratio)?
                    .push(&leg.action)?
                    .push(&leg.exchange)?;
            }
        }

        if server_version >= MIN_SERVER_VER_SYNT_REALTIME_BARS {
            fields.push(self.keep_up_to_date)?;
        }
        if server_version >= MIN_SERVER_VER_LINKING {
            fields.push(tag_values_to_tws_options(&self.chart_options))?;
        }
        Ok(())
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let msg = protobuf::HistoricalDataRequest {
            req_id: Some(self.req_id),
            contract: Some(contract_to_proto(&self.contract, None)),
            end_date_time: non_empty(self.end_date_time.clone()),
            bar_size_setting: non_empty(self.bar_size_setting.clone()),
            duration: non_empty(self.duration_str.clone()),
            use_rth: (self.use_rth != 0).then_some(true),
            what_to_show: non_empty(self.what_to_show.clone()),
            format_date: Some(self.format_date),
            keep_up_to_date: self.keep_up_to_date.then_some(true),
            chart_options: tag_values_to_map(&self.chart_options),
        };
        Ok(Some(msg.encode_to_vec()))
    }
}

/// Scanner subscription request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScannerSubscriptionRequest {
    /// Request id.
    pub req_id: i32,
    /// Subscription.
    pub subscription: ScannerSubscription,
    /// Scanner subscription options.
    pub scanner_subscription_options: Vec<TagValue>,
    /// Scanner subscription filter options.
    pub scanner_subscription_filter_options: Vec<TagValue>,
}

impl EncodableRequest for ScannerSubscriptionRequest {
    fn message(&self) -> Outgoing {
        Outgoing::ReqScannerSubscription
    }

    fn encode_fields(&self, fields: &mut FieldSink) -> TwsApiResult<()> {
        let sub = &self.subscription;
        fields
            .push(4)?
            .push(self.req_id)?
            .push(sub.number_of_rows)?
            .push(&sub.instrument)?
            .push(&sub.location_code)?
            .push(&sub.scan_code)?
            .push_empty(sub.above_price)?
            .push_empty(sub.below_price)?
            .push_empty(sub.above_volume)?
            .push_empty(sub.market_cap_above)?
            .push_empty(sub.market_cap_below)?
            .push(&sub.moody_rating_above)?
            .push(&sub.moody_rating_below)?
            .push(&sub.sp_rating_above)?
            .push(&sub.sp_rating_below)?
            .push(&sub.maturity_date_above)?
            .push(&sub.maturity_date_below)?
            .push_empty(sub.coupon_rate_above)?
            .push_empty(sub.coupon_rate_below)?
            .push(sub.exclude_convertible)?
            .push_empty(sub.average_option_volume_above)?
            .push(&sub.scanner_setting_pairs)?
            .push(&sub.stock_type_filter)?;
        encode_tag_values(fields, &self.scanner_subscription_options)?;
        encode_tag_values(fields, &self.scanner_subscription_filter_options)
    }

    fn encode_protobuf(&self) -> TwsApiResult<Option<Vec<u8>>> {
        let mut subscription = scanner_subscription_to_proto(&self.subscription);
        subscription.scanner_subscription_options =
            tag_values_to_map(&self.scanner_subscription_options);
        subscription.scanner_subscription_filter_options =
            tag_values_to_map(&self.scanner_subscription_filter_options);

        let msg = protobuf::ScannerSubscriptionRequest {
            req_id: Some(self.req_id),
            scanner_subscription: Some(subscription),
        };
        Ok(Some(msg.encode_to_vec()))
    }
}

fn encode_contract_core(fields: &mut FieldSink, contract: &Contract) -> TwsApiResult<()> {
    fields
        .push(contract.con_id)?
        .push(&contract.symbol)?
        .push(&contract.sec_type)?
        .push(&contract.last_trade_date_or_contract_month)?
        .push_empty(contract.strike)?
        .push(&contract.right)?
        .push(&contract.multiplier)?
        .push(&contract.exchange)?
        .push(&contract.primary_exchange)?
        .push(&contract.currency)?
        .push(&contract.local_symbol)?
        .push(&contract.trading_class)?
        .push(contract.include_expired)?
        .push(&contract.sec_id_type)?
        .push(&contract.sec_id)?;
    Ok(())
}

fn encode_contract_market_data(
    fields: &mut FieldSink,
    contract: &Contract,
    server_version: i32,
) -> TwsApiResult<()> {
    fields
        .push(&contract.symbol)?
        .push(&contract.sec_type)?
        .push(&contract.last_trade_date_or_contract_month)?
        .push_empty(contract.strike)?
        .push(&contract.right)?
        .push(&contract.multiplier)?
        .push(&contract.exchange)?
        .push(&contract.primary_exchange)?
        .push(&contract.currency)?
        .push(&contract.local_symbol)?;
    if server_version >= MIN_SERVER_VER_TRADING_CLASS {
        fields.push(&contract.trading_class)?;
    }
    Ok(())
}

fn encode_contract_historical_data(
    fields: &mut FieldSink,
    contract: &Contract,
    server_version: i32,
) -> TwsApiResult<()> {
    fields
        .push(&contract.symbol)?
        .push(&contract.sec_type)?
        .push(&contract.last_trade_date_or_contract_month)?
        .push_empty(contract.strike)?
        .push(&contract.right)?
        .push(&contract.multiplier)?
        .push(&contract.exchange)?
        .push(&contract.primary_exchange)?
        .push(&contract.currency)?
        .push(&contract.local_symbol)?;
    if server_version >= MIN_SERVER_VER_TRADING_CLASS {
        fields.push(&contract.trading_class)?;
    }
    fields.push(contract.include_expired)?;
    Ok(())
}

fn encode_contract_for_calculation(
    fields: &mut FieldSink,
    contract: &Contract,
    server_version: i32,
) -> TwsApiResult<()> {
    fields
        .push(contract.con_id)?
        .push(&contract.symbol)?
        .push(&contract.sec_type)?
        .push(&contract.last_trade_date_or_contract_month)?
        .push_empty(contract.strike)?
        .push(&contract.right)?
        .push(&contract.multiplier)?
        .push(&contract.exchange)?
        .push(&contract.primary_exchange)?
        .push(&contract.currency)?
        .push(&contract.local_symbol)?;
    if server_version >= MIN_SERVER_VER_TRADING_CLASS {
        fields.push(&contract.trading_class)?;
    }
    Ok(())
}

fn encode_place_order_fields(
    fields: &mut FieldSink,
    server_version: i32,
    order_id: i32,
    contract: &Contract,
    order: &Order,
    extra_fields: &str,
) -> TwsApiResult<()> {
    if server_version < MIN_SERVER_VER_ORDER_CONTAINER {
        fields.push(45)?;
    }

    fields.push(order_id)?;
    if server_version >= MIN_SERVER_VER_PLACE_ORDER_CONID {
        fields.push(contract.con_id)?;
    }
    fields
        .push(&contract.symbol)?
        .push(&contract.sec_type)?
        .push(&contract.last_trade_date_or_contract_month)?
        .push_empty(contract.strike)?
        .push(&contract.right)?
        .push(&contract.multiplier)?
        .push(&contract.exchange)?
        .push(&contract.primary_exchange)?
        .push(&contract.currency)?
        .push(&contract.local_symbol)?;
    if server_version >= MIN_SERVER_VER_TRADING_CLASS {
        fields.push(&contract.trading_class)?;
    }
    if server_version >= MIN_SERVER_VER_SEC_ID_TYPE {
        fields.push(&contract.sec_id_type)?.push(&contract.sec_id)?;
    }

    fields.push(&order.action)?;
    fields.push(order.total_quantity.to_string())?;
    fields
        .push(&order.order_type)?
        .push_empty(order.limit_price)?
        .push_empty(order.aux_price)?
        .push(&order.tif)?
        .push(&order.oca_group)?
        .push(&order.account)?
        .push(&order.open_close)?
        .push(order.origin as i32)?
        .push(&order.order_ref)?
        .push(order.transmit)?
        .push(order.parent_id)?
        .push(order.block_order)?
        .push(order.sweep_to_fill)?
        .push(order.display_size)?
        .push(order.trigger_method)?
        .push(order.outside_rth)?
        .push(order.hidden)?;

    if contract.sec_type == "BAG" {
        fields.push(contract.combo_legs.len())?;
        for leg in &contract.combo_legs {
            fields
                .push(leg.con_id)?
                .push(leg.ratio)?
                .push(&leg.action)?
                .push(&leg.exchange)?
                .push(leg.open_close as i32)?
                .push(leg.short_sale_slot)?
                .push(&leg.designated_location)?;
            if server_version >= MIN_SERVER_VER_SSHORTX_OLD {
                fields.push(leg.exempt_code)?;
            }
        }
    }

    if server_version >= MIN_SERVER_VER_ORDER_COMBO_LEGS_PRICE && contract.sec_type == "BAG" {
        fields.push(order.order_combo_legs.len())?;
        for leg in &order.order_combo_legs {
            fields.push_empty(leg.price)?;
        }
    }

    if server_version >= MIN_SERVER_VER_SMART_COMBO_ROUTING_PARAMS && contract.sec_type == "BAG" {
        encode_tag_values(fields, &order.smart_combo_routing_params)?;
    }

    fields
        .push("")?
        .push(order.discretionary_amount)?
        .push(&order.good_after_time)?
        .push(&order.good_till_date)?
        .push(&order.fa_group)?
        .push(&order.fa_method)?
        .push(&order.fa_percentage)?;
    if server_version < 177 {
        fields.push("")?;
    }
    if server_version >= MIN_SERVER_VER_MODELS_SUPPORT {
        fields.push(&order.model_code)?;
    }

    fields
        .push(order.short_sale_slot)?
        .push(&order.designated_location)?;
    if server_version >= MIN_SERVER_VER_SSHORTX_OLD {
        fields.push(order.exempt_code)?;
    }

    fields
        .push(order.oca_type)?
        .push(&order.rule80a)?
        .push(&order.settling_firm)?
        .push(order.all_or_none)?
        .push_empty(order.min_qty)?
        .push_empty(order.percent_offset)?
        .push(false)?
        .push(false)?
        .push_empty(UNSET_DOUBLE)?
        .push(order.auction_strategy as i32)?
        .push_empty(order.starting_price)?
        .push_empty(order.stock_ref_price)?
        .push_empty(order.delta)?
        .push_empty(order.stock_range_lower)?
        .push_empty(order.stock_range_upper)?
        .push(order.override_percentage_constraints)?
        .push_empty(order.volatility)?
        .push_empty(order.volatility_type)?
        .push(&order.delta_neutral_order_type)?
        .push_empty(order.delta_neutral_aux_price)?;

    if server_version >= MIN_SERVER_VER_DELTA_NEUTRAL_CONID
        && !order.delta_neutral_order_type.is_empty()
    {
        fields
            .push(order.delta_neutral_con_id)?
            .push(&order.delta_neutral_settling_firm)?
            .push(&order.delta_neutral_clearing_account)?
            .push(&order.delta_neutral_clearing_intent)?;
    }
    if server_version >= MIN_SERVER_VER_DELTA_NEUTRAL_OPEN_CLOSE
        && !order.delta_neutral_order_type.is_empty()
    {
        fields
            .push(&order.delta_neutral_open_close)?
            .push(order.delta_neutral_short_sale)?
            .push(order.delta_neutral_short_sale_slot)?
            .push(&order.delta_neutral_designated_location)?;
    }

    fields
        .push(order.continuous_update)?
        .push_empty(order.reference_price_type)?
        .push_empty(order.trail_stop_price)?;
    if server_version >= MIN_SERVER_VER_TRAILING_PERCENT {
        fields.push_empty(order.trailing_percent)?;
    }

    if server_version >= MIN_SERVER_VER_SCALE_ORDERS2 {
        fields
            .push_empty(order.scale_init_level_size)?
            .push_empty(order.scale_subs_level_size)?;
    } else {
        fields.push("")?.push_empty(order.scale_init_level_size)?;
    }
    fields.push_empty(order.scale_price_increment)?;
    if server_version >= MIN_SERVER_VER_SCALE_ORDERS3
        && order.scale_price_increment != UNSET_DOUBLE
        && order.scale_price_increment > 0.0
    {
        fields
            .push_empty(order.scale_price_adjust_value)?
            .push_empty(order.scale_price_adjust_interval)?
            .push_empty(order.scale_profit_offset)?
            .push(order.scale_auto_reset)?
            .push_empty(order.scale_init_position)?
            .push_empty(order.scale_init_fill_qty)?
            .push(order.scale_random_percent)?;
    }
    if server_version >= MIN_SERVER_VER_SCALE_TABLE {
        fields
            .push(&order.scale_table)?
            .push(&order.active_start_time)?
            .push(&order.active_stop_time)?;
    }

    if server_version >= MIN_SERVER_VER_HEDGE_ORDERS {
        fields.push(&order.hedge_type)?;
        if !order.hedge_type.is_empty() {
            fields.push(&order.hedge_param)?;
        }
    }
    if server_version >= MIN_SERVER_VER_OPT_OUT_SMART_ROUTING {
        fields.push(order.opt_out_smart_routing)?;
    }
    if server_version >= MIN_SERVER_VER_PTA_ORDERS {
        fields
            .push(&order.clearing_account)?
            .push(&order.clearing_intent)?;
    }
    if server_version >= MIN_SERVER_VER_NOT_HELD {
        fields.push(order.not_held)?;
    }
    if server_version >= MIN_SERVER_VER_DELTA_NEUTRAL {
        if let Some(delta) = &contract.delta_neutral_contract {
            fields
                .push(true)?
                .push(delta.con_id)?
                .push(delta.delta)?
                .push(delta.price)?;
        } else {
            fields.push(false)?;
        }
    }
    if server_version >= MIN_SERVER_VER_ALGO_ORDERS {
        fields.push(&order.algo_strategy)?;
        if !order.algo_strategy.is_empty() {
            encode_tag_values(fields, &order.algo_params)?;
        }
    }
    if server_version >= MIN_SERVER_VER_ALGO_ID {
        fields.push(&order.algo_id)?;
    }
    fields.push(order.what_if)?;
    if server_version >= MIN_SERVER_VER_LINKING {
        fields.push(tag_values_to_tws_options(&order.order_misc_options))?;
    }
    if server_version >= MIN_SERVER_VER_ORDER_SOLICITED {
        fields.push(order.solicited)?;
    }
    if server_version >= MIN_SERVER_VER_RANDOMIZE_SIZE_AND_PRICE {
        fields
            .push(order.randomize_size)?
            .push(order.randomize_price)?;
    }
    if server_version >= MIN_SERVER_VER_PEGGED_TO_BENCHMARK {
        fields.push(order.conditions.len())?;
        if !order.conditions.is_empty() {
            for condition in &order.conditions {
                encode_order_condition(fields, condition)?;
            }
            fields
                .push(order.conditions_ignore_rth)?
                .push(order.conditions_cancel_order)?;
        }
        fields
            .push(&order.adjusted_order_type)?
            .push(order.trigger_price)?
            .push(order.limit_price_offset)?
            .push(order.adjusted_stop_price)?
            .push(order.adjusted_stop_limit_price)?
            .push(order.adjusted_trailing_amount)?
            .push(order.adjustable_trailing_unit)?;
    }
    if server_version >= MIN_SERVER_VER_EXT_OPERATOR {
        fields.push(&order.ext_operator)?;
    }
    if server_version >= MIN_SERVER_VER_SOFT_DOLLAR_TIER {
        fields
            .push(&order.soft_dollar_tier.name)?
            .push(&order.soft_dollar_tier.value)?;
    }
    if server_version >= MIN_SERVER_VER_CASH_QTY {
        fields.push(order.cash_qty)?;
    }
    if server_version >= MIN_SERVER_VER_DECISION_MAKER {
        fields
            .push(&order.mifid2_decision_maker)?
            .push(&order.mifid2_decision_algo)?;
    }
    if server_version >= MIN_SERVER_VER_MIFID_EXECUTION {
        fields
            .push(&order.mifid2_execution_trader)?
            .push(&order.mifid2_execution_algo)?;
    }
    if server_version >= MIN_SERVER_VER_AUTO_PRICE_FOR_HEDGE {
        fields.push(order.dont_use_auto_price_for_hedge)?;
    }
    if server_version >= MIN_SERVER_VER_ORDER_CONTAINER {
        fields.push(order.is_oms_container)?;
    }
    if server_version >= MIN_SERVER_VER_D_PEG_ORDERS {
        fields.push(order.discretionary_up_to_limit_price)?;
    }
    if server_version >= MIN_SERVER_VER_PRICE_MGMT_ALGO {
        if order.use_price_mgmt_algo == UNSET_INTEGER {
            fields.push_empty(UNSET_INTEGER)?;
        } else {
            fields.push(order.use_price_mgmt_algo != 0)?;
        }
    }
    if server_version >= MIN_SERVER_VER_DURATION {
        fields.push(order.duration)?;
    }
    if server_version >= MIN_SERVER_VER_POST_TO_ATS {
        fields.push(order.post_to_ats)?;
    }
    if server_version >= MIN_SERVER_VER_AUTO_CANCEL_PARENT {
        fields.push(order.auto_cancel_parent)?;
    }
    if server_version >= MIN_SERVER_VER_ADVANCED_ORDER_REJECT {
        fields.push(&order.advanced_error_override)?;
    }
    if server_version >= MIN_SERVER_VER_MANUAL_ORDER_TIME {
        fields.push(&order.manual_order_time)?;
    }
    if server_version >= MIN_SERVER_VER_CUSTOMER_ACCOUNT {
        fields.push(&order.customer_account)?;
    }
    if server_version >= MIN_SERVER_VER_PROFESSIONAL_CUSTOMER {
        fields.push(order.professional_customer)?;
    }
    if server_version >= MIN_SERVER_VER_INCLUDE_OVERNIGHT {
        fields.push(order.include_overnight)?;
    }
    if server_version >= MIN_SERVER_VER_CME_TAGGING_FIELDS {
        fields.push(order.manual_order_indicator)?;
    }
    if server_version >= MIN_SERVER_VER_IMBALANCE_ONLY {
        fields.push(order.imbalance_only)?;
    }

    fields.push_raw(extra_fields);
    Ok(())
}

fn encode_order_condition(
    fields: &mut FieldSink,
    condition: &crate::types::OrderCondition,
) -> TwsApiResult<()> {
    use crate::types::OrderCondition;

    match condition {
        OrderCondition::Price {
            is_conjunction_connection,
            trigger_method,
            con_id,
            exchange,
            is_more,
            price,
        } => {
            fields
                .push(1)?
                .push(if *is_conjunction_connection { "a" } else { "o" })?
                .push(*is_more)?
                .push(*price)?
                .push(*con_id)?
                .push(&exchange[..])?
                .push(*trigger_method)?;
        }
        OrderCondition::Time {
            is_conjunction_connection,
            is_more,
            time,
        } => {
            fields
                .push(3)?
                .push(if *is_conjunction_connection { "a" } else { "o" })?
                .push(*is_more)?
                .push(&time[..])?;
        }
        OrderCondition::Margin {
            is_conjunction_connection,
            is_more,
            percent,
        } => {
            fields
                .push(4)?
                .push(if *is_conjunction_connection { "a" } else { "o" })?
                .push(*is_more)?
                .push(*percent)?;
        }
        OrderCondition::Execution {
            is_conjunction_connection,
            sec_type,
            exchange,
            symbol,
        } => {
            fields
                .push(5)?
                .push(if *is_conjunction_connection { "a" } else { "o" })?
                .push(&sec_type[..])?
                .push(&exchange[..])?
                .push(&symbol[..])?;
        }
        OrderCondition::Volume {
            is_conjunction_connection,
            con_id,
            exchange,
            is_more,
            volume,
        } => {
            fields
                .push(6)?
                .push(if *is_conjunction_connection { "a" } else { "o" })?
                .push(*is_more)?
                .push(*volume)?
                .push(*con_id)?
                .push(&exchange[..])?;
        }
        OrderCondition::PercentChange {
            is_conjunction_connection,
            con_id,
            exchange,
            is_more,
            change_percent,
        } => {
            fields
                .push(7)?
                .push(if *is_conjunction_connection { "a" } else { "o" })?
                .push(*is_more)?
                .push(*change_percent)?
                .push(*con_id)?
                .push(&exchange[..])?;
        }
    }
    Ok(())
}

fn encode_tag_values(fields: &mut FieldSink, values: &[TagValue]) -> TwsApiResult<()> {
    fields.push(values.len())?;
    for value in values {
        fields.push(&value.tag)?.push(&value.value)?;
    }
    Ok(())
}

fn tag_values_to_tws_options(values: &[TagValue]) -> String {
    values
        .iter()
        .map(|value| format!("{}={};", value.tag, value.value))
        .collect::<String>()
}

fn tag_values_to_map(values: &[TagValue]) -> std::collections::HashMap<String, String> {
    values
        .iter()
        .map(|value| (value.tag.clone(), value.value.clone()))
        .collect()
}

fn contract_to_proto(contract: &Contract, order: Option<&Order>) -> protobuf::Contract {
    protobuf::Contract {
        con_id: valid_i32(contract.con_id),
        symbol: non_empty(contract.symbol.clone()),
        sec_type: non_empty(contract.sec_type.clone()),
        last_trade_date_or_contract_month: non_empty(
            contract.last_trade_date_or_contract_month.clone(),
        ),
        strike: valid_f64(contract.strike),
        right: non_empty(contract.right.clone()),
        multiplier: contract.multiplier.parse::<f64>().ok(),
        exchange: non_empty(contract.exchange.clone()),
        primary_exch: non_empty(contract.primary_exchange.clone()),
        currency: non_empty(contract.currency.clone()),
        local_symbol: non_empty(contract.local_symbol.clone()),
        trading_class: non_empty(contract.trading_class.clone()),
        sec_id_type: non_empty(contract.sec_id_type.clone()),
        sec_id: non_empty(contract.sec_id.clone()),
        description: non_empty(contract.description.clone()),
        issuer_id: non_empty(contract.issuer_id.clone()),
        delta_neutral_contract: contract.delta_neutral_contract.as_ref().map(|delta| {
            protobuf::DeltaNeutralContract {
                con_id: valid_i32(delta.con_id),
                delta: valid_f64(delta.delta),
                price: valid_f64(delta.price),
            }
        }),
        include_expired: contract.include_expired.then_some(true),
        combo_legs_descrip: non_empty(contract.combo_legs_description.clone()),
        combo_legs: contract
            .combo_legs
            .iter()
            .enumerate()
            .map(|(idx, leg)| {
                let per_leg_price = order
                    .and_then(|order| order.order_combo_legs.get(idx))
                    .and_then(|leg| valid_f64(leg.price));
                protobuf::ComboLeg {
                    con_id: valid_i32(leg.con_id),
                    ratio: valid_i32(leg.ratio),
                    action: non_empty(leg.action.clone()),
                    exchange: non_empty(leg.exchange.clone()),
                    open_close: valid_i32(leg.open_close as i32),
                    short_sales_slot: valid_i32(leg.short_sale_slot),
                    designated_location: non_empty(leg.designated_location.clone()),
                    exempt_code: valid_i32(leg.exempt_code),
                    per_leg_price,
                }
            })
            .collect(),
        last_trade_date: non_empty(contract.last_trade_date.clone()),
    }
}

fn order_to_proto(order: &Order) -> protobuf::Order {
    let mut msg = protobuf::Order {
        client_id: valid_i32(order.client_id),
        order_id: valid_i32(order.order_id),
        perm_id: Some(order.perm_id),
        parent_id: valid_i32(order.parent_id),
        action: non_empty(order.action.clone()),
        total_quantity: Some(order.total_quantity.to_string()),
        display_size: valid_i32(order.display_size),
        order_type: non_empty(order.order_type.clone()),
        lmt_price: valid_f64(order.limit_price),
        aux_price: valid_f64(order.aux_price),
        tif: non_empty(order.tif.clone()),
        account: non_empty(order.account.clone()),
        settling_firm: non_empty(order.settling_firm.clone()),
        clearing_account: non_empty(order.clearing_account.clone()),
        clearing_intent: non_empty(order.clearing_intent.clone()),
        all_or_none: order.all_or_none.then_some(true),
        block_order: order.block_order.then_some(true),
        hidden: order.hidden.then_some(true),
        outside_rth: order.outside_rth.then_some(true),
        sweep_to_fill: order.sweep_to_fill.then_some(true),
        percent_offset: valid_f64(order.percent_offset),
        trailing_percent: valid_f64(order.trailing_percent),
        trail_stop_price: valid_f64(order.trail_stop_price),
        min_qty: valid_i32(order.min_qty),
        good_after_time: non_empty(order.good_after_time.clone()),
        good_till_date: non_empty(order.good_till_date.clone()),
        oca_group: non_empty(order.oca_group.clone()),
        order_ref: non_empty(order.order_ref.clone()),
        rule80_a: non_empty(order.rule80a.clone()),
        oca_type: valid_i32(order.oca_type),
        trigger_method: valid_i32(order.trigger_method),
        active_start_time: non_empty(order.active_start_time.clone()),
        active_stop_time: non_empty(order.active_stop_time.clone()),
        fa_group: non_empty(order.fa_group.clone()),
        fa_method: non_empty(order.fa_method.clone()),
        fa_percentage: non_empty(order.fa_percentage.clone()),
        volatility: valid_f64(order.volatility),
        volatility_type: valid_i32(order.volatility_type),
        continuous_update: order.continuous_update.then_some(true),
        reference_price_type: valid_i32(order.reference_price_type),
        delta_neutral_order_type: non_empty(order.delta_neutral_order_type.clone()),
        delta_neutral_aux_price: valid_f64(order.delta_neutral_aux_price),
        delta_neutral_con_id: valid_i32(order.delta_neutral_con_id),
        delta_neutral_open_close: non_empty(order.delta_neutral_open_close.clone()),
        delta_neutral_short_sale: order.delta_neutral_short_sale.then_some(true),
        delta_neutral_short_sale_slot: valid_i32(order.delta_neutral_short_sale_slot),
        delta_neutral_designated_location: non_empty(
            order.delta_neutral_designated_location.clone(),
        ),
        scale_init_level_size: valid_i32(order.scale_init_level_size),
        scale_subs_level_size: valid_i32(order.scale_subs_level_size),
        scale_price_increment: valid_f64(order.scale_price_increment),
        scale_price_adjust_value: valid_f64(order.scale_price_adjust_value),
        scale_price_adjust_interval: valid_i32(order.scale_price_adjust_interval),
        scale_profit_offset: valid_f64(order.scale_profit_offset),
        scale_auto_reset: order.scale_auto_reset.then_some(true),
        scale_init_position: valid_i32(order.scale_init_position),
        scale_init_fill_qty: valid_i32(order.scale_init_fill_qty),
        scale_random_percent: order.scale_random_percent.then_some(true),
        scale_table: non_empty(order.scale_table.clone()),
        hedge_type: non_empty(order.hedge_type.clone()),
        hedge_param: non_empty(order.hedge_param.clone()),
        algo_strategy: non_empty(order.algo_strategy.clone()),
        algo_id: non_empty(order.algo_id.clone()),
        what_if: order.what_if.then_some(true),
        transmit: order.transmit.then_some(true),
        override_percentage_constraints: order.override_percentage_constraints.then_some(true),
        open_close: non_empty(order.open_close.clone()),
        origin: valid_i32(order.origin as i32),
        short_sale_slot: valid_i32(order.short_sale_slot),
        designated_location: non_empty(order.designated_location.clone()),
        exempt_code: valid_i32(order.exempt_code),
        delta_neutral_settling_firm: non_empty(order.delta_neutral_settling_firm.clone()),
        delta_neutral_clearing_account: non_empty(order.delta_neutral_clearing_account.clone()),
        delta_neutral_clearing_intent: non_empty(order.delta_neutral_clearing_intent.clone()),
        discretionary_amt: valid_f64(order.discretionary_amount),
        opt_out_smart_routing: order.opt_out_smart_routing.then_some(true),
        starting_price: valid_f64(order.starting_price),
        stock_ref_price: valid_f64(order.stock_ref_price),
        delta: valid_f64(order.delta),
        stock_range_lower: valid_f64(order.stock_range_lower),
        stock_range_upper: valid_f64(order.stock_range_upper),
        not_held: order.not_held.then_some(true),
        solicited: order.solicited.then_some(true),
        randomize_size: order.randomize_size.then_some(true),
        randomize_price: order.randomize_price.then_some(true),
        reference_contract_id: valid_i32(order.reference_contract_id),
        pegged_change_amount: valid_f64(order.pegged_change_amount),
        is_pegged_change_amount_decrease: order.is_pegged_change_amount_decrease.then_some(true),
        reference_change_amount: valid_f64(order.reference_change_amount),
        reference_exchange_id: non_empty(order.reference_exchange_id.clone()),
        adjusted_order_type: non_empty(order.adjusted_order_type.clone()),
        trigger_price: valid_f64(order.trigger_price),
        adjusted_stop_price: valid_f64(order.adjusted_stop_price),
        adjusted_stop_limit_price: valid_f64(order.adjusted_stop_limit_price),
        adjusted_trailing_amount: valid_f64(order.adjusted_trailing_amount),
        adjustable_trailing_unit: valid_i32(order.adjustable_trailing_unit),
        lmt_price_offset: valid_f64(order.limit_price_offset),
        model_code: non_empty(order.model_code.clone()),
        ext_operator: non_empty(order.ext_operator.clone()),
        soft_dollar_tier: Some(protobuf::SoftDollarTier {
            name: non_empty(order.soft_dollar_tier.name.clone()),
            value: non_empty(order.soft_dollar_tier.value.clone()),
            display_name: non_empty(order.soft_dollar_tier.display_name.clone()),
        }),
        cash_qty: valid_f64(order.cash_qty),
        mifid2_decision_maker: non_empty(order.mifid2_decision_maker.clone()),
        mifid2_decision_algo: non_empty(order.mifid2_decision_algo.clone()),
        mifid2_execution_trader: non_empty(order.mifid2_execution_trader.clone()),
        mifid2_execution_algo: non_empty(order.mifid2_execution_algo.clone()),
        dont_use_auto_price_for_hedge: order.dont_use_auto_price_for_hedge.then_some(true),
        is_oms_container: order.is_oms_container.then_some(true),
        discretionary_up_to_limit_price: order.discretionary_up_to_limit_price.then_some(true),
        auto_cancel_date: non_empty(order.auto_cancel_date.clone()),
        filled_quantity: non_empty(order.filled_quantity.to_string()),
        ref_futures_con_id: valid_i32(order.ref_futures_con_id),
        auto_cancel_parent: order.auto_cancel_parent.then_some(true),
        shareholder: non_empty(order.shareholder.clone()),
        imbalance_only: order.imbalance_only.then_some(true),
        route_marketable_to_bbo: valid_i32(order.route_marketable_to_bbo),
        parent_perm_id: Some(order.parent_perm_id),
        use_price_mgmt_algo: valid_i32(order.use_price_mgmt_algo),
        duration: valid_i32(order.duration),
        post_to_ats: valid_i32(order.post_to_ats),
        advanced_error_override: non_empty(order.advanced_error_override.clone()),
        manual_order_time: non_empty(order.manual_order_time.clone()),
        min_trade_qty: valid_i32(order.min_trade_qty),
        min_compete_size: valid_i32(order.min_compete_size),
        compete_against_best_offset: valid_f64(order.compete_against_best_offset),
        mid_offset_at_whole: valid_f64(order.mid_offset_at_whole),
        mid_offset_at_half: valid_f64(order.mid_offset_at_half),
        customer_account: non_empty(order.customer_account.clone()),
        professional_customer: order.professional_customer.then_some(true),
        bond_accrued_interest: non_empty(order.bond_accrued_interest.clone()),
        include_overnight: order.include_overnight.then_some(true),
        manual_order_indicator: valid_i32(order.manual_order_indicator),
        submitter: non_empty(order.submitter.clone()),
        deactivate: order.deactivate.then_some(true),
        post_only: order.post_only.then_some(true),
        allow_pre_open: order.allow_pre_open.then_some(true),
        ignore_open_auction: order.ignore_open_auction.then_some(true),
        seek_price_improvement: valid_i32(order.seek_price_improvement),
        what_if_type: valid_i32(order.what_if_type),
        hedge_max_size: valid_i32(order.hedge_max_size),
        ..protobuf::Order::default()
    };
    msg.algo_params = tag_values_to_map(&order.algo_params);
    msg.smart_combo_routing_params = tag_values_to_map(&order.smart_combo_routing_params);
    msg.order_misc_options = tag_values_to_map(&order.order_misc_options);
    msg
}

fn attached_orders_to_proto(order: &Order) -> Option<protobuf::AttachedOrders> {
    let attached = protobuf::AttachedOrders {
        sl_order_id: valid_i32(order.stop_loss_order_id),
        sl_order_type: non_empty(order.stop_loss_order_type.clone()),
        pt_order_id: valid_i32(order.profit_taker_order_id),
        pt_order_type: non_empty(order.profit_taker_order_type.clone()),
    };
    (attached.sl_order_id.is_some()
        || attached.sl_order_type.is_some()
        || attached.pt_order_id.is_some()
        || attached.pt_order_type.is_some())
    .then_some(attached)
}

pub(crate) fn validate_order_parameters(
    order: &Order,
    server_version: i32,
) -> Option<OrderFieldValidation> {
    if server_version < MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1 {
        if order.deactivate {
            return Some(OrderFieldValidation {
                parameter: "deactivate",
                min_version: MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1,
            });
        }
        if order.post_only {
            return Some(OrderFieldValidation {
                parameter: "postOnly",
                min_version: MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1,
            });
        }
        if order.allow_pre_open {
            return Some(OrderFieldValidation {
                parameter: "allowPreOpen",
                min_version: MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1,
            });
        }
        if order.ignore_open_auction {
            return Some(OrderFieldValidation {
                parameter: "ignoreOpenAuction",
                min_version: MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1,
            });
        }
    }

    if server_version < MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_2 {
        if order.route_marketable_to_bbo != UNSET_INTEGER {
            return Some(OrderFieldValidation {
                parameter: "routeMarketableToBbo",
                min_version: MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_2,
            });
        }
        if order.seek_price_improvement != UNSET_INTEGER {
            return Some(OrderFieldValidation {
                parameter: "seekPriceImprovement",
                min_version: MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_2,
            });
        }
        if order.what_if_type != UNSET_INTEGER {
            return Some(OrderFieldValidation {
                parameter: "whatIfType",
                min_version: MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_2,
            });
        }
    }

    if server_version < MIN_SERVER_VER_HEDGE_MAX_SIZE && order.hedge_max_size != UNSET_INTEGER {
        return Some(OrderFieldValidation {
            parameter: "hedgeMaxSize",
            min_version: MIN_SERVER_VER_HEDGE_MAX_SIZE,
        });
    }

    None
}

pub(crate) fn validate_attached_orders_parameters(
    order: &Order,
    server_version: i32,
) -> Option<OrderFieldValidation> {
    if server_version < MIN_SERVER_VER_ATTACHED_ORDERS {
        if order.stop_loss_order_id != UNSET_INTEGER {
            return Some(OrderFieldValidation {
                parameter: "slOrderId",
                min_version: MIN_SERVER_VER_ATTACHED_ORDERS,
            });
        }
        if !order.stop_loss_order_type.is_empty() {
            return Some(OrderFieldValidation {
                parameter: "slOrderType",
                min_version: MIN_SERVER_VER_ATTACHED_ORDERS,
            });
        }
        if order.profit_taker_order_id != UNSET_INTEGER {
            return Some(OrderFieldValidation {
                parameter: "ptOrderId",
                min_version: MIN_SERVER_VER_ATTACHED_ORDERS,
            });
        }
        if !order.profit_taker_order_type.is_empty() {
            return Some(OrderFieldValidation {
                parameter: "ptOrderType",
                min_version: MIN_SERVER_VER_ATTACHED_ORDERS,
            });
        }
    }

    None
}

#[cfg(test)]
#[allow(clippy::items_after_test_module, clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::server_versions::{
        MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1, MIN_SERVER_VER_ATTACHED_ORDERS,
        MIN_SERVER_VER_HEDGE_MAX_SIZE,
    };

    #[test]
    fn validate_order_parameters_rejects_new_fields_on_old_server_versions() {
        let mut order = Order::default();
        order.deactivate = true;
        let validation =
            validate_order_parameters(&order, MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1 - 1)
                .expect("expected deactivate to be rejected");
        assert_eq!(validation.parameter, "deactivate");
        assert_eq!(
            validation.min_version,
            MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1
        );

        let mut order = Order::default();
        order.hedge_max_size = 1;
        let validation = validate_order_parameters(&order, MIN_SERVER_VER_HEDGE_MAX_SIZE - 1)
            .expect("expected hedgeMaxSize to be rejected");
        assert_eq!(validation.parameter, "hedgeMaxSize");
        assert_eq!(validation.min_version, MIN_SERVER_VER_HEDGE_MAX_SIZE);
    }

    #[test]
    fn validate_attached_orders_parameters_rejects_low_versions() {
        let mut order = Order::default();
        order.stop_loss_order_id = 1;
        let validation =
            validate_attached_orders_parameters(&order, MIN_SERVER_VER_ATTACHED_ORDERS - 1)
                .expect("expected attached orders to be rejected");
        assert_eq!(validation.parameter, "slOrderId");
        assert_eq!(validation.min_version, MIN_SERVER_VER_ATTACHED_ORDERS);
    }

    #[test]
    fn validate_order_parameters_allows_supported_defaults() {
        let order = Order::default();
        assert!(
            validate_order_parameters(&order, MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1 - 1)
                .is_none()
        );
        assert!(
            validate_attached_orders_parameters(&order, MIN_SERVER_VER_ATTACHED_ORDERS - 1)
                .is_none()
        );
    }
}

fn order_cancel_to_proto(order_cancel: &OrderCancel) -> protobuf::OrderCancel {
    protobuf::OrderCancel {
        manual_order_cancel_time: non_empty(order_cancel.manual_order_cancel_time.clone()),
        ext_operator: non_empty(order_cancel.ext_operator.clone()),
        manual_order_indicator: valid_i32(order_cancel.manual_order_indicator),
    }
}

fn execution_filter_to_proto(filter: &ExecutionFilter) -> protobuf::ExecutionFilter {
    protobuf::ExecutionFilter {
        client_id: valid_i32(filter.client_id),
        acct_code: non_empty(filter.acct_code.clone()),
        time: non_empty(filter.time.clone()),
        symbol: non_empty(filter.symbol.clone()),
        sec_type: non_empty(filter.sec_type.clone()),
        exchange: non_empty(filter.exchange.clone()),
        side: non_empty(filter.side.clone()),
        last_n_days: None,
        specific_dates: Vec::new(),
    }
}

fn scanner_subscription_to_proto(
    subscription: &ScannerSubscription,
) -> protobuf::ScannerSubscription {
    protobuf::ScannerSubscription {
        number_of_rows: valid_i32(subscription.number_of_rows),
        instrument: non_empty(subscription.instrument.clone()),
        location_code: non_empty(subscription.location_code.clone()),
        scan_code: non_empty(subscription.scan_code.clone()),
        above_price: valid_f64(subscription.above_price),
        below_price: valid_f64(subscription.below_price),
        above_volume: (subscription.above_volume != UNSET_INTEGER)
            .then_some(i64::from(subscription.above_volume)),
        market_cap_above: valid_f64(subscription.market_cap_above),
        market_cap_below: valid_f64(subscription.market_cap_below),
        moody_rating_above: non_empty(subscription.moody_rating_above.clone()),
        moody_rating_below: non_empty(subscription.moody_rating_below.clone()),
        sp_rating_above: non_empty(subscription.sp_rating_above.clone()),
        sp_rating_below: non_empty(subscription.sp_rating_below.clone()),
        maturity_date_above: non_empty(subscription.maturity_date_above.clone()),
        maturity_date_below: non_empty(subscription.maturity_date_below.clone()),
        coupon_rate_above: valid_f64(subscription.coupon_rate_above),
        coupon_rate_below: valid_f64(subscription.coupon_rate_below),
        exclude_convertible: subscription.exclude_convertible.then_some(true),
        average_option_volume_above: (subscription.average_option_volume_above != UNSET_INTEGER)
            .then_some(i64::from(subscription.average_option_volume_above)),
        scanner_setting_pairs: non_empty(subscription.scanner_setting_pairs.clone()),
        stock_type_filter: non_empty(subscription.stock_type_filter.clone()),
        scanner_subscription_filter_options: Default::default(),
        scanner_subscription_options: Default::default(),
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn valid_i32(value: i32) -> Option<i32> {
    (value != UNSET_INTEGER).then_some(value)
}

fn valid_f64(value: f64) -> Option<f64> {
    (value != UNSET_DOUBLE && value.is_finite()).then_some(value)
}

fn ensure_server_version(server_version: i32, min_version: i32) -> TwsApiResult<()> {
    if server_version < min_version {
        return Err(TwsApiError::UnsupportedServerVersion {
            server_version,
            min_version,
        });
    }
    Ok(())
}
