// ── 会话 (Session) ────────────────────────────────────────────────────────────
pub const INIT_CONNECT: u32 = 1001;
pub const GET_GLOBAL_STATE: u32 = 1002;
pub const NOTIFY: u32 = 1003; // push
pub const KEEP_ALIVE: u32 = 1004;
pub const GET_USER_INFO: u32 = 1005;
pub const VERIFICATION: u32 = 1006;
pub const GET_DELAY_STATISTICS: u32 = 1007;
pub const TEST_CMD: u32 = 1008;

// ── 交易 (Trade) ──────────────────────────────────────────────────────────────
pub const TRD_GET_ACC_LIST: u32 = 2001;
pub const TRD_UNLOCK_TRADE: u32 = 2005;
pub const TRD_SUB_ACC_PUSH: u32 = 2008;
pub const TRD_GET_FUNDS: u32 = 2101;
pub const TRD_GET_POSITION_LIST: u32 = 2102;
pub const TRD_GET_MAX_TRD_QTYS: u32 = 2111;
pub const TRD_GET_COMBO_MAX_TRD_QTYS: u32 = 2112;
pub const TRD_GET_ORDER_LIST: u32 = 2201;
pub const TRD_PLACE_ORDER: u32 = 2202;
pub const TRD_MODIFY_ORDER: u32 = 2205;
pub const TRD_UPDATE_ORDER: u32 = 2208; // push
pub const TRD_GET_ORDER_FILL_LIST: u32 = 2211;
pub const TRD_UPDATE_ORDER_FILL: u32 = 2218; // push
pub const TRD_GET_HISTORY_ORDER_LIST: u32 = 2221;
pub const TRD_GET_HISTORY_ORDER_FILL_LIST: u32 = 2222;
pub const TRD_GET_MARGIN_RATIO: u32 = 2223;
pub const TRD_GET_ORDER_FEE: u32 = 2225;
pub const TRD_FLOW_SUMMARY: u32 = 2226;
pub const TRD_PLACE_COMBO_ORDER: u32 = 2227;

