//! T051 (US10) — a `from_admin` refusal carrying a `SessionStatus` (tag 573) reason propagates
//! onto the outbound Logout (FR-013).
//!
//! `on_before_reset`'s own firing (T050) is a transport-layer concern, not a `Session` one:
//! `Session` is deliberately sans-IO and never holds an `Application` handle (the same boundary
//! `Action::ResetStore` already established for the durable store reset), so the transport is
//! what actually invokes `Application` callbacks. See `crates/truefix-transport/tests/
//! application_hooks.rs` for the end-to-end `on_before_reset` test.

use truefix_core::{Field, Message, Reject};
use truefix_session::{Action, Event, Role, Session, SessionConfig};

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn logon(seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
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
fn reject_with_session_status_stamps_tag_573_on_the_outbound_logout() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));

    let reject = Reject {
        reason: 0,
        ref_tag: None,
        text: Some("session not active".to_owned()),
        session_status: Some(1), // 1 = SessionActiveNotLoggedOnYet (illustrative)
    };
    let actions = s.reject_logon(&reject);
    let msgs = sends(&actions);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].msg_type(), Some("5")); // Logout
    assert_eq!(msgs[0].body.get(573).unwrap().as_str().unwrap(), "1");
}

#[test]
fn reject_without_session_status_omits_tag_573() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));

    let reject = Reject {
        reason: 0,
        ref_tag: None,
        text: Some("bad credentials".to_owned()),
        session_status: None,
    };
    let actions = s.reject_logon(&reject);
    let msgs = sends(&actions);
    assert!(msgs[0].body.get(573).is_none());
}
