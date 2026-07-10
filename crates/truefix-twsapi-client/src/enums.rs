//! Semantic enums for values that are represented as integers or strings on the wire.

macro_rules! int_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $value:expr => $label:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+,
            Unknown(i32),
        }

        impl $name {
            /// Converts a wire value without rejecting values added by a newer TWS version.
            pub const fn from_i32(value: i32) -> Self {
                match value {
                    $($value => Self::$variant,)+
                    other => Self::Unknown(other),
                }
            }

            /// Returns the value used by the TWS wire protocol.
            pub const fn as_i32(self) -> i32 {
                match self {
                    $(Self::$variant => $value,)+
                    Self::Unknown(value) => value,
                }
            }

            /// Returns the official symbolic name when known.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)+
                    Self::Unknown(_) => "UNKNOWN",
                }
            }
        }
    };
}

int_enum! {
    /// IB market-data tick type.
    TickType {
        BidSize = 0 => "BID_SIZE",
        Bid = 1 => "BID",
        Ask = 2 => "ASK",
        AskSize = 3 => "ASK_SIZE",
        Last = 4 => "LAST",
        LastSize = 5 => "LAST_SIZE",
        High = 6 => "HIGH",
        Low = 7 => "LOW",
        Volume = 8 => "VOLUME",
        Close = 9 => "CLOSE",
        BidOptionComputation = 10 => "BID_OPTION_COMPUTATION",
        AskOptionComputation = 11 => "ASK_OPTION_COMPUTATION",
        LastOptionComputation = 12 => "LAST_OPTION_COMPUTATION",
        ModelOption = 13 => "MODEL_OPTION",
        Open = 14 => "OPEN",
        Low13Week = 15 => "LOW_13_WEEK",
        High13Week = 16 => "HIGH_13_WEEK",
        Low26Week = 17 => "LOW_26_WEEK",
        High26Week = 18 => "HIGH_26_WEEK",
        Low52Week = 19 => "LOW_52_WEEK",
        High52Week = 20 => "HIGH_52_WEEK",
        AverageVolume = 21 => "AVG_VOLUME",
        OpenInterest = 22 => "OPEN_INTEREST",
        OptionHistoricalVolatility = 23 => "OPTION_HISTORICAL_VOL",
        OptionImpliedVolatility = 24 => "OPTION_IMPLIED_VOL",
        OptionBidExchange = 25 => "OPTION_BID_EXCH",
        OptionAskExchange = 26 => "OPTION_ASK_EXCH",
        OptionCallOpenInterest = 27 => "OPTION_CALL_OPEN_INTEREST",
        OptionPutOpenInterest = 28 => "OPTION_PUT_OPEN_INTEREST",
        OptionCallVolume = 29 => "OPTION_CALL_VOLUME",
        OptionPutVolume = 30 => "OPTION_PUT_VOLUME",
        IndexFuturePremium = 31 => "INDEX_FUTURE_PREMIUM",
        BidExchange = 32 => "BID_EXCH",
        AskExchange = 33 => "ASK_EXCH",
        AuctionVolume = 34 => "AUCTION_VOLUME",
        AuctionPrice = 35 => "AUCTION_PRICE",
        AuctionImbalance = 36 => "AUCTION_IMBALANCE",
        MarkPrice = 37 => "MARK_PRICE",
        BidEfpComputation = 38 => "BID_EFP_COMPUTATION",
        AskEfpComputation = 39 => "ASK_EFP_COMPUTATION",
        LastEfpComputation = 40 => "LAST_EFP_COMPUTATION",
        OpenEfpComputation = 41 => "OPEN_EFP_COMPUTATION",
        HighEfpComputation = 42 => "HIGH_EFP_COMPUTATION",
        LowEfpComputation = 43 => "LOW_EFP_COMPUTATION",
        CloseEfpComputation = 44 => "CLOSE_EFP_COMPUTATION",
        LastTimestamp = 45 => "LAST_TIMESTAMP",
        Shortable = 46 => "SHORTABLE",
        NotUsed = 47 => "NOT_USED",
        RealTimeVolume = 48 => "RT_VOLUME",
        Halted = 49 => "HALTED",
        BidYield = 50 => "BID_YIELD",
        AskYield = 51 => "ASK_YIELD",
        LastYield = 52 => "LAST_YIELD",
        CustomerOptionComputation = 53 => "CUST_OPTION_COMPUTATION",
        TradeCount = 54 => "TRADE_COUNT",
        TradeRate = 55 => "TRADE_RATE",
        VolumeRate = 56 => "VOLUME_RATE",
        LastRthTrade = 57 => "LAST_RTH_TRADE",
        RealTimeHistoricalVolatility = 58 => "RT_HISTORICAL_VOL",
        IbDividends = 59 => "IB_DIVIDENDS",
        BondFactorMultiplier = 60 => "BOND_FACTOR_MULTIPLIER",
        RegulatoryImbalance = 61 => "REGULATORY_IMBALANCE",
        NewsTick = 62 => "NEWS_TICK",
        ShortTermVolume3Min = 63 => "SHORT_TERM_VOLUME_3_MIN",
        ShortTermVolume5Min = 64 => "SHORT_TERM_VOLUME_5_MIN",
        ShortTermVolume10Min = 65 => "SHORT_TERM_VOLUME_10_MIN",
        DelayedBid = 66 => "DELAYED_BID",
        DelayedAsk = 67 => "DELAYED_ASK",
        DelayedLast = 68 => "DELAYED_LAST",
        DelayedBidSize = 69 => "DELAYED_BID_SIZE",
        DelayedAskSize = 70 => "DELAYED_ASK_SIZE",
        DelayedLastSize = 71 => "DELAYED_LAST_SIZE",
        DelayedHigh = 72 => "DELAYED_HIGH",
        DelayedLow = 73 => "DELAYED_LOW",
        DelayedVolume = 74 => "DELAYED_VOLUME",
        DelayedClose = 75 => "DELAYED_CLOSE",
        DelayedOpen = 76 => "DELAYED_OPEN",
        RealTimeTradeVolume = 77 => "RT_TRD_VOLUME",
        CreditmanMarkPrice = 78 => "CREDITMAN_MARK_PRICE",
        CreditmanSlowMarkPrice = 79 => "CREDITMAN_SLOW_MARK_PRICE",
        DelayedBidOption = 80 => "DELAYED_BID_OPTION",
        DelayedAskOption = 81 => "DELAYED_ASK_OPTION",
        DelayedLastOption = 82 => "DELAYED_LAST_OPTION",
        DelayedModelOption = 83 => "DELAYED_MODEL_OPTION",
        LastExchange = 84 => "LAST_EXCH",
        LastRegulatoryTime = 85 => "LAST_REG_TIME",
        FuturesOpenInterest = 86 => "FUTURES_OPEN_INTEREST",
        AverageOptionVolume = 87 => "AVG_OPT_VOLUME",
        DelayedLastTimestamp = 88 => "DELAYED_LAST_TIMESTAMP",
        ShortableShares = 89 => "SHORTABLE_SHARES",
        DelayedHalted = 90 => "DELAYED_HALTED",
        Reuters2MutualFunds = 91 => "REUTERS_2_MUTUAL_FUNDS",
        EtfNavClose = 92 => "ETF_NAV_CLOSE",
        EtfNavPriorClose = 93 => "ETF_NAV_PRIOR_CLOSE",
        EtfNavBid = 94 => "ETF_NAV_BID",
        EtfNavAsk = 95 => "ETF_NAV_ASK",
        EtfNavLast = 96 => "ETF_NAV_LAST",
        EtfFrozenNavLast = 97 => "ETF_FROZEN_NAV_LAST",
        EtfNavHigh = 98 => "ETF_NAV_HIGH",
        EtfNavLow = 99 => "ETF_NAV_LOW",
        SocialMarketAnalytics = 100 => "SOCIAL_MARKET_ANALYTICS",
        EstimatedIpoMidpoint = 101 => "ESTIMATED_IPO_MIDPOINT",
        FinalIpoLast = 102 => "FINAL_IPO_LAST",
        DelayedYieldBid = 103 => "DELAYED_YIELD_BID",
        DelayedYieldAsk = 104 => "DELAYED_YIELD_ASK",
        OddLotBid = 105 => "ODD_LOT_BID",
        OddLotAsk = 106 => "ODD_LOT_ASK",
        OddLotBidSize = 107 => "ODD_LOT_BID_SIZE",
        OddLotAskSize = 108 => "ODD_LOT_ASK_SIZE",
        OddLotBidExchange = 109 => "ODD_LOT_BID_EXCH",
        OddLotAskExchange = 110 => "ODD_LOT_ASK_EXCH",
        NotSet = 111 => "NOT_SET"
    }
}

