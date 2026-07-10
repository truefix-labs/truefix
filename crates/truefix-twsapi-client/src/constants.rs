use rust_decimal::Decimal;

/// TWS API's sentinel for "no valid request/order id".
pub const NO_VALID_ID: i32 = -1;

/// Maximum message length used by the official client: 16 MiB minus one byte.
pub const MAX_MSG_LEN: usize = 0xFF_FFFF;

/// TWS API integer unset sentinel.
pub const UNSET_INTEGER: i32 = i32::MAX;

/// TWS API double unset sentinel.
pub const UNSET_DOUBLE: f64 = f64::MAX;

/// TWS API long unset sentinel.
pub const UNSET_LONG: i64 = i64::MAX;

/// TWS API's sentinel for an unset decimal field.
pub const UNSET_DECIMAL: Decimal = Decimal::MAX;

/// String representation used by the official client for positive infinity.
pub const INFINITY_STR: &str = "Infinity";

/// Standard tags accepted by `reqAccountSummary`.
pub struct AccountSummaryTags;

impl AccountSummaryTags {
    pub const ACCOUNT_TYPE: &str = "AccountType";
    pub const NET_LIQUIDATION: &str = "NetLiquidation";
    pub const TOTAL_CASH_VALUE: &str = "TotalCashValue";
    pub const SETTLED_CASH: &str = "SettledCash";
    pub const ACCRUED_CASH: &str = "AccruedCash";
    pub const BUYING_POWER: &str = "BuyingPower";
    pub const EQUITY_WITH_LOAN_VALUE: &str = "EquityWithLoanValue";
    pub const PREVIOUS_DAY_EQUITY_WITH_LOAN_VALUE: &str = "PreviousDayEquityWithLoanValue";
    pub const GROSS_POSITION_VALUE: &str = "GrossPositionValue";
    pub const REQ_T_EQUITY: &str = "ReqTEquity";
    pub const REQ_T_MARGIN: &str = "ReqTMargin";
    pub const SMA: &str = "SMA";
    pub const INIT_MARGIN_REQ: &str = "InitMarginReq";
    pub const MAINT_MARGIN_REQ: &str = "MaintMarginReq";
    pub const AVAILABLE_FUNDS: &str = "AvailableFunds";
    pub const EXCESS_LIQUIDITY: &str = "ExcessLiquidity";
    pub const CUSHION: &str = "Cushion";
    pub const FULL_INIT_MARGIN_REQ: &str = "FullInitMarginReq";
    pub const FULL_MAINT_MARGIN_REQ: &str = "FullMaintMarginReq";
    pub const FULL_AVAILABLE_FUNDS: &str = "FullAvailableFunds";
    pub const FULL_EXCESS_LIQUIDITY: &str = "FullExcessLiquidity";
    pub const LOOK_AHEAD_NEXT_CHANGE: &str = "LookAheadNextChange";
    pub const LOOK_AHEAD_INIT_MARGIN_REQ: &str = "LookAheadInitMarginReq";
    pub const LOOK_AHEAD_MAINT_MARGIN_REQ: &str = "LookAheadMaintMarginReq";
    pub const LOOK_AHEAD_AVAILABLE_FUNDS: &str = "LookAheadAvailableFunds";
    pub const LOOK_AHEAD_EXCESS_LIQUIDITY: &str = "LookAheadExcessLiquidity";
    pub const HIGHEST_SEVERITY: &str = "HighestSeverity";
    pub const DAY_TRADES_REMAINING: &str = "DayTradesRemaining";
    pub const LEVERAGE: &str = "Leverage";

    /// All tags in the order used by the official Python client.
    pub const ALL: &str = "AccountType,NetLiquidation,TotalCashValue,SettledCash,AccruedCash,BuyingPower,EquityWithLoanValue,PreviousDayEquityWithLoanValue,GrossPositionValue,ReqTEquity,ReqTMargin,SMA,InitMarginReq,MaintMarginReq,AvailableFunds,ExcessLiquidity,Cushion,FullInitMarginReq,FullMaintMarginReq,FullAvailableFunds,FullExcessLiquidity,LookAheadNextChange,LookAheadInitMarginReq,LookAheadMaintMarginReq,LookAheadAvailableFunds,LookAheadExcessLiquidity,HighestSeverity,DayTradesRemaining,Leverage";
}

/// TWS error code and its standard description prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCode {
    pub code: i32,
    pub message: &'static str,
}

