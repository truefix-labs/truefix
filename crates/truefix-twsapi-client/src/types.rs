use rust_decimal::Decimal;

use crate::constants::{UNSET_DOUBLE, UNSET_INTEGER};
use crate::enums::{OptionExerciseType, TriggerMethod};

/// Market data ticker id.
pub type TickerId = i32;
/// TWS order id.
pub type OrderId = i32;
/// Financial-advisor data type.
pub type FaDataType = i32;

/// A generic tag/value pair.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagValue {
    /// Tag name.
    pub tag: String,
    /// Tag value.
    pub value: String,
}

/// Soft-dollar tier metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoftDollarTier {
    /// Tier name.
    pub name: String,
    /// Tier value.
    pub value: String,
    /// Display name.
    pub display_name: String,
}

/// A reason why a contract cannot be traded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IneligibilityReason {
    pub id: String,
    pub description: String,
}

/// A market-depth exchange description.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DepthMarketDataDescription {
    pub exchange: String,
    pub security_type: String,
    pub listing_exchange: String,
    pub service_data_type: String,
    pub aggregate_group: i32,
}

/// Price increment for a market rule.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PriceIncrement {
    pub low_edge: f64,
    pub increment: f64,
}

/// Account family-code mapping.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FamilyCode {
    pub account_id: String,
    pub family_code: String,
}

/// News provider metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewsProvider {
    pub code: String,
    pub name: String,
}

/// Structured WSH event payload. The original JSON is retained for fields added by IB later.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WshEventData {
    pub req_id: i32,
    pub con_id: i32,
    pub filter: String,
    pub fill_watchlist: bool,
    pub fill_portfolio: bool,
    pub fill_position: bool,
    pub fill_account: bool,
    pub data_json: String,
}

impl WshEventData {
    /// Parses the standard WSH JSON fields while retaining the original payload.
    pub fn from_json(req_id: i32, data_json: impl Into<String>) -> Self {
        let data_json = data_json.into();
        #[derive(serde::Deserialize, Default)]
        struct Raw {
            #[serde(rename = "conId", default)]
            con_id: i32,
            #[serde(default)]
            filter: String,
            #[serde(rename = "fillWatchlist", default)]
            fill_watchlist: bool,
            #[serde(rename = "fillPortfolio", default)]
            fill_portfolio: bool,
            #[serde(rename = "fillPosition", default)]
            fill_position: bool,
            #[serde(rename = "fillAccount", default)]
            fill_account: bool,
        }
        let raw = serde_json::from_str::<Raw>(&data_json).unwrap_or_default();
        Self {
            req_id,
            con_id: raw.con_id,
            filter: raw.filter,
            fill_watchlist: raw.fill_watchlist,
            fill_portfolio: raw.fill_portfolio,
            fill_position: raw.fill_position,
            fill_account: raw.fill_account,
            data_json,
        }
    }
}

/// Combo leg open/close value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum LegOpenClose {
    /// Same as combo.
    #[default]
    SamePosition = 0,
    /// Open position.
    OpenPosition = 1,
    /// Close position.
    ClosePosition = 2,
    /// Unknown.
    Unknown = 3,
}

/// Contract combo leg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboLeg {
    /// Contract id.
    pub con_id: i32,
    /// Leg ratio.
    pub ratio: i32,
    /// BUY/SELL/SHORT.
    pub action: String,
    /// Exchange.
    pub exchange: String,
    /// Open/close marker.
    pub open_close: LegOpenClose,
    /// Short-sale slot.
    pub short_sale_slot: i32,
    /// Designated location.
    pub designated_location: String,
    /// Exempt code.
    pub exempt_code: i32,
}

impl Default for ComboLeg {
    fn default() -> Self {
        Self {
            con_id: 0,
            ratio: 0,
            action: String::new(),
            exchange: String::new(),
            open_close: LegOpenClose::SamePosition,
            short_sale_slot: 0,
            designated_location: String::new(),
            exempt_code: -1,
        }
    }
}

/// Delta-neutral contract.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeltaNeutralContract {
    /// Contract id.
    pub con_id: i32,
    /// Delta.
    pub delta: f64,
    /// Price.
    pub price: f64,
}

