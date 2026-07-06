//! T158 (NEW-88) — an invalid/typo'd `DefaultApplVerID(1137)` on a FIXT 1.1 Logon is rejected
//! against the registered `FixtDictionaries` application keys, rather than stored unvalidated.

use truefix_core::{Field, Message};
use truefix_dict::{FixtDictionaries, ValidationOptions, load_fix44, load_fixt11};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIXT.1.1", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn logon_with_default_appl_ver_id(appl_ver_id: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIXT.1.1"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::int(34, 1));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m.body.set(Field::string(1137, appl_ver_id));
    m
}

fn session_with_fixt_dictionaries() -> Session {
    let mut s = Session::new(cfg());
    let dicts = FixtDictionaries::new(load_fixt11().unwrap())
        .with_application("FIX.4.4", load_fix44().unwrap());
    s.set_fixt_dictionaries(dicts, ValidationOptions::default());
    s.handle(Event::Connected);
    s
}

#[test]
fn an_invalid_default_appl_ver_id_is_rejected() {
    let mut s = session_with_fixt_dictionaries();
    let actions = s.handle(Event::Received(logon_with_default_appl_ver_id("FIX.9.9")));

    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "an unregistered DefaultApplVerID must reject the Logon, not accept it"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("5"))),
        "a Logout must be sent"
    );
    assert!(
        actions.iter().any(|a| matches!(a, Action::Disconnect)),
        "the connection must be torn down"
    );
}

#[test]
fn a_valid_default_appl_ver_id_still_logs_on() {
    let mut s = session_with_fixt_dictionaries();
    s.handle(Event::Received(logon_with_default_appl_ver_id("FIX.4.4")));

    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.negotiated_appl_ver_id(), Some("FIX.4.4"));
}

#[test]
fn no_fixt_dictionaries_configured_leaves_default_appl_ver_id_unvalidated() {
    // Only a `FixtDictionaries`-backed session has anything to validate against; a session with
    // no dictionary configured at all is unaffected by this fix.
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon_with_default_appl_ver_id("FIX.9.9")));

    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.negotiated_appl_ver_id(), Some("FIX.9.9"));
}
