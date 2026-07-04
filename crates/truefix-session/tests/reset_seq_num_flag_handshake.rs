//! T011/T012 (US1, feature 007): the `ResetSeqNumFlag` handshake (BUG-28) — an acceptor's Logon
//! response echoes the inbound flag rather than static config (FR-005), and an initiator verifies
//! the echo (or infers a reset from `MsgSeqNum == 1`) before treating it as acknowledged (FR-006).

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn acc_cfg(reset_on_logon: bool) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.reset_on_logon = reset_on_logon;
    c
}

fn init_cfg(reset_on_logon: bool) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    c.heartbeat_interval = 30;
    c.reset_on_logon = reset_on_logon;
    c
}

fn inbound_logon(seq: i64, reset_flag: bool) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::int(34, seq));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    if reset_flag {
        m.body.set(Field::string(141, "Y"));
    }
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

fn has_disconnect(actions: &[Action]) -> bool {
    actions.iter().any(|a| matches!(a, Action::Disconnect))
}

// --- FR-005: acceptor response echoes the INBOUND flag, not static config ---

#[test]
fn acceptor_echoes_reset_flag_when_inbound_logon_carries_it_even_if_config_says_no() {
    let mut s = Session::new(acc_cfg(false)); // config: reset_on_logon = false
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(inbound_logon(1, true))); // inbound carries the flag
    let resp = sent(&actions)
        .into_iter()
        .find(|m| m.msg_type() == Some("A"))
        .expect("Logon response");
    assert_eq!(
        resp.body.get(141).and_then(|f| f.as_str().ok()),
        Some("Y"),
        "response must echo the inbound flag even though config.reset_on_logon is false"
    );
}

#[test]
fn acceptor_does_not_echo_reset_flag_when_inbound_logon_lacks_it_even_if_config_says_yes() {
    let mut s = Session::new(acc_cfg(true)); // config: reset_on_logon = true
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(inbound_logon(1, false))); // inbound has no flag
    let resp = sent(&actions)
        .into_iter()
        .find(|m| m.msg_type() == Some("A"))
        .expect("Logon response");
    assert_eq!(
        resp.body.get(141).and_then(|f| f.as_str().ok()),
        None,
        "response must not echo a flag the inbound Logon never carried, even though \
         config.reset_on_logon is true"
    );
}

// --- FR-006: initiator verifies the echo (or infers from seq==1) ---

fn response_logon(seq: i64, reset_flag: bool) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "SERVER"));
    m.header.set(Field::string(56, "CLIENT"));
    m.header.set(Field::int(34, seq));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    if reset_flag {
        m.body.set(Field::string(141, "Y"));
    }
    m
}

#[test]
fn initiator_accepts_a_response_that_echoes_the_reset_flag() {
    let mut s = Session::new(init_cfg(true));
    s.handle(Event::Connected); // sends our own Logon with ResetSeqNumFlag=Y
    let actions = s.handle(Event::Received(response_logon(1, true)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert!(!has_disconnect(&actions));
}

#[test]
fn initiator_infers_an_acknowledged_reset_from_response_seq_one_without_an_explicit_echo() {
    let mut s = Session::new(init_cfg(true));
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(response_logon(1, false))); // no flag, but seq==1
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert!(!has_disconnect(&actions));
}

#[test]
fn initiator_rejects_a_response_that_neither_echoes_nor_is_seq_one() {
    let mut s = Session::new(init_cfg(true));
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(response_logon(5, false))); // no flag, seq != 1
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "an unacknowledged reset must be treated as a handshake failure"
    );
    assert!(has_disconnect(&actions));
    assert!(
        sent(&actions).iter().any(|m| m.msg_type() == Some("5")),
        "expected a Logout before disconnect"
    );
}

#[test]
fn initiator_without_reset_on_logon_does_not_check_the_flag_at_all() {
    let mut s = Session::new(init_cfg(false)); // no reset requested
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(response_logon(5, false)));
    assert_eq!(
        s.state(),
        SessionState::LoggedOn,
        "an initiator that never requested a reset has nothing to verify"
    );
    assert!(!has_disconnect(&actions));
}
