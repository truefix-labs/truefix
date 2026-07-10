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
}