/// TWS contract.
#[derive(Debug, Clone, PartialEq)]
pub struct Contract {
    /// Contract id.
    pub con_id: i32,
    /// Symbol.
    pub symbol: String,
    /// Security type.
    pub sec_type: String,
    /// Last trade date or contract month.
    pub last_trade_date_or_contract_month: String,
    /// Last trade date.
    pub last_trade_date: String,
    /// Strike.
    pub strike: f64,
    /// Right.
    pub right: String,
    /// Multiplier.
    pub multiplier: String,
    /// Exchange.
    pub exchange: String,
    /// Primary exchange.
    pub primary_exchange: String,
    /// Currency.
    pub currency: String,
    /// Local symbol.
    pub local_symbol: String,
    /// Trading class.
    pub trading_class: String,
    /// Include expired contracts.
    pub include_expired: bool,
    /// Security id type.
    pub sec_id_type: String,
    /// Security id.
    pub sec_id: String,
    /// Description.
    pub description: String,
    /// Issuer id.
    pub issuer_id: String,
    /// Combo legs description.
    pub combo_legs_description: String,
    /// Combo legs.
    pub combo_legs: Vec<ComboLeg>,
    /// Optional delta-neutral contract.
    pub delta_neutral_contract: Option<DeltaNeutralContract>,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            con_id: 0,
            symbol: String::new(),
            sec_type: String::new(),
            last_trade_date_or_contract_month: String::new(),
            last_trade_date: String::new(),
            strike: UNSET_DOUBLE,
            right: String::new(),
            multiplier: String::new(),
            exchange: String::new(),
            primary_exchange: String::new(),
            currency: String::new(),
            local_symbol: String::new(),
            trading_class: String::new(),
            include_expired: false,
            sec_id_type: String::new(),
            sec_id: String::new(),
            description: String::new(),
            issuer_id: String::new(),
            combo_legs_description: String::new(),
            combo_legs: Vec::new(),
            delta_neutral_contract: None,
        }
    }
}

/// Contract details returned by TWS.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContractDetails {
    /// Contract.
    pub contract: Contract,
    /// Market name.
    pub market_name: String,
    /// Minimum tick.
    pub min_tick: f64,
    /// Supported order types.
    pub order_types: String,
    /// Valid exchanges.
    pub valid_exchanges: String,
    /// Price magnifier.
    pub price_magnifier: i32,
    /// Underlying conId.
    pub under_con_id: i32,
    /// Long name.
    pub long_name: String,
    /// Contract month.
    pub contract_month: String,
    /// Industry.
    pub industry: String,
    /// Category.
    pub category: String,
    /// Subcategory.
    pub subcategory: String,
    /// Time zone id.
    pub time_zone_id: String,
    /// Trading hours.
    pub trading_hours: String,
    /// Liquid hours.
    pub liquid_hours: String,
    /// Market rule ids.
    pub market_rule_ids: String,
    /// CUSIP.
    pub cusip: String,
    /// Issue date.
    pub issue_date: String,
    /// Ratings.
    pub ratings: String,
    /// Bond type.
    pub bond_type: String,
    /// Coupon.
    pub coupon: f64,
    /// Coupon type.
    pub coupon_type: String,
    /// Convertible flag.
    pub convertible: bool,
    /// Callable flag.
    pub callable: bool,
    /// Puttable flag.
    pub puttable: bool,
    /// Description append.
    pub desc_append: String,
    /// Next option date.
    pub next_option_date: String,
    /// Next option type.
    pub next_option_type: String,
    /// Next option partial flag.
    pub next_option_partial: bool,
    /// Bond notes.
    pub bond_notes: String,
    /// Real expiration date.
    pub real_expiration_date: String,
    /// Stock type.
    pub stock_type: String,
    /// Minimum order size.
    pub min_size: Decimal,
    /// Order-size increment.
    pub size_increment: Decimal,
    /// Suggested order-size increment.
    pub suggested_size_increment: Decimal,
    /// Fund name.
    pub fund_name: String,
    /// Fund family.
    pub fund_family: String,
    /// Fund type code.
    pub fund_type: String,
    /// Fund front load.
    pub fund_front_load: String,
    /// Fund back load.
    pub fund_back_load: String,
    /// Fund back-load interval.
    pub fund_back_load_time_interval: String,
    /// Fund management fee.
    pub fund_management_fee: String,
    /// Whether the fund is closed.
    pub fund_closed: bool,
    /// Whether the fund is closed to new investors.
    pub fund_closed_for_new_investors: bool,
    /// Whether the fund is closed to new money.
    pub fund_closed_for_new_money: bool,
    /// Fund notification amount.
    pub fund_notify_amount: String,
    /// Minimum initial purchase.
    pub fund_minimum_initial_purchase: String,
    /// Minimum subsequent purchase.
    pub fund_minimum_subsequent_purchase: String,
    /// Blue-sky states.
    pub fund_blue_sky_states: String,
    /// Blue-sky territories.
    pub fund_blue_sky_territories: String,
    /// Distribution policy indicator.
    pub fund_distribution_policy_indicator: String,
    /// Asset type.
    pub fund_asset_type: String,
    /// Ineligibility reasons.
    pub ineligibility_reason_list: Vec<IneligibilityReason>,
    /// First event contract.
    pub event_contract1: String,
    /// First event contract description.
    pub event_contract_description1: String,
    /// Second event contract description.
    pub event_contract_description2: String,
    /// Minimum algorithmic order size.
    pub min_algo_size: Decimal,
    /// Last-price precision.
    pub last_price_precision: Decimal,
    /// Last-size precision.
    pub last_size_precision: Decimal,
}

