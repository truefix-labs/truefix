/// Offset added to an outgoing message id when the payload is protobuf encoded.
pub const PROTOBUF_MSG_ID: i32 = 200;

/// Incoming TWS API message identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Incoming {
    /// Tick price callback.
    TickPrice = 1,
    /// Tick size callback.
    TickSize = 2,
    /// Order status callback.
    OrderStatus = 3,
    /// Error callback.
    ErrorMessage = 4,
    /// Open order callback.
    OpenOrder = 5,
    /// Account value callback.
    AccountValue = 6,
    /// Portfolio value callback.
    PortfolioValue = 7,
    /// Account update time callback.
    AccountUpdateTime = 8,
    /// Next valid id callback.
    NextValidId = 9,
    /// Contract data callback.
    ContractData = 10,
    /// Execution data callback.
    ExecutionData = 11,
    /// Market depth callback.
    MarketDepth = 12,
    /// Market depth L2 callback.
    MarketDepthL2 = 13,
    /// News bulletin callback.
    NewsBulletins = 14,
    /// Managed accounts callback.
    ManagedAccounts = 15,
    /// Receive FA callback.
    ReceiveFa = 16,
    /// Historical data callback.
    HistoricalData = 17,
    /// Bond contract data callback.
    BondContractData = 18,
    /// Scanner parameters callback.
    ScannerParameters = 19,
    /// Scanner data callback.
    ScannerData = 20,
    /// Option computation callback.
    TickOptionComputation = 21,
    /// Generic tick callback.
    TickGeneric = 45,
    /// String tick callback.
    TickString = 46,
    /// EFP tick callback.
    TickEfp = 47,
    /// Current time callback.
    CurrentTime = 49,
    /// Real-time bar callback.
    RealTimeBars = 50,
    /// Contract data end callback.
    ContractDataEnd = 52,
    /// Open order end callback.
    OpenOrderEnd = 53,
    /// Account download end callback.
    AccountDownloadEnd = 54,
    /// Execution data end callback.
    ExecutionDataEnd = 55,
    /// Delta-neutral validation callback.
    DeltaNeutralValidation = 56,
    /// Tick snapshot end callback.
    TickSnapshotEnd = 57,
    /// Market data type callback.
    MarketDataType = 58,
    /// Commission and fees report callback.
    CommissionAndFeesReport = 59,
    /// Position data callback.
    PositionData = 61,
    /// Position end callback.
    PositionEnd = 62,
    /// Account summary callback.
    AccountSummary = 63,
    /// Account summary end callback.
    AccountSummaryEnd = 64,
    /// Verify message API callback.
    VerifyMessageApi = 65,
    /// Verify completed callback.
    VerifyCompleted = 66,
    /// Display group list callback.
    DisplayGroupList = 67,
    /// Display group updated callback.
    DisplayGroupUpdated = 68,
    /// Verify-and-auth message callback.
    VerifyAndAuthMessageApi = 69,
    /// Verify-and-auth completed callback.
    VerifyAndAuthCompleted = 70,
    /// Position multi callback.
    PositionMulti = 71,
    /// Position multi end callback.
    PositionMultiEnd = 72,
    /// Account update multi callback.
    AccountUpdateMulti = 73,
    /// Account update multi end callback.
    AccountUpdateMultiEnd = 74,
    /// Security definition option parameter callback.
    SecurityDefinitionOptionParameter = 75,
    /// Security definition option parameter end callback.
    SecurityDefinitionOptionParameterEnd = 76,
    /// Soft-dollar tiers callback.
    SoftDollarTiers = 77,
    /// Family codes callback.
    FamilyCodes = 78,
    /// Symbol samples callback.
    SymbolSamples = 79,
    /// Market depth exchanges callback.
    MarketDepthExchanges = 80,
    /// Tick request parameters callback.
    TickRequestParameters = 81,
    /// Smart components callback.
    SmartComponents = 82,
    /// News article callback.
    NewsArticle = 83,
    /// Tick news callback.
    TickNews = 84,
    /// News providers callback.
    NewsProviders = 85,
    /// Historical news callback.
    HistoricalNews = 86,
    /// Historical news end callback.
    HistoricalNewsEnd = 87,
    /// Head timestamp callback.
    HeadTimestamp = 88,
    /// Histogram data callback.
    HistogramData = 89,
    /// Historical data update callback.
    HistoricalDataUpdate = 90,
    /// Reroute market data request callback.
    RerouteMarketDataRequest = 91,
    /// Reroute market depth request callback.
    RerouteMarketDepthRequest = 92,
    /// Market rule callback.
    MarketRule = 93,
    /// PnL callback.
    Pnl = 94,
    /// PnL single callback.
    PnlSingle = 95,
    /// Historical ticks callback.
    HistoricalTicks = 96,
    /// Historical bid/ask ticks callback.
    HistoricalTicksBidAsk = 97,
    /// Historical last ticks callback.
    HistoricalTicksLast = 98,
    /// Tick-by-tick callback.
    TickByTick = 99,
    /// Order bound callback.
    OrderBound = 100,
    /// Completed order callback.
    CompletedOrder = 101,
    /// Completed orders end callback.
    CompletedOrdersEnd = 102,
    /// Replace FA end callback.
    ReplaceFaEnd = 103,
    /// WSH metadata callback.
    WshMetaData = 104,
    /// WSH event data callback.
    WshEventData = 105,
    /// Historical schedule callback.
    HistoricalSchedule = 106,
    /// User info callback.
    UserInfo = 107,
    /// Historical data end callback.
    HistoricalDataEnd = 108,
    /// Current time in milliseconds callback.
    CurrentTimeInMillis = 109,
    /// Config response callback.
    ConfigResponse = 110,
    /// Update config response callback.
    UpdateConfigResponse = 111,
}

