use std::time::Duration;

use crate::error::{OkxError, OkxResult};

/// Credentials bound to one immutable client identity.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    api_key: String,
    secret: String,
    passphrase: String,
}

impl Credentials {
    /// Creates credentials after rejecting empty values.
    pub fn new(
        api_key: impl Into<String>,
        secret: impl Into<String>,
        passphrase: impl Into<String>,
    ) -> OkxResult<Self> {
        let credentials = Self {
            api_key: api_key.into(),
            secret: secret.into(),
            passphrase: passphrase.into(),
        };
        if credentials.api_key.is_empty()
            || credentials.secret.is_empty()
            || credentials.passphrase.is_empty()
        {
            return Err(OkxError::InvalidConfiguration(
                "API key, secret, and passphrase must be non-empty".to_owned(),
            ));
        }
        Ok(credentials)
    }

    pub(crate) fn api_key(&self) -> &str {
        &self.api_key
    }
    pub(crate) fn secret(&self) -> &str {
        &self.secret
    }
    pub(crate) fn passphrase(&self) -> &str {
        &self.passphrase
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Credentials(REDACTED)")
    }
}

/// Marker required to select a live trading environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveTradingConfirmation(());

impl LiveTradingConfirmation {
    /// Explicitly acknowledges that the client may submit live asset-changing requests.
    pub const fn acknowledge_risk() -> Self {
        Self(())
    }
}

/// Endpoint environment selected once at construction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Environment {
    /// Simulated trading endpoints and declaration header.
    #[default]
    Demo,
    /// Production endpoints, permitted only with an explicit confirmation.
    Live(LiveTradingConfirmation),
    /// Caller-controlled endpoints for tests or supported regional deployments.
    Custom {
        rest_base: String,
        public_ws: String,
        private_ws: String,
        business_ws: String,
        simulated: bool,
    },
}

impl Environment {
    const OKX_REST_BASE: &str = "https://www.okx.com";
    const LIVE_PUBLIC_WS: &str = "wss://ws.okx.com:8443/ws/v5/public";
    const LIVE_PRIVATE_WS: &str = "wss://ws.okx.com:8443/ws/v5/private";
    const LIVE_BUSINESS_WS: &str = "wss://ws.okx.com:8443/ws/v5/business";
    const DEMO_PUBLIC_WS: &str = "wss://wspap.okx.com:8443/ws/v5/public";
    const DEMO_PRIVATE_WS: &str = "wss://wspap.okx.com:8443/ws/v5/private";
    const DEMO_BUSINESS_WS: &str = "wss://wspap.okx.com:8443/ws/v5/business";

    /// Returns the REST API base URL for this environment.
    pub fn rest_base(&self) -> &str {
        match self {
            Self::Demo | Self::Live(_) => Self::OKX_REST_BASE,
            Self::Custom { rest_base, .. } => rest_base,
        }
    }

    /// Returns the public WebSocket URL for this environment.
    pub fn public_ws_url(&self) -> &str {
        match self {
            Self::Demo => Self::DEMO_PUBLIC_WS,
            Self::Live(_) => Self::LIVE_PUBLIC_WS,
            Self::Custom { public_ws, .. } => public_ws,
        }
    }

    /// Returns the private WebSocket URL for this environment.
    pub fn private_ws_url(&self) -> &str {
        match self {
            Self::Demo => Self::DEMO_PRIVATE_WS,
            Self::Live(_) => Self::LIVE_PRIVATE_WS,
            Self::Custom { private_ws, .. } => private_ws,
        }
    }

    /// Returns the business WebSocket URL for this environment.
    pub fn business_ws_url(&self) -> &str {
        match self {
            Self::Demo => Self::DEMO_BUSINESS_WS,
            Self::Live(_) => Self::LIVE_BUSINESS_WS,
            Self::Custom { business_ws, .. } => business_ws,
        }
    }
}

/// Fully immutable client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub environment: Environment,
    pub credentials: Option<Credentials>,
    pub timeout: Duration,
    pub proxy: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            environment: Environment::Demo,
            credentials: None,
            timeout: Duration::from_secs(15),
            proxy: None,
        }
    }
}

impl ClientConfig {
    /// Returns a builder-like Demo configuration with optional credentials.
    pub fn demo(credentials: Option<Credentials>) -> Self {
        Self {
            credentials,
            ..Self::default()
        }
    }

    /// Creates an explicitly confirmed live configuration.
    pub fn live(credentials: Credentials, confirmation: LiveTradingConfirmation) -> Self {
        Self {
            environment: Environment::Live(confirmation),
            credentials: Some(credentials),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_redact_debug_output() {
        let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
        let rendered = format!("{credentials:?}");
        assert!(rendered.contains("REDACTED"));
        assert!(!rendered.contains("secret"));
    }

    #[test]
    fn empty_credential_is_rejected() {
        assert!(Credentials::new("", "secret", "passphrase").is_err());
    }

    #[test]
    fn demo_is_the_default_environment() {
        assert_eq!(ClientConfig::default().environment, Environment::Demo);
    }

    #[test]
    fn standard_environments_expose_documented_endpoints() {
        let demo = Environment::Demo;
        assert_eq!(demo.rest_base(), "https://www.okx.com");
        assert_eq!(
            demo.public_ws_url(),
            "wss://wspap.okx.com:8443/ws/v5/public"
        );
        assert_eq!(
            demo.private_ws_url(),
            "wss://wspap.okx.com:8443/ws/v5/private"
        );
        assert_eq!(
            demo.business_ws_url(),
            "wss://wspap.okx.com:8443/ws/v5/business"
        );

        let live = Environment::Live(LiveTradingConfirmation::acknowledge_risk());
        assert_eq!(live.rest_base(), "https://www.okx.com");
        assert_eq!(live.public_ws_url(), "wss://ws.okx.com:8443/ws/v5/public");
        assert_eq!(live.private_ws_url(), "wss://ws.okx.com:8443/ws/v5/private");
        assert_eq!(
            live.business_ws_url(),
            "wss://ws.okx.com:8443/ws/v5/business"
        );
    }
}
