use crate::types::{
    common::DecimalValue,
    gateway::{GatewayBalance, GatewayPosition, GatewayProject, NativeFields},
};
use serde::{Deserialize, Deserializer};

/// Deserializes OKX optional numeric fields, treating its `""` sentinel as absent.
fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<DecimalValue>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(serde_json::Value::String(ref value)) if value.is_empty() => Ok(None),
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
}

/// Account-level balance summary returned by `/api/v5/account/balance`.
///
/// Currency balances are nested in [`Self::details`]; the endpoint does not return
/// `Balance` records directly.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AccountBalance {
    pub details: Vec<Balance>,
    /// OKX account-level fields outside the currency balance representation.
    #[serde(flatten)]
    pub native_fields: NativeFields,
}

/// Account balance with native availability and equity fields.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Balance {
    #[serde(rename = "ccy")]
    pub currency: String,
    #[serde(
        rename = "availBal",
        default,
        deserialize_with = "empty_string_as_none"
    )]
    pub available: Option<DecimalValue>,
    #[serde(rename = "eq", default, deserialize_with = "empty_string_as_none")]
    pub equity: Option<DecimalValue>,
    #[serde(
        rename = "frozenBal",
        default,
        deserialize_with = "empty_string_as_none"
    )]
    pub frozen: Option<DecimalValue>,
    /// OKX fields outside the core balance representation.
    #[serde(flatten)]
    pub native_fields: NativeFields,
}
/// Open position returned by the account service.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Position {
    #[serde(rename = "instId")]
    pub instrument_id: String,
    #[serde(rename = "pos")]
    pub quantity: DecimalValue,
    #[serde(rename = "avgPx", default, deserialize_with = "empty_string_as_none")]
    pub average_price: Option<DecimalValue>,
    #[serde(rename = "upl", default, deserialize_with = "empty_string_as_none")]
    pub unrealized_pnl: Option<DecimalValue>,
    /// OKX fields outside the core position representation.
    #[serde(flatten)]
    pub native_fields: NativeFields,
}

impl GatewayProject for Balance {
    type Projection = GatewayBalance;

    fn project_gateway(&self) -> Self::Projection {
        GatewayBalance {
            currency: self.currency.clone(),
            available: self.available,
            equity: self.equity,
            native_fields: self.native_fields.clone(),
        }
    }
}

impl GatewayProject for Position {
    type Projection = GatewayPosition;

    fn project_gateway(&self) -> Self::Projection {
        GatewayPosition {
            instrument_id: self.instrument_id.clone(),
            quantity: self.quantity,
            average_price: self.average_price,
            unrealized_pnl: self.unrealized_pnl,
            native_fields: self.native_fields.clone(),
        }
    }
}
/// Account bill preserving the exchange's accounting classification.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Bill {
    #[serde(rename = "billId")]
    pub bill_id: String,
    #[serde(rename = "instId")]
    pub instrument_id: Option<String>,
    #[serde(rename = "balChg", default, deserialize_with = "empty_string_as_none")]
    pub balance_change: Option<DecimalValue>,
    #[serde(rename = "ts")]
    pub timestamp: String,
}
/// Current leverage for an instrument or currency scope.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Leverage {
    pub lever: DecimalValue,
    #[serde(rename = "mgnMode")]
    pub margin_mode: String,
    #[serde(rename = "instId")]
    pub instrument_id: Option<String>,
}
/// Margin adjustment acknowledgement.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MarginAdjustment {
    #[serde(rename = "amt")]
    pub amount: DecimalValue,
    #[serde(rename = "instId")]
    pub instrument_id: String,
}
/// Position-risk record supplied by OKX.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PositionRisk {
    #[serde(rename = "instId")]
    pub instrument_id: Option<String>,
    #[serde(
        rename = "liquidationPx",
        default,
        deserialize_with = "empty_string_as_none"
    )]
    pub liquidation_price: Option<DecimalValue>,
    #[serde(
        rename = "mgnRatio",
        default,
        deserialize_with = "empty_string_as_none"
    )]
    pub margin_ratio: Option<DecimalValue>,
}
/// Fee schedule for a product and account level.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FeeRate {
    #[serde(rename = "instType")]
    pub instrument_type: String,
    #[serde(rename = "maker", default, deserialize_with = "empty_string_as_none")]
    pub maker: Option<DecimalValue>,
    #[serde(rename = "taker", default, deserialize_with = "empty_string_as_none")]
    pub taker: Option<DecimalValue>,
}

#[cfg(test)]
mod tests {
    use super::{AccountBalance, FeeRate, Position};

    #[test]
    fn account_balance_preserves_nested_currency_details() {
        let summary: AccountBalance = serde_json::from_value(serde_json::json!({
            "totalEq": "12.5",
            "details": [{
                "ccy": "BTC", "availBal": "", "eq": "", "frozenBal": ""
            }]
        }))
        .unwrap();

        assert_eq!(summary.details.len(), 1);
        assert_eq!(summary.details[0].currency, "BTC");
        assert_eq!(summary.details[0].available, None);
        assert_eq!(
            summary.native_fields.get("totalEq").unwrap().as_str(),
            Some("12.5")
        );
    }

    #[test]
    fn empty_numeric_sentinels_decode_as_none() {
        let position: Position = serde_json::from_value(serde_json::json!({
            "instId": "BTC-USDT-SWAP", "pos": "0", "avgPx": "", "upl": ""
        }))
        .unwrap();
        let fee: FeeRate = serde_json::from_value(serde_json::json!({
            "instType": "SWAP", "maker": "", "taker": ""
        }))
        .unwrap();

        assert_eq!(position.average_price, None);
        assert_eq!(position.unrealized_pnl, None);
        assert_eq!(fee.maker, None);
        assert_eq!(fee.taker, None);
    }
}
