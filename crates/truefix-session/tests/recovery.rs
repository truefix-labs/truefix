//! T032–T035 — sequence recovery, resend, SequenceReset, and NextExpectedMsgSeqNum.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg(role: Role) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", role);
    c.heartbeat_interval = 30;
    c.reset_on_logon = true;
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

fn with(mut m: Message, tag: u32, value: &str) -> Message {
    m.body.set(Field::string(tag, value));
    m
}

fn with_header(mut m: Message, tag: u32, value: &str) -> Message {
    m.header.set(Field::string(tag, value));
    m
}

fn sends(actions: &[Action]) -> Vec<&Message> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Send(m) => Some(m),
            Action::Disconnect => None,
        })
        .collect()
}

fn logged_on_acceptor() -> Session {
    let mut s = Session::new(cfg(Role::Acceptor));
    s.handle(Event::Connected);
    // Logon with ResetSeqNumFlag=Y, seq 1
    let logon = with(msg("A", 1), 108, "30");
    let logon = with(logon, 141, "Y");
    s.handle(Event::Received(logon));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.next_in_seq(), 2);
    s
}

#[test]
fn high_seq_triggers_resend_request_and_queues() {
    let mut s = logged_on_acceptor();
    // expected = 2; receive seq 5 -> gap
    let actions = s.handle(Event::Received(msg("0", 5)));
    let out = sends(&actions);
    assert_eq!(out.len(), 1);
    let rr = out[0];
    assert_eq!(rr.msg_type(), Some("2")); // ResendRequest
    assert_eq!(rr.body.get(7).unwrap().as_int().unwrap(), 2); // BeginSeqNo = expected
                                                              // still expecting 2 (the high message is queued, not processed)
    assert_eq!(s.next_in_seq(), 2);
}

#[test]
fn gap_fills_then_queued_message_processed() {
    let mut s = logged_on_acceptor();
    s.handle(Event::Received(msg("0", 5))); // queue 5, request resend from 2
                                            // peer gap-fills 2..5 via SequenceReset-GapFill NewSeqNo=5
    let sr = with(with(msg("4", 2), 123, "Y"), 36, "5");
    s.handle(Event::Received(sr));
    // now expected should have advanced through the queued 5 -> 6
    assert_eq!(s.next_in_seq(), 6);
}

#[test]
fn low_seq_without_possdup_disconnects() {
    let mut s = logged_on_acceptor(); // expected 2
    let actions = s.handle(Event::Received(msg("0", 1))); // too low, no PossDup
    assert_eq!(s.state(), SessionState::Disconnected);
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn low_seq_with_possdup_is_ignored() {
    let mut s = logged_on_acceptor(); // expected 2
    let dup = with_header(msg("0", 1), 43, "Y"); // PossDupFlag=Y (header field)
    let actions = s.handle(Event::Received(dup));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert!(actions.is_empty());
    assert_eq!(s.next_in_seq(), 2);
}

#[test]
fn resend_request_resends_app_with_possdup_and_gapfills_admin() {
    // Build an initiator that has sent: logon(1, admin), an app msg(2), a heartbeat(3, admin).
    let mut s = Session::new(cfg(Role::Initiator));
    s.handle(Event::Connected); // logon seq 1 (admin)
    s.send_app(with(msg("D", 0), 55, "AAPL")); // app seq 2 (seq stamped by engine)
                                               // log on so test requests/heartbeats are valid; simulate counter logon
    let logon = with(with(msg("A", 1), 108, "30"), 141, "Y");
    s.handle(Event::Received(logon));

    // Peer asks us to resend 1..3
    let rr = {
        let mut m = msg("2", 2);
        m.body.set(Field::int(7, 1));
        m.body.set(Field::int(16, 3));
        m
    };
    let actions = s.handle(Event::Received(rr));
    let out = sends(&actions);
    // Expect: gap-fill for the admin logon(1), then resent app(2) with PossDup, then gap-fill for 3+
    assert!(
        out.iter().any(|m| m.msg_type() == Some("4")),
        "expected a SequenceReset-GapFill for admin messages"
    );
    let resent_app = out
        .iter()
        .find(|m| m.msg_type() == Some("D"))
        .expect("app message should be resent");
    assert_eq!(resent_app.header.get(43).unwrap().as_str().unwrap(), "Y"); // PossDupFlag
    assert!(resent_app.header.get(122).is_some(), "OrigSendingTime set"); // OrigSendingTime
}

#[test]
fn chunked_resend_request_caps_end() {
    let mut c = cfg(Role::Acceptor);
    c.resend_request_chunk_size = 3;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    let logon = with(with(msg("A", 1), 108, "30"), 141, "Y");
    s.handle(Event::Received(logon)); // expected 2

    let actions = s.handle(Event::Received(msg("0", 10))); // big gap from 2
    let out = sends(&actions);
    let rr = out.iter().find(|m| m.msg_type() == Some("2")).unwrap();
    assert_eq!(rr.body.get(7).unwrap().as_int().unwrap(), 2); // begin
    assert_eq!(rr.body.get(16).unwrap().as_int().unwrap(), 4); // end = 2 + 3 - 1
}

#[test]
fn sequence_reset_reset_mode_sets_expected() {
    let mut s = logged_on_acceptor(); // expected 2
                                      // Reset mode (no GapFill): NewSeqNo=10 authoritative
    let sr = with(msg("4", 99), 36, "10");
    s.handle(Event::Received(sr));
    assert_eq!(s.next_in_seq(), 10);
}

#[test]
fn seed_sequences_resumes_outbound_seq() {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Initiator);
    c.reset_on_logon = false; // otherwise logon resets to 1
    let mut s = Session::new(c);
    s.seed_sequences(5, 7);
    let actions = s.handle(Event::Connected);
    let logon = sends(&actions)[0];
    assert_eq!(logon.header.get(34).unwrap().as_int().unwrap(), 5); // resumes from seeded value
    assert_eq!(s.next_in_seq(), 7);
}

#[test]
fn next_expected_msg_seq_num_included_on_logon_when_enabled() {
    let mut c = cfg(Role::Initiator);
    c.enable_next_expected_msg_seq_num = true;
    let mut s = Session::new(c);
    let actions = s.handle(Event::Connected);
    let out = sends(&actions);
    let logon = out.iter().find(|m| m.msg_type() == Some("A")).unwrap();
    assert!(
        logon.body.get(789).is_some(),
        "NextExpectedMsgSeqNum (789) should be present on logon"
    );
    assert_eq!(logon.body.get(789).unwrap().as_int().unwrap(), 1);
}