/// Order combo leg.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderComboLeg {
    /// Leg price.
    pub price: f64,
}

impl Default for OrderComboLeg {
    fn default() -> Self {
        Self {
            price: UNSET_DOUBLE,
        }
    }
}

/// Order origin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum Origin {
    /// Customer order.
    #[default]
    Customer = 0,
    /// Firm order.
    Firm = 1,
    /// Unknown origin.
    Unknown = 2,
}

/// Auction strategy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum AuctionStrategy {
    /// Unset.
    #[default]
    Unset = 0,
    /// Match.
    Match = 1,
    /// Improvement.
    Improvement = 2,
    /// Transparent.
    Transparent = 3,
}

/// TWS order.
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    /// Soft-dollar tier.
    pub soft_dollar_tier: SoftDollarTier,
    /// Order id.
    pub order_id: i32,
    /// Client id.
    pub client_id: i32,
    /// Permanent id.
    pub perm_id: i64,
    /// BUY/SELL.
    pub action: String,
    /// Total quantity.
    pub total_quantity: Decimal,
    /// Order type.
    pub order_type: String,
    /// Limit price.
    pub limit_price: f64,
    /// Auxiliary price.
    pub aux_price: f64,
    /// Time in force.
    pub tif: String,
    /// Active start time.
    pub active_start_time: String,
    /// Active stop time.
    pub active_stop_time: String,
    /// OCA group.
    pub oca_group: String,
    /// OCA type.
    pub oca_type: i32,
    /// Order ref.
    pub order_ref: String,
    /// Transmit flag.
    pub transmit: bool,
    /// Parent order id.
    pub parent_id: i32,
    /// Block order.
    pub block_order: bool,
    /// Sweep to fill.
    pub sweep_to_fill: bool,
    /// Display size.
    pub display_size: i32,
    /// Trigger method.
    pub trigger_method: i32,
    /// Outside regular trading hours.
    pub outside_rth: bool,
    /// Hidden order.
    pub hidden: bool,
    /// Good-after time.
    pub good_after_time: String,
    /// Good-till date.
    pub good_till_date: String,
    /// Rule 80A.
    pub rule80a: String,
    /// All-or-none flag.
    pub all_or_none: bool,
    /// Minimum quantity.
    pub min_qty: i32,
    /// Percent offset.
    pub percent_offset: f64,
    /// Override percentage constraints.
    pub override_percentage_constraints: bool,
    /// Trail stop price.
    pub trail_stop_price: f64,
    /// Trailing percent.
    pub trailing_percent: f64,
    /// Financial-advisor group.
    pub fa_group: String,
    /// Financial-advisor method.
    pub fa_method: String,
    /// Financial-advisor percentage.
    pub fa_percentage: String,
    /// Short-sale designated location.
    pub designated_location: String,
    /// Open/close marker.
    pub open_close: String,
    /// Order origin.
    pub origin: Origin,
    /// Short-sale slot.
    pub short_sale_slot: i32,
    /// Exempt code.
    pub exempt_code: i32,
    /// Discretionary amount.
    pub discretionary_amount: f64,
    /// Opt out of SMART routing.
    pub opt_out_smart_routing: bool,
    /// Auction strategy.
    pub auction_strategy: AuctionStrategy,
    /// Starting price.
    pub starting_price: f64,
    /// Stock reference price.
    pub stock_ref_price: f64,
    /// Delta.
    pub delta: f64,
    /// Stock range lower.
    pub stock_range_lower: f64,
    /// Stock range upper.
    pub stock_range_upper: f64,
    /// Randomize price.
    pub randomize_price: bool,
    /// Randomize size.
    pub randomize_size: bool,
    /// Volatility.
    pub volatility: f64,
    /// Volatility type.
    pub volatility_type: i32,
    /// Delta-neutral order type.
    pub delta_neutral_order_type: String,
    /// Delta-neutral auxiliary price.
    pub delta_neutral_aux_price: f64,
    /// Delta-neutral contract id.
    pub delta_neutral_con_id: i32,
    /// Delta-neutral settling firm.
    pub delta_neutral_settling_firm: String,
    /// Delta-neutral clearing account.
    pub delta_neutral_clearing_account: String,
    /// Delta-neutral clearing intent.
    pub delta_neutral_clearing_intent: String,
    /// Delta-neutral open/close marker.
    pub delta_neutral_open_close: String,
    /// Delta-neutral short sale flag.
    pub delta_neutral_short_sale: bool,
    /// Delta-neutral short-sale slot.
    pub delta_neutral_short_sale_slot: i32,
    /// Delta-neutral designated location.
    pub delta_neutral_designated_location: String,
    /// Continuous update flag.
    pub continuous_update: bool,
    /// Reference price type.
    pub reference_price_type: i32,
    /// Combo basis points.
    pub basis_points: f64,
    /// Combo basis-points type.
    pub basis_points_type: i32,
    /// Initial scale level size.
    pub scale_init_level_size: i32,
    /// Subsequent scale level size.
    pub scale_subs_level_size: i32,
    /// Scale price increment.
    pub scale_price_increment: f64,
    /// Scale price adjust value.
    pub scale_price_adjust_value: f64,
    /// Scale price adjust interval.
    pub scale_price_adjust_interval: i32,
    /// Scale profit offset.
    pub scale_profit_offset: f64,
    /// Scale auto-reset flag.
    pub scale_auto_reset: bool,
    /// Scale initial position.
    pub scale_init_position: i32,
    /// Scale initial fill quantity.
    pub scale_init_fill_qty: i32,
    /// Scale random percent flag.
    pub scale_random_percent: bool,
    /// Scale table.
    pub scale_table: String,
    /// Hedge type.
    pub hedge_type: String,
    /// Hedge parameter.
    pub hedge_param: String,
    /// Hedge maximum size.
    pub hedge_max_size: i32,
    /// Account.
    pub account: String,
    /// Settling firm.
    pub settling_firm: String,
    /// Clearing account.
    pub clearing_account: String,
    /// Clearing intent.
    pub clearing_intent: String,
    /// Model code.
    pub model_code: String,
    /// Algo strategy.
    pub algo_strategy: String,
    /// Algo parameters.
    pub algo_params: Vec<TagValue>,
    /// Smart combo routing parameters.
    pub smart_combo_routing_params: Vec<TagValue>,
    /// Algo id.
    pub algo_id: String,
    /// What-if flag.
    pub what_if: bool,
    /// Not-held flag.
    pub not_held: bool,
    /// Solicited flag.
    pub solicited: bool,
    /// Order combo legs.
    pub order_combo_legs: Vec<OrderComboLeg>,
    /// Misc options.
    pub order_misc_options: Vec<TagValue>,
    /// Reference contract id.
    pub reference_contract_id: i32,
    /// Pegged change amount.
    pub pegged_change_amount: f64,
    /// Pegged change amount decrease flag.
    pub is_pegged_change_amount_decrease: bool,
    /// Reference change amount.
    pub reference_change_amount: f64,
    /// Reference exchange id.
    pub reference_exchange_id: String,
    /// Adjusted order type.
    pub adjusted_order_type: String,
    /// Trigger price.
    pub trigger_price: f64,
    /// Adjusted stop price.
    pub adjusted_stop_price: f64,
    /// Adjusted stop limit price.
    pub adjusted_stop_limit_price: f64,
    /// Adjusted trailing amount.
    pub adjusted_trailing_amount: f64,
    /// Adjustable trailing unit.
    pub adjustable_trailing_unit: i32,
    /// Limit price offset.
    pub limit_price_offset: f64,
    /// Conditions.
    pub conditions: Vec<OrderCondition>,
    /// Cancel if conditions match.
    pub conditions_cancel_order: bool,
    /// Ignore RTH for conditions.
    pub conditions_ignore_rth: bool,
    /// External operator.
    pub ext_operator: String,
    /// Native cash quantity.
    pub cash_qty: f64,
    /// MiFID II decision maker.
    pub mifid2_decision_maker: String,
    /// MiFID II decision algo.
    pub mifid2_decision_algo: String,
    /// MiFID II execution trader.
    pub mifid2_execution_trader: String,
    /// MiFID II execution algo.
    pub mifid2_execution_algo: String,
    /// Do not use auto price for hedge.
    pub dont_use_auto_price_for_hedge: bool,
    /// OMS container flag.
    pub is_oms_container: bool,
    /// Discretionary up-to-limit price flag.
    pub discretionary_up_to_limit_price: bool,
    /// Auto-cancel date.
    pub auto_cancel_date: String,
    /// Filled quantity.
    pub filled_quantity: Decimal,
    /// Reference futures contract id.
    pub ref_futures_con_id: i32,
    /// Auto-cancel parent flag.
    pub auto_cancel_parent: bool,
    /// Shareholder.
    pub shareholder: String,
    /// Imbalance only flag.
    pub imbalance_only: bool,
    /// Route marketable order to BBO.
    pub route_marketable_to_bbo: i32,
    /// Parent permanent id.
    pub parent_perm_id: i64,
    /// Use price-management algo.
    pub use_price_mgmt_algo: i32,
    /// Duration.
    pub duration: i32,
    /// Post to ATS.
    pub post_to_ats: i32,
    /// Advanced error override.
    pub advanced_error_override: String,
    /// Manual order time.
    pub manual_order_time: String,
    /// Minimum trade quantity.
    pub min_trade_qty: i32,
    /// Minimum compete size.
    pub min_compete_size: i32,
    /// Compete against best offset.
    pub compete_against_best_offset: f64,
    /// Mid offset at whole.
    pub mid_offset_at_whole: f64,
    /// Mid offset at half.
    pub mid_offset_at_half: f64,
    /// Customer account.
    pub customer_account: String,
    /// Professional customer flag.
    pub professional_customer: bool,
    /// Bond accrued interest.
    pub bond_accrued_interest: String,
    /// Include overnight flag.
    pub include_overnight: bool,
    /// Manual order indicator.
    pub manual_order_indicator: i32,
    /// Submitter.
    pub submitter: String,
    /// Post-only flag.
    pub post_only: bool,
    /// Allow pre-open flag.
    pub allow_pre_open: bool,
    /// Ignore open auction flag.
    pub ignore_open_auction: bool,
    /// Deactivate flag.
    pub deactivate: bool,
    /// Seek price improvement.
    pub seek_price_improvement: i32,
    /// What-if type.
    pub what_if_type: i32,
    /// Stop-loss attached order id.
    pub stop_loss_order_id: i32,
    /// Stop-loss attached order type.
    pub stop_loss_order_type: String,
    /// Profit-taker attached order id.
    pub profit_taker_order_id: i32,
    /// Profit-taker attached order type.
    pub profit_taker_order_type: String,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            soft_dollar_tier: SoftDollarTier::default(),
            order_id: 0,
            client_id: 0,
            perm_id: 0,
            action: String::new(),
            total_quantity: Decimal::from_i128_with_scale(0, 0),
            order_type: String::new(),
            limit_price: UNSET_DOUBLE,
            aux_price: UNSET_DOUBLE,
            tif: String::new(),
            active_start_time: String::new(),
            active_stop_time: String::new(),
            oca_group: String::new(),
            oca_type: 0,
            order_ref: String::new(),
            transmit: true,
            parent_id: 0,
            block_order: false,
            sweep_to_fill: false,
            display_size: 0,
            trigger_method: 0,
            outside_rth: false,
            hidden: false,
            good_after_time: String::new(),
            good_till_date: String::new(),
            rule80a: String::new(),
            all_or_none: false,
            min_qty: UNSET_INTEGER,
            percent_offset: UNSET_DOUBLE,
            override_percentage_constraints: false,
            trail_stop_price: UNSET_DOUBLE,
            trailing_percent: UNSET_DOUBLE,
            fa_group: String::new(),
            fa_method: String::new(),
            fa_percentage: String::new(),
            designated_location: String::new(),
            open_close: String::new(),
            origin: Origin::Customer,
            short_sale_slot: 0,
            exempt_code: -1,
            discretionary_amount: 0.0,
            opt_out_smart_routing: false,
            auction_strategy: AuctionStrategy::Unset,
            starting_price: UNSET_DOUBLE,
            stock_ref_price: UNSET_DOUBLE,
            delta: UNSET_DOUBLE,
            stock_range_lower: UNSET_DOUBLE,
            stock_range_upper: UNSET_DOUBLE,
            randomize_price: false,
            randomize_size: false,
            volatility: UNSET_DOUBLE,
            volatility_type: UNSET_INTEGER,
            delta_neutral_order_type: String::new(),
            delta_neutral_aux_price: UNSET_DOUBLE,
            delta_neutral_con_id: 0,
            delta_neutral_settling_firm: String::new(),
            delta_neutral_clearing_account: String::new(),
            delta_neutral_clearing_intent: String::new(),
            delta_neutral_open_close: String::new(),
            delta_neutral_short_sale: false,
            delta_neutral_short_sale_slot: 0,
            delta_neutral_designated_location: String::new(),
            continuous_update: false,
            reference_price_type: UNSET_INTEGER,
            basis_points: UNSET_DOUBLE,
            basis_points_type: UNSET_INTEGER,
            scale_init_level_size: UNSET_INTEGER,
            scale_subs_level_size: UNSET_INTEGER,
            scale_price_increment: UNSET_DOUBLE,
            scale_price_adjust_value: UNSET_DOUBLE,
            scale_price_adjust_interval: UNSET_INTEGER,
            scale_profit_offset: UNSET_DOUBLE,
            scale_auto_reset: false,
            scale_init_position: UNSET_INTEGER,
            scale_init_fill_qty: UNSET_INTEGER,
            scale_random_percent: false,
            scale_table: String::new(),
            hedge_type: String::new(),
            hedge_param: String::new(),
            hedge_max_size: UNSET_INTEGER,
            account: String::new(),
            settling_firm: String::new(),
            clearing_account: String::new(),
            clearing_intent: String::new(),
            model_code: String::new(),
            algo_strategy: String::new(),
            algo_params: Vec::new(),
            smart_combo_routing_params: Vec::new(),
            algo_id: String::new(),
            what_if: false,
            not_held: false,
            solicited: false,
            order_combo_legs: Vec::new(),
            order_misc_options: Vec::new(),
            reference_contract_id: 0,
            pegged_change_amount: 0.0,
            is_pegged_change_amount_decrease: false,
            reference_change_amount: 0.0,
            reference_exchange_id: String::new(),
            adjusted_order_type: String::new(),
            trigger_price: UNSET_DOUBLE,
            adjusted_stop_price: UNSET_DOUBLE,
            adjusted_stop_limit_price: UNSET_DOUBLE,
            adjusted_trailing_amount: UNSET_DOUBLE,
            adjustable_trailing_unit: 0,
            limit_price_offset: UNSET_DOUBLE,
            conditions: Vec::new(),
            conditions_cancel_order: false,
            conditions_ignore_rth: false,
            ext_operator: String::new(),
            cash_qty: UNSET_DOUBLE,
            mifid2_decision_maker: String::new(),
            mifid2_decision_algo: String::new(),
            mifid2_execution_trader: String::new(),
            mifid2_execution_algo: String::new(),
            dont_use_auto_price_for_hedge: false,
            is_oms_container: false,
            discretionary_up_to_limit_price: false,
            auto_cancel_date: String::new(),
            filled_quantity: Decimal::from_i128_with_scale(0, 0),
            ref_futures_con_id: 0,
            auto_cancel_parent: false,
            shareholder: String::new(),
            imbalance_only: false,
            route_marketable_to_bbo: UNSET_INTEGER,
            parent_perm_id: 0,
            use_price_mgmt_algo: UNSET_INTEGER,
            duration: UNSET_INTEGER,
            post_to_ats: UNSET_INTEGER,
            advanced_error_override: String::new(),
            manual_order_time: String::new(),
            min_trade_qty: UNSET_INTEGER,
            min_compete_size: UNSET_INTEGER,
            compete_against_best_offset: UNSET_DOUBLE,
            mid_offset_at_whole: UNSET_DOUBLE,
            mid_offset_at_half: UNSET_DOUBLE,
            customer_account: String::new(),
            professional_customer: false,
            bond_accrued_interest: String::new(),
            include_overnight: false,
            manual_order_indicator: UNSET_INTEGER,
            submitter: String::new(),
            post_only: false,
            allow_pre_open: false,
            ignore_open_auction: false,
            deactivate: false,
            seek_price_improvement: UNSET_INTEGER,
            what_if_type: UNSET_INTEGER,
            stop_loss_order_id: UNSET_INTEGER,
            stop_loss_order_type: String::new(),
            profit_taker_order_id: UNSET_INTEGER,
            profit_taker_order_type: String::new(),
        }
    }
}

