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
        self.into_data_with_metadata(ResponseMetadata::default())
            .map(|response| response.data)
    }

    /// Converts an exchange envelope while retaining HTTP response metadata.
    pub fn into_data_with_metadata(self, metadata: ResponseMetadata) -> OkxResult<OkxResponse<T>> {
        if self.code == "0" {
            Ok(OkxResponse {
                data: self.data,
                metadata,
            })
        } else if self.code == "50102" {
            Err(OkxError::ClockSkew)
        } else {
            Err(OkxError::Exchange {
                code: self.code,
                message: self.msg,
                request_id: metadata.request_id,
            })
        }
    }
}

/// Decodes and validates one response body.
pub fn decode_envelope<T: DeserializeOwned>(body: &[u8]) -> OkxResult<Vec<T>> {
    serde_json::from_slice::<ResponseEnvelope<T>>(body)?.into_data()
}

/// Decodes an OKX envelope and preserves pagination and correlation metadata.
pub fn decode_envelope_with_metadata<T: DeserializeOwned>(
    body: &[u8],
    metadata: ResponseMetadata,
) -> OkxResult<OkxResponse<T>> {
    serde_json::from_slice::<ResponseEnvelope<T>>(body)?.into_data_with_metadata(metadata)
}

/// Server-preserved cursor metadata for list responses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageMetadata {
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Metadata returned with an OKX REST response.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseMetadata {
    /// Pagination cursors sent by OKX for list endpoints.
    pub page: PageMetadata,
    /// OKX request correlation identifier, if supplied by the server.
    pub request_id: Option<String>,
}

impl ResponseMetadata {
    /// Extracts response metadata from OKX HTTP headers.
    pub fn from_headers(headers: &reqwest::header::HeaderMap) -> Self {
        Self {
            page: page_metadata(headers),
            request_id: headers
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
        }
    }
}

/// Decoded OKX response data with HTTP metadata retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OkxResponse<T> {
    /// Decoded OKX response records.
    pub data: Vec<T>,
    /// Pagination and request correlation metadata.
    pub metadata: ResponseMetadata,
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

    #[test]
    fn exchange_error_preserves_request_id_from_metadata() {
        let result: OkxResult<OkxResponse<serde_json::Value>> = decode_envelope_with_metadata(
            br#"{"code":"51008","msg":"insufficient balance","data":[]}"#,
            ResponseMetadata {
                request_id: Some("okx-request-42".to_owned()),
                ..ResponseMetadata::default()
            },
        );
        assert!(matches!(
            result,
            Err(OkxError::Exchange { request_id: Some(request_id), .. })
                if request_id == "okx-request-42"
        ));
    }
}
