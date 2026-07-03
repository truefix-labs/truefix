//! T052 — dictionary validation wired into the session inbound path.

use truefix_core::{Field, Message};
use truefix_dict::{load_fix44, ValidationOptions};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false; // app fixtures use fixed timestamps
    c
}

fn logon() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::int(34, 1));
    m.body.set(Field::int(108, 30));
    m.body.set(Field::string(141, "Y"));
    m
}

fn new_order(side: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(11, "ORD1"));
    m.body.set(Field::string(21, "1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::string(54, side)); // Side
    m.body.set(Field::string(60, "20240101-00:00:00"));
    m.body.set(Field::string(40, "2"));
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

fn logged_on() -> Session {
    let mut s = Session::new(cfg());
    s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon()));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.next_in_seq(), 2);
    s
}

#[test]
fn valid_app_message_is_accepted() {
    let mut s = logged_on();
    let actions = s.handle(Event::Received(new_order("1", 2))); // valid Side
                                                                // no reject emitted; sequence advanced
    assert!(sent(&actions).iter().all(|m| m.msg_type() != Some("3")));
    assert_eq!(s.next_in_seq(), 3);
}

#[test]
fn invalid_app_message_is_rejected_and_seq_advances() {
    let mut s = logged_on();
    let actions = s.handle(Event::Received(new_order("Z", 2))); // bad Side enum
    let out = sent(&actions);
    let reject = out
        .iter()
        .find(|m| m.msg_type() == Some("3"))
        .expect("a session-level Reject");
    assert_eq!(reject.body.get(45).unwrap().as_int().unwrap(), 2); // RefSeqNum
    assert_eq!(reject.body.get(373).unwrap().as_int().unwrap(), 5); // ValueIsIncorrect
    assert_eq!(s.next_in_seq(), 3); // message consumed
}

#[test]
fn unknown_msg_type_triggers_business_reject() {
    let mut s = logged_on();
    // structurally valid but unknown application MsgType "UU"
    let mut m = new_order("1", 2);
    m.header.set(Field::string(35, "UU"));
    let actions = s.handle(Event::Received(m));
    let out = sent(&actions);
    assert!(
        out.iter().any(|m| m.msg_type() == Some("j")),
        "expected a BusinessMessageReject (35=j)"
    );
}

#[test]
fn admin_messages_are_not_dictionary_rejected() {
    // Without a dictionary set, behavior is unchanged; with one, admin still flows.
    let mut s = logged_on();
    let mut hb = Message::new();
    hb.header.set(Field::string(8, "FIX.4.4"));
    hb.header.set(Field::string(35, "0"));
    hb.header.set(Field::int(34, 2));
    let actions = s.handle(Event::Received(hb));
    assert!(sent(&actions).iter().all(|m| m.msg_type() != Some("3")));
    assert_eq!(s.next_in_seq(), 3);
}