// ── 行情 (Quote) ──────────────────────────────────────────────────────────────
pub const QOT_SUB: u32 = 3001;
pub const QOT_REG_QOT_PUSH: u32 = 3002;
pub const QOT_GET_SUB_INFO: u32 = 3003;
pub const QOT_GET_BASIC_QOT: u32 = 3004;
pub const QOT_UPDATE_BASIC_QOT: u32 = 3005; // push
pub const QOT_GET_KL: u32 = 3006;
pub const QOT_UPDATE_KL: u32 = 3007; // push
pub const QOT_GET_RT: u32 = 3008;
pub const QOT_UPDATE_RT: u32 = 3009; // push
pub const QOT_GET_TICKER: u32 = 3010;
pub const QOT_UPDATE_TICKER: u32 = 3011; // push
pub const QOT_GET_ORDER_BOOK: u32 = 3012;
pub const QOT_UPDATE_ORDER_BOOK: u32 = 3013; // push
pub const QOT_GET_BROKER: u32 = 3014;
pub const QOT_UPDATE_BROKER: u32 = 3015; // push
pub const QOT_UPDATE_PRICE_REMINDER: u32 = 3019; // push
pub const QOT_REQUEST_HISTORY_KL: u32 = 3103;
pub const QOT_REQUEST_HISTORY_KL_QUOTA: u32 = 3104;
pub const QOT_REQUEST_REHAB: u32 = 3105;
pub const QOT_REQUEST_TRADE_DATE: u32 = 3219;
pub const QOT_GET_STATIC_INFO: u32 = 3202;
pub const QOT_GET_SECURITY_SNAPSHOT: u32 = 3203;
pub const QOT_GET_PLATE_SET: u32 = 3204;
pub const QOT_GET_PLATE_SECURITY: u32 = 3205;
pub const QOT_GET_REFERENCE: u32 = 3206;
pub const QOT_GET_OWNER_PLATE: u32 = 3207;
pub const QOT_GET_HOLDING_CHANGE_LIST: u32 = 3208;
pub const QOT_GET_OPTION_CHAIN: u32 = 3209;
pub const QOT_GET_WARRANT: u32 = 3210;
pub const QOT_GET_CAPITAL_FLOW: u32 = 3211;
pub const QOT_GET_CAPITAL_DISTRIBUTION: u32 = 3212;
pub const QOT_GET_USER_SECURITY: u32 = 3213;
pub const QOT_MODIFY_USER_SECURITY: u32 = 3214;
pub const QOT_STOCK_FILTER: u32 = 3215;
pub const QOT_GET_CODE_CHANGE: u32 = 3216;
pub const QOT_GET_IPO_LIST: u32 = 3217;
pub const QOT_GET_FUTURE_INFO: u32 = 3218;
pub const QOT_SET_PRICE_REMINDER: u32 = 3220;
pub const QOT_GET_PRICE_REMINDER: u32 = 3221;
pub const QOT_GET_USER_SECURITY_GROUP: u32 = 3222;
pub const QOT_GET_MARKET_STATE: u32 = 3223;
pub const QOT_GET_OPTION_EXPIRATION_DATE: u32 = 3224;
pub const QOT_GET_FINANCIALS_EARNINGS_PRICE_MOVE: u32 = 3225;
pub const QOT_GET_FINANCIALS_EARNINGS_PRICE_HISTORY: u32 = 3226;
pub const QOT_GET_FINANCIALS_STATEMENTS: u32 = 3227;
pub const QOT_GET_FINANCIALS_REVENUE_BREAKDOWN: u32 = 3228;
pub const QOT_GET_RESEARCH_ANALYST_CONSENSUS: u32 = 3229;
pub const QOT_GET_RESEARCH_RATING_SUMMARY: u32 = 3230;
pub const QOT_GET_RESEARCH_MORNINGSTAR_REPORT: u32 = 3231;
pub const QOT_GET_VALUATION_DETAIL: u32 = 3232;
pub const QOT_GET_VALUATION_PLATE_STOCK_LIST: u32 = 3233;
pub const QOT_GET_CORPORATE_ACTIONS_DIVIDENDS: u32 = 3234;
pub const QOT_GET_CORPORATE_ACTIONS_BUYBACKS: u32 = 3235;
pub const QOT_GET_CORPORATE_ACTIONS_STOCK_SPLITS: u32 = 3236;
pub const QOT_GET_SHAREHOLDERS_OVERVIEW: u32 = 3237;
pub const QOT_GET_SHAREHOLDERS_HOLDING_CHANGES: u32 = 3238;
pub const QOT_GET_SHAREHOLDERS_HOLDER_DETAIL: u32 = 3239;
pub const QOT_GET_SHAREHOLDERS_INSTITUTIONAL: u32 = 3240;
pub const QOT_GET_INSIDER_HOLDER_LIST: u32 = 3241;
pub const QOT_GET_INSIDER_TRADE_LIST: u32 = 3242;
pub const QOT_GET_COMPANY_PROFILE: u32 = 3243;
pub const QOT_GET_COMPANY_EXECUTIVES: u32 = 3244;
pub const QOT_GET_COMPANY_EXECUTIVE_BACKGROUND: u32 = 3245;
pub const QOT_GET_COMPANY_OPERATIONAL_EFFICIENCY: u32 = 3246;
pub const QOT_GET_TOP_TEN_BUY_SELL_BROKERS: u32 = 3247;
pub const QOT_GET_DAILY_SHORT_VOLUME: u32 = 3248;
pub const QOT_GET_SHORT_INTEREST: u32 = 3249;
pub const QOT_GET_OPTION_VOLATILITY: u32 = 3250;
pub const QOT_GET_OPTION_EXERCISE_PROBABILITY: u32 = 3251;
pub const QOT_STOCK_SCREEN: u32 = 3252;
pub const QOT_OPTION_SCREEN: u32 = 3253;
pub const QOT_WARRANT_SCREEN: u32 = 3254;
pub const QOT_GET_OPTION_QUOTE: u32 = 3255;
pub const QOT_GET_OPTION_STRATEGY: u32 = 3256;
pub const QOT_GET_OPTION_STRATEGY_ANALYSIS: u32 = 3257;
pub const QOT_GET_OPTION_STRATEGY_SPREAD: u32 = 3258;
pub const QOT_GET_INDICATOR_LIST: u32 = 3259;
pub const QOT_REQUEST_INDICATOR_CALC: u32 = 3260;
pub const QOT_GET_SEARCH_QUOTE: u32 = 3262;
pub const QOT_GET_SEARCH_NEWS: u32 = 3263;
pub const QOT_GET_OPTION_MARKET_STATISTIC: u32 = 3301;
pub const QOT_GET_OPTION_UNDERLYING_HIS_STATISTIC: u32 = 3302;
pub const QOT_GET_OPTION_UNDERLYING_OVERVIEW: u32 = 3303;
pub const QOT_GET_OPTION_UNDERLYING_HIS_VOLATILITY: u32 = 3304;
pub const QOT_GET_OPTION_UNDERLYING_RANK: u32 = 3305;
pub const QOT_GET_OPTION_RANK: u32 = 3306;
pub const QOT_GET_OPTION_EVENT: u32 = 3307;
pub const QOT_GET_OPTION_EVENT_ALERT: u32 = 3308;
pub const QOT_SET_OPTION_EVENT_ALERT: u32 = 3309;
pub const QOT_GET_OPTION_ZERO_DTE_SCREENER: u32 = 3311;
pub const QOT_GET_OPTION_ZERO_DTE_CONTRACT: u32 = 3312;
pub const QOT_GET_OPTION_EARNINGS_SCREENER: u32 = 3313;
pub const QOT_GET_OPTION_SELLER_SCREENER: u32 = 3314;
pub const QOT_GET_EARNINGS_CALENDAR: u32 = 3401;
pub const QOT_GET_MACRO_INDICATOR_LIST: u32 = 3402;
pub const QOT_GET_MACRO_INDICATOR_HISTORY: u32 = 3403;
pub const QOT_GET_FED_WATCH_TARGET_RATE: u32 = 3404;
pub const QOT_GET_FED_WATCH_DOT_PLOT: u32 = 3405;
pub const QOT_GET_EARNINGS_BEAT_RANK: u32 = 3406;
pub const QOT_GET_DIVIDEND_RANK: u32 = 3407;
pub const QOT_GET_DIVIDEND_CALENDAR: u32 = 3408;
pub const QOT_GET_ECONOMIC_CALENDAR: u32 = 3409;
pub const QOT_GET_US_PRE_MARKET_RANK: u32 = 3410;
pub const QOT_GET_US_AFTER_HOURS_RANK: u32 = 3411;
pub const QOT_GET_US_OVERNIGHT_RANK: u32 = 3412;
pub const QOT_GET_TOP_MOVERS_RANK: u32 = 3413;
pub const QOT_GET_HOT_LIST: u32 = 3414;
pub const QOT_GET_SHORT_SELLING_RANK: u32 = 3415;
pub const QOT_GET_PERIOD_CHANGE_RANK: u32 = 3416;
pub const QOT_GET_HIGH_DIVIDEND_SOE_RANK: u32 = 3417;
pub const QOT_GET_INSTITUTION_LIST: u32 = 3418;
pub const QOT_GET_INSTITUTION_PROFILE: u32 = 3419;
pub const QOT_GET_INSTITUTION_DISTRIBUTION: u32 = 3420;
pub const QOT_GET_INSTITUTION_HOLDING_CHANGE: u32 = 3421;
pub const QOT_GET_INSTITUTION_HOLDING_LIST: u32 = 3422;
pub const QOT_GET_ARK_FUND_HOLDING: u32 = 3423;
pub const QOT_GET_ARK_STOCK_DYNAMIC: u32 = 3424;
pub const QOT_GET_ARK_ACTIVE_TRANSACTION: u32 = 3425;
pub const QOT_GET_RATING_CHANGE: u32 = 3426;
pub const QOT_GET_INDUSTRIAL_CHAIN_LIST: u32 = 3427;
pub const QOT_GET_INDUSTRIAL_CHAIN_DETAIL: u32 = 3428;
pub const QOT_GET_INDUSTRIAL_CHAIN_BY_PLATE: u32 = 3429;
pub const QOT_GET_INDUSTRIAL_PLATE_INFO: u32 = 3430;
pub const QOT_GET_INDUSTRIAL_PLATE_STOCK: u32 = 3431;
pub const QOT_GET_HEAT_MAP_DATA: u32 = 3432;
pub const QOT_GET_RISE_FALL_DISTRIBUTION: u32 = 3433;
pub const SKILL_WRAP_TECHNICAL_UNUSUAL: u32 = 3801;
pub const SKILL_WRAP_FINANCIAL_UNUSUAL: u32 = 3802;
pub const SKILL_WRAP_DERIVATIVE_UNUSUAL: u32 = 3803;
pub const QOT_PUSH_INDICATOR_CALC: u32 = 3261; // push
pub const QOT_UPDATE_OPTION_EVENT: u32 = 3310; // push

