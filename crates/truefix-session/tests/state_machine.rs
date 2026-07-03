//! T023 — session state-machine transition tests (deterministic, sans-IO).

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg(role: Role) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.2", "ME", "YOU", role);
    c.heartbeat_interval = 1;
    c
}

fn inbound(msg_type: &str, seq: i64, hb: Option<i64>, test_req: Option<&str>) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.2"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::int(34, seq));
    if let Some(h) = hb {
        m.body.set(Field::int(108, h));
    }
    if let Some(t) = test_req {
        m.body.set(Field::string(112, t));
    }
    m
}

fn first_send(actions: &[Action]) -> Option<&Message> {
    actions.iter().find_map(|a| match a {
        Action::Send(m) | Action::Resend(m, _) => Some(m),
        Action::Disconnect | Action::ResetStore => None,
    })
}

#[test]
fn initiator_sends_logon_on_connect() {
    let mut s = Session::new(cfg(Role::Initiator));
    let actions = s.handle(Event::Connected);
    assert_eq!(s.state(), SessionState::AwaitingLogon);
    assert_eq!(first_send(&actions).and_then(Message::msg_type), Some("A"));
}

#[test]
fn acceptor_waits_then_responds_to_logon() {
    let mut s = Session::new(cfg(Role::Acceptor));
    assert!(s.handle(Event::Connected).is_empty());
    assert_eq!(s.state(), SessionState::AwaitingLogon);

    let actions = s.handle(Event::Received(inbound("A", 1, Some(1), None)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(first_send(&actions).and_then(Message::msg_type), Some("A"));
}

#[test]
fn initiator_becomes_logged_on_when_logon_received() {
    let mut s = Session::new(cfg(Role::Initiator));
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(inbound("A", 1, Some(1), None)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert!(actions.is_empty()); // initiator already sent its logon
}

// --- T019 (US3, feature 005): duplicate Logon rejection (GAP-18a/FR-010) ---

#[test]
fn a_second_logon_on_an_already_logged_on_session_is_rejected() {
    let mut s = Session::new(cfg(Role::Acceptor));
    s.handle(Event::Connected);
    s.handle(Event::Received(inbound("A", 1, Some(1), None)));
    assert_eq!(s.state(), SessionState::LoggedOn);

    let actions = s.handle(Event::Received(inbound("A", 2, Some(1), None)));
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a duplicate Logon must be rejected (logout + disconnect), not silently ignored"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("5"))),
        "expected a Logout"
    );
}

#[test]
fn acceptor_logon_reports_seq_state_after_consuming_logon() {
    // 789/369 on the acceptor's Logon must reflect having consumed the inbound Logon:
    // NextExpectedMsgSeqNum=2 (next expected from peer) and LastMsgSeqNumProcessed=1.
    let mut c = cfg(Role::Acceptor);
    c.enable_next_expected_msg_seq_num = true;
    c.enable_last_msg_seq_num_processed = true;
    let mut s = Session::new(c);
    assert!(s.handle(Event::Connected).is_empty());

    let actions = s.handle(Event::Received(inbound("A", 1, Some(1), None)));
    let reply = first_send(&actions).expect("logon reply");
    assert_eq!(reply.msg_type(), Some("A"));
    assert_eq!(reply.body.get(789).unwrap().as_str().unwrap(), "2");
    assert_eq!(reply.header.get(369).unwrap().as_str().unwrap(), "1");
}

#[test]
fn test_request_is_answered_with_heartbeat() {
    let mut s = logged_on_acceptor();
    let actions = s.handle(Event::Received(inbound("1", 2, None, Some("ABC"))));
    let reply = first_send(&actions).expect("heartbeat reply");
    assert_eq!(reply.msg_type(), Some("0"));
    assert_eq!(reply.body.get(112).unwrap().as_str().unwrap(), "ABC");
}

#[test]
fn heartbeat_emitted_after_idle_tick() {
    let mut s = logged_on_acceptor();
    let actions = s.handle(Event::Tick); // hb interval = 1
    assert_eq!(first_send(&actions).and_then(Message::msg_type), Some("0"));
}

#[test]
fn start_logout_then_counter_logout_disconnects() {
    let mut s = logged_on_acceptor();
    let actions = s.handle(Event::StartLogout);
    assert_eq!(s.state(), SessionState::AwaitingLogout);
    assert_eq!(first_send(&actions).and_then(Message::msg_type), Some("5"));

    let actions = s.handle(Event::Received(inbound("5", 2, None, None)));
    assert_eq!(s.state(), SessionState::Disconnected);
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn unsolicited_logout_is_answered_and_disconnects() {
    let mut s = logged_on_acceptor();
    let actions = s.handle(Event::Received(inbound("5", 2, None, None)));
    assert_eq!(s.state(), SessionState::Disconnected);
    assert_eq!(first_send(&actions).and_then(Message::msg_type), Some("5"));
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn silent_peer_disconnects_after_threshold() {
    let mut s = logged_on_acceptor();
    // hb=1 => disconnect when ticks_since_recv >= 2*1+2 = 4
    let mut disconnected = false;
    for _ in 0..5 {
        if s.handle(Event::Tick)
            .iter()
            .any(|a| matches!(a, Action::Disconnect))
        {
            disconnected = true;
            break;
        }
    }
    assert!(disconnected);
    assert_eq!(s.state(), SessionState::Disconnected);
}

fn logged_on_acceptor() -> Session {
    let mut s = Session::new(cfg(Role::Acceptor));
    s.handle(Event::Connected);
    s.handle(Event::Received(inbound("A", 1, Some(1), None)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    s
}
