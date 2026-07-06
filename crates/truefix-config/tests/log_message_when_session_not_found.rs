//! T051/T052 (US1, feature 009, `NEW-96`): `LogMessageWhenSessionNotFound` was registered `Impl`
//! in the key registry (`crates/truefix-config/src/keys.rs`), and `Services` genuinely has and
//! consumes a matching field, but no `.cfg` -> `ResolvedSession`/`Services` path existed for it at
//! all -- every `Services { ... }` construction site in `crates/truefix/src/lib.rs` built via
//! `..Services::default()`, so the setting was permanently `false` regardless of what a `.cfg`
//! file set.

use truefix_config::{ResolvedSession, SessionSettings};

fn resolved(cfg: &str) -> ResolvedSession {
    SessionSettings::parse(cfg)
        .unwrap()
        .resolve()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
}

fn base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

#[test]
fn defaults_to_false_when_unset() {
    let rs = resolved(&base(""));
    assert!(!rs.log_message_when_session_not_found);
}

#[test]
fn log_message_when_session_not_found_y_maps_to_true() {
    let rs = resolved(&base("LogMessageWhenSessionNotFound=Y\n"));
    assert!(
        rs.log_message_when_session_not_found,
        "LogMessageWhenSessionNotFound=Y must map to ResolvedSession.\
         log_message_when_session_not_found = true (NEW-96)"
    );
}

#[test]
fn log_message_when_session_not_found_n_maps_to_false() {
    let rs = resolved(&base("LogMessageWhenSessionNotFound=N\n"));
    assert!(!rs.log_message_when_session_not_found);
}