impl ErrorCode {
    pub const FAIL_SEND_REQ_MKT_DATA: Self = Self {
        code: 510,
        message: "Request Market Data Sending Error - ",
    };
    pub const FAIL_SEND_CAN_MKT_DATA: Self = Self {
        code: 511,
        message: "Cancel Market Data Sending Error - ",
    };
    pub const FAIL_SEND_ORDER: Self = Self {
        code: 512,
        message: "Order Sending Error - ",
    };
    pub const FAIL_SEND_CANCEL_ORDER: Self = Self {
        code: 513,
        message: "Cancel Order Sending Error - ",
    };
    pub const FAIL_SEND_REQ_CONTRACT_DATA: Self = Self {
        code: 518,
        message: "Request Contract Data Sending Error - ",
    };
    pub const FAIL_SEND_REQ_HISTORICAL_DATA: Self = Self {
        code: 527,
        message: "Request Historical Data Sending Error - ",
    };
    pub const FAIL_SEND_CAN_HISTORICAL_DATA: Self = Self {
        code: 528,
        message: "Request Historical Data Sending Error - ",
    };
    pub const FAIL_SEND_REQ_REAL_TIME_BARS: Self = Self {
        code: 529,
        message: "Request Real-time Bar Data Sending Error - ",
    };
    pub const FAIL_SEND_CAN_REAL_TIME_BARS: Self = Self {
        code: 530,
        message: "Cancel Real-time Bar Data Sending Error - ",
    };
    pub const FAIL_SEND_REQ_CURRENT_TIME: Self = Self {
        code: 531,
        message: "Request Current Time Sending Error - ",
    };
    pub const FAIL_SEND_REQ_POSITIONS: Self = Self {
        code: 540,
        message: "Request Positions Sending Error - ",
    };
    pub const FAIL_SEND_CAN_POSITIONS: Self = Self {
        code: 541,
        message: "Cancel Positions Sending Error - ",
    };
    pub const FAIL_SEND_REQ_ACCOUNT_DATA: Self = Self {
        code: 542,
        message: "Request Account Data Sending Error - ",
    };
    pub const INVALID_SYMBOL: Self = Self {
        code: 579,
        message: "Invalid symbol in string - ",
    };
    pub const ERROR_ENCODING_PROTOBUF: Self = Self {
        code: 588,
        message: "Error encoding protobuf - ",
    };

