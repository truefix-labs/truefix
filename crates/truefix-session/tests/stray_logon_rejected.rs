//! T067/T068 (US2, feature 007): a stray Logon arriving while the session is not actually
//! `AwaitingLogon` (`AwaitingLogout` or `Disconnected` — `LoggedOn` is already handled separately
//! as a duplicate-Logon rejection) is rejected rather than silently ignored (BUG-90/FR-028),
//! matching QFJ's `nextLogon()`, which disconnects on an unsolicited/out-of-sequence Logon.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn msg(msg_type: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

fn logon(seq: i64) -> Message {
    let mut m = msg("A", seq);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

#[test]
fn a_stray_logon_while_awaiting_logout_is_rejected() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);

    s.handle(Event::StartLogout);
    assert_eq!(s.state(), SessionState::AwaitingLogout);

    let actions = s.handle(Event::Received(logon(2)));
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a stray Logon while AwaitingLogout must be rejected, not silently ignored"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn a_stray_logon_while_disconnected_is_rejected() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);

    // Drive the session into Disconnected via the existing, unrelated duplicate-Logon rejection
    // (GAP-18a, feature 005/006): any second Logon while already LoggedOn disconnects.
    s.handle(Event::Received(logon(2)));
    assert_eq!(s.state(), SessionState::Disconnected);

    // A *third* Logon, arriving while already Disconnected, is the actual case under test here.
    let actions = s.handle(Event::Received(logon(3)));
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a stray Logon while already Disconnected must be rejected, not silently processed"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn a_normal_logon_while_awaiting_logon_still_succeeds() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
}
