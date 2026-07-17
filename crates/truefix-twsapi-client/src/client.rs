use std::collections::VecDeque;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::comm;
use crate::constants::MAX_MSG_LEN;
use crate::decoder;
use crate::error::{TwsApiError, TwsApiResult};
use crate::events::{Event, Wrapper};
use crate::message::Outgoing;
use crate::requests::{
    AccountDataRequest, AccountSummaryRequest, AccountUpdatesMultiRequest, AutoOpenOrdersRequest,
    CalculateImpliedVolatilityRequest, CalculateOptionPriceRequest, CancelMarketDepthRequest,
    CancelOrderRequest, CompletedOrdersRequest, ContractDetailsRequest, EmptyRequest,
    EncodableRequest, ExecutionRequest, ExerciseOptionsRequest, FieldSink, FinancialAdvisorRequest,
    GlobalCancelRequest, HeadTimestampRequest, HistogramDataRequest, HistoricalDataRequest,
    HistoricalNewsRequest, HistoricalTicksRequest, IdRequest, MarketDataRequest,
    MarketDataTypeRequest, MarketDepthRequest, MatchingSymbolsRequest, NewsArticleRequest,
    NewsBulletinsRequest, PlaceOrderRequest, PnlRequest, PnlSingleRequest, PositionsMultiRequest,
    RawRequest, RealTimeBarsRequest, ReplaceFinancialAdvisorRequest, ScannerSubscriptionRequest,
    SecDefOptParamsRequest, SetServerLogLevelRequest, SmartComponentsRequest, StartApiRequest,
    SubscribeToGroupEventsRequest, TickByTickRequest, UpdateDisplayGroupRequest,
    VerifyAndAuthMessageRequest, VerifyAndAuthRequest, VerifyMessageRequest, VerifyRequest,
    VersionedRequest, WshEventDataRequest, encode_request_frame_with_protobuf,
    protobuf_min_server_version, validate_attached_orders_parameters, validate_order_parameters,
};
use crate::server_versions::{
    MAX_CLIENT_VER, MIN_CLIENT_VER, MIN_SERVER_VER_OPTIONAL_CAPABILITIES, MIN_SERVER_VER_POSITIONS,
    MIN_SERVER_VER_PROTOBUF,
};
use crate::types::{OrderCancel, TagValue};

/// Client connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// No active socket.
    Disconnected,
    /// Socket opened and handshake in progress.
    Connecting,
    /// Handshake completed.
    Connected,
}

/// TWS/Gateway client configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// Host name or IP address. Empty values are normalized to `127.0.0.1`.
    pub host: String,
    /// TWS/Gateway socket port.
    pub port: u16,
    /// TWS API client id.
    pub client_id: i32,
    /// Optional connect options appended to the enhanced handshake version string.
    pub connect_options: Option<String>,
    /// Optional capabilities sent by `start_api` when supported by the server.
    pub optional_capabilities: Option<String>,
    /// Whether to use the IB protobuf extension after `startApi`. Some local
    /// Gateway deployments negotiate a modern version but reject `jextend`.
    pub prefer_protobuf: bool,
}

impl ClientConfig {
    /// Creates a new configuration.
    pub fn new(host: impl Into<String>, port: u16, client_id: i32) -> Self {
        let host = host.into();
        Self {
            host: if host.is_empty() {
                "127.0.0.1".to_owned()
            } else {
                host
            },
            port,
            client_id,
            connect_options: None,
            optional_capabilities: None,
            prefer_protobuf: true,
        }
    }
}

/// Thin TWS/Gateway protocol client.
#[derive(Debug)]
pub struct TwsApiClient {
    config: ClientConfig,
    stream: TcpStream,
    state: ConnectionState,
    server_version: i32,
    connection_time: String,
    api_ready: bool,
    pending_events: VecDeque<Event>,
}

/// An owned asynchronous event loop. Spawn `run` on a Tokio task when a background reader is desired.
pub struct EventPump<W> {
    client: TwsApiClient,
    wrapper: W,
}

