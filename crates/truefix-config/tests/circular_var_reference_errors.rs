//! T147 (NEW-46) — a self-referential or circular `${var}` reference errors instead of silently
//! emitting literal unresolved text.

use truefix_config::{ConfigError, SessionSettings};

#[test]
fn self_referential_variable_is_a_circular_reference_error() {
    let cfg = "[SESSION]\nSenderCompID=A\nX=${X}\n";
    let err = SessionSettings::parse(cfg).unwrap_err();
    assert!(matches!(
        err,
        ConfigError::CircularVariableReference { name, .. } if name == "X"
    ));
}

#[test]
fn two_hop_circular_variable_reference_is_an_error() {
    let cfg = "[SESSION]\nSenderCompID=A\nX=${Y}\nY=${X}\n";
    let err = SessionSettings::parse(cfg).unwrap_err();
    assert!(matches!(err, ConfigError::CircularVariableReference { .. }));
}

#[test]
fn a_non_circular_transitive_chain_resolves_fully() {
    let cfg = "[SESSION]\nSenderCompID=A\nX=${Y}\nY=literal-value\n";
    let s = SessionSettings::parse(cfg).unwrap();
    assert_eq!(s.sessions()[0].get("X"), Some(&"literal-value".to_owned()));
}
