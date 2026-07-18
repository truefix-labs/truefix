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
    #[serde(rename = "QUOTE")]
    Quote,
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
#[serde(rename_all = "camelCase")]
pub(crate) struct OAuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    pub name: String,
    pub epic: String,
    pub instrument_type: Option<String>,
    pub expiry: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketSnapshot {
    pub market_status: MarketStatus,
    pub bid: Option<f64>,
    pub offer: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct MarketsResponse {
    pub markets: Vec<MarketData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketData {
    pub epic: String,
    pub instrument_name: String,
    pub bid: Option<f64>,
    pub offer: Option<f64>,
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
    pub close_price: PricePoint,
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
