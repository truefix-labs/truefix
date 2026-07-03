//! T038 (US5) — the engine mechanisms behind typed callback outcomes (FR-016):
//! `Session::reject_logon` (from_admin -> Err(Reject)), `Session::discard_sent` (to_app ->
//! Err(DoNotSend)), and `Session::business_reject` (from_app -> Err(BusinessReject)).

use truefix_core::{BusinessReject, Field, Message, Reject};
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

fn resend_request(seq: i64, begin: i64, end: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "2"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(7, begin));
    m.body.set(Field::int(16, end));
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
fn reject_logon_sends_logout_with_text_and_disconnects() {
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
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].msg_type(), Some("5")); // Logout
    assert_eq!(
        msgs[0].body.get(58).unwrap().as_str().unwrap(),
        "bad credentials"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn discarded_sent_message_gap_fills_instead_of_replaying() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));

    // Send an application message (outbound seq 2); simulate `to_app` returning DoNotSend by
    // discarding it from the resend store immediately (as the transport would).
    let mut order = Message::new();
    order.header.set(Field::string(35, "D"));
    order.body.set(Field::string(11, "SUPPRESSED"));
    let actions = s.send_app(order);
    let seq = match &actions[0] {
        Action::Send(m) | Action::Resend(m, _) => {
            m.header.get(34).and_then(|f| f.as_int().ok()).unwrap_or(0)
        }
        Action::Disconnect | Action::ResetStore => 0,
    };
    assert_eq!(seq, 2);
    s.discard_sent(seq as u64);

    // A ResendRequest for that seq should now gap-fill rather than replay the suppressed content.
    let actions = s.handle(Event::Received(resend_request(2, 2, 2)));
    let msgs = sends(&actions);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].msg_type(), Some("4")); // SequenceReset, not the suppressed order
    assert_eq!(msgs[0].body.get(123).unwrap().as_str().unwrap(), "Y"); // GapFillFlag
}

#[test]
fn business_reject_carries_reason_and_ref_tag() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));

    let mut inbound = Message::new();
    inbound.header.set(Field::string(8, "FIX.4.4"));
    inbound.header.set(Field::string(35, "D"));
    inbound.header.set(Field::int(34, 2));
    inbound.header.set(Field::string(49, "CLIENT"));
    inbound.header.set(Field::string(56, "SERVER"));

    let breject = BusinessReject {
        reason: 99,
        ref_tag: Some(11),
        text: Some("unknown symbol".to_owned()),
    };
    let action = s.business_reject(&inbound, &breject);
    let Action::Send(msg) = action else {
        panic!("expected a Send action");
    };
    assert_eq!(msg.msg_type(), Some("j")); // BusinessMessageReject
    assert_eq!(msg.body.get(380).unwrap().as_str().unwrap(), "99"); // BusinessRejectReason
    assert_eq!(msg.body.get(371).unwrap().as_str().unwrap(), "11"); // RefTagID
    assert_eq!(
        msg.body.get(58).unwrap().as_str().unwrap(),
        "unknown symbol"
    );
}