/// Order cancellation metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderCancel {
    /// Manual order cancel time.
    pub manual_order_cancel_time: String,
    /// Ext operator.
    pub ext_operator: String,
    /// Manual order indicator.
    pub manual_order_indicator: i32,
}

/// Execution filter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionFilter {
    /// Client id.
    pub client_id: i32,
    /// Account code.
    pub acct_code: String,
    /// Time.
    pub time: String,
    /// Symbol.
    pub symbol: String,
    /// Security type.
    pub sec_type: String,
    /// Exchange.
    pub exchange: String,
    /// Side.
    pub side: String,
}

/// Execution details returned by TWS.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Execution {
    /// Order id.
    pub order_id: i32,
    /// Execution id.
    pub exec_id: String,
    /// Execution time.
    pub time: String,
    /// Account number.
    pub acct_number: String,
    /// Exchange.
    pub exchange: String,
    /// Side.
    pub side: String,
    /// Executed quantity.
    pub shares: Decimal,
    /// Execution price.
    pub price: f64,
    /// Permanent id.
    pub perm_id: i64,
    /// Client id.
    pub client_id: i32,
    /// Liquidation marker.
    pub liquidation: i32,
    /// Cumulative quantity.
    pub cum_qty: Decimal,
    /// Average price.
    pub avg_price: f64,
    /// Order reference.
    pub order_ref: String,
    /// Economic value rule.
    pub ev_rule: String,
    /// Economic value multiplier.
    pub ev_multiplier: f64,
    /// Model code.
    pub model_code: String,
    /// Last liquidity marker.
    pub last_liquidity: i32,
    /// Price revision pending marker.
    pub pending_price_revision: bool,
    /// Submitter.
    pub submitter: String,
    /// Option exercise or lapse type.
    pub opt_exercise_or_lapse_type: i32,
}

