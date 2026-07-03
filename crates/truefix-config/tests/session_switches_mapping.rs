//! T029 (US4) — the 12 session config switches map from a settings file into `SessionConfig`
//! (FR-008).

use truefix_config::SessionSettings;
use truefix_session::SessionConfig;

fn resolved(cfg: &str) -> SessionConfig {
    SessionSettings::parse(cfg)
        .unwrap()
        .resolve()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .session
}

fn base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

#[test]
fn defaults_match_session_config_new() {
    let c = resolved(&base(""));
    let d = SessionConfig::new("FIX.4.4", "S", "T", c.role);
    assert_eq!(
        c.send_redundant_resend_requests,
        d.send_redundant_resend_requests
    );
    assert_eq!(c.reset_on_error, d.reset_on_error);
    assert_eq!(c.disconnect_on_error, d.disconnect_on_error);
    assert_eq!(c.disable_heart_beat_check, d.disable_heart_beat_check);
    assert_eq!(c.refresh_on_logon, d.refresh_on_logon);
    assert_eq!(
        c.force_resend_when_corrupted_store,
        d.force_resend_when_corrupted_store
    );
    assert_eq!(c.logon_tags, d.logon_tags);
}

#[test]
fn all_boolean_switches_map_from_y() {
    let c = resolved(&base(
        "SendRedundantResendRequests=Y\nResetOnError=Y\nDisconnectOnError=Y\n\
         DisableHeartBeatCheck=Y\nRefreshOnLogon=Y\nForceResendWhenCorruptedStore=Y\n",
    ));
    assert!(c.send_redundant_resend_requests);
    assert!(c.reset_on_error);
    assert!(c.disconnect_on_error);
    assert!(c.disable_heart_beat_check);
    assert!(c.refresh_on_logon);
    assert!(c.force_resend_when_corrupted_store);
}

// --- 005/T027 (US3, GAP-08/FR-009): `RequiresOrigSendingTime` also gates the session-layer
// anti-replay check, in addition to the pre-existing dictionary-level validation toggle. ---

#[test]
fn requires_orig_sending_time_defaults_to_false() {
    let c = resolved(&base(""));
    assert!(!c.requires_orig_sending_time_on_low_seq);
}

#[test]
fn requires_orig_sending_time_y_sets_the_session_level_switch() {
    let c = resolved(&base("RequiresOrigSendingTime=Y\n"));
    assert!(c.requires_orig_sending_time_on_low_seq);
}

#[test]
fn logon_tag_maps_tag_and_value() {
    let c = resolved(&base("LogonTag=9001=HOUSE-ID\n"));
    assert_eq!(c.logon_tags, vec![(9001, "HOUSE-ID".to_owned())]);
}

#[test]
fn malformed_logon_tag_is_a_typed_error() {
    let err = SessionSettings::parse(&base("LogonTag=not-a-tag\n"))
        .unwrap()
        .resolve()
        .unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::InvalidValue { .. }
    ));
}

#[test]
fn numbered_logon_tags_are_all_parsed_in_order() {
    // T077 (US8, feature 006): GAP-12, `LogonTag`/`LogonTag1`/`LogonTag2`/… all map in, in
    // ascending numeric-suffix order (`LogonTag` itself sorts first).
    let c = resolved(&base(
        "LogonTag=9001=HOUSE-ID\nLogonTag1=9002=DESK-ID\nLogonTag2=9003=TRADER-ID\n",
    ));
    assert_eq!(
        c.logon_tags,
        vec![
            (9001, "HOUSE-ID".to_owned()),
            (9002, "DESK-ID".to_owned()),
            (9003, "TRADER-ID".to_owned()),
        ]
    );
}

#[test]
fn logon_tag1_alone_without_bare_logon_tag_still_parses() {
    let c = resolved(&base("LogonTag1=9002=DESK-ID\n"));
    assert_eq!(c.logon_tags, vec![(9002, "DESK-ID".to_owned())]);
}

#[test]
fn malformed_numbered_logon_tag_is_a_typed_error() {
    let err = SessionSettings::parse(&base("LogonTag1=not-a-tag\n"))
        .unwrap()
        .resolve()
        .unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::InvalidValue { .. }
    ));
}

/// T078 (US14, FR-019): `InChanCapacity` absent means unbounded/unchanged behavior (`None`),
/// matching the pre-US14 default `SessionConfig::new` already had for `in_chan_capacity`.
#[test]
fn in_chan_capacity_defaults_to_none() {
    let c = resolved(&base(""));
    assert_eq!(c.in_chan_capacity, None);
}

#[test]
fn in_chan_capacity_maps_from_settings() {
    let c = resolved(&base("InChanCapacity=64\n"));
    assert_eq!(c.in_chan_capacity, Some(64));
}

#[test]
fn malformed_in_chan_capacity_is_a_typed_error() {
    let err = SessionSettings::parse(&base("InChanCapacity=not-a-number\n"))
        .unwrap()
        .resolve()
        .unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::InvalidValue { .. }
    ));
}
