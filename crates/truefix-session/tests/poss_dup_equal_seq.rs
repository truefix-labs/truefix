//! T061/T062 (US2, feature 007): a `PossDupFlag=Y` message whose sequence equals (or is above) the
//! expected value still gets the `OrigSendingTime`-vs-`SendingTime` anti-replay check (BUG-57/
//! FR-024) — previously `poss_dup_anti_replay_violation` was only ever called from the
//! `Ordering::Less` (too-low) branch of `on_received`, so a falsified/replayed PossDup message
//! arriving at the expected (or a higher) sequence number was processed as if genuine, matching
//! QFJ's `validatePossDup`, which runs unconditionally for any `PossDupFlag=Y` message.

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

fn with_header(mut m: Message, tag: u32, value: &str) -> Message {
    m.header.set(Field::string(tag, value));
    m
}

fn logon(seq: i64) -> Message {
    let mut m = msg("A", seq);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn logged_on() -> Session {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.next_in_seq(), 2);
    s
}

/// A falsified PossDup message: OrigSendingTime *after* its own SendingTime -- a replay/
/// falsification signal per FIX Appendix B, regardless of what sequence number it carries.
fn falsified_poss_dup(msg_type: &str, seq: i64) -> Message {
    let mut m = msg(msg_type, seq);
    m.header.set(Field::string(43, "Y")); // PossDupFlag
    m = with_header(m, 52, "20240101-00:00:05"); // SendingTime
    m.header.set(Field::string(122, "20240101-00:00:10")); // OrigSendingTime, later than SendingTime
    m
}

#[test]
fn a_falsified_poss_dup_message_at_the_expected_sequence_is_rejected() {
    let mut s = logged_on();
    let actions = s.handle(Event::Received(falsified_poss_dup("0", 2))); // seq 2 == expected
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a falsified PossDup message at the expected sequence must be rejected, not processed as \
         if genuine"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn a_falsified_poss_dup_message_at_a_higher_than_expected_sequence_is_rejected() {
    let mut s = logged_on();
    let actions = s.handle(Event::Received(falsified_poss_dup("0", 5))); // seq 5, expected 2
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a falsified PossDup message above the expected sequence must also be rejected"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn a_genuine_poss_dup_message_at_the_expected_sequence_is_still_processed_normally() {
    let mut s = logged_on();
    let mut m = msg("0", 2);
    m.header.set(Field::string(43, "Y"));
    m = with_header(m, 52, "20240101-00:00:10");
    m.header.set(Field::string(122, "20240101-00:00:05")); // OrigSendingTime NOT later -- genuine
    let actions = s.handle(Event::Received(m));
    assert_eq!(
        s.state(),
        SessionState::LoggedOn,
        "a genuine (non-falsified) PossDup message at the expected sequence must still be \
         processed normally, not rejected"
    );
    assert!(!actions.iter().any(|a| matches!(a, Action::Disconnect)));
    assert_eq!(s.next_in_seq(), 3);
}
