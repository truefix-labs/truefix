//! T051/T052 (US2, feature 007): three Logon edge cases QuickFIX/J rejects but TrueFix previously
//! accepted, silently desyncing the session:
//! - `BUG-43`: `NextExpectedMsgSeqNum(789)` higher than what we've actually sent.
//! - `BUG-44`: a Logon missing `MsgSeqNum(34)` entirely.
//! - `BUG-95`: a negative `HeartBtInt(108)`.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn base_logon() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(108, 30));
    m
}

fn logged_on_session() -> Session {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    let mut logon = base_logon();
    logon.header.set(Field::int(34, 1));
    s.handle(Event::Received(logon));
    assert_eq!(s.state(), SessionState::LoggedOn);
    s
}

#[test]
fn logon_missing_msg_seq_num_is_rejected() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    let logon = base_logon(); // no tag 34 at all
    let actions = s.handle(Event::Received(logon));
    assert_ne!(
        s.state(),
        SessionState::LoggedOn,
        "a Logon missing MsgSeqNum must be rejected, not silently accepted (desyncing next_in_seq)"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn logon_with_negative_heart_bt_int_is_rejected() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    let mut logon = base_logon();
    logon.header.set(Field::int(34, 1));
    logon.body.set(Field::int(108, -5));
    let actions = s.handle(Event::Received(logon));
    assert_ne!(
        s.state(),
        SessionState::LoggedOn,
        "a Logon with a negative HeartBtInt must be rejected, not silently accepted with a \
         leftover/default heartbeat interval"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn logon_with_next_expected_higher_than_sent_is_rejected() {
    let mut s = logged_on_session();
    // We have sent exactly 1 message so far (our own Logon response, seq 1) -- next_out_seq is 2.
    assert_eq!(s.next_out_seq(), 2);

    // The peer reconnects (fresh Logon, in-order at seq 2 -- next_in_seq is already 2 from the
    // first Logon) claiming it expects seq 100 next from us -- far beyond anything we've actually
    // sent.
    s.handle(Event::Connected);
    let mut logon2 = base_logon();
    logon2.header.set(Field::int(34, 2));
    logon2.body.set(Field::int(789, 100));
    let actions = s.handle(Event::Received(logon2));
    assert_ne!(
        s.state(),
        SessionState::LoggedOn,
        "a Logon whose NextExpectedMsgSeqNum exceeds what we've actually sent must be rejected, \
         not silently ignored, leaving the session desynced"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn a_normal_valid_logon_still_succeeds() {
    let s = logged_on_session();
    assert_eq!(s.state(), SessionState::LoggedOn);
}