/// Account allocation details for an order state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OrderAllocation {
    /// Account.
    pub account: String,
    /// Current position.
    pub position: Decimal,
    /// Desired position.
    pub position_desired: Decimal,
    /// Position after allocation.
    pub position_after: Decimal,
    /// Desired allocation quantity.
    pub desired_alloc_qty: Decimal,
    /// Allowed allocation quantity.
    pub allowed_alloc_qty: Decimal,
    /// Monetary allocation flag.
    pub is_monetary: bool,
}

/// Order state returned by TWS.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OrderState {
    /// Status.
    pub status: String,
    /// Initial margin before the order.
    pub init_margin_before: f64,
    /// Maintenance margin before the order.
    pub maint_margin_before: f64,
    /// Equity with loan before the order.
    pub equity_with_loan_before: f64,
    /// Initial margin change.
    pub init_margin_change: f64,
    /// Maintenance margin change.
    pub maint_margin_change: f64,
    /// Equity with loan change.
    pub equity_with_loan_change: f64,
    /// Initial margin after the order.
    pub init_margin_after: f64,
    /// Maintenance margin after the order.
    pub maint_margin_after: f64,
    /// Equity with loan after the order.
    pub equity_with_loan_after: f64,
    /// Commission and fees.
    pub commission_and_fees: f64,
    /// Minimum commission and fees.
    pub min_commission_and_fees: f64,
    /// Maximum commission and fees.
    pub max_commission_and_fees: f64,
    /// Commission and fees currency.
    pub commission_and_fees_currency: String,
    /// Margin currency.
    pub margin_currency: String,
    /// Initial margin before the order outside RTH.
    pub init_margin_before_outside_rth: f64,
    /// Maintenance margin before the order outside RTH.
    pub maint_margin_before_outside_rth: f64,
    /// Equity with loan before the order outside RTH.
    pub equity_with_loan_before_outside_rth: f64,
    /// Initial margin change outside RTH.
    pub init_margin_change_outside_rth: f64,
    /// Maintenance margin change outside RTH.
    pub maint_margin_change_outside_rth: f64,
    /// Equity with loan change outside RTH.
    pub equity_with_loan_change_outside_rth: f64,
    /// Initial margin after the order outside RTH.
    pub init_margin_after_outside_rth: f64,
    /// Maintenance margin after the order outside RTH.
    pub maint_margin_after_outside_rth: f64,
    /// Equity with loan after the order outside RTH.
    pub equity_with_loan_after_outside_rth: f64,
    /// Suggested size.
    pub suggested_size: String,
    /// Reject reason.
    pub reject_reason: String,
    /// Allocation details.
    pub order_allocations: Vec<OrderAllocation>,
    /// Warning text.
    pub warning_text: String,
    /// Completed time.
    pub completed_time: String,
    /// Completed status.
    pub completed_status: String,
}

