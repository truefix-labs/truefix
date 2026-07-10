use base64::{Engine, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    config::Credentials,
    error::{OkxError, OkxResult},
    request::CanonicalRequest,
};

type HmacSha256 = Hmac<Sha256>;

/// Clock abstraction allowing deterministic signing tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

/// Production system clock.
#[derive(Debug, Default)]
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

/// Returns a V5 REST signature and ISO-8601 timestamp for one canonical request.
pub fn sign_rest(
    credentials: &Credentials,
    request: &CanonicalRequest,
    now: OffsetDateTime,
) -> OkxResult<(String, String)> {
    let timestamp = now
        .format(&Rfc3339)
        .map_err(|error| OkxError::Signing(error.to_string()))?;
    let payload = format!(
        "{}{}{}{}",
        timestamp,
        request.method.as_str(),
        request.path_and_query,
        String::from_utf8_lossy(&request.body)
    );
    let mut signer = HmacSha256::new_from_slice(credentials.secret().as_bytes())
        .map_err(|error| OkxError::Signing(error.to_string()))?;
    signer.update(payload.as_bytes());
    Ok((STANDARD.encode(signer.finalize().into_bytes()), timestamp))
}

/// Returns a V5 WebSocket login signature for the supplied Unix-second timestamp.
pub fn sign_websocket_login(
    credentials: &Credentials,
    timestamp_seconds: i64,
) -> OkxResult<String> {
    let payload = format!("{timestamp_seconds}GET/users/self/verify");
    let mut signer = HmacSha256::new_from_slice(credentials.secret().as_bytes())
        .map_err(|error| OkxError::Signing(error.to_string()))?;
    signer.update(payload.as_bytes());
    Ok(STANDARD.encode(signer.finalize().into_bytes()))
}

/// Private header fields excluding the secret.
pub fn private_headers(
    credentials: &Credentials,
    signature: String,
    timestamp: String,
) -> [(String, String); 4] {
    [
        ("OK-ACCESS-KEY".to_owned(), credentials.api_key().to_owned()),
        ("OK-ACCESS-SIGN".to_owned(), signature),
        ("OK-ACCESS-TIMESTAMP".to_owned(), timestamp),
        (
            "OK-ACCESS-PASSPHRASE".to_owned(),
            credentials.passphrase().to_owned(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::RetrySafety;
    use std::collections::BTreeMap;
    use time::macros::datetime;

    #[test]
    fn rest_signature_uses_canonical_payload() {
        let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
        let request = CanonicalRequest::new(
            reqwest::Method::GET,
            "/api/v5/account/balance",
            BTreeMap::new(),
            None::<&serde_json::Value>,
            RetrySafety::ReadOnly,
            true,
        )
        .unwrap();
        let (signature, timestamp) =
            sign_rest(&credentials, &request, datetime!(2024-01-01 0:00 UTC)).unwrap();
        assert_eq!(timestamp, "2024-01-01T00:00:00Z");
        assert!(!signature.is_empty());
    }

    #[test]
    fn websocket_signature_is_stable() {
        let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
        assert_eq!(
            sign_websocket_login(&credentials, 1_700_000_000).unwrap(),
            sign_websocket_login(&credentials, 1_700_000_000).unwrap()
        );
    }

    #[test]
    fn rest_signing_covers_encoded_query_and_empty_body() {
        let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
        let mut query = BTreeMap::new();
        query.insert("instId".to_owned(), "BTC-USDT SWAP".to_owned());
        let request = CanonicalRequest::new(
            reqwest::Method::GET,
            "/api/v5/market/ticker",
            query,
            None::<&serde_json::Value>,
            RetrySafety::ReadOnly,
            false,
        )
        .unwrap();
        let (signature, timestamp) =
            sign_rest(&credentials, &request, datetime!(2024-01-01 0:00 UTC)).unwrap();
        assert_eq!(timestamp, "2024-01-01T00:00:00Z");
        assert!(!signature.is_empty());
        assert!(request.path_and_query.contains("BTC-USDT+SWAP"));
        assert!(request.body.is_empty());
    }

    #[test]
    fn private_headers_do_not_contain_the_secret() {
        let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
        let headers = private_headers(&credentials, "signature".to_owned(), "time".to_owned());
        assert!(headers.iter().all(|(_, value)| value != "secret"));
        assert!(headers.iter().any(|(name, _)| name == "OK-ACCESS-SIGN"));
    }
}
