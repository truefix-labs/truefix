use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::{OkxError, OkxResult};

/// Standard OKX response envelope preserving server result details.
#[derive(Debug, Deserialize)]
pub struct ResponseEnvelope<T> {
    pub code: String,
    pub msg: String,
    pub data: Vec<T>,
}

impl<T> ResponseEnvelope<T> {
    /// Converts an exchange success envelope into its data collection.
    pub fn into_data(self) -> OkxResult<Vec<T>> {
        if self.code == "0" {
            Ok(self.data)
        } else {
            Err(OkxError::Exchange {
                code: self.code,
                message: self.msg,
                request_id: None,
            })
        }
    }
}

/// Decodes and validates one response body.
pub fn decode_envelope<T: DeserializeOwned>(body: &[u8]) -> OkxResult<Vec<T>> {
    serde_json::from_slice::<ResponseEnvelope<T>>(body)?.into_data()
}

/// Server-preserved cursor metadata for list responses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageMetadata {
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Extracts pagination cursors without inventing offset semantics.
pub fn page_metadata(headers: &reqwest::header::HeaderMap) -> PageMetadata {
    let value = |name| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    PageMetadata {
        before: value("OK-BEFORE"),
        after: value("OK-AFTER"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_exchange_error_envelope() {
        let result: OkxResult<Vec<serde_json::Value>> =
            decode_envelope(br#"{"code":"50011","msg":"rate limit","data":[]}"#);
        assert!(matches!(result, Err(OkxError::Exchange { .. })));
    }
}