impl Incoming {
    /// All known incoming TWS API message ids.
    pub const ALL: &'static [Self] = &[
        Self::TickPrice,
        Self::TickSize,
        Self::OrderStatus,
        Self::ErrorMessage,
        Self::OpenOrder,
        Self::AccountValue,
        Self::PortfolioValue,
        Self::AccountUpdateTime,
        Self::NextValidId,
        Self::ContractData,
        Self::ExecutionData,
        Self::MarketDepth,
        Self::MarketDepthL2,
        Self::NewsBulletins,
        Self::ManagedAccounts,
        Self::ReceiveFa,
        Self::HistoricalData,
        Self::BondContractData,
        Self::ScannerParameters,
        Self::ScannerData,
        Self::TickOptionComputation,
        Self::TickGeneric,
        Self::TickString,
        Self::TickEfp,
        Self::CurrentTime,
        Self::RealTimeBars,
        Self::ContractDataEnd,
        Self::OpenOrderEnd,
        Self::AccountDownloadEnd,
        Self::ExecutionDataEnd,
        Self::DeltaNeutralValidation,
        Self::TickSnapshotEnd,
        Self::MarketDataType,
        Self::CommissionAndFeesReport,
        Self::PositionData,
        Self::PositionEnd,
        Self::AccountSummary,
        Self::AccountSummaryEnd,
        Self::VerifyMessageApi,
        Self::VerifyCompleted,
        Self::DisplayGroupList,
        Self::DisplayGroupUpdated,
        Self::VerifyAndAuthMessageApi,
        Self::VerifyAndAuthCompleted,
        Self::PositionMulti,
        Self::PositionMultiEnd,
        Self::AccountUpdateMulti,
        Self::AccountUpdateMultiEnd,
        Self::SecurityDefinitionOptionParameter,
        Self::SecurityDefinitionOptionParameterEnd,
        Self::SoftDollarTiers,
        Self::FamilyCodes,
        Self::SymbolSamples,
        Self::MarketDepthExchanges,
        Self::TickRequestParameters,
        Self::SmartComponents,
        Self::NewsArticle,
        Self::TickNews,
        Self::NewsProviders,
        Self::HistoricalNews,
        Self::HistoricalNewsEnd,
        Self::HeadTimestamp,
        Self::HistogramData,
        Self::HistoricalDataUpdate,
        Self::RerouteMarketDataRequest,
        Self::RerouteMarketDepthRequest,
        Self::MarketRule,
        Self::Pnl,
        Self::PnlSingle,
        Self::HistoricalTicks,
        Self::HistoricalTicksBidAsk,
        Self::HistoricalTicksLast,
        Self::TickByTick,
        Self::OrderBound,
        Self::CompletedOrder,
        Self::CompletedOrdersEnd,
        Self::ReplaceFaEnd,
        Self::WshMetaData,
        Self::WshEventData,
        Self::HistoricalSchedule,
        Self::UserInfo,
        Self::HistoricalDataEnd,
        Self::CurrentTimeInMillis,
        Self::ConfigResponse,
        Self::UpdateConfigResponse,
    ];
}

