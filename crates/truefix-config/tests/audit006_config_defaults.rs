//! Shared fixtures for audit 006 config default and strict parsing tests.
//!
//! NEW-102/NEW-103: heartbeat-timeout/test-request-delay multiplier defaults match QFJ.
//! NEW-104/NEW-111: `ValidateFieldsOutOfOrder`/`AllowPosDup` defaults match QFJ.
//! NEW-112: `EnabledProtocols` rejects unrecognized values instead of silently falling to Tls12.
//! NEW-113: every boolean config key rejects non-Y/N values instead of silently defaulting.

use truefix_config::{ConfigError, ResolvedSession, SessionSettings};

fn acceptor_cfg(extra_default: &str, extra_session: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n{extra_default}\n[SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra_session}\n"
    )
}

#[test]
fn audit006_config_fixture_builds_acceptor_session() {
    let cfg = acceptor_cfg("HeartBtInt=30", "ValidateFieldsOutOfOrder=Y");
    assert!(cfg.contains("[SESSION]"));
    assert!(cfg.contains("SocketAcceptPort=5001"));
}

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

fn initiator_base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=initiator\n\
         SocketConnectHost=127.0.0.1\nSocketConnectPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

// --- NEW-102/NEW-103: multiplier defaults match QFJ ---

#[test]
fn audit006_heartbeat_timeout_multiplier_default_matches_qfj() {
    let rs = resolved(&initiator_base(""));
    assert_eq!(rs.session.heartbeat_timeout_multiplier, 1.4);
}

#[test]
fn audit006_test_request_delay_multiplier_default_matches_qfj() {
    let rs = resolved(&initiator_base(""));
    assert_eq!(rs.session.test_request_delay_multiplier, 0.5);
}

// --- NEW-112: EnabledProtocols rejects unrecognized values ---

#[test]
fn audit006_enabled_protocols_unrecognized_value_is_a_typed_error() {
    let err = resolve_err(&initiator_base(
        "SocketUseSSL=Y\nSocketKeyStore=/nonexistent.pem\nEnabledProtocols=SSL3.0\n",
    ));
    match err {
        ConfigError::InvalidValue { key, .. } => assert_eq!(key, "EnabledProtocols"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn audit006_enabled_protocols_tls12_is_recognized() {
    let rs = resolved(&initiator_base(
        "SocketUseSSL=Y\nSocketKeyStore=/nonexistent.pem\nEnabledProtocols=TLSv1.2\n",
    ));
    assert_eq!(
        rs.tls.expect("tls configured").min_version,
        Some(truefix_config::TlsVersion::Tls12)
    );
}

// --- NEW-113: every boolean key is strict, not just SocketUseSSL ---

#[test]
fn audit006_check_latency_non_y_n_value_is_a_typed_error() {
    let err = resolve_err(&initiator_base("CheckLatency=true\n"));
    match err {
        ConfigError::InvalidValue { key, .. } => assert_eq!(key, "CheckLatency"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn audit006_disconnect_on_error_yes_is_a_typed_error_not_silent_false() {
    let err = resolve_err(&initiator_base("DisconnectOnError=yes\n"));
    match err {
        ConfigError::InvalidValue { key, .. } => assert_eq!(key, "DisconnectOnError"),
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn audit006_reset_on_logon_y_n_still_work() {
    let rs = resolved(&initiator_base("ResetOnLogon=N\n"));
    assert!(!rs.session.reset_on_logon);
    let rs = resolved(&initiator_base("ResetOnLogon=Y\n"));
    assert!(rs.session.reset_on_logon);
}

// --- NEW-104/NEW-111: default parity for dictionary validation options ---

#[test]
fn audit006_validate_fields_out_of_order_defaults_true() {
    let rs = resolved(&format!(
        "{}UseDataDictionary=Y\nDataDictionary=FIX.4.4\n",
        initiator_base("")
    ));
    let (_, opts) = rs.validator.expect("validator wired");
    assert!(opts.validate_fields_out_of_order);
}

#[test]
fn audit006_allow_pos_dup_defaults_false() {
    let rs = resolved(&format!(
        "{}UseDataDictionary=Y\nDataDictionary=FIX.4.4\n",
        initiator_base("")
    ));
    let (_, opts) = rs.validator.expect("validator wired");
    assert!(!opts.allow_pos_dup);
}