impl<W: Wrapper> EventPump<W> {
    /// Runs until the connection closes or an event cannot be decoded.
    pub async fn run(mut self) -> TwsApiResult<()> {
        self.client.run_with(&mut self.wrapper).await
    }
}

impl TwsApiClient {
    /// Moves this client into an owned asynchronous event pump.
    pub fn into_event_pump<W: Wrapper>(self, wrapper: W) -> EventPump<W> {
        EventPump {
            client: self,
            wrapper,
        }
    }
    /// Opens a TCP connection and performs the enhanced TWS API handshake.
    pub async fn connect(config: ClientConfig) -> TwsApiResult<Self> {
        let addr = format!("{}:{}", config.host, config.port);
        let mut stream = TcpStream::connect(addr).await?;
        let handshake = comm::make_client_handshake(
            MIN_CLIENT_VER,
            MAX_CLIENT_VER,
            config.connect_options.as_deref(),
        );
        stream.write_all(&handshake).await?;

        let mut client = Self {
            config,
            stream,
            state: ConnectionState::Connecting,
            server_version: 0,
            connection_time: String::new(),
            api_ready: false,
            pending_events: VecDeque::new(),
        };
        client.read_handshake().await?;
        client.state = ConnectionState::Connected;
        client.start_api().await?;
        Ok(client)
    }

    /// Returns the negotiated server version.
    pub const fn server_version(&self) -> i32 {
        self.server_version
    }

    /// Returns the connection time reported by TWS/Gateway.
    pub fn connection_time(&self) -> &str {
        &self.connection_time
    }

    /// Returns the current connection state.
    pub const fn state(&self) -> ConnectionState {
        self.state
    }

    /// Closes the socket connection.
    pub async fn disconnect(&mut self) -> TwsApiResult<()> {
        self.state = ConnectionState::Disconnected;
        self.stream.shutdown().await?;
        Ok(())
    }

    /// Returns whether initial API callbacks have been received after `startApi`.
    pub const fn api_ready(&self) -> bool {
        self.api_ready
    }

    /// Sends `startApi`.
    pub async fn start_api(&mut self) -> TwsApiResult<()> {
        self.send_request(StartApiRequest {
            client_id: self.config.client_id,
            optional_capabilities: self.config.optional_capabilities.clone(),
            include_optional_capabilities: self.server_version
                >= MIN_SERVER_VER_OPTIONAL_CAPABILITIES,
        })
        .await
    }

    /// Sends `reqCurrentTime`.
    pub async fn req_current_time(&mut self) -> TwsApiResult<()> {
        self.send_request(VersionedRequest {
            message: Outgoing::ReqCurrentTime,
            version: 1,
        })
        .await
    }

