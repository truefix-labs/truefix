//! T014 (US2) — restart-survivable resend at the session layer.
//!
//! Simulates a process restart by building a fresh `Session`, seeding the sequence numbers and the
//! previously-sent message bodies (as the transport does from the store), then driving a
//! ResendRequest. Persisted application messages replay as PossDup; with `PersistMessages=N` there
//! is nothing to replay, so the range collapses to a SequenceReset-GapFill (FR-001/002/003).

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig};

fn acc_cfg(persist: bool) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.reset_on_logon = false; // continuity across restart
    c.check_latency = false; // fixed timestamps
    c.persist_messages = persist;
    c
}

/// An outbound (server-sent) application message stored under `seq`.
fn stored_app(seq: u64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, seq as i64));
    m.header.set(Field::string(49, "SERVER"));
    m.header.set(Field::string(56, "CLIENT"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(11, &format!("ORDER-{seq}")));
    m
}

/// An inbound (client-sent) message.
fn inbound(msg_type: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

fn logon(seq: i64) -> Message {
    let mut m = inbound("A", seq);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn resend_request(seq: i64, begin: i64, end: i64) -> Message {
    let mut m = inbound("2", seq);
    m.body.set(Field::int(7, begin));
    m.body.set(Field::int(16, end));
    m
}

fn sends(actions: Vec<Action>) -> Vec<Message> {
    actions
        .into_iter()
        .filter_map(|a| match a {
            Action::Send(m) => Some(m),
            Action::Disconnect | Action::ResetStore => None,
        })
        .collect()
}

/// Bring a freshly-"restarted" acceptor back to logged-on with seeded sequences + sent messages.
fn restarted_session(persist: bool, seeded: Vec<(u64, Message)>) -> Session {
    let mut s = Session::new(acc_cfg(persist));
    // As the transport does on startup, before the handshake:
    s.seed_sequences(4, 4); // pretend seq 1..3 were used before the restart
    s.seed_sent_messages(seeded);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(4))); // peer reconnects, continues at seq 4 (no reset)
    s
}

#[test]
fn persisted_messages_replay_as_poss_dup_after_restart() {
    let seeded = vec![(2u64, stored_app(2)), (3u64, stored_app(3))];
    let mut s = restarted_session(true, seeded);

    // Peer asks for the pre-restart application messages 2..3.
    let out = sends(s.handle(Event::Received(resend_request(5, 2, 3))));
    assert_eq!(out.len(), 2, "both application messages should replay");
    for (i, seq) in [2, 3].into_iter().enumerate() {
        let m = &out[i];
        assert_eq!(m.msg_type(), Some("D"));
        assert_eq!(m.header.get(34).unwrap().as_str().unwrap(), seq.to_string());
        assert_eq!(m.header.get(43).unwrap().as_str().unwrap(), "Y"); // PossDupFlag
        assert_eq!(
            m.body.get(11).unwrap().as_str().unwrap(),
            format!("ORDER-{seq}")
        );
    }
}

#[test]
fn persist_messages_off_gap_fills_after_restart() {
    // seed_sent_messages is a no-op when persistence is disabled, so nothing is available to replay.
    let seeded = vec![(2u64, stored_app(2)), (3u64, stored_app(3))];
    let mut s = restarted_session(false, seeded);

    let out = sends(s.handle(Event::Received(resend_request(5, 2, 3))));
    assert_eq!(out.len(), 1, "the range collapses to a single GapFill");
    let m = &out[0];
    assert_eq!(m.msg_type(), Some("4")); // SequenceReset
    assert_eq!(m.body.get(123).unwrap().as_str().unwrap(), "Y"); // GapFillFlag
    assert_eq!(m.body.get(36).unwrap().as_str().unwrap(), "4"); // NewSeqNo = end+1
}
