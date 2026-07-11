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

/// A millisecond Unix timestamp after which OKX must reject an order or amendment request.
///
/// The value is sent as the `expTime` REST header or WebSocket argument. It must be based on
/// the same (optionally offset) server-time basis as the corresponding request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpirationTime(i64);

impl ExpirationTime {
    /// Creates an expiration timestamp. Zero and negative Unix timestamps are rejected.
    pub fn new(unix_milliseconds: i64) -> Result<Self, &'static str> {
        if unix_milliseconds > 0 {
            Ok(Self(unix_milliseconds))
        } else {
            Err("expiration time must be a positive Unix-millisecond timestamp")
        }
    }

    /// Returns the exact Unix-millisecond timestamp sent to OKX.
    pub const fn unix_milliseconds(self) -> i64 {
        self.0
    }
}

impl serde::Serialize for ExpirationTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::ExpirationTime;

    #[test]
    fn expiration_time_is_a_positive_string_timestamp() {
        let expiry = ExpirationTime::new(1_704_067_200_123).unwrap();
        assert_eq!(serde_json::to_value(expiry).unwrap(), "1704067200123");
        assert!(ExpirationTime::new(0).is_err());
    }
}

/// Native long-tail REST record retaining product-specific exchange fields.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NativeRecord {
    #[serde(flatten)]
    pub fields: std::collections::BTreeMap<String, serde_json::Value>,
}