    /// Sends `reqIds`.
    pub async fn req_ids(&mut self, num_ids: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::ReqIds,
            version: Some(1),
            req_id: num_ids,
        })
        .await
    }

    /// Sends `reqPositions`.
    pub async fn req_positions(&mut self) -> TwsApiResult<()> {
        if self.server_version < MIN_SERVER_VER_POSITIONS {
            return Err(TwsApiError::UnsupportedServerVersion {
                server_version: self.server_version,
                min_version: MIN_SERVER_VER_POSITIONS,
            });
        }
        self.send_request(VersionedRequest {
            message: Outgoing::ReqPositions,
            version: 1,
        })
        .await
    }

    /// Sends any field-encoded request.
    pub async fn send_request<R>(&mut self, request: R) -> TwsApiResult<()>
    where
        R: EncodableRequest,
    {
        let frame = encode_request_frame_with_protobuf(
            &request,
            self.server_version,
            self.api_ready && self.config.prefer_protobuf,
        )?;
        self.stream.write_all(&frame).await?;
        Ok(())
    }

    /// Sends a raw field-encoded request. `fields` must contain only fields after the message id.
    pub async fn raw_request(&mut self, message: Outgoing, fields: String) -> TwsApiResult<()> {
        self.send_request(RawRequest { message, fields }).await
    }

    /// Sends `setServerLogLevel`.
    pub async fn set_server_log_level(&mut self, log_level: i32) -> TwsApiResult<()> {
        self.send_request(SetServerLogLevelRequest { log_level })
            .await
    }

    /// Sends `reqMktData`.
    pub async fn req_mkt_data(&mut self, request: MarketDataRequest) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelMktData`.
    pub async fn cancel_mkt_data(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelMktData,
            version: Some(2),
            req_id,
        })
        .await
    }

    /// Sends `reqMarketDataType`.
    pub async fn req_market_data_type(&mut self, market_data_type: i32) -> TwsApiResult<()> {
        self.send_request(MarketDataTypeRequest { market_data_type })
            .await
    }

    /// Sends `reqSmartComponents`.
    pub async fn req_smart_components(
        &mut self,
        req_id: i32,
        bbo_exchange: &str,
    ) -> TwsApiResult<()> {
        self.send_request(SmartComponentsRequest {
            req_id,
            bbo_exchange: bbo_exchange.to_owned(),
        })
        .await
    }

    /// Sends `reqMarketRule`.
    pub async fn req_market_rule(&mut self, market_rule_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::ReqMarketRule,
            version: None,
            req_id: market_rule_id,
        })
        .await
    }

    /// Sends `reqMktDepth`.
    pub async fn req_mkt_depth(&mut self, request: MarketDepthRequest) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelMktDepth`.
    pub async fn cancel_mkt_depth(
        &mut self,
        req_id: i32,
        is_smart_depth: bool,
    ) -> TwsApiResult<()> {
        self.send_request(CancelMarketDepthRequest {
            req_id,
            is_smart_depth,
        })
        .await
    }

    /// Sends `placeOrder`.
    pub async fn place_order(&mut self, request: PlaceOrderRequest) -> TwsApiResult<()> {
        if self.use_protobuf(Outgoing::PlaceOrder) {
            if let Some(validation) = validate_order_parameters(&request.order, self.server_version)
            {
                return Err(TwsApiError::UnsupportedRequestParameter {
                    server_version: self.server_version,
                    min_version: validation.min_version,
                    parameter: validation.parameter,
                });
            }
            if let Some(validation) =
                validate_attached_orders_parameters(&request.order, self.server_version)
            {
                return Err(TwsApiError::UnsupportedRequestParameter {
                    server_version: self.server_version,
                    min_version: validation.min_version,
                    parameter: validation.parameter,
                });
            }
        }
        self.send_request(request).await
    }

    /// Sends `cancelOrder`.
    pub async fn cancel_order(
        &mut self,
        order_id: i32,
        order_cancel: OrderCancel,
    ) -> TwsApiResult<()> {
        self.send_request(CancelOrderRequest {
            order_id,
            order_cancel,
        })
        .await
    }

    /// Sends `reqOpenOrders`.
    pub async fn req_open_orders(&mut self) -> TwsApiResult<()> {
        self.send_request(VersionedRequest {
            message: Outgoing::ReqOpenOrders,
            version: 1,
        })
        .await
    }

    /// Sends `reqAutoOpenOrders`.
    pub async fn req_auto_open_orders(&mut self, auto_bind: bool) -> TwsApiResult<()> {
        self.send_request(AutoOpenOrdersRequest { auto_bind }).await
    }

    /// Sends `reqAllOpenOrders`.
    pub async fn req_all_open_orders(&mut self) -> TwsApiResult<()> {
        self.send_request(VersionedRequest {
            message: Outgoing::ReqAllOpenOrders,
            version: 1,
        })
        .await
    }

    /// Sends `reqGlobalCancel`.
    pub async fn req_global_cancel(&mut self, order_cancel: OrderCancel) -> TwsApiResult<()> {
        self.send_request(GlobalCancelRequest { order_cancel })
            .await
    }

    /// Sends `reqAccountUpdates`.
    pub async fn req_account_updates(
        &mut self,
        subscribe: bool,
        account_code: &str,
    ) -> TwsApiResult<()> {
        self.send_request(AccountDataRequest {
            subscribe,
            account_code: account_code.to_owned(),
        })
        .await
    }

    /// Sends `reqAccountSummary`.
    pub async fn req_account_summary(
        &mut self,
        req_id: i32,
        group_name: &str,
        tags: &str,
    ) -> TwsApiResult<()> {
        self.send_request(AccountSummaryRequest {
            req_id,
            group_name: group_name.to_owned(),
            tags: tags.to_owned(),
        })
        .await
    }

    /// Sends `cancelAccountSummary`.
    pub async fn cancel_account_summary(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelAccountSummary,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `cancelPositions`.
    pub async fn cancel_positions(&mut self) -> TwsApiResult<()> {
        self.send_request(VersionedRequest {
            message: Outgoing::CancelPositions,
            version: 1,
        })
        .await
    }

    /// Sends `reqPositionsMulti`.
    pub async fn req_positions_multi(
        &mut self,
        req_id: i32,
        account: &str,
        model_code: &str,
    ) -> TwsApiResult<()> {
        self.send_request(PositionsMultiRequest {
            req_id,
            account: account.to_owned(),
            model_code: model_code.to_owned(),
        })
        .await
    }

    /// Sends `cancelPositionsMulti`.
    pub async fn cancel_positions_multi(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelPositionsMulti,
            version: Some(1),
            req_id,
        })
        .await
    }

    /// Sends `reqAccountUpdatesMulti`.
    pub async fn req_account_updates_multi(
        &mut self,
        req_id: i32,
        account: &str,
        model_code: &str,
        ledger_and_nlv: bool,
    ) -> TwsApiResult<()> {
        self.send_request(AccountUpdatesMultiRequest {
            req_id,
            account: account.to_owned(),
            model_code: model_code.to_owned(),
            ledger_and_nlv,
        })
        .await
    }

    /// Sends `cancelAccountUpdatesMulti`.
    pub async fn cancel_account_updates_multi(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelAccountUpdatesMulti,
            version: Some(1),
            req_id,
        })
        .await
    }

    /// Sends `reqPnL`.
    pub async fn req_pnl(
        &mut self,
        req_id: i32,
        account: &str,
        model_code: &str,
    ) -> TwsApiResult<()> {
        self.send_request(PnlRequest {
            req_id,
            account: account.to_owned(),
            model_code: model_code.to_owned(),
        })
        .await
    }

    /// Sends `cancelPnL`.
    pub async fn cancel_pnl(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelPnl,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqPnLSingle`.
    pub async fn req_pnl_single(
        &mut self,
        req_id: i32,
        account: &str,
        model_code: &str,
        con_id: i32,
    ) -> TwsApiResult<()> {
        self.send_request(PnlSingleRequest {
            req_id,
            account: account.to_owned(),
            model_code: model_code.to_owned(),
            con_id,
        })
        .await
    }

    /// Sends `cancelPnLSingle`.
    pub async fn cancel_pnl_single(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelPnlSingle,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqExecutions`.
    pub async fn req_executions(&mut self, request: ExecutionRequest) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `reqContractDetails`.
    pub async fn req_contract_details(
        &mut self,
        request: ContractDetailsRequest,
    ) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelContractData`.
    pub async fn cancel_contract_data(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelContractData,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqMktDepthExchanges`.
    pub async fn req_mkt_depth_exchanges(&mut self) -> TwsApiResult<()> {
        self.send_request(EmptyRequest {
            message: Outgoing::ReqMktDepthExchanges,
        })
        .await
    }

    /// Sends `reqNewsBulletins`.
    pub async fn req_news_bulletins(&mut self, all_messages: bool) -> TwsApiResult<()> {
        self.send_request(NewsBulletinsRequest { all_messages })
            .await
    }

    /// Sends `cancelNewsBulletins`.
    pub async fn cancel_news_bulletins(&mut self) -> TwsApiResult<()> {
        self.send_request(VersionedRequest {
            message: Outgoing::CancelNewsBulletins,
            version: 1,
        })
        .await
    }

    /// Sends `reqManagedAccts`.
    pub async fn req_managed_accounts(&mut self) -> TwsApiResult<()> {
        self.send_request(VersionedRequest {
            message: Outgoing::ReqManagedAccounts,
            version: 1,
        })
        .await
    }

    /// Sends `requestFA`.
    pub async fn request_fa(&mut self, fa_data_type: i32) -> TwsApiResult<()> {
        self.send_request(FinancialAdvisorRequest { fa_data_type })
            .await
    }

    /// Sends `replaceFA`.
    pub async fn replace_fa(
        &mut self,
        req_id: i32,
        fa_data_type: i32,
        xml: &str,
    ) -> TwsApiResult<()> {
        self.send_request(ReplaceFinancialAdvisorRequest {
            req_id,
            fa_data_type,
            xml: xml.to_owned(),
        })
        .await
    }

    /// Sends `reqHistoricalData`.
    pub async fn req_historical_data(
        &mut self,
        request: HistoricalDataRequest,
    ) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelHistoricalData`.
    pub async fn cancel_historical_data(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelHistoricalData,
            version: Some(1),
            req_id,
        })
        .await
    }

    /// Sends `reqScannerParameters`.
    pub async fn req_scanner_parameters(&mut self) -> TwsApiResult<()> {
        self.send_request(VersionedRequest {
            message: Outgoing::ReqScannerParameters,
            version: 1,
        })
        .await
    }

    /// Sends `reqScannerSubscription`.
    pub async fn req_scanner_subscription(
        &mut self,
        request: ScannerSubscriptionRequest,
    ) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelScannerSubscription`.
    pub async fn cancel_scanner_subscription(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelScannerSubscription,
            version: Some(1),
            req_id,
        })
        .await
    }

    /// Sends a request id plus optional tag values.
    pub async fn id_request_with_options(
        &mut self,
        message: Outgoing,
        req_id: i32,
        options: &[TagValue],
    ) -> TwsApiResult<()> {
        let mut fields = FieldSink::default();
        fields.push(req_id)?.push(options.len())?;
        for option in options {
            fields.push(&option.tag)?.push(&option.value)?;
        }
        self.raw_request(message, fields.into_string()).await
    }

    /// Sends `reqTickByTickData`.
    pub async fn req_tick_by_tick_data(&mut self, request: TickByTickRequest) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelTickByTickData`.
    pub async fn cancel_tick_by_tick_data(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelTickByTickData,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `calculateImpliedVolatility`.
    pub async fn calculate_implied_volatility(
        &mut self,
        request: CalculateImpliedVolatilityRequest,
    ) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelCalculateImpliedVolatility`.
    pub async fn cancel_calculate_implied_volatility(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelCalcImpliedVolat,
            version: Some(1),
            req_id,
        })
        .await
    }

    /// Sends `calculateOptionPrice`.
    pub async fn calculate_option_price(
        &mut self,
        request: CalculateOptionPriceRequest,
    ) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelCalculateOptionPrice`.
    pub async fn cancel_calculate_option_price(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelCalcOptionPrice,
            version: Some(1),
            req_id,
        })
        .await
    }

    /// Sends `exerciseOptions`.
    pub async fn exercise_options(&mut self, request: ExerciseOptionsRequest) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `reqHeadTimeStamp`.
    pub async fn req_head_timestamp(&mut self, request: HeadTimestampRequest) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelHeadTimeStamp`.
    pub async fn cancel_head_timestamp(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelHeadTimestamp,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqHistogramData`.
    pub async fn req_histogram_data(&mut self, request: HistogramDataRequest) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelHistogramData`.
    pub async fn cancel_histogram_data(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelHistogramData,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqHistoricalTicks`.
    pub async fn req_historical_ticks(
        &mut self,
        request: HistoricalTicksRequest,
    ) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelHistoricalTicks`.
    pub async fn cancel_historical_ticks(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelHistoricalTicks,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqRealTimeBars`.
    pub async fn req_real_time_bars(&mut self, request: RealTimeBarsRequest) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelRealTimeBars`.
    pub async fn cancel_real_time_bars(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelRealTimeBars,
            version: Some(1),
            req_id,
        })
        .await
    }

    /// Sends `reqNewsProviders`.
    pub async fn req_news_providers(&mut self) -> TwsApiResult<()> {
        self.send_request(EmptyRequest {
            message: Outgoing::ReqNewsProviders,
        })
        .await
    }

    /// Sends `reqNewsArticle`.
    pub async fn req_news_article(
        &mut self,
        req_id: i32,
        provider_code: &str,
        article_id: &str,
        options: &[TagValue],
    ) -> TwsApiResult<()> {
        self.send_request(NewsArticleRequest {
            req_id,
            provider_code: provider_code.to_owned(),
            article_id: article_id.to_owned(),
            options: options.to_vec(),
        })
        .await
    }

    /// Sends `reqHistoricalNews`.
    pub async fn req_historical_news(
        &mut self,
        request: HistoricalNewsRequest,
    ) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `queryDisplayGroups`.
    pub async fn query_display_groups(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::QueryDisplayGroups,
            version: Some(1),
            req_id,
        })
        .await
    }

    /// Sends `subscribeToGroupEvents`.
    pub async fn subscribe_to_group_events(
        &mut self,
        req_id: i32,
        group_id: i32,
    ) -> TwsApiResult<()> {
        self.send_request(SubscribeToGroupEventsRequest { req_id, group_id })
            .await
    }

    /// Sends `updateDisplayGroup`.
    pub async fn update_display_group(
        &mut self,
        req_id: i32,
        contract_info: &str,
    ) -> TwsApiResult<()> {
        self.send_request(UpdateDisplayGroupRequest {
            req_id,
            contract_info: contract_info.to_owned(),
        })
        .await
    }

    /// Sends `unsubscribeFromGroupEvents`.
    pub async fn unsubscribe_from_group_events(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::UnsubscribeFromGroupEvents,
            version: Some(1),
            req_id,
        })
        .await
    }

    /// Sends `verifyRequest`.
    pub async fn verify_request(&mut self, api_name: &str, api_version: &str) -> TwsApiResult<()> {
        self.send_request(VerifyRequest {
            api_name: api_name.to_owned(),
            api_version: api_version.to_owned(),
        })
        .await
    }

    /// Sends `verifyMessage`.
    pub async fn verify_message(&mut self, api_data: &str) -> TwsApiResult<()> {
        self.send_request(VerifyMessageRequest {
            api_data: api_data.to_owned(),
        })
        .await
    }

    /// Sends `verifyAndAuthRequest`.
    pub async fn verify_and_auth_request(
        &mut self,
        api_name: &str,
        api_version: &str,
        opaque_isv_key: &str,
    ) -> TwsApiResult<()> {
        self.send_request(VerifyAndAuthRequest {
            api_name: api_name.to_owned(),
            api_version: api_version.to_owned(),
            opaque_isv_key: opaque_isv_key.to_owned(),
        })
        .await
    }

    /// Sends `verifyAndAuthMessage`.
    pub async fn verify_and_auth_message(
        &mut self,
        api_data: &str,
        xyz_response: &str,
    ) -> TwsApiResult<()> {
        self.send_request(VerifyAndAuthMessageRequest {
            api_data: api_data.to_owned(),
            xyz_response: xyz_response.to_owned(),
        })
        .await
    }

    /// Sends `reqSecDefOptParams`.
    pub async fn req_sec_def_opt_params(
        &mut self,
        req_id: i32,
        underlying_symbol: &str,
        fut_fop_exchange: &str,
        underlying_sec_type: &str,
        underlying_con_id: i32,
    ) -> TwsApiResult<()> {
        self.send_request(SecDefOptParamsRequest {
            req_id,
            underlying_symbol: underlying_symbol.to_owned(),
            fut_fop_exchange: fut_fop_exchange.to_owned(),
            underlying_sec_type: underlying_sec_type.to_owned(),
            underlying_con_id,
        })
        .await
    }

    /// Sends `reqSoftDollarTiers`.
    pub async fn req_soft_dollar_tiers(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::ReqSoftDollarTiers,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqFamilyCodes`.
    pub async fn req_family_codes(&mut self) -> TwsApiResult<()> {
        self.send_request(EmptyRequest {
            message: Outgoing::ReqFamilyCodes,
        })
        .await
    }

    /// Sends `reqMatchingSymbols`.
    pub async fn req_matching_symbols(&mut self, req_id: i32, pattern: &str) -> TwsApiResult<()> {
        self.send_request(MatchingSymbolsRequest {
            req_id,
            pattern: pattern.to_owned(),
        })
        .await
    }

    /// Sends `reqCompletedOrders`.
    pub async fn req_completed_orders(&mut self, api_only: bool) -> TwsApiResult<()> {
        self.send_request(CompletedOrdersRequest { api_only }).await
    }

    /// Sends `reqWshMetaData`.
    pub async fn req_wsh_meta_data(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::ReqWshMetaData,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `cancelWshMetaData`.
    pub async fn cancel_wsh_meta_data(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelWshMetaData,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqWshEventData`.
    pub async fn req_wsh_event_data(&mut self, request: WshEventDataRequest) -> TwsApiResult<()> {
        self.send_request(request).await
    }

    /// Sends `cancelWshEventData`.
    pub async fn cancel_wsh_event_data(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::CancelWshEventData,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqUserInfo`.
    pub async fn req_user_info(&mut self, req_id: i32) -> TwsApiResult<()> {
        self.send_request(IdRequest {
            message: Outgoing::ReqUserInfo,
            version: None,
            req_id,
        })
        .await
    }

    /// Sends `reqCurrentTimeInMillis`.
    pub async fn req_current_time_in_millis(&mut self) -> TwsApiResult<()> {
        self.send_request(EmptyRequest {
            message: Outgoing::ReqCurrentTimeInMillis,
        })
        .await
    }

    /// Sends `reqConfig` protobuf payload.
    pub async fn req_config_protobuf(&mut self, protobuf_data: &[u8]) -> TwsApiResult<()> {
        self.send_protobuf(Outgoing::ReqConfig, protobuf_data).await
    }

    /// Sends `updateConfig` protobuf payload.
    pub async fn update_config_protobuf(&mut self, protobuf_data: &[u8]) -> TwsApiResult<()> {
        self.send_protobuf(Outgoing::UpdateConfig, protobuf_data)
            .await
    }

    /// Sends an already-serialized protobuf payload with the protobuf-adjusted outgoing id.
    pub async fn send_protobuf(&mut self, msg: Outgoing, protobuf_data: &[u8]) -> TwsApiResult<()> {
        let frame = comm::make_msg_proto(msg.protobuf_id(), protobuf_data);
        self.stream.write_all(&frame).await?;
        Ok(())
    }

    /// Returns whether this server version supports protobuf for `message`.
    pub fn use_protobuf(&self, message: Outgoing) -> bool {
        protobuf_min_server_version(message)
            .is_some_and(|min_version| self.server_version >= min_version)
    }

    /// Sends protobuf for `message` only when the negotiated server version supports it.
    pub async fn send_protobuf_request(
        &mut self,
        message: Outgoing,
        protobuf_data: &[u8],
    ) -> TwsApiResult<()> {
        if !self.use_protobuf(message) {
            return Err(TwsApiError::UnsupportedServerVersion {
                server_version: self.server_version,
                min_version: protobuf_min_server_version(message).unwrap_or(i32::MAX),
            });
        }
        self.send_protobuf(message, protobuf_data).await
    }

    /// Reads one raw payload frame from the socket.
    pub async fn read_payload(&mut self) -> TwsApiResult<Vec<u8>> {
        let size = self.stream.read_u32().await?;
        let size = usize::try_from(size).map_err(|_| TwsApiError::FrameTooLarge(size))?;
        if size > MAX_MSG_LEN {
            return Err(TwsApiError::FrameTooLarge(size as u32));
        }
        let mut payload = vec![0; size];
        self.stream.read_exact(&mut payload).await?;
        Ok(payload)
    }

    /// Reads and decodes one incoming event.
    pub async fn read_event(&mut self) -> TwsApiResult<Event> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(event);
        }
        let payload = self.read_payload().await?;
        let event =
            decoder::decode_payload(self.server_version >= MIN_SERVER_VER_PROTOBUF, &payload)?;
        if matches!(
            event,
            Event::NextValidId { .. } | Event::ManagedAccounts { .. }
        ) {
            self.api_ready = true;
        }
        if let Some(next_event) = follow_up_event(&event) {
            self.pending_events.push_back(next_event);
        }
        Ok(event)
    }

    /// Reads one event and dispatches it to typed [`Wrapper`] callbacks.
    pub async fn read_event_with<W: Wrapper>(&mut self, wrapper: &mut W) -> TwsApiResult<Event> {
        let event = self.read_event().await?;
        wrapper.dispatch(event.clone());
        Ok(event)
    }

    /// Runs the event loop until the socket closes or a decoding error occurs.
    pub async fn run_with<W: Wrapper>(&mut self, wrapper: &mut W) -> TwsApiResult<()> {
        loop {
            self.read_event_with(wrapper).await?;
        }
    }

    async fn read_handshake(&mut self) -> TwsApiResult<()> {
        loop {
            let payload = self.read_payload().await?;
            let fields = comm::read_fields(&payload);
            if fields.len() == 2 {
                let mut fields = fields.into_iter();
                let server_version =
                    parse_i32(fields.next().ok_or(TwsApiError::MalformedHandshake)?)?;
                self.server_version = server_version;
                self.connection_time =
                    String::from_utf8_lossy(fields.next().ok_or(TwsApiError::MalformedHandshake)?)
                        .into_owned();
                return Ok(());
            }

            let event = decoder::decode_payload(false, &payload)?;
            self.pending_events.push_back(event.clone());
            if let Some(next_event) = follow_up_event(&event) {
                self.pending_events.push_back(next_event);
            }
        }
    }
}

fn follow_up_event(event: &Event) -> Option<Event> {
    match event {
        Event::ScannerData { req_id, .. } => Some(Event::ScannerDataEnd { req_id: *req_id }),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::follow_up_event;
    use crate::events::{Event, ScannerDataRow};
    use crate::types::Contract;

    #[test]
    fn scanner_data_enqueues_end_event() {
        let event = Event::ScannerData {
            req_id: 7,
            rows: vec![ScannerDataRow {
                rank: 1,
                contract: Box::new(Contract::default()),
                market_name: "NASDAQ".to_owned(),
                distance: "0".to_owned(),
                benchmark: "bm".to_owned(),
                projection: "proj".to_owned(),
                combo_key: "ck".to_owned(),
            }],
        };

        assert_eq!(
            follow_up_event(&event),
            Some(Event::ScannerDataEnd { req_id: 7 })
        );
    }
}

fn parse_i32(field: &[u8]) -> TwsApiResult<i32> {
    let text = String::from_utf8_lossy(field).into_owned();
    text.parse::<i32>()
        .map_err(|source| TwsApiError::InvalidInteger {
            field: text,
            source,
        })
}
