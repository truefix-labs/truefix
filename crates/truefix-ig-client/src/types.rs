//! Request and response types for the supported IG REST operations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Direction {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrderType {
    #[serde(rename = "MARKET")]
    Market,
    #[serde(rename = "LIMIT")]
    Limit,
    #[serde(rename = "STOP")]
    Stop,
    #[serde(rename = "QUOTE")]
    Quote,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimeInForce {
    #[serde(rename = "GOOD_TILL_CANCELLED")]
    GoodTillCancelled,
    #[serde(rename = "GOOD_TILL_DATE")]
    GoodTillDate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PositionTimeInForce {
    #[serde(rename = "FILL_OR_KILL")]
    FillOrKill,
    #[serde(rename = "EXECUTE_AND_ELIMINATE")]
    ExecuteAndEliminate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketStatus {
    #[serde(rename = "TRADEABLE")]
    Tradeable,
    #[serde(rename = "CLOSED")]
    Closed,
    #[serde(rename = "EDITS_ONLY")]
    EditsOnly,
    #[serde(rename = "OFFLINE")]
    Offline,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub current_account_id: Option<String>,
    pub lightstreamer_endpoint: String,
    pub client_id: String,
    pub currency_iso_code: Option<String>,
    pub dealing_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct V3LoginResponse {
    pub account_id: Option<String>,
    pub current_account_id: Option<String>,
    pub lightstreamer_endpoint: String,
    pub client_id: String,
    pub currency_iso_code: Option<String>,
    pub dealing_enabled: Option<bool>,
    pub oauth_token: Option<OAuthToken>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthToken {
    #[serde(alias = "accessToken")]
    pub access_token: String,
    #[serde(alias = "refreshToken")]
    pub refresh_token: String,
    #[serde(
        alias = "expiresIn",
        deserialize_with = "deserialize_u64_string_or_number"
    )]
    pub expires_in: u64,
}

fn deserialize_u64_string_or_number<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Number(u64),
        String(String),
    }

    match Value::deserialize(deserializer)? {
        Value::Number(value) => Ok(value),
        Value::String(value) => value.parse().map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Deserialize)]