/// Outgoing TWS API message identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Outgoing {
    /// Request market data.
    ReqMktData = 1,
    /// Cancel market data.
    CancelMktData = 2,
    /// Place order.
    PlaceOrder = 3,
    /// Cancel order.
    CancelOrder = 4,
    /// Request open orders.
    ReqOpenOrders = 5,
    /// Request account data.
    ReqAcctData = 6,
    /// Request executions.
    ReqExecutions = 7,
    /// Request ids.
    ReqIds = 8,
    /// Request contract data.
    ReqContractData = 9,
    /// Request market depth.
    ReqMktDepth = 10,
    /// Cancel market depth.
    CancelMktDepth = 11,
    /// Request news bulletins.
    ReqNewsBulletins = 12,
    /// Cancel news bulletins.
    CancelNewsBulletins = 13,
    /// Set server log level.
    SetServerLogLevel = 14,
    /// Request auto-open orders.
    ReqAutoOpenOrders = 15,
    /// Request all open orders.
    ReqAllOpenOrders = 16,
    /// Request managed accounts.
    ReqManagedAccounts = 17,
    /// Request FA data.
    ReqFa = 18,
    /// Replace FA data.
    ReplaceFa = 19,
    /// Request historical data.
    ReqHistoricalData = 20,
    /// Exercise options.
    ExerciseOptions = 21,
    /// Request scanner subscription.
    ReqScannerSubscription = 22,
    /// Cancel scanner subscription.
    CancelScannerSubscription = 23,
    /// Request scanner parameters.
    ReqScannerParameters = 24,
    /// Cancel historical data.
    CancelHistoricalData = 25,
    /// Request current time.
    ReqCurrentTime = 49,
    /// Request real-time bars.
    ReqRealTimeBars = 50,
    /// Cancel real-time bars.
    CancelRealTimeBars = 51,
    /// Request implied volatility calculation.
    ReqCalcImpliedVolat = 54,
    /// Request option price calculation.
    ReqCalcOptionPrice = 55,
    /// Cancel implied volatility calculation.
    CancelCalcImpliedVolat = 56,
    /// Cancel option price calculation.
    CancelCalcOptionPrice = 57,
    /// Request global cancel.
    ReqGlobalCancel = 58,
    /// Request market data type.
    ReqMarketDataType = 59,
    /// Request positions.
    ReqPositions = 61,
    /// Request account summary.
    ReqAccountSummary = 62,
    /// Cancel account summary.
    CancelAccountSummary = 63,
    /// Cancel positions.
    CancelPositions = 64,
    /// Verify request.
    VerifyRequest = 65,
    /// Verify message.
    VerifyMessage = 66,
    /// Query display groups.
    QueryDisplayGroups = 67,
    /// Subscribe to group events.
    SubscribeToGroupEvents = 68,
    /// Update display group.
    UpdateDisplayGroup = 69,
    /// Unsubscribe from group events.
    UnsubscribeFromGroupEvents = 70,
    /// Start API.
    StartApi = 71,
    /// Verify and auth request.
    VerifyAndAuthRequest = 72,
    /// Verify and auth message.
    VerifyAndAuthMessage = 73,
    /// Request positions multi.
    ReqPositionsMulti = 74,
    /// Cancel positions multi.
    CancelPositionsMulti = 75,
    /// Request account updates multi.
    ReqAccountUpdatesMulti = 76,
    /// Cancel account updates multi.
    CancelAccountUpdatesMulti = 77,
    /// Request security definition option parameters.
    ReqSecDefOptParams = 78,
    /// Request soft dollar tiers.
    ReqSoftDollarTiers = 79,
    /// Request family codes.
    ReqFamilyCodes = 80,
    /// Request matching symbols.
    ReqMatchingSymbols = 81,
    /// Request market depth exchanges.
    ReqMktDepthExchanges = 82,
    /// Request smart components.
    ReqSmartComponents = 83,
    /// Request news article.
    ReqNewsArticle = 84,
    /// Request news providers.
    ReqNewsProviders = 85,
    /// Request historical news.
    ReqHistoricalNews = 86,
    /// Request head timestamp.
    ReqHeadTimestamp = 87,
    /// Request histogram data.
    ReqHistogramData = 88,
    /// Cancel histogram data.
    CancelHistogramData = 89,
    /// Cancel head timestamp.
    CancelHeadTimestamp = 90,
    /// Request market rule.
    ReqMarketRule = 91,
    /// Request PnL.
    ReqPnl = 92,
    /// Cancel PnL.
    CancelPnl = 93,
    /// Request PnL single.
    ReqPnlSingle = 94,
    /// Cancel PnL single.
    CancelPnlSingle = 95,
    /// Request historical ticks.
    ReqHistoricalTicks = 96,
    /// Request tick-by-tick data.
    ReqTickByTickData = 97,
    /// Cancel tick-by-tick data.
    CancelTickByTickData = 98,
    /// Request completed orders.
    ReqCompletedOrders = 99,
    /// Request WSH metadata.
    ReqWshMetaData = 100,
    /// Cancel WSH metadata.
    CancelWshMetaData = 101,
    /// Request WSH event data.
    ReqWshEventData = 102,
    /// Cancel WSH event data.
    CancelWshEventData = 103,
    /// Request user info.
    ReqUserInfo = 104,
    /// Request current time in milliseconds.
    ReqCurrentTimeInMillis = 105,
    /// Cancel contract data.
    CancelContractData = 106,
    /// Cancel historical ticks.
    CancelHistoricalTicks = 107,
    /// Request config.
    ReqConfig = 108,
    /// Update config.
    UpdateConfig = 109,
}