/// Commission and fees report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommissionAndFeesReport {
    /// Execution id.
    pub exec_id: String,
    /// Commission and fees.
    pub commission_and_fees: f64,
    /// Currency.
    pub currency: String,
    /// Realized profit and loss.
    pub realized_pnl: f64,
    /// Bond yield.
    pub bond_yield: f64,
    /// Yield redemption date.
    pub yield_redemption_date: String,
}

/// Contract description returned by matching symbol queries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContractDescription {
    /// Contract.
    pub contract: Contract,
    /// Derivative security types.
    pub derivative_sec_types: Vec<String>,
}

/// Smart routing component.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmartComponent {
    /// Bit number.
    pub bit_number: i32,
    /// Exchange.
    pub exchange: String,
    /// Exchange letter.
    pub exchange_letter: String,
}

/// Historical trading session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoricalSession {
    /// Session start.
    pub start_date_time: String,
    /// Session end.
    pub end_date_time: String,
    /// Reference date.
    pub ref_date: String,
}

/// Historical midpoint/trade tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoricalTick {
    /// Unix timestamp seconds.
    pub time: i64,
    /// Price.
    pub price: f64,
    /// Size.
    pub size: Decimal,
}

/// Bid/ask historical tick attributes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickAttribBidAsk {
    /// Bid past low marker.
    pub bid_past_low: bool,
    /// Ask past high marker.
    pub ask_past_high: bool,
}

