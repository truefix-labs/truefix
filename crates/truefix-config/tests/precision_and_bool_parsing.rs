//! T093/T094 (US3, feature 007): `TimeStampPrecision=seconds` (lowercase) parses correctly
//! (BUG-63, FR-036); `SocketUseSSL=true` (not `Y`/`N`) is a typed config error rather than
//! silently `false` (BUG-64, FR-036) — previously any non-"Y" value silently disabled TLS with no
//! diagnostic at all, and every other parser in this file was already case-insensitive except this
//! one.

use truefix_config::{ConfigError, ResolvedSession, SessionSettings};
use truefix_session::TimeStampPrecision;

fn resolved(cfg: &str) -> ResolvedSession {
    SessionSettings::parse(cfg)
        .unwrap()
        .resolve()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
}

fn resolve_err(cfg: &str) -> ConfigError {
    SessionSettings::parse(cfg).unwrap().resolve().unwrap_err()
}

fn base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=initiator\n\
         SocketConnectHost=127.0.0.1\nSocketConnectPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

// --- BUG-63: TimeStampPrecision is case-insensitive ---

#[test]
fn timestamp_precision_lowercase_seconds_parses() {
    let rs = resolved(&base("TimeStampPrecision=seconds\n"));
    assert_eq!(rs.session.timestamp_precision, TimeStampPrecision::Seconds);
}

#[test]
fn timestamp_precision_mixed_case_millis_parses() {
    let rs = resolved(&base("TimeStampPrecision=Millis\n"));
    assert_eq!(
        rs.session.timestamp_precision,
        TimeStampPrecision::Milliseconds
    );
}

#[test]
fn timestamp_precision_uppercase_still_parses() {
    let rs = resolved(&base("TimeStampPrecision=NANOS\n"));
    assert_eq!(
        rs.session.timestamp_precision,
        TimeStampPrecision::Nanoseconds
    );
}

#[test]
fn timestamp_precision_unrecognized_value_is_still_a_typed_error() {
    let err = resolve_err(&base("TimeStampPrecision=nonsense\n"));
    assert!(matches!(err, ConfigError::InvalidValue { .. }));
}

// --- BUG-64: SocketUseSSL rejects a non-Y/N value instead of silently disabling TLS ---

#[test]
fn socket_use_ssl_true_is_a_typed_error_not_silent_false() {
    let err = resolve_err(&base("SocketUseSSL=true\n"));
    match err {
        ConfigError::InvalidValue { key, .. } => assert_eq!(key, "SocketUseSSL"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn socket_use_ssl_y_still_enables_tls_path() {
    // Y alone (no key/trust store) should fail for a *different*, TLS-specific reason (missing
    // key store) -- proving Y was recognized and TLS resolution was actually attempted, not
    // short-circuited.
    let err = resolve_err(&base("SocketUseSSL=Y\n"));
    assert!(matches!(err, ConfigError::MissingRequired { .. }));
}

#[test]
fn socket_use_ssl_n_disables_tls_with_no_error() {
    let rs = resolved(&base("SocketUseSSL=N\n"));
    assert!(rs.tls.is_none());
}

#[test]
fn socket_use_ssl_absent_defaults_to_no_tls_with_no_error() {
    let rs = resolved(&base(""));
    assert!(rs.tls.is_none());
}
