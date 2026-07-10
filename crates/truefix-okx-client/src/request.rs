use std::collections::BTreeMap;

use serde::Serialize;

use crate::error::{OkxError, OkxResult};

/// Whether a request can be retried automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrySafety {
    ReadOnly,
    RequiresReconciliation,
    NeverReplay,
}

/// Exact bytes and metadata for an outbound REST operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalRequest {
    pub method: reqwest::Method,
    pub path_and_query: String,
    pub body: Vec<u8>,
    pub retry_safety: RetrySafety,
    pub requires_auth: bool,
}

impl CanonicalRequest {
    /// Constructs a request with deterministic query ordering and exact JSON bytes.
    pub fn new<T: Serialize + ?Sized>(
        method: reqwest::Method,
        path: &str,
        query: BTreeMap<String, String>,
        body: Option<&T>,
        retry_safety: RetrySafety,
        requires_auth: bool,
    ) -> OkxResult<Self> {
        if !path.starts_with('/') {
            return Err(OkxError::InvalidConfiguration(
                "request paths must start with '/'".to_owned(),
            ));
        }
        let encoded_query = serde_urlencoded::to_string(
            query
                .into_iter()
                .filter(|(_, value)| !value.is_empty())
                .collect::<BTreeMap<_, _>>(),
        )
        .map_err(|error| OkxError::Signing(error.to_string()))?;
        let path_and_query = if encoded_query.is_empty() {
            path.to_owned()
        } else {
            format!("{path}?{encoded_query}")
        };
        let body = match body {
            Some(value) => serde_json::to_vec(value).map_err(OkxError::Decode)?,
            None => Vec::new(),
        };
        Ok(Self {
            method,
            path_and_query,
            body,
            retry_safety,
            requires_auth,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_is_sorted_and_body_is_exact_json() {
        let mut query = BTreeMap::new();
        query.insert("z".to_owned(), "last value".to_owned());
        query.insert("a".to_owned(), "first".to_owned());
        let request = CanonicalRequest::new(
            reqwest::Method::POST,
            "/api/v5/test",
            query,
            Some(&serde_json::json!({"sz":"1.2"})),
            RetrySafety::NeverReplay,
            true,
        )
        .unwrap();
        assert_eq!(request.path_and_query, "/api/v5/test?a=first&z=last+value");
        assert_eq!(request.body, br#"{"sz":"1.2"}"#);
    }

    #[test]
    fn invalid_paths_are_rejected_before_signing() {
        let result = CanonicalRequest::new(
            reqwest::Method::GET,
            "api/v5/market/ticker",
            BTreeMap::new(),
            None::<&serde_json::Value>,
            RetrySafety::ReadOnly,
            false,
        );
        assert!(matches!(result, Err(OkxError::InvalidConfiguration(_))));
    }

    #[test]
    fn empty_query_values_are_omitted() {
        let mut query = BTreeMap::new();
        query.insert("instId".to_owned(), "BTC-USDT".to_owned());
        query.insert("after".to_owned(), String::new());
        let request = CanonicalRequest::new(
            reqwest::Method::GET,
            "/api/v5/market/ticker",
            query,
            None::<&serde_json::Value>,
            RetrySafety::ReadOnly,
            false,
        )
        .unwrap();
        assert_eq!(
            request.path_and_query,
            "/api/v5/market/ticker?instId=BTC-USDT"
        );
    }
}
