//! T008 (US1) — settings→engine mapping fails fast with typed, key-naming errors (FR-015).

use truefix_config::{ConfigError, SessionSettings};

fn resolve_err(cfg: &str) -> ConfigError {
    SessionSettings::parse(cfg)
        .expect("parse")
        .resolve()
        .expect_err("expected a resolution error")
}

#[test]
fn missing_required_key_names_key() {
    let cfg = "[DEFAULT]\nBeginString=FIX.4.4\n\
               [SESSION]\nConnectionType=acceptor\nSenderCompID=S\nTargetCompID=T\n";
    match resolve_err(cfg) {
        ConfigError::MissingRequired { key, .. } => assert_eq!(key, "SocketAcceptPort"),
        other => panic!("expected MissingRequired, got {other:?}"),
    }
}

#[test]
fn unknown_connection_type_is_typed() {
    let cfg = "[DEFAULT]\nBeginString=FIX.4.4\n\
               [SESSION]\nConnectionType=proxy\nSenderCompID=S\nTargetCompID=T\nSocketAcceptPort=5001\n";
    assert!(matches!(
        resolve_err(cfg),
        ConfigError::UnknownConnectionType { .. }
    ));
}

#[test]
fn invalid_integer_value_names_key() {
    let cfg = "[DEFAULT]\nBeginString=FIX.4.4\n\
               [SESSION]\nConnectionType=acceptor\nSenderCompID=S\nTargetCompID=T\n\
               SocketAcceptPort=5001\nHeartBtInt=soon\n";
    match resolve_err(cfg) {
        ConfigError::InvalidValue { key, .. } => assert_eq!(key, "HeartBtInt"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn default_value_is_overridden_per_session() {
    let cfg = "[DEFAULT]\nBeginString=FIX.4.4\nHeartBtInt=30\nConnectionType=acceptor\n\
               SocketAcceptPort=5001\n\
               [SESSION]\nSenderCompID=S\nTargetCompID=T\nHeartBtInt=45\n";
    let resolved = SessionSettings::parse(cfg).unwrap().resolve().unwrap();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].session.heartbeat_interval, 45);
}