pub struct AccountsResponse {
    pub accounts: Vec<Account>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account_id: String,
    pub account_name: String,
    pub preferred: bool,
    pub balance: Option<AccountBalance>,
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalance {
    pub balance: f64,
    pub available: f64,
    pub deposit: f64,
    pub profit_loss: f64,
}

#[derive(Debug, Deserialize)]
pub struct PositionsResponse {
    pub positions: Vec<Position>,
}

#[derive(Debug, Deserialize)]
pub struct Position {
    pub position: PositionDetail,
    pub market: PositionMarket,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionDetail {
    pub deal_id: String,
    pub direction: Direction,
    pub currency: String,
    pub size: Option<f64>,
    pub level: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionMarket {
    pub epic: String,
    pub instrument_name: String,
    pub bid: Option<f64>,
    pub offer: Option<f64>,
    pub market_status: Option<MarketStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketDetails {
    pub instrument: Instrument,
    pub snapshot: MarketSnapshot,
    #[serde(default)]
    pub dealing_rules: Option<DealingRules>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    pub name: String,
    pub epic: String,
    pub instrument_type: Option<String>,
    pub expiry: Option<String>,
    #[serde(default)]
    pub currencies: Vec<InstrumentCurrency>,
    #[serde(default)]
    pub lot_size: Option<f64>,
    #[serde(default)]
    pub stops_limits_allowed: Option<bool>,
    #[serde(default)]
    pub streaming_prices_available: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentCurrency {
    pub code: String,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DealingRules {
    #[serde(default)]
    pub min_deal_size: Option<RuleValue>,
    #[serde(default)]
    pub min_normal_stop_or_limit_distance: Option<RuleValue>,
    #[serde(default)]
    pub max_stop_or_limit_distance: Option<RuleValue>,
    #[serde(default)]
    pub market_order_preference: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RuleValue {
    pub value: f64,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketSnapshot {
    pub market_status: MarketStatus,
    pub bid: Option<f64>,
    pub offer: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    #[serde(default)]
    pub decimal_places_factor: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct MarketsResponse {
    pub markets: Vec<MarketData>,
}

/// One account-visible IG instrument category returned by `GET /categories`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentCategory {
    pub code: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Account-visible instrument categories for the authenticated account.
#[derive(Debug, Deserialize)]
pub struct InstrumentCategoriesResponse {
    #[serde(default)]
    pub categories: Vec<InstrumentCategory>,
}

/// One page returned by `GET /categories/{categoryId}/instruments`.
#[derive(Debug)]
pub struct CategoryInstrumentsResponse {
    pub instruments: Vec<MarketData>,
}

impl<'de> Deserialize<'de> for CategoryInstrumentsResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Instruments {
                #[serde(default)]
                instruments: Vec<MarketData>,
            },
            Markets {
                #[serde(default)]
                markets: Vec<MarketData>,
            },
            Direct(Vec<MarketData>),
        }

        Ok(Self {
            instruments: match Wire::deserialize(deserializer)? {
                Wire::Instruments { instruments } => instruments,
                Wire::Markets { markets } => markets,
                Wire::Direct(instruments) => instruments,
            },
        })
    }
}

/// Detailed account activity used to recover authoritative fills and deal state.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityHistoryResponse {
    #[serde(default)]
    pub activities: Vec<ActivityHistoryItem>,
    #[serde(default)]
    pub metadata: Option<ActivityHistoryMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityHistoryMetadata {
    #[serde(default)]
    pub paging: Option<ActivityHistoryPaging>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityHistoryPaging {
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityHistoryItem {
    pub date: String,
    pub deal_id: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub details: Option<ActivityHistoryDetails>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityHistoryDetails {
    #[serde(default)]
    pub epic: Option<String>,
    #[serde(default)]
    pub actions: Vec<ActivityHistoryAction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityHistoryAction {
    #[serde(default)]
    pub action_type: Option<String>,
    #[serde(default)]
    pub affected_deal_id: Option<String>,
    #[serde(default)]
    pub deal_reference: Option<String>,
    #[serde(default)]
    pub direction: Option<Direction>,
    #[serde(default)]
    pub level: Option<f64>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub market_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketData {
    pub epic: String,
    pub instrument_name: String,
    #[serde(default)]
    pub instrument_type: Option<String>,
    #[serde(default)]
    pub expiry: Option<String>,
    pub bid: Option<f64>,
    pub offer: Option<f64>,
    #[serde(default)]
    pub market_status: Option<MarketStatus>,
}

#[derive(Debug, Clone, Default)]
pub struct HistoricalPricesQuery<'a> {
    pub resolution: &'a str,
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub max: Option<u32>,
}
impl<'a> HistoricalPricesQuery<'a> {
    pub fn new(resolution: &'a str) -> Self {
        Self {
            resolution,
            ..Self::default()
        }
    }
    pub fn from(mut self, value: &'a str) -> Self {
        self.from = Some(value);
        self
    }
    pub fn to(mut self, value: &'a str) -> Self {
        self.to = Some(value);
        self
    }
    pub fn max(mut self, value: u32) -> Self {
        self.max = Some(value);
        self
    }
}

#[derive(Debug, Deserialize)]
pub struct HistoricalPricesResponse {
    pub prices: Vec<HistoricalPrice>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalPrice {
    pub snapshot_time: Option<String>,
    pub snapshot_time_utc: Option<String>,
    pub open_price: PricePoint,
    pub high_price: PricePoint,
    pub low_price: PricePoint,
    pub close_price: PricePoint,
    #[serde(default)]
    pub last_traded_volume: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PricePoint {
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub last_traded: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePositionRequest {
    pub currency_code: String,
    pub direction: Direction,
    pub epic: String,
    pub expiry: String,
    pub force_open: bool,
    pub guaranteed_stop: bool,
    pub order_type: OrderType,
    pub time_in_force: PositionTimeInForce,
    pub size: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_level: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DealReferenceResponse {
    pub deal_reference: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchAccountResponse {
    pub dealing_enabled: bool,
    #[serde(default)]
    pub has_active_demo_accounts: bool,
    #[serde(default)]
    pub has_active_live_accounts: bool,
    #[serde(default)]
    pub trailing_stops_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingOrdersResponse {
    #[serde(default)]
    pub working_orders: Vec<WorkingOrder>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingOrder {
    pub working_order_data: WorkingOrderData,
    pub market_data: MarketData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingOrderData {
    pub deal_id: String,
    #[serde(default)]
    pub deal_reference: Option<String>,
    pub direction: Direction,
    pub epic: String,
    pub order_size: f64,
    pub order_level: f64,
    #[serde(rename = "orderType")]
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    #[serde(default)]
    pub good_till_date: Option<String>,
    #[serde(default)]
    pub guaranteed_stop: bool,
    #[serde(default)]
    pub stop_level: Option<f64>,
    #[serde(default)]
    pub stop_distance: Option<f64>,
    #[serde(default)]
    pub limit_level: Option<f64>,
    #[serde(default)]
    pub limit_distance: Option<f64>,
    #[serde(default)]
    pub currency_code: Option<String>,
    #[serde(default)]
    pub created_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkingOrderRequest {
    pub epic: String,
    pub direction: Direction,
    pub size: f64,
    pub level: f64,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub guaranteed_stop: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub good_till_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deal_reference: Option<String>,
    pub currency_code: String,
    pub expiry: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkingOrderRequest {
    pub level: f64,
    #[serde(rename = "type")]
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub guaranteed_stop: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub good_till_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_distance: Option<f64>,
}

/// Authoritative acknowledgement returned by `GET /confirms/{dealReference}`.
/// Optional fields keep the client forward-compatible with IG's product- and
/// rejection-specific response shapes.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DealConfirmation {
    pub deal_reference: String,
    #[serde(default)]
    pub deal_id: Option<String>,
    pub deal_status: DealStatus,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub level: Option<f64>,
    #[serde(default)]
    pub size: Option<f64>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DealStatus {
    Accepted,
    Rejected,
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deal_confirmation_accepts_product_specific_optional_fields() {
        let confirmation: DealConfirmation = serde_json::from_value(serde_json::json!({
            "dealReference": "reference-1",
            "dealId": "deal-1",
            "dealStatus": "ACCEPTED",
            "status": "OPEN",
            "level": 123.45,
            "size": 2,
            "epic": "CS.D.AAPL.CFD.IP"
        }))
        .unwrap();
        assert_eq!(confirmation.deal_status, DealStatus::Accepted);
        assert_eq!(confirmation.deal_id.as_deref(), Some("deal-1"));
    }

    #[test]
    fn category_catalogue_accepts_account_visible_instrument_pages() {
        let categories: InstrumentCategoriesResponse = serde_json::from_value(serde_json::json!({
            "categories": [{"code": "INDICES", "name": "Indices"}]
        }))
        .unwrap();
        assert_eq!(categories.categories[0].code, "INDICES");

        let page: CategoryInstrumentsResponse = serde_json::from_value(serde_json::json!({
            "instruments": [{
                "epic": "IX.D.FTSE.CFD.IP",
                "instrumentName": "FTSE 100",
                "instrumentType": "INDICES",
                "expiry": "-",
                "marketStatus": "TRADEABLE"
            }]
        }))
        .unwrap();
        assert_eq!(page.instruments[0].epic, "IX.D.FTSE.CFD.IP");
        assert_eq!(
            page.instruments[0].market_status,
            Some(MarketStatus::Tradeable)
        );
        let direct: CategoryInstrumentsResponse = serde_json::from_value(serde_json::json!([{
            "epic": "CS.D.EURUSD.CFD.IP",
            "instrumentName": "EUR/USD",
            "instrumentType": "CURRENCIES",
            "expiry": "DFB"
        }]))
        .unwrap();
        assert_eq!(direct.instruments[0].epic, "CS.D.EURUSD.CFD.IP");
    }

    #[test]
    fn detailed_activity_accepts_fill_evidence() {
        let response: ActivityHistoryResponse = serde_json::from_value(serde_json::json!({
            "activities": [{
                "date": "2026-08-12T06:00:00",
                "dealId": "deal-1",
                "status": "ACCEPTED",
                "details": {
                    "epic": "CS.D.EURUSD.CFD.IP",
                    "actions": [{
                        "actionType": "POSITION_OPENED",
                        "affectedDealId": "deal-1",
                        "direction": "BUY",
                        "level": 1.1,
                        "size": 2.0
                    }]
                }
            }],
            "metadata": {"paging": {"next": null, "size": 1}}
        }))
        .unwrap();
        assert_eq!(response.activities[0].deal_id, "deal-1");
        assert_eq!(
            response.activities[0]
                .details
                .as_ref()
                .unwrap()
                .actions
                .len(),
            1
        );
    }

    #[test]
    fn otc_market_order_serializes_required_time_in_force() {
        let request = CreatePositionRequest {
            currency_code: "USD".into(),
            direction: Direction::Buy,
            epic: "IX.D.DAX.IFD.IP".into(),
            expiry: "DFB".into(),
            force_open: true,
            guaranteed_stop: false,
            order_type: OrderType::Market,
            time_in_force: PositionTimeInForce::FillOrKill,
            size: 1.0,
            level: None,
            limit_level: None,
            stop_level: None,
        };
        let encoded = serde_json::to_value(request).unwrap();
        assert_eq!(encoded["timeInForce"], "FILL_OR_KILL");
    }
}
