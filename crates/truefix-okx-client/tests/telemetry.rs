use truefix_okx_client::{
    config::Credentials, error::OkxError, limiter::RateLimiter, request::RetrySafety,
};

#[test]
fn telemetry_context_is_redacted_and_writes_are_not_retried() {
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    assert!(!format!("{credentials:?}").contains("secret"));
    assert!(!RateLimiter::may_retry(
        RetrySafety::NeverReplay,
        &OkxError::UnknownCompletion
    ));
}