/// Last trade historical tick attributes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickAttribLast {
    /// Past limit marker.
    pub past_limit: bool,
    /// Unreported marker.
    pub unreported: bool,
}

/// Historical bid/ask tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoricalTickBidAsk {
    /// Unix timestamp seconds.
    pub time: i64,
    /// Attributes.
    pub tick_attrib_bid_ask: TickAttribBidAsk,
    /// Bid price.
    pub price_bid: f64,
    /// Ask price.
    pub price_ask: f64,
    /// Bid size.
    pub size_bid: Decimal,
    /// Ask size.
    pub size_ask: Decimal,
}

/// Historical last trade tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoricalTickLast {
    /// Unix timestamp seconds.
    pub time: i64,
    /// Attributes.
    pub tick_attrib_last: TickAttribLast,
    /// Price.
    pub price: f64,
    /// Size.
    pub size: Decimal,
    /// Exchange.
    pub exchange: String,
    /// Special conditions.
    pub special_conditions: String,
}

/// Tick-by-tick payload.
#[derive(Debug, Clone, PartialEq)]
pub enum TickByTick {
    /// Last trade tick.
    Last(HistoricalTickLast),
    /// Bid/ask tick.
    BidAsk(HistoricalTickBidAsk),
    /// Midpoint tick.
    MidPoint(HistoricalTick),
}

