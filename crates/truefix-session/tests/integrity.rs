//! T029 (US4) — inbound identity checks + correctness switches (FR-007/009/010).

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 1;
    c.check_latency = false;
    c
}

fn logon(begin: &str, sender: &str, target: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, sender));
    m.header.set(Field::string(56, target));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 1)); // 1-second heartbeat (adopted by the acceptor)
    m
}

fn sent(actions: &[Action]) -> Option<&Message> {
    actions.iter().find_map(|a| match a {
        Action::Send(m) => Some(m),
        Action::Disconnect => None,
    })
}

fn disconnects(actions: &[Action]) -> bool {
    actions.iter().any(|a| matches!(a, Action::Disconnect))
}

#[test]
fn mismatched_sender_comp_id_aborts_session() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    // Peer's SenderCompID (49) should be CLIENT (our target); WRONG is a CompID problem.
    let actions = s.handle(Event::Received(logon("FIX.4.4", "WRONG", "SERVER")));
    assert_eq!(sent(&actions).and_then(Message::msg_type), Some("5")); // Logout
    assert!(disconnects(&actions));
    assert_eq!(s.state(), SessionState::Disconnected);
}

#[test]
fn mismatched_begin_string_aborts_session() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(logon("FIX.4.2", "CLIENT", "SERVER")));
    assert_eq!(sent(&actions).and_then(Message::msg_type), Some("5"));
    assert!(disconnects(&actions));
}

#[test]
fn matching_identity_logs_on() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(logon("FIX.4.4", "CLIENT", "SERVER")));
    assert_eq!(sent(&actions).and_then(Message::msg_type), Some("A")); // Logon response
    assert_eq!(s.state(), SessionState::LoggedOn);
}

#[test]
fn heartbeat_timeout_multiplier_controls_disconnect() {
    let mut c = acc_cfg();
    c.heartbeat_timeout_multiplier = 1; // disconnect at hb*1 + 2 = 3 idle ticks
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon("FIX.4.4", "CLIENT", "SERVER")));

    // Two idle ticks: still alive (a TestRequest may be emitted, but no disconnect yet).
    assert!(!disconnects(&s.handle(Event::Tick)));
    assert!(!disconnects(&s.handle(Event::Tick)));
    // Third idle tick crosses hb*1 + 2 = 3 → disconnect.
    assert!(disconnects(&s.handle(Event::Tick)));
}
