//! T079/T080 (US2, feature 009, `NEW-34`): a non-gap-fill `SequenceReset` with `NewSeqNo=0`
//! present was silently accepted as a no-op (neither applying nor rejecting it) -- the existing
//! zero-filter (`.filter(|&n| n > 0)`) treated a present-but-zero `NewSeqNo` as if the tag were
//! absent entirely for validation purposes, but `new_seq_present` (a separate presence check) was
//! still `true`, so the "missing tag" rejection path was also skipped.

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

fn with(mut m: Message, tag: u32, value: &str) -> Message {
    m.body.set(Field::string(tag, value));
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

fn logged_on_acceptor() -> Session {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    let logon = with(msg("A", 1), 108, "30");
    let logon = with(logon, 141, "Y");
    s.handle(Event::Received(logon));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.next_in_seq(), 2);
    s
}

#[test]
fn sequence_reset_reset_mode_with_new_seq_no_zero_is_rejected() {
    let mut s = logged_on_acceptor(); // expected 2
    let sr = with(msg("4", 2), 36, "0"); // NewSeqNo=0, present but zero, no GapFillFlag
    let actions = s.handle(Event::Received(sr));
    assert_eq!(
        s.next_in_seq(),
        2,
        "NewSeqNo=0 must not be silently accepted as a no-op (NEW-34)"
    );
    let out = sends(&actions);
    let reject = out
        .iter()
        .find(|m| m.msg_type() == Some("3"))
        .expect("a session Reject rejecting NewSeqNo=0, not a silent no-op accept");
    assert_eq!(reject.body.get(371).unwrap().as_int().unwrap(), 36); // RefTagID = NewSeqNo
}