/// Scanner subscription.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScannerSubscription {
    /// Number of rows.
    pub number_of_rows: i32,
    /// Instrument.
    pub instrument: String,
    /// Location code.
    pub location_code: String,
    /// Scan code.
    pub scan_code: String,
    /// Above price.
    pub above_price: f64,
    /// Below price.
    pub below_price: f64,
    /// Above volume.
    pub above_volume: i32,
    /// Market cap above.
    pub market_cap_above: f64,
    /// Market cap below.
    pub market_cap_below: f64,
    /// Moody rating above.
    pub moody_rating_above: String,
    /// Moody rating below.
    pub moody_rating_below: String,
    /// SP rating above.
    pub sp_rating_above: String,
    /// SP rating below.
    pub sp_rating_below: String,
    /// Maturity date above.
    pub maturity_date_above: String,
    /// Maturity date below.
    pub maturity_date_below: String,
    /// Coupon rate above.
    pub coupon_rate_above: f64,
    /// Coupon rate below.
    pub coupon_rate_below: f64,
    /// Exclude convertible.
    pub exclude_convertible: bool,
    /// Average option volume above.
    pub average_option_volume_above: i32,
    /// Scanner setting pairs.
    pub scanner_setting_pairs: String,
    /// Stock type filter.
    pub stock_type_filter: String,
}

/// Order condition.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderCondition {
    /// Price condition.
    Price {
        /// AND if true, OR if false.
        is_conjunction_connection: bool,
        /// Trigger method.
        trigger_method: i32,
        /// Contract id.
        con_id: i32,
        /// Exchange.
        exchange: String,
        /// More-than comparison.
        is_more: bool,
        /// Price.
        price: f64,
    },
    /// Time condition.
    Time {
        /// AND if true, OR if false.
        is_conjunction_connection: bool,
        /// More-than comparison.
        is_more: bool,
        /// Time string.
        time: String,
    },
    /// Margin condition.
    Margin {
        /// AND if true, OR if false.
        is_conjunction_connection: bool,
        /// More-than comparison.
        is_more: bool,
        /// Percent.
        percent: f64,
    },
    /// Execution condition.
    Execution {
        /// AND if true, OR if false.
        is_conjunction_connection: bool,
        /// Security type.
        sec_type: String,
        /// Exchange.
        exchange: String,
        /// Symbol.
        symbol: String,
    },
    /// Volume condition.
    Volume {
        /// AND if true, OR if false.
        is_conjunction_connection: bool,
        /// Contract id.
        con_id: i32,
        /// Exchange.
        exchange: String,
        /// More-than comparison.
        is_more: bool,
        /// Volume.
        volume: i32,
    },
    /// Percent-change condition.
    PercentChange {
        /// AND if true, OR if false.
        is_conjunction_connection: bool,
        /// Contract id.
        con_id: i32,
        /// Exchange.
        exchange: String,
        /// More-than comparison.
        is_more: bool,
        /// Change percent.
        change_percent: f64,
    },
}

impl OrderCondition {
    /// Returns the typed trigger method for a price condition.
    pub fn trigger_method(&self) -> Option<TriggerMethod> {
        match self {
            Self::Price { trigger_method, .. } => Some(TriggerMethod::from_i32(*trigger_method)),
            _ => None,
        }
    }
}

impl Execution {
    /// Returns the typed option exercise/lapse result.
    pub const fn option_exercise_type(&self) -> OptionExerciseType {
        OptionExerciseType::from_i32(self.opt_exercise_or_lapse_type)
    }

    /// Returns the typed liquidity classification.
    pub const fn liquidity(&self) -> crate::enums::Liquidities {
        crate::enums::Liquidities::from_i32(self.last_liquidity)
    }
}

/// Historical bar data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BarData {
    /// Date.
    pub date: String,
    /// Open.
    pub open: f64,
    /// High.
    pub high: f64,
    /// Low.
    pub low: f64,
    /// Close.
    pub close: f64,
    /// Volume.
    pub volume: Decimal,
    /// WAP.
    pub wap: Decimal,
    /// Bar count.
    pub bar_count: i32,
}

/// Real-time five-second bar. This is intentionally distinct from historical bars because its
/// timestamp and end time have different semantics in the TWS API.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RealTimeBar {
    pub time: i64,
    pub end_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Decimal,
    pub wap: Decimal,
    pub count: i32,
}
