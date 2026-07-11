use truefix_okx_client::{ClientConfig, Credentials, Environment, LiveTradingConfirmation};

#[test]
fn demo_is_the_default_even_when_credentials_are_supplied() {
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let config = ClientConfig::demo(Some(credentials));
    assert_eq!(config.environment, Environment::Demo);
}

#[test]
fn live_environment_requires_the_typed_confirmation_token() {
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let config = ClientConfig::live(credentials, LiveTradingConfirmation::acknowledge_risk());
    assert!(matches!(config.environment, Environment::Live(_)));
}

#[test]
fn credential_debug_representation_never_discloses_identity_material() {
    let credentials = Credentials::new("api-key", "api-secret", "passphrase").unwrap();
    let rendered = format!("{credentials:?}");
    for secret in ["api-key", "api-secret", "passphrase"] {
        assert!(!rendered.contains(secret));
    }
}
