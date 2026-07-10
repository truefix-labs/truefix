use rust_decimal::Decimal;

/// Exact price or size representation used in public models.
pub type DecimalValue = Decimal;
/// OKX instrument identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstrumentId(pub String);
/// Opaque cursor pair returned by OKX list endpoints.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageCursor {
    pub before: Option<String>,
    pub after: Option<String>,
}
/// Millisecond Unix timestamp represented as text by the OKX wire API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MillisecondTimestamp(pub String);

/// Native long-tail REST record retaining product-specific exchange fields.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NativeRecord {
    #[serde(flatten)]
    pub fields: std::collections::BTreeMap<String, serde_json::Value>,
}
