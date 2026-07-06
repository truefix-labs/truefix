//! T007/T009 (US1, feature 009, `NEW-02`): `on_resend_request` filtered `BeginSeqNo=0` to the
//! same sentinel used for "absent", so `begin == 0 || begin > end` silently dropped a
//! `ResendRequest` whose `BeginSeqNo=0` -- a protocol-valid value meaning "from the beginning"
//! (sequence 1), per FIX spec and QuickFIX/J/Go's own handling. Only `EndSeqNo=0` ("to infinity")
//! was correctly handled; `BeginSeqNo=0` was not.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig};

fn cfg(begin_string: &str) -> SessionConfig {
    let mut c = SessionConfig::new(begin_string, "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    // This test is about resend/gap-fill behavior with pre-existing sent sequence state, not
    // about NEW-03's acceptor ResetOnLogon fix -- disable it explicitly so the seeded sequences
    // survive the Logon handshake below (SessionConfig::new's default is `true`).
    c.reset_on_logon = false;
    c
}

fn logon(begin_string: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin_string));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn resend_request(begin_string: &str, begin: i64, end: i64, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin_string));
    m.header.set(Field::string(35, "2"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(7, begin));
    m.body.set(Field::int(16, end));
    m
}

/// A session that has (ostensibly) already sent 4 messages with nothing stored (so the resend
/// resolves as a single gap-fill covering the whole requested range) -- `next_out_seq` seeded to
/// 5, which then advances to 6 once the acceptor sends its own Logon response during the
/// handshake below (consuming outbound sequence 5).
fn logged_on_having_sent(begin_string: &str) -> Session {
    let mut s = Session::new(cfg(begin_string));
    s.seed_sequences(5, 1);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(begin_string, 1)));
    s
}

#[test]
fn begin_seq_no_zero_is_treated_as_sequence_one_not_dropped() {
    let mut s = logged_on_having_sent("FIX.4.4");
    let actions = s.handle(Event::Received(resend_request("FIX.4.4", 0, 0, 2)));
    assert!(
        !actions.is_empty(),
        "BeginSeqNo=0 must be treated as 'from sequence 1' and answered, not silently dropped \
         (NEW-02)"
    );
    let gap_fill = actions
        .iter()
        .find_map(|a| match a {
            Action::Send(m) if m.msg_type() == Some("4") => Some(m),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected a SequenceReset-GapFill, got: {actions:?}"));
    let new_seq_no = gap_fill
        .body
        .get(36)
        .and_then(|f| f.as_int().ok())
        .expect("NewSeqNo(36) present");
    assert_eq!(
        new_seq_no, 6,
        "the gap-fill must cover the full range starting from sequence 1 through the current \
         next_out_seq (5, seeded, plus 1 for the acceptor's own Logon response)"
    );
}