// ── Push ID 集合 ──────────────────────────────────────────────────────────────

/// All proto IDs that are server-initiated push frames (not responses to requests).
pub const ALL_PUSH_IDS: &[u32] = &[
    NOTIFY,
    TRD_UPDATE_ORDER,
    TRD_UPDATE_ORDER_FILL,
    QOT_UPDATE_BROKER,
    QOT_UPDATE_ORDER_BOOK,
    QOT_UPDATE_KL,
    QOT_UPDATE_RT,
    QOT_UPDATE_BASIC_QOT,
    QOT_UPDATE_TICKER,
    QOT_UPDATE_PRICE_REMINDER,
    QOT_UPDATE_OPTION_EVENT,
    QOT_PUSH_INDICATOR_CALC,
];

/// Returns `true` if `proto_id` is a server-initiated push frame.
pub fn is_push(proto_id: u32) -> bool {
    ALL_PUSH_IDS.contains(&proto_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_push_ids_return_true() {
        for &id in ALL_PUSH_IDS {
            assert!(is_push(id), "expected is_push({id}) == true, but got false");
        }
    }

    #[test]
    fn non_push_ids_return_false() {
        let non_push = [
            INIT_CONNECT,
            GET_GLOBAL_STATE,
            KEEP_ALIVE,
            TRD_GET_ACC_LIST,
            TRD_PLACE_ORDER,
            TRD_GET_FUNDS,
            QOT_SUB,
            QOT_GET_BASIC_QOT,
            QOT_GET_KL,
            QOT_GET_ORDER_BOOK,
        ];
        for id in non_push {
            assert!(
                !is_push(id),
                "expected is_push({id}) == false, but got true"
            );
        }
    }

    #[test]
    fn unknown_id_returns_false() {
        assert!(!is_push(0));
        assert!(!is_push(9999));
        assert!(!is_push(u32::MAX));
    }
}