    /// Common client-side errors from the official Python error catalog.
    pub const ALL: &[Self] = &[
        Self {
            code: 501,
            message: "Already connected.",
        },
        Self {
            code: 502,
            message: "Couldn't connect to TWS.",
        },
        Self {
            code: 503,
            message: "The TWS is out of date and must be upgraded.",
        },
        Self {
            code: 504,
            message: "Not connected",
        },
        Self {
            code: 505,
            message: "Fatal Error: Unknown message id.",
        },
        Self {
            code: 507,
            message: "Bad message length",
        },
        Self {
            code: 508,
            message: "Bad message",
        },
        Self {
            code: 510,
            message: "Request Market Data Sending Error - ",
        },
        Self {
            code: 511,
            message: "Cancel Market Data Sending Error - ",
        },
        Self {
            code: 512,
            message: "Order Sending Error - ",
        },
        Self {
            code: 513,
            message: "Account Update Request Sending Error - ",
        },
        Self {
            code: 514,
            message: "Request For Executions Sending Error - ",
        },
        Self {
            code: 515,
            message: "Cancel Order Sending Error - ",
        },
        Self {
            code: 516,
            message: "Request Open Order Sending Error - ",
        },
        Self {
            code: 518,
            message: "Request Contract Data Sending Error - ",
        },
        Self {
            code: 519,
            message: "Request Market Depth Sending Error - ",
        },
        Self {
            code: 520,
            message: "Failed to create socket",
        },
        Self {
            code: 521,
            message: "Set Server Log Level Sending Error - ",
        },
        Self {
            code: 522,
            message: "FA Information Request Sending Error - ",
        },
        Self {
            code: 523,
            message: "FA Information Replace Sending Error - ",
        },
        Self {
            code: 524,
            message: "Request Scanner Subscription Sending Error - ",
        },
        Self {
            code: 525,
            message: "Cancel Scanner Subscription Sending Error - ",
        },
        Self {
            code: 526,
            message: "Request Scanner Parameter Sending Error - ",
        },
        Self {
            code: 527,
            message: "Request Historical Data Sending Error - ",
        },
        Self {
            code: 528,
            message: "Request Historical Data Sending Error - ",
        },
        Self {
            code: 529,
            message: "Request Real-time Bar Data Sending Error - ",
        },
        Self {
            code: 530,
            message: "Cancel Real-time Bar Data Sending Error - ",
        },
        Self {
            code: 531,
            message: "Request Current Time Sending Error - ",
        },
        Self {
            code: 534,
            message: "Request Calculate Implied Volatility Sending Error - ",
        },
        Self {
            code: 535,
            message: "Request Calculate Option Price Sending Error - ",
        },
        Self {
            code: 536,
            message: "Cancel Calculate Implied Volatility Sending Error - ",
        },
        Self {
            code: 537,
            message: "Cancel Calculate Option Price Sending Error - ",
        },
        Self {
            code: 538,
            message: "Request Global Cancel Sending Error - ",
        },
        Self {
            code: 539,
            message: "Request Market Data Type Sending Error - ",
        },
        Self {
            code: 540,
            message: "Request Positions Sending Error - ",
        },
        Self {
            code: 541,
            message: "Cancel Positions Sending Error - ",
        },
        Self {
            code: 542,
            message: "Request Account Data Sending Error - ",
        },
        Self {
            code: 543,
            message: "Cancel Account Data Sending Error - ",
        },
        Self {
            code: 544,
            message: "Verify Request Sending Error - ",
        },
        Self {
            code: 545,
            message: "Verify Message Sending Error - ",
        },
        Self {
            code: 546,
            message: "Query Display Groups Sending Error - ",
        },
        Self {
            code: 547,
            message: "Subscribe To Group Events Sending Error - ",
        },
        Self {
            code: 548,
            message: "Update Display Group Sending Error - ",
        },
        Self {
            code: 549,
            message: "Unsubscribe From Group Events Sending Error - ",
        },
        Self {
            code: 550,
            message: "Start API Sending Error - ",
        },
        Self {
            code: 551,
            message: "Verify And Auth Request Sending Error - ",
        },
        Self {
            code: 552,
            message: "Verify And Auth Message Sending Error - ",
        },
        Self {
            code: 553,
            message: "Request Positions Multi Sending Error - ",
        },
        Self {
            code: 554,
            message: "Cancel Positions Multi Sending Error - ",
        },
        Self {
            code: 555,
            message: "Request Account Updates Multi Sending Error - ",
        },
        Self {
            code: 556,
            message: "Cancel Account Updates Multi Sending Error - ",
        },
        Self {
            code: 557,
            message: "Request Security Definition Option Params Sending Error - ",
        },
        Self {
            code: 558,
            message: "Request Soft Dollar Tiers Sending Error - ",
        },
        Self {
            code: 559,
            message: "Request Family Codes Sending Error - ",
        },
        Self {
            code: 560,
            message: "Request Matching Symbols Sending Error - ",
        },
        Self {
            code: 561,
            message: "Request Market Depth Exchanges Sending Error - ",
        },
        Self {
            code: 562,
            message: "Request Smart Components Sending Error - ",
        },
        Self {
            code: 563,
            message: "Request News Providers Sending Error - ",
        },
        Self {
            code: 564,
            message: "Request News Article Sending Error - ",
        },
        Self {
            code: 565,
            message: "Request Historical News Sending Error - ",
        },
        Self {
            code: 566,
            message: "Request Head Time Stamp Sending Error - ",
        },
        Self {
            code: 567,
            message: "Request Histogram Data Sending Error - ",
        },
        Self {
            code: 568,
            message: "Cancel Request Histogram Data Sending Error - ",
        },
        Self {
            code: 569,
            message: "Cancel Head Time Stamp Sending Error - ",
        },
        Self {
            code: 570,
            message: "Request Market Rule Sending Error - ",
        },
        Self {
            code: 571,
            message: "Request PnL Sending Error - ",
        },
        Self {
            code: 572,
            message: "Cancel PnL Sending Error - ",
        },
        Self {
            code: 573,
            message: "Request PnL Single Error - ",
        },
        Self {
            code: 574,
            message: "Cancel PnL Single Sending Error - ",
        },
        Self {
            code: 575,
            message: "Request Historical Ticks Error - ",
        },
        Self {
            code: 576,
            message: "Request Tick-By-Tick Data Sending Error - ",
        },
        Self {
            code: 577,
            message: "Cancel Tick-By-Tick Data Sending Error - ",
        },
        Self {
            code: 578,
            message: "Request Completed Orders Sending Error - ",
        },
        Self {
            code: 579,
            message: "Invalid symbol in string - ",
        },
        Self {
            code: 580,
            message: "Request WSH Meta Data Sending Error - ",
        },
        Self {
            code: 581,
            message: "Cancel WSH Meta Data Sending Error - ",
        },
        Self {
            code: 582,
            message: "Request WSH Event Data Sending Error - ",
        },
        Self {
            code: 583,
            message: "Cancel WSH Event Data Sending Error - ",
        },
        Self {
            code: 584,
            message: "Request User Info Sending Error - ",
        },
        Self {
            code: 585,
            message: "FA Profile is not supported anymore, use FA Group instead - ",
        },
        Self {
            code: 587,
            message: "Request Current Time In Millis Sending Error - ",
        },
        Self {
            code: 588,
            message: "Error encoding protobuf - ",
        },
        Self {
            code: 589,
            message: "Cancel Market Depth Sending Error - ",
        },
        Self {
            code: 590,
            message: "Cancel Contract Data Sending Error - ",
        },
        Self {
            code: 591,
            message: "Cancel Historical Ticks Sending Error - ",
        },
        Self {
            code: 592,
            message: "Request Config Sending Error - ",
        },
        Self {
            code: 593,
            message: "Update Config Request Sending Error - ",
        },
    ];

    pub fn from_code(code: i32) -> Option<Self> {
        Self::ALL.iter().find(|error| error.code == code).copied()
    }
}

/// News bulletin message types.
pub mod news {
    pub const NEWS_MSG: i32 = 1;
    pub const EXCHANGE_AVAIL_MSG: i32 = 2;
    pub const EXCHANGE_UNAVAIL_MSG: i32 = 3;
}
