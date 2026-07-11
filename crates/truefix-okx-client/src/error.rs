use std::time::Duration;

use thiserror::Error;

/// Result alias returned by the OKX client.
pub type OkxResult<T> = Result<T, OkxError>;

/// Recoverable errors emitted by the OKX client.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OkxError {
    #[error("invalid client configuration: {0}")]
    InvalidConfiguration(String),
    #[error("live trading requires explicit confirmation")]
    LiveTradingConfirmationRequired,
    #[error("credentials are required for this operation")]
    MissingCredentials,
    #[error("failed to construct a signed request: {0}")]
    Signing(String),
    #[error("server clock skew prevents authentication")]
    ClockSkew,
    #[error("HTTP transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("request timed out")]
    Timeout,
    #[error("rate limited; retry after {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },
    #[error("OKX rejected request with code {code}: {message}")]
    Exchange {
        code: String,
        message: String,
        request_id: Option<String>,
    },
    #[error("one or more order operations were rejected")]
    PartialFailure {
        /// Every acknowledgement returned by OKX, including any items that succeeded.
        /// Retaining the complete set lets callers reconcile an indeterminate batch without
        /// treating the enclosing request as successful.
        acknowledgements: Vec<crate::types::order::OrderAck>,
    },
    #[error("response decoding failed: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("connection lost before completion could be confirmed")]
    UnknownCompletion,
    #[error("WebSocket transport error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
}
