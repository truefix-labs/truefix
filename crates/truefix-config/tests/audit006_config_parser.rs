//! NEW-128: initiator missing-port error names `SocketConnectPort` precisely.
//! NEW-129: duplicate `[SESSION]` identities are rejected.
//! NEW-130: a `#` inside a value with no preceding whitespace boundary is preserved.

use truefix_config::{ConfigError, SessionSettings};

#[test]
fn audit006_initiator_missing_port_names_socket_connect_port() {
    let cfg = "[SESSION]\nBeginString=FIX.4.4\nConnectionType=initiator\n\
               SenderCompID=S\nTargetCompID=T\nSocketConnectHost=127.0.0.1\n";
    let err = SessionSettings::parse(cfg).unwrap().resolve().unwrap_err();
    match err {
        ConfigError::MissingRequired { key, .. } => assert_eq!(key, "SocketConnectPort"),
        other => panic!("expected MissingRequired, got {other:?}"),
    }
}

#[test]
fn audit006_initiator_invalid_port_names_socket_connect_port() {
    let cfg = "[SESSION]\nBeginString=FIX.4.4\nConnectionType=initiator\n\
               SenderCompID=S\nTargetCompID=T\nSocketConnectHost=127.0.0.1\n\
               SocketConnectPort=notaport\n";
    let err = SessionSettings::parse(cfg).unwrap().resolve().unwrap_err();
    match err {
        ConfigError::InvalidValue { key, .. } => assert_eq!(key, "SocketConnectPort"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn audit006_duplicate_session_identity_is_rejected() {
    let cfg = "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
               [SESSION]\nSenderCompID=S\nTargetCompID=T\n\
               [SESSION]\nSenderCompID=S\nTargetCompID=T\n";
    let err = SessionSettings::parse(cfg).unwrap_err();
    assert!(matches!(err, ConfigError::DuplicateSession { .. }));
}

#[test]
fn audit006_sessions_distinguished_by_session_qualifier_are_not_duplicates() {
    let cfg = "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
               [SESSION]\nSenderCompID=S\nTargetCompID=T\nSessionQualifier=A\n\
               [SESSION]\nSenderCompID=S\nTargetCompID=T\nSessionQualifier=B\n";
    let settings = SessionSettings::parse(cfg).unwrap();
    assert_eq!(settings.sessions().len(), 2);
}

#[test]
fn audit006_sessions_distinguished_by_target_comp_id_are_not_duplicates() {
    let cfg = "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
               [SESSION]\nSenderCompID=S\nTargetCompID=T1\n\
               [SESSION]\nSenderCompID=S\nTargetCompID=T2\n";
    let settings = SessionSettings::parse(cfg).unwrap();
    assert_eq!(settings.sessions().len(), 2);
}

#[test]
fn audit006_hash_immediately_after_non_whitespace_is_preserved_in_value() {
    let cfg = "[SESSION]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
               SenderCompID=S\nTargetCompID=T\nPassword=ab#cd\n";
    let settings = SessionSettings::parse(cfg).unwrap();
    assert_eq!(
        settings.sessions()[0].get("Password"),
        Some(&"ab#cd".to_string())
    );
}
