use thiserror::Error;

/// Result alias returned by the IG client.
pub type IgResult<T> = Result<T, IgError>;

/// Recoverable errors emitted by the IG client.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IgError {
    #[error("invalid client configuration: {0}")]
    InvalidConfiguration(String),
    #[error("live trading requires explicit confirmation")]
    LiveTradingConfirmationRequired,
    #[error("credentials are required for this operation")]
    MissingCredentials,
    #[error("the session is not authenticated")]
    MissingSession,
    #[error("IG response omitted required {0} header")]
    MissingToken(&'static str),
    #[error("HTTP transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("IG rejected request with status {status}: {code}{message}")]
    Exchange {
        status: u16,
        code: String,
        message: String,
    },
    #[error("response decoding failed: {0}")]
    Decode(#[from] serde_json::Error),
}
