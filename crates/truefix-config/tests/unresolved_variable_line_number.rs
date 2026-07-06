//! T172 (NEW-47) — `UnresolvedVariable` reports the original source line, even after per-session
//! merging with `[DEFAULT]`, rather than always reporting `line: 0`.

use truefix_config::{ConfigError, SessionSettings};

#[test]
fn unresolved_variable_in_a_plain_session_reports_its_source_line() {
    let cfg = "[SESSION]\nSenderCompID=A\nX=${Missing}\n";
    // Line 1 is `[SESSION]`, line 3 is the offending `X=${Missing}`.
    let err = SessionSettings::parse(cfg).unwrap_err();
    assert_eq!(
        err,
        ConfigError::UnresolvedVariable {
            line: 3,
            name: "Missing".to_owned()
        }
    );
}

#[test]
fn unresolved_variable_inherited_from_default_reports_the_default_section_s_line() {
    let cfg = "[DEFAULT]\nX=${Missing}\n\n[SESSION]\nSenderCompID=A\n";
    // Line 1 is `[DEFAULT]`, line 2 is `X=${Missing}` — the key isn't overridden by the session,
    // so the reported line must still point at its original DEFAULT declaration, not line 0.
    let err = SessionSettings::parse(cfg).unwrap_err();
    assert_eq!(
        err,
        ConfigError::UnresolvedVariable {
            line: 2,
            name: "Missing".to_owned()
        }
    );
}

#[test]
fn unresolved_variable_overridden_per_session_reports_the_session_s_own_line() {
    let cfg = "[DEFAULT]\nX=default-value\n\n[SESSION]\nSenderCompID=A\nX=${Missing}\n";
    // The session's own `X=${Missing}` (line 6) overrides the default and must be the line
    // attributed to the error, not the default's line 2.
    let err = SessionSettings::parse(cfg).unwrap_err();
    assert_eq!(
        err,
        ConfigError::UnresolvedVariable {
            line: 6,
            name: "Missing".to_owned()
        }
    );
}