int_enum! {
    /// Requested market-data mode.
    MarketDataType {
        RealTime = 1 => "REALTIME",
        Frozen = 2 => "FROZEN",
        Delayed = 3 => "DELAYED",
        DelayedFrozen = 4 => "DELAYED_FROZEN"
    }
}

int_enum! {
    /// Financial Advisor XML data type.
    FaDataType {
        Groups = 1 => "GROUPS",
        Aliases = 3 => "ALIASES"
    }
}

int_enum! {
    /// Liquidity classification reported with executions.
    Liquidities {
        None = 0 => "None",
        Added = 1 => "Added",
        Remove = 2 => "Remove",
        RoundedOut = 3 => "RoundedOut"
    }
}

int_enum! {
    /// Option exercise/lapse result.
    OptionExerciseType {
        None = -1 => "None",
        Exercise = 1 => "Exercise",
        Lapse = 2 => "Lapse",
        DoNothing = 3 => "DoNothing",
        Assigned = 100 => "Assigned",
        AutoexerciseClearing = 101 => "AutoexerciseClearing",
        Expired = 102 => "Expired",
        Netting = 103 => "Netting",
        AutoexerciseTrading = 200 => "AutoexerciseTrading"
    }
}

int_enum! {
    /// Price-condition trigger method.
    TriggerMethod {
        Default = 0 => "Default",
        DoubleBidAsk = 1 => "DoubleBidAsk",
        Last = 2 => "Last",
        DoubleLast = 3 => "DoubleLast",
        BidAsk = 4 => "BidAsk",
        LastBidAsk = 7 => "LastBidAsk",
        MidPoint = 8 => "MidPoint"
    }
}