impl Outgoing {
    /// Numeric protocol id.
    pub const fn id(self) -> i32 {
        self as i32
    }

    /// Numeric protobuf protocol id.
    pub const fn protobuf_id(self) -> i32 {
        self.id() + PROTOBUF_MSG_ID
    }
}

impl TryFrom<i32> for Incoming {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::TickPrice,
            2 => Self::TickSize,
            3 => Self::OrderStatus,
            4 => Self::ErrorMessage,
            5 => Self::OpenOrder,
            6 => Self::AccountValue,
            7 => Self::PortfolioValue,
            8 => Self::AccountUpdateTime,
            9 => Self::NextValidId,
            10 => Self::ContractData,
            11 => Self::ExecutionData,
            12 => Self::MarketDepth,
            13 => Self::MarketDepthL2,
            14 => Self::NewsBulletins,
            15 => Self::ManagedAccounts,
            16 => Self::ReceiveFa,
            17 => Self::HistoricalData,
            18 => Self::BondContractData,
            19 => Self::ScannerParameters,
            20 => Self::ScannerData,
            21 => Self::TickOptionComputation,
            45 => Self::TickGeneric,
            46 => Self::TickString,
            47 => Self::TickEfp,
            49 => Self::CurrentTime,
            50 => Self::RealTimeBars,
            52 => Self::ContractDataEnd,
            53 => Self::OpenOrderEnd,
            54 => Self::AccountDownloadEnd,
            55 => Self::ExecutionDataEnd,
            56 => Self::DeltaNeutralValidation,
            57 => Self::TickSnapshotEnd,
            58 => Self::MarketDataType,
            59 => Self::CommissionAndFeesReport,
            61 => Self::PositionData,
            62 => Self::PositionEnd,
            63 => Self::AccountSummary,
            64 => Self::AccountSummaryEnd,
            65 => Self::VerifyMessageApi,
            66 => Self::VerifyCompleted,
            67 => Self::DisplayGroupList,
            68 => Self::DisplayGroupUpdated,
            69 => Self::VerifyAndAuthMessageApi,
            70 => Self::VerifyAndAuthCompleted,
            71 => Self::PositionMulti,
            72 => Self::PositionMultiEnd,
            73 => Self::AccountUpdateMulti,
            74 => Self::AccountUpdateMultiEnd,
            75 => Self::SecurityDefinitionOptionParameter,
            76 => Self::SecurityDefinitionOptionParameterEnd,
            77 => Self::SoftDollarTiers,
            78 => Self::FamilyCodes,
            79 => Self::SymbolSamples,
            80 => Self::MarketDepthExchanges,
            81 => Self::TickRequestParameters,
            82 => Self::SmartComponents,
            83 => Self::NewsArticle,
            84 => Self::TickNews,
            85 => Self::NewsProviders,
            86 => Self::HistoricalNews,
            87 => Self::HistoricalNewsEnd,
            88 => Self::HeadTimestamp,
            89 => Self::HistogramData,
            90 => Self::HistoricalDataUpdate,
            91 => Self::RerouteMarketDataRequest,
            92 => Self::RerouteMarketDepthRequest,
            93 => Self::MarketRule,
            94 => Self::Pnl,
            95 => Self::PnlSingle,
            96 => Self::HistoricalTicks,
            97 => Self::HistoricalTicksBidAsk,
            98 => Self::HistoricalTicksLast,
            99 => Self::TickByTick,
            100 => Self::OrderBound,
            101 => Self::CompletedOrder,
            102 => Self::CompletedOrdersEnd,
            103 => Self::ReplaceFaEnd,
            104 => Self::WshMetaData,
            105 => Self::WshEventData,
            106 => Self::HistoricalSchedule,
            107 => Self::UserInfo,
            108 => Self::HistoricalDataEnd,
            109 => Self::CurrentTimeInMillis,
            110 => Self::ConfigResponse,
            111 => Self::UpdateConfigResponse,
            _ => return Err(()),
        })
    }
}
