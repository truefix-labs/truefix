//! Initiator-role regression coverage for NEW-120, NEW-139, and NEW-140 (audit 006) — the
//! existing unit tests in `audit006_session_lifecycle.rs`/`audit006_sequence_reset.rs` exercise
//! these three findings only for the acceptor role; this file closes the role-coverage gap the
//! `session-lifecycle.md` contract calls for on role-sensitive protocol behavior.

use truefix_core::{Field, Message};
use truefix_dict::{ValidationOptions, load_fix44};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Initiator);
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

fn heartbeat(seq: i64) -> Message {
    msg("0", seq)
}

fn new_order(side: &str, seq: i64) -> Message {
    let mut m = msg("D", seq);
    m.body.set(Field::string(11, "ORD1"));
    m.body.set(Field::string(21, "1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::string(54, side));
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

// --- NEW-120 (initiator role): queued validation disconnect still sends Logout first ---

#[test]
fn new_120_initiator_queued_validation_disconnect_sends_logout_before_disconnect() {
    let mut c = cfg();
    c.disconnect_on_error = true;
    let mut s = Session::new(c);
    s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);

    let first = s.handle(Event::Received(new_order("Z", 3)));
    assert!(
        first
            .iter()
            .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("2"))),
        "out-of-order invalid app message should first request the missing gap"
    );

    let actions = s.handle(Event::Received(new_order("1", 2)));

    assert_eq!(s.state(), SessionState::Disconnected);
    let out = sent(&actions);
    assert!(out.iter().any(|m| m.msg_type() == Some("3")));
    assert!(
        out.iter().any(|m| m.msg_type() == Some("5")),
        "initiator queued validation failure with DisconnectOnError must send Logout before \
         disconnect, matching the acceptor-role behavior"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

// --- NEW-139 (initiator role): schedule-driven logout waits for peer Logout or timeout ---

#[test]
fn new_139_initiator_schedule_logout_waits_for_peer_or_timeout() {
    let now = time::OffsetDateTime::now_utc();
    let mut c = cfg();
    c.logout_timeout = 2;
    let start = (now - time::Duration::minutes(5)).time();
    let end = (now + time::Duration::seconds(1)).time();
    c.schedule = Some(truefix_session::Schedule::daily(start, end));
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);

    std::thread::sleep(std::time::Duration::from_millis(1500));

    let actions = s.handle(Event::Tick);
    assert_eq!(s.state(), SessionState::AwaitingLogout);
    assert!(sent(&actions).iter().any(|m| m.msg_type() == Some("5")));
    assert!(
        !actions.iter().any(|a| matches!(a, Action::Disconnect)),
        "initiator schedule exit should begin graceful logout and wait for peer Logout/timeout, \
         matching the acceptor-role behavior"
    );

    let actions = s.handle(Event::Tick);
    assert_eq!(s.state(), SessionState::AwaitingLogout);
    assert!(!actions.iter().any(|a| matches!(a, Action::Disconnect)));

    let actions = s.handle(Event::Tick);
    assert_eq!(s.state(), SessionState::Disconnected);
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

// --- NEW-140 (initiator role): gap-fill SequenceReset carries LastMsgSeqNumProcessed ---

#[test]
fn new_140_initiator_gap_fill_responses_include_last_processed_metadata_when_enabled() {
    let mut c = cfg();
    c.enable_last_msg_seq_num_processed = true;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    s.handle(Event::Received(heartbeat(2)));

    let mut resend_request = msg("2", 3);
    resend_request.body.set(Field::int(7, 1));
    resend_request.body.set(Field::int(16, 3));
    let actions = s.handle(Event::Received(resend_request));

    let gap_fill = sent(&actions)
        .into_iter()
        .find(|m| m.msg_type() == Some("4"))
        .expect("admin range should be answered with a SequenceReset-GapFill");
    assert_eq!(gap_fill.header.get(369).unwrap().as_int().unwrap(), 3);
}
