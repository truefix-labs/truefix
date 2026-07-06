//! T031/T033 (US1, feature 009, `NEW-63`): a Logon failing dictionary validation with
//! `disconnect_on_error=false` sent only a bare session-level Reject and left the session
//! stranded in `AwaitingLogon` -- every other Logon-rejection path in `on_logon` routes through
//! `reject_logon()` (Logout + disconnect) unconditionally. Only the dictionary-validation-failure
//! path was gated on `disconnect_on_error`.

use truefix_core::{Field, Message};
use truefix_dict::{ValidationOptions, load_fix44};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg(disconnect_on_error: bool) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c.disconnect_on_error = disconnect_on_error;
    c
}

/// A dictionary-invalid Logon: missing the required EncryptMethod(98) field.
fn dict_invalid_logon() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::int(34, 1));
    m.body.set(Field::int(108, 30));
    m
}

fn sends(actions: &[Action]) -> Vec<&Message> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Send(m) | Action::Resend(m, _) => Some(m),
            Action::Disconnect | Action::ResetStore => None,
        })
        .collect()
}

#[test]
fn dictionary_invalid_logon_sends_logout_and_disconnects_even_without_disconnect_on_error() {
    let mut s = Session::new(cfg(false));
    s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(dict_invalid_logon()));

    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a dictionary-invalid Logon must disconnect the session (not strand it in \
         AwaitingLogon), regardless of disconnect_on_error, got actions: {actions:?}"
    );
    assert!(
        actions.iter().any(|a| matches!(a, Action::Disconnect)),
        "expected Action::Disconnect, got: {actions:?}"
    );
    let out = sends(&actions);
    assert!(
        out.iter().any(|m| m.msg_type() == Some("5")),
        "expected a Logout among the rejection actions, got: {out:?}"
    );
}

#[test]
fn dictionary_invalid_logon_sends_logout_and_disconnects_with_disconnect_on_error() {
    // Control: the disconnect_on_error=true case already worked before this fix.
    let mut s = Session::new(cfg(true));
    s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(dict_invalid_logon()));
    assert_eq!(s.state(), SessionState::Disconnected);
    let out = sends(&actions);
    assert!(out.iter().any(|m| m.msg_type() == Some("5")));
}
