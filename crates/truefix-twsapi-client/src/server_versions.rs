/// First server version using enhanced handshake and frame length prefixes.
pub const MIN_CLIENT_VER: i32 = 100;

/// Portfolio allocation order support.
pub const MIN_SERVER_VER_PTA_ORDERS: i32 = 39;
/// Delta-neutral contract support.
pub const MIN_SERVER_VER_DELTA_NEUTRAL: i32 = 40;
/// Chained contract data support.
pub const MIN_SERVER_VER_CONTRACT_DATA_CHAIN: i32 = 40;
/// Scale order v2 support.
pub const MIN_SERVER_VER_SCALE_ORDERS2: i32 = 40;
/// Algo order support.
pub const MIN_SERVER_VER_ALGO_ORDERS: i32 = 41;
/// Chained execution data support.
pub const MIN_SERVER_VER_EXECUTION_DATA_CHAIN: i32 = 42;
/// Not-held order support.
pub const MIN_SERVER_VER_NOT_HELD: i32 = 44;
/// Security-id type support.
pub const MIN_SERVER_VER_SEC_ID_TYPE: i32 = 45;
/// Place order by conId support.
pub const MIN_SERVER_VER_PLACE_ORDER_CONID: i32 = 46;
/// Market data by conId support.
pub const MIN_SERVER_VER_REQ_MKT_DATA_CONID: i32 = 47;
/// Calculate implied volatility request support.
pub const MIN_SERVER_VER_REQ_CALC_IMPLIED_VOLAT: i32 = 49;
/// Calculate option price request support.
pub const MIN_SERVER_VER_REQ_CALC_OPTION_PRICE: i32 = 50;
/// Old short sale slot support.
pub const MIN_SERVER_VER_SSHORTX_OLD: i32 = 51;
/// Short sale slot support.
pub const MIN_SERVER_VER_SSHORTX: i32 = 52;
/// Global cancel request support.
pub const MIN_SERVER_VER_REQ_GLOBAL_CANCEL: i32 = 53;
/// Hedge order support.
pub const MIN_SERVER_VER_HEDGE_ORDERS: i32 = 54;
/// Market data type request support.
pub const MIN_SERVER_VER_REQ_MARKET_DATA_TYPE: i32 = 55;
/// Smart routing opt-out support.
pub const MIN_SERVER_VER_OPT_OUT_SMART_ROUTING: i32 = 56;
/// Smart combo routing parameter support.
pub const MIN_SERVER_VER_SMART_COMBO_ROUTING_PARAMS: i32 = 57;
/// Delta-neutral conId support.
pub const MIN_SERVER_VER_DELTA_NEUTRAL_CONID: i32 = 58;
/// Scale order v3 support.
pub const MIN_SERVER_VER_SCALE_ORDERS3: i32 = 60;
/// Combo leg price support.
pub const MIN_SERVER_VER_ORDER_COMBO_LEGS_PRICE: i32 = 61;
/// Trailing percent support.
pub const MIN_SERVER_VER_TRAILING_PERCENT: i32 = 62;
/// Delta-neutral open/close support.
pub const MIN_SERVER_VER_DELTA_NEUTRAL_OPEN_CLOSE: i32 = 66;
/// Position request support.
pub const MIN_SERVER_VER_POSITIONS: i32 = 67;
/// Account summary support.
pub const MIN_SERVER_VER_ACCOUNT_SUMMARY: i32 = 67;
/// Trading class support.
pub const MIN_SERVER_VER_TRADING_CLASS: i32 = 68;
/// Scale table support.
pub const MIN_SERVER_VER_SCALE_TABLE: i32 = 69;
/// Account linking support.
pub const MIN_SERVER_VER_LINKING: i32 = 70;
/// Algo id support.
pub const MIN_SERVER_VER_ALGO_ID: i32 = 71;
/// Optional capabilities support.
pub const MIN_SERVER_VER_OPTIONAL_CAPABILITIES: i32 = 72;
/// Solicited order support.
pub const MIN_SERVER_VER_ORDER_SOLICITED: i32 = 73;
/// Linking authentication support.
pub const MIN_SERVER_VER_LINKING_AUTH: i32 = 74;
/// Primary exchange support.
pub const MIN_SERVER_VER_PRIMARYEXCH: i32 = 75;
/// Randomized order size/price support.
pub const MIN_SERVER_VER_RANDOMIZE_SIZE_AND_PRICE: i32 = 76;
/// Fractional position support.
pub const MIN_SERVER_VER_FRACTIONAL_POSITIONS: i32 = 101;
/// Pegged-to-benchmark support.
pub const MIN_SERVER_VER_PEGGED_TO_BENCHMARK: i32 = 102;
/// Model support.
pub const MIN_SERVER_VER_MODELS_SUPPORT: i32 = 103;
/// Security definition option parameter request support.
pub const MIN_SERVER_VER_SEC_DEF_OPT_PARAMS_REQ: i32 = 104;
/// Extended operator support.
pub const MIN_SERVER_VER_EXT_OPERATOR: i32 = 105;
/// Soft dollar tier support.
pub const MIN_SERVER_VER_SOFT_DOLLAR_TIER: i32 = 106;
/// Family codes request support.
pub const MIN_SERVER_VER_REQ_FAMILY_CODES: i32 = 107;
/// Matching symbols request support.
pub const MIN_SERVER_VER_REQ_MATCHING_SYMBOLS: i32 = 108;
/// Past limit support.
pub const MIN_SERVER_VER_PAST_LIMIT: i32 = 109;
/// Market data size multiplier support.
pub const MIN_SERVER_VER_MD_SIZE_MULTIPLIER: i32 = 110;
/// Cash quantity support.
pub const MIN_SERVER_VER_CASH_QTY: i32 = 111;
/// Market depth exchanges request support.
pub const MIN_SERVER_VER_REQ_MKT_DEPTH_EXCHANGES: i32 = 112;
/// News tick support.
pub const MIN_SERVER_VER_TICK_NEWS: i32 = 113;
/// Smart components request support.
pub const MIN_SERVER_VER_REQ_SMART_COMPONENTS: i32 = 114;
/// News providers request support.
pub const MIN_SERVER_VER_REQ_NEWS_PROVIDERS: i32 = 115;
/// News article request support.
pub const MIN_SERVER_VER_REQ_NEWS_ARTICLE: i32 = 116;
/// Historical news request support.
pub const MIN_SERVER_VER_REQ_HISTORICAL_NEWS: i32 = 117;
/// Head timestamp request support.
pub const MIN_SERVER_VER_REQ_HEAD_TIMESTAMP: i32 = 118;
/// Histogram request support.
pub const MIN_SERVER_VER_REQ_HISTOGRAM: i32 = 119;
/// Service data type support.
pub const MIN_SERVER_VER_SERVICE_DATA_TYPE: i32 = 120;
/// Aggregate group support.
pub const MIN_SERVER_VER_AGG_GROUP: i32 = 121;
/// Underlying information support.
pub const MIN_SERVER_VER_UNDERLYING_INFO: i32 = 122;
/// Cancel head timestamp support.
pub const MIN_SERVER_VER_CANCEL_HEADTIMESTAMP: i32 = 123;
/// Synthetic real-time bar support.
pub const MIN_SERVER_VER_SYNT_REALTIME_BARS: i32 = 124;
/// CFD reroute support.
pub const MIN_SERVER_VER_CFD_REROUTE: i32 = 125;
/// Market rule support.
pub const MIN_SERVER_VER_MARKET_RULES: i32 = 126;
/// PnL support.
pub const MIN_SERVER_VER_PNL: i32 = 127;
/// News query origin support.
pub const MIN_SERVER_VER_NEWS_QUERY_ORIGINS: i32 = 128;
/// Unrealized PnL support.
pub const MIN_SERVER_VER_UNREALIZED_PNL: i32 = 129;
/// Historical ticks support.
pub const MIN_SERVER_VER_HISTORICAL_TICKS: i32 = 130;
/// Market cap price support.
pub const MIN_SERVER_VER_MARKET_CAP_PRICE: i32 = 131;
/// Pre-open bid/ask support.
pub const MIN_SERVER_VER_PRE_OPEN_BID_ASK: i32 = 132;
/// Real expiration date support.
pub const MIN_SERVER_VER_REAL_EXPIRATION_DATE: i32 = 134;
/// Realized PnL support.
pub const MIN_SERVER_VER_REALIZED_PNL: i32 = 135;
/// Last liquidity support.
pub const MIN_SERVER_VER_LAST_LIQUIDITY: i32 = 136;
/// Tick-by-tick data support.
pub const MIN_SERVER_VER_TICK_BY_TICK: i32 = 137;
/// Decision maker support.
pub const MIN_SERVER_VER_DECISION_MAKER: i32 = 138;
/// MiFID execution support.
pub const MIN_SERVER_VER_MIFID_EXECUTION: i32 = 139;
/// Tick-by-tick ignore-size support.
pub const MIN_SERVER_VER_TICK_BY_TICK_IGNORE_SIZE: i32 = 140;
/// Auto price for hedge support.
pub const MIN_SERVER_VER_AUTO_PRICE_FOR_HEDGE: i32 = 141;
/// What-if extension field support.
pub const MIN_SERVER_VER_WHAT_IF_EXT_FIELDS: i32 = 142;
/// Scanner generic option support.
pub const MIN_SERVER_VER_SCANNER_GENERIC_OPTS: i32 = 143;
/// API bind order support.
pub const MIN_SERVER_VER_API_BIND_ORDER: i32 = 144;
/// Order container support.
pub const MIN_SERVER_VER_ORDER_CONTAINER: i32 = 145;
/// Smart depth support.
pub const MIN_SERVER_VER_SMART_DEPTH: i32 = 146;
/// Remove-null-all-casting support.
pub const MIN_SERVER_VER_REMOVE_NULL_ALL_CASTING: i32 = 147;
/// D-PEG order support.
pub const MIN_SERVER_VER_D_PEG_ORDERS: i32 = 148;
/// Market depth primary exchange support.
pub const MIN_SERVER_VER_MKT_DEPTH_PRIM_EXCHANGE: i32 = 149;
/// Completed orders support.
pub const MIN_SERVER_VER_COMPLETED_ORDERS: i32 = 150;
/// Price management algo support.
pub const MIN_SERVER_VER_PRICE_MGMT_ALGO: i32 = 151;
/// Stock type support.
pub const MIN_SERVER_VER_STOCK_TYPE: i32 = 152;
/// ASCII7 message encoding support.
pub const MIN_SERVER_VER_ENCODE_MSG_ASCII7: i32 = 153;
/// Send all family codes support.
pub const MIN_SERVER_VER_SEND_ALL_FAMILY_CODES: i32 = 154;
/// No default open/close support.
pub const MIN_SERVER_VER_NO_DEFAULT_OPEN_CLOSE: i32 = 155;
/// Price-based volatility support.
pub const MIN_SERVER_VER_PRICE_BASED_VOLATILITY: i32 = 156;
/// Replace FA end support.
pub const MIN_SERVER_VER_REPLACE_FA_END: i32 = 157;
/// Duration support.
pub const MIN_SERVER_VER_DURATION: i32 = 158;
/// Market-data-in-shares support.
pub const MIN_SERVER_VER_MARKET_DATA_IN_SHARES: i32 = 159;
/// Post-to-ATS support.
pub const MIN_SERVER_VER_POST_TO_ATS: i32 = 160;
/// WSH event calendar support.
pub const MIN_SERVER_VER_WSHE_CALENDAR: i32 = 161;
/// Parent auto-cancel support.
pub const MIN_SERVER_VER_AUTO_CANCEL_PARENT: i32 = 162;
/// Fractional size support.
pub const MIN_SERVER_VER_FRACTIONAL_SIZE_SUPPORT: i32 = 163;
/// Size rules support.
pub const MIN_SERVER_VER_SIZE_RULES: i32 = 164;
/// Historical schedule support.
pub const MIN_SERVER_VER_HISTORICAL_SCHEDULE: i32 = 165;
/// Advanced order reject support.
pub const MIN_SERVER_VER_ADVANCED_ORDER_REJECT: i32 = 166;
/// User info support.
pub const MIN_SERVER_VER_USER_INFO: i32 = 167;
/// Crypto aggregated trade support.
pub const MIN_SERVER_VER_CRYPTO_AGGREGATED_TRADES: i32 = 168;
/// Manual order time support.
pub const MIN_SERVER_VER_MANUAL_ORDER_TIME: i32 = 169;
/// PegBest/PegMid offset support.
pub const MIN_SERVER_VER_PEGBEST_PEGMID_OFFSETS: i32 = 170;
/// WSH event data filters support.
pub const MIN_SERVER_VER_WSH_EVENT_DATA_FILTERS: i32 = 171;
/// IPO price support.
pub const MIN_SERVER_VER_IPO_PRICES: i32 = 172;
/// WSH event data filter date support.
pub const MIN_SERVER_VER_WSH_EVENT_DATA_FILTERS_DATE: i32 = 173;
/// Instrument timezone support.
pub const MIN_SERVER_VER_INSTRUMENT_TIMEZONE: i32 = 174;
/// HMDS market-data-in-shares support.
pub const MIN_SERVER_VER_HMDS_MARKET_DATA_IN_SHARES: i32 = 175;
/// Bond issuer id support.
pub const MIN_SERVER_VER_BOND_ISSUERID: i32 = 176;
/// FA profile de-support marker.
pub const MIN_SERVER_VER_FA_PROFILE_DESUPPORT: i32 = 177;
/// Pending price revision support.
pub const MIN_SERVER_VER_PENDING_PRICE_REVISION: i32 = 178;
/// Fund data fields support.
pub const MIN_SERVER_VER_FUND_DATA_FIELDS: i32 = 179;
/// Manual exercise option order time support.
pub const MIN_SERVER_VER_MANUAL_ORDER_TIME_EXERCISE_OPTIONS: i32 = 180;
/// Open order AD strategy support.
pub const MIN_SERVER_VER_OPEN_ORDER_AD_STRATEGY: i32 = 181;
/// Last trade date support.
pub const MIN_SERVER_VER_LAST_TRADE_DATE: i32 = 182;
/// Customer account support.
pub const MIN_SERVER_VER_CUSTOMER_ACCOUNT: i32 = 183;
/// Professional customer support.
pub const MIN_SERVER_VER_PROFESSIONAL_CUSTOMER: i32 = 184;
/// Bond accrued interest support.
pub const MIN_SERVER_VER_BOND_ACCRUED_INTEREST: i32 = 185;
/// Ineligibility reason support.
pub const MIN_SERVER_VER_INELIGIBILITY_REASONS: i32 = 186;
/// RFQ fields support.
pub const MIN_SERVER_VER_RFQ_FIELDS: i32 = 187;
/// Bond trading hours support.
pub const MIN_SERVER_VER_BOND_TRADING_HOURS: i32 = 188;
/// Overnight inclusion support.
pub const MIN_SERVER_VER_INCLUDE_OVERNIGHT: i32 = 189;
/// Undo RFQ fields support.
pub const MIN_SERVER_VER_UNDO_RFQ_FIELDS: i32 = 190;
/// Long permanent id support.
pub const MIN_SERVER_VER_PERM_ID_AS_LONG: i32 = 191;
/// CME tagging field support.
pub const MIN_SERVER_VER_CME_TAGGING_FIELDS: i32 = 192;
/// CME tagging field support in open-order callbacks.
pub const MIN_SERVER_VER_CME_TAGGING_FIELDS_IN_OPEN_ORDER: i32 = 193;
/// Error time support.
pub const MIN_SERVER_VER_ERROR_TIME: i32 = 194;
/// Full order preview field support.
pub const MIN_SERVER_VER_FULL_ORDER_PREVIEW_FIELDS: i32 = 195;
/// Historical data end message support.
pub const MIN_SERVER_VER_HISTORICAL_DATA_END: i32 = 196;
/// Current time in millis support.
pub const MIN_SERVER_VER_CURRENT_TIME_IN_MILLIS: i32 = 197;
/// Submitter support.
pub const MIN_SERVER_VER_SUBMITTER: i32 = 198;
/// Imbalance-only support.
pub const MIN_SERVER_VER_IMBALANCE_ONLY: i32 = 199;
/// Parametrized days of executions support.
pub const MIN_SERVER_VER_PARAMETRIZED_DAYS_OF_EXECUTIONS: i32 = 200;
/// Protobuf message framing support.
pub const MIN_SERVER_VER_PROTOBUF: i32 = 201;
/// Zero-strike support.
pub const MIN_SERVER_VER_ZERO_STRIKE: i32 = 202;
/// Protobuf place order support.
pub const MIN_SERVER_VER_PROTOBUF_PLACE_ORDER: i32 = 203;
/// Protobuf completed order support.
pub const MIN_SERVER_VER_PROTOBUF_COMPLETED_ORDER: i32 = 204;
/// Protobuf contract data support.
pub const MIN_SERVER_VER_PROTOBUF_CONTRACT_DATA: i32 = 205;
/// Protobuf market data support.
pub const MIN_SERVER_VER_PROTOBUF_MARKET_DATA: i32 = 206;
/// Protobuf account and position support.
pub const MIN_SERVER_VER_PROTOBUF_ACCOUNTS_POSITIONS: i32 = 207;
/// Protobuf historical data support.
pub const MIN_SERVER_VER_PROTOBUF_HISTORICAL_DATA: i32 = 208;
/// Protobuf news data support.
pub const MIN_SERVER_VER_PROTOBUF_NEWS_DATA: i32 = 209;
/// Protobuf scan data support.
pub const MIN_SERVER_VER_PROTOBUF_SCAN_DATA: i32 = 210;
/// Protobuf REST message group 1 support.
pub const MIN_SERVER_VER_PROTOBUF_REST_MESSAGES_1: i32 = 211;
/// Protobuf REST message group 2 support.
pub const MIN_SERVER_VER_PROTOBUF_REST_MESSAGES_2: i32 = 212;
/// Protobuf REST message group 3 support.
pub const MIN_SERVER_VER_PROTOBUF_REST_MESSAGES_3: i32 = 213;
/// UTC date-time Z suffix support.
pub const MIN_SERVER_VER_ADD_Z_SUFFIX_TO_UTC_DATE_TIME: i32 = 214;
/// Cancel contract data support.
pub const MIN_SERVER_VER_CANCEL_CONTRACT_DATA: i32 = 215;
/// Additional order params group 1 support.
pub const MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_1: i32 = 216;
/// Additional order params group 2 support.
pub const MIN_SERVER_VER_ADDITIONAL_ORDER_PARAMS_2: i32 = 217;
/// Attached order support.
pub const MIN_SERVER_VER_ATTACHED_ORDERS: i32 = 218;
/// Config request support.
pub const MIN_SERVER_VER_CONFIG: i32 = 219;
/// Market data volume share units support.
pub const MIN_SERVER_VER_MARKET_DATA_VOLUMES_IN_SHARES: i32 = 220;
/// Config update support.
pub const MIN_SERVER_VER_UPDATE_CONFIG: i32 = 221;
/// Fractional last size support.
pub const MIN_SERVER_VER_FRACTIONAL_LAST_SIZE: i32 = 222;
/// Hedge max size support.
pub const MIN_SERVER_VER_HEDGE_MAX_SIZE: i32 = 223;
/// Security definition precision support.
pub const MIN_SERVER_VER_USE_PRECISION_FROM_SEC_DEF: i32 = 224;
/// Odd-lot bid/ask quote support.
pub const MIN_SERVER_VER_ODD_LOT_BID_ASK_QUOTES: i32 = 225;

/// Latest client protocol version supported by this port.
pub const MAX_CLIENT_VER: i32 = MIN_SERVER_VER_ODD_LOT_BID_ASK_QUOTES;
