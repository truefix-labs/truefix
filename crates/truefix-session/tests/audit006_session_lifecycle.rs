//! Shared helpers for audit 006 session lifecycle tests.
//!
//! Concrete tests in this file should include the `NEW-*` finding ID in the test name or comment.

use time::OffsetDateTime;
use truefix_core::{Field, Message};
use truefix_dict::{ValidationOptions, load_fix44};
use truefix_session::{Action, Event, Role, Schedule, Session, SessionConfig, SessionState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuditRole {
    Acceptor,
    Initiator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LifecycleCase {
    role: AuditRole,
    finding: &'static str,
    expected_action_count: usize,
}

impl LifecycleCase {
    fn acceptor(finding: &'static str, expected_action_count: usize) -> Self {
        Self {
            role: AuditRole::Acceptor,
            finding,
            expected_action_count,
        }
    }

    fn initiator(finding: &'static str, expected_action_count: usize) -> Self {
        Self {
            role: AuditRole::Initiator,
            finding,
            expected_action_count,
        }
    }
}

fn assert_lifecycle_case(case: LifecycleCase, actual_action_count: usize) {
    assert_eq!(
        actual_action_count, case.expected_action_count,
        "{} {:?}",
        case.finding, case.role
    );
}

#[test]
fn audit006_lifecycle_helper_checks_action_counts() {
    assert_lifecycle_case(LifecycleCase::acceptor("NEW-120", 2), 2);
    assert_lifecycle_case(LifecycleCase::initiator("NEW-97", 1), 1);
}

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

fn logon_without_encrypt_method(seq: i64) -> Message {
    let mut m = msg("A", seq);
    m.body.set(Field::int(108, 30));
    m
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

fn resend_request(seq: i64, begin: i64, end: i64) -> Message {
    let mut m = msg("2", seq);
    m.body.set(Field::int(7, begin));
    m.body.set(Field::int(16, end));
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

fn logged_on_with_dictionary(disconnect_on_error: bool) -> Session {
    let mut c = cfg();
    c.disconnect_on_error = disconnect_on_error;
    let mut s = Session::new(c);
    s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    s
}

#[test]
fn new_97_pre_logon_logout_is_handled_as_logout_not_logon_state_reject() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);

    let actions = s.handle(Event::Received(msg("5", 1)));

    assert_eq!(s.state(), SessionState::Disconnected);
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
    assert!(
        !sent(&actions)
            .iter()
            .filter(|m| m.msg_type() == Some("5"))
            .filter_map(|m| m.body.get(58).and_then(|f| f.as_str().ok()))
            .any(|text| text == "Logon state is not valid"),
        "pre-logon Logout must use normal Logout handling, got {actions:?}"
    );
}

#[test]
fn new_98_dictionary_invalid_logon_consumes_its_sequence_before_disconnect() {
    let mut s = Session::new(cfg());
    s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    s.handle(Event::Connected);
    let invalid = logon_without_encrypt_method(1);

    let actions = s.handle(Event::Received(invalid));

    assert_eq!(s.state(), SessionState::Disconnected);
    assert_eq!(s.next_in_seq(), 2);
    assert!(sent(&actions).iter().any(|m| m.msg_type() == Some("3")));
    assert!(sent(&actions).iter().any(|m| m.msg_type() == Some("5")));
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn new_100_teardown_logout_is_not_persisted_for_later_replay() {
    let mut c = cfg();
    c.reset_on_logon = false;
    c.force_resend_when_corrupted_store = true;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);

    let logout_actions = s.handle(Event::Received(msg("5", 2)));

    assert_eq!(s.state(), SessionState::Disconnected);
    assert!(
        sent(&logout_actions)
            .iter()
            .any(|m| m.msg_type() == Some("5"))
    );

    s.handle(Event::Connected);
    let reconnect_seq = s.next_in_seq() as i64;
    let reconnect_actions = s.handle(Event::Received(logon(reconnect_seq)));
    assert_eq!(
        s.state(),
        SessionState::LoggedOn,
        "reconnect Logon seq {reconnect_seq}, actions {reconnect_actions:?}, next_in {}",
        s.next_in_seq()
    );

    let replay_actions = s.handle(Event::Received(resend_request(4, 2, 2)));
    assert!(
        sent(&replay_actions)
            .iter()
            .all(|m| m.msg_type() != Some("5")),
        "teardown Logout must not be persisted and replayed, even when admin resend is forced"
    );
}

#[test]
fn new_120_queued_validation_disconnect_sends_logout_before_disconnect() {
    let mut s = logged_on_with_dictionary(true);

    let first = s.handle(Event::Received(new_order("Z", 3)));
    assert_eq!(s.state(), SessionState::LoggedOn);
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
        "queued validation failure with DisconnectOnError must send Logout before disconnect"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn new_139_schedule_logout_waits_for_peer_or_timeout() {
    let now = OffsetDateTime::now_utc();
    let mut c = cfg();
    c.logout_timeout = 2;
    let start = (now - time::Duration::minutes(5)).time();
    let end = (now + time::Duration::seconds(1)).time();
    c.schedule = Some(Schedule::daily(start, end));
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
        "schedule exit should begin graceful logout and wait for peer Logout/timeout"
    );

    let actions = s.handle(Event::Tick);
    assert_eq!(s.state(), SessionState::AwaitingLogout);
    assert!(!actions.iter().any(|a| matches!(a, Action::Disconnect)));

    let actions = s.handle(Event::Tick);
    assert_eq!(s.state(), SessionState::Disconnected);
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn new_142_acceptor_does_not_mark_logged_on_before_logon_sequence_gap_is_resolved() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);

    let actions = s.handle(Event::Received(logon(3)));

    assert_eq!(
        s.state(),
        SessionState::AwaitingLogon,
        "acceptor must not enter LoggedOn while the inbound Logon has an unresolved sequence gap"
    );
    let out = sent(&actions);
    assert!(
        out.iter().any(|m| m.msg_type() == Some("2")),
        "too-high Logon should request resend for the missing inbound range"
    );
    assert!(
        out.iter().all(|m| m.msg_type() != Some("A")),
        "acceptor must not send its Logon response until sequence handling completes"
    );
}
