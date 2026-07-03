//! T035 (US5, feature 005) — the 5 identity keys (SenderSubID/SenderLocationID/TargetSubID/
//! TargetLocationID/SessionQualifier) map from a settings file into `SessionConfig` (GAP-47/
//! FR-012).

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
fn identity_fields_default_to_none() {
    let c = resolved(&base(""));
    assert_eq!(c.sender_sub_id, None);
    assert_eq!(c.sender_location_id, None);
    assert_eq!(c.target_sub_id, None);
    assert_eq!(c.target_location_id, None);
    assert_eq!(c.session_qualifier, None);
}

#[test]
fn all_five_identity_keys_map_from_settings() {
    let c = resolved(&base(
        "SenderSubID=SS\nSenderLocationID=SL\nTargetSubID=TS\nTargetLocationID=TL\n\
         SessionQualifier=Q1\n",
    ));
    assert_eq!(c.sender_sub_id.as_deref(), Some("SS"));
    assert_eq!(c.sender_location_id.as_deref(), Some("SL"));
    assert_eq!(c.target_sub_id.as_deref(), Some("TS"));
    assert_eq!(c.target_location_id.as_deref(), Some("TL"));
    assert_eq!(c.session_qualifier.as_deref(), Some("Q1"));
}

#[test]
fn session_id_carries_the_full_identity() {
    let c = resolved(&base(
        "SenderSubID=SS\nSenderLocationID=SL\nTargetSubID=TS\nTargetLocationID=TL\n\
         SessionQualifier=Q1\n",
    ));
    let id = c.session_id();
    assert_eq!(id.sender_sub_id.as_deref(), Some("SS"));
    assert_eq!(id.sender_location_id.as_deref(), Some("SL"));
    assert_eq!(id.target_sub_id.as_deref(), Some("TS"));
    assert_eq!(id.target_location_id.as_deref(), Some("TL"));
    assert_eq!(id.session_qualifier.as_deref(), Some("Q1"));
}

#[test]
fn two_sessions_differing_only_by_qualifier_produce_distinct_session_ids() {
    let cfg = "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\nSessionQualifier=A\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\nSessionQualifier=B\n";
    let sessions = SessionSettings::parse(cfg).unwrap().resolve().unwrap();
    assert_eq!(sessions.len(), 2);
    let ids: Vec<_> = sessions.iter().map(|s| s.session.session_id()).collect();
    assert_ne!(ids[0], ids[1]);
}
