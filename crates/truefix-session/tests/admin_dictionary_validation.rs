//! T034/T035 (US1, feature 007): a dictionary-invalid admin message is rejected via the plain
//! session-level `Reject` path when a dictionary is configured (BUG-89/FR-011) — previously
//! `validate_app` returned `None` unconditionally for every admin-typed message
//! (`is_admin_type(msg.msg_type())`), so a malformed Logon/Heartbeat/etc. sailed through
//! unvalidated regardless of whether a dictionary was attached.

use truefix_core::{Field, Message};
use truefix_dict::{ValidationOptions, load_fix44};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

/// A Logon missing the dictionary-required `EncryptMethod(98)` field.
fn invalid_logon() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::int(34, 1));
    m.body.set(Field::int(108, 30)); // HeartBtInt only -- EncryptMethod(98) deliberately omitted
    m
}

fn valid_logon() -> Message {
    let mut m = invalid_logon();
    m.body.set(Field::int(98, 0));
    m
}

fn sent(actions: &[Action]) -> Vec<&Message> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Send(m) | Action::Resend(m, _) => Some(m),
            Action::Disconnect | Action::ResetStore => None,
        })
        .collect()
}

#[test]
fn a_dictionary_invalid_logon_is_rejected_via_the_plain_reject_path() {
    let mut s = Session::new(cfg());
    s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(invalid_logon()));

    assert_ne!(
        s.state(),
        SessionState::LoggedOn,
        "a dictionary-invalid Logon must not log the session on"
    );
    let out = sent(&actions);
    let reject = out
        .iter()
        .find(|m| m.msg_type() == Some("3"))
        .expect("a plain session-level Reject (35=3), not a BusinessMessageReject (35=j)");
    assert!(
        !out.iter().any(|m| m.msg_type() == Some("j")),
        "a missing-required-field Logon failure must route through Reject, not \
         BusinessMessageReject -- got {out:?}"
    );
    assert!(
        reject.body.get(373).is_some(),
        "the Reject should carry a SessionRejectReason"
    );
}

#[test]
fn a_dictionary_valid_logon_still_logs_on() {
    let mut s = Session::new(cfg());
    s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    s.handle(Event::Connected);
    s.handle(Event::Received(valid_logon()));
    assert_eq!(s.state(), SessionState::LoggedOn);
}

/// A Heartbeat carrying an unknown (not-in-dictionary) tag is rejected via the plain session-level
/// `Reject` path too, once logged on.
#[test]
fn a_dictionary_invalid_heartbeat_is_rejected_via_the_plain_reject_path() {
    let mut s = Session::new(cfg());
    s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    s.handle(Event::Connected);
    s.handle(Event::Received(valid_logon()));
    assert_eq!(s.state(), SessionState::LoggedOn);

    let mut hb = Message::new();
    hb.header.set(Field::string(8, "FIX.4.4"));
    hb.header.set(Field::string(35, "0"));
    hb.header.set(Field::string(49, "YOU"));
    hb.header.set(Field::string(56, "ME"));
    hb.header.set(Field::int(34, 2));
    hb.body.set(Field::string(4321, "not-a-real-tag")); // unknown tag, below the UDF range (5000+)

    let actions = s.handle(Event::Received(hb));
    let out = sent(&actions);
    assert!(
        out.iter().any(|m| m.msg_type() == Some("3")),
        "an unknown-tag Heartbeat should be rejected via the plain Reject path, got {out:?}"
    );
}
