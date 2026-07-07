//! Audit 006 SequenceReset coverage for NEW-99/NEW-119/NEW-140.

use truefix_core::{Field, Message};
use truefix_dict::{ValidationOptions, load_fix44};
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

fn heartbeat(seq: i64) -> Message {
    msg("0", seq)
}

fn sequence_reset(seq: i64, new_seq_no: i64, gap_fill: Option<&str>) -> Message {
    let mut m = msg("4", seq);
    m.body.set(Field::int(36, new_seq_no));
    if let Some(value) = gap_fill {
        m.body.set(Field::string(123, value));
    }
    m
}

fn logged_on(c: SessionConfig) -> Session {
    let mut s = Session::new(c.clone());
    if c.begin_string == "FIX.4.4" {
        s.set_dictionary(load_fix44().unwrap(), ValidationOptions::default());
    }
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    s
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

#[test]
fn new_99_sequence_reset_is_dictionary_validated_before_application() {
    let mut s = logged_on(cfg());
    let invalid_gap_fill = sequence_reset(2, 5, Some("Z"));

    let actions = s.handle(Event::Received(invalid_gap_fill));

    assert_eq!(
        s.next_in_seq(),
        3,
        "invalid in-order SequenceReset should consume its own seq but not apply NewSeqNo"
    );
    let out = sent(&actions);
    let reject = out
        .iter()
        .find(|m| m.msg_type() == Some("3"))
        .expect("invalid GapFillFlag should produce a session Reject");
    assert_eq!(reject.body.get(371).unwrap().as_int().unwrap(), 123);
}

#[test]
fn new_119_plain_sequence_reset_cleans_stale_queued_messages() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));

    s.handle(Event::Received(heartbeat(3)));
    assert_eq!(s.queued_len(), 1);

    s.handle(Event::Received(sequence_reset(99, 4, None)));

    assert_eq!(s.next_in_seq(), 4);
    assert_eq!(
        s.queued_len(),
        0,
        "plain reset to NewSeqNo=4 must discard queued seq 3 as stale"
    );
}

#[test]
fn new_140_gap_fill_responses_include_last_processed_metadata_when_enabled() {
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