/// Fund classification values use string codes in the TWS protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FundAssetType {
    None,
    Others,
    MoneyMarket,
    FixedIncome,
    MultiAsset,
    Equity,
    Sector,
    Guaranteed,
    Alternative,
    Unknown,
}

impl FundAssetType {
    pub const fn code(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Others => "000",
            Self::MoneyMarket => "001",
            Self::FixedIncome => "002",
            Self::MultiAsset => "003",
            Self::Equity => "004",
            Self::Sector => "005",
            Self::Guaranteed => "006",
            Self::Alternative => "007",
            Self::Unknown => "",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "None" => Self::None,
            "000" => Self::Others,
            "001" => Self::MoneyMarket,
            "002" => Self::FixedIncome,
            "003" => Self::MultiAsset,
            "004" => Self::Equity,
            "005" => Self::Sector,
            "006" => Self::Guaranteed,
            "007" => Self::Alternative,
            _ => Self::Unknown,
        }
    }
}

/// Fund distribution policy indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FundDistributionPolicyIndicator {
    None,
    AccumulationFund,
    IncomeFund,
    Unknown,
}

impl FundDistributionPolicyIndicator {
    pub const fn code(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::AccumulationFund => "N",
            Self::IncomeFund => "Y",
            Self::Unknown => "",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "None" => Self::None,
            "N" => Self::AccumulationFund,
            "Y" => Self::IncomeFund,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_wire_values_are_preserved() {
        assert_eq!(TickType::from_i32(999).as_i32(), 999);
        assert_eq!(MarketDataType::from_i32(3), MarketDataType::Delayed);
    }

    #[test]
    fn string_enum_codes_round_trip() {
        assert_eq!(FundAssetType::from_code("004"), FundAssetType::Equity);
        assert_eq!(FundDistributionPolicyIndicator::IncomeFund.code(), "Y");
    }
}
