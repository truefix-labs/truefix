use std::time::Duration;

use crate::error::{IgError, IgResult};

/// Credentials bound to one IG identity.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    identifier: String,
    password: String,
    api_key: String,
}

impl Credentials {
    /// Creates credentials after rejecting empty values.
    pub fn new(
        identifier: impl Into<String>,
        password: impl Into<String>,
        api_key: impl Into<String>,
    ) -> IgResult<Self> {
        let credentials = Self {
            identifier: identifier.into(),
            password: password.into(),
            api_key: api_key.into(),
        };
        if credentials.identifier.is_empty()
            || credentials.password.is_empty()
            || credentials.api_key.is_empty()
        {
            return Err(IgError::InvalidConfiguration(
                "identifier, password, and API key must be non-empty".to_owned(),
            ));
        }
        Ok(credentials)
    }

    pub(crate) fn identifier(&self) -> &str {
        &self.identifier
    }
    pub(crate) fn password(&self) -> &str {
        &self.password
    }
    pub(crate) fn api_key(&self) -> &str {
        &self.api_key
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Credentials(REDACTED)")
    }
}

/// Marker required to select IG's live environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveTradingConfirmation(());

impl LiveTradingConfirmation {
    /// Explicitly acknowledges that this client may submit live trading requests.
    pub const fn acknowledge_risk() -> Self {
        Self(())
    }
}

/// Endpoint environment selected when the client is constructed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Environment {
    /// IG demo dealing API, suitable for simulated trading.
    #[default]
    Demo,
    /// IG production dealing API, permitted only with explicit confirmation.
    Live(LiveTradingConfirmation),
    /// Caller-controlled endpoint, primarily for tests or supported regional routing.
    Custom { rest_base: String },
}

impl Environment {
    const DEMO_REST_BASE: &str = "https://demo-api.ig.com/gateway/deal";
    const LIVE_REST_BASE: &str = "https://api.ig.com/gateway/deal";

    /// Returns the dealing API base URL.
    pub fn rest_base(&self) -> &str {
        match self {
            Self::Demo => Self::DEMO_REST_BASE,
            Self::Live(_) => Self::LIVE_REST_BASE,
            Self::Custom { rest_base } => rest_base,
        }
    }
}

/// Session authentication protocol used for REST requests.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AuthenticationVersion {
    /// IG's CST and X-SECURITY-TOKEN session authentication.
    #[default]
    V2,
    /// OAuth authentication. IG requires the active account to be selected at login.
    V3 { account_id: String },
}

/// Fully immutable client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub environment: Environment,
    pub credentials: Option<Credentials>,
    pub authentication: AuthenticationVersion,
    pub timeout: Duration,
    pub proxy: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            environment: Environment::Demo,
            credentials: None,
            authentication: AuthenticationVersion::V2,
            timeout: Duration::from_secs(15),
            proxy: None,
        }
    }
}

impl ClientConfig {
    /// Returns a demo configuration with optional credentials.
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

    /// Selects OAuth v3 authentication for the supplied IG account.
    pub fn with_v3_authentication(mut self, account_id: impl Into<String>) -> IgResult<Self> {
        let account_id = account_id.into();
        if account_id.is_empty() {
            return Err(IgError::InvalidConfiguration(
                "v3 authentication requires a non-empty account ID".to_owned(),
            ));
        }
        self.authentication = AuthenticationVersion::V3 { account_id };
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_redact_debug_output() {
        let credentials = Credentials::new("user", "secret", "key").unwrap();
        assert!(!format!("{credentials:?}").contains("secret"));
    }

    #[test]
    fn demo_is_default() {
        assert_eq!(ClientConfig::default().environment, Environment::Demo);
    }

    #[test]
    fn v3_requires_an_account_id() {
        assert!(ClientConfig::default().with_v3_authentication("").is_err());
        assert_eq!(
            ClientConfig::default()
                .with_v3_authentication("ABC123")
                .unwrap()
                .authentication,
            AuthenticationVersion::V3 {
                account_id: "ABC123".to_owned(),
            }
        );
    }
}
