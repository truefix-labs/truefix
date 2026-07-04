//! T016 (US1, feature 007): a `ResendRequest` for a sequence range higher than what's been sent
//! yet is answered immediately, not deferred until our own outstanding gap resolves (BUG-29) —
//! avoiding a `ResendRequest`-vs-`ResendRequest` deadlock.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c
}

fn logon(seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::int(34, seq));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn heartbeat(seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "0"));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::int(34, seq));
    m
}

fn resend_request(seq: i64, begin: i64, end: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "2"));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::int(34, seq));
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

fn logged_on() -> Session {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    s
}

#[test]
fn a_too_high_resend_request_is_answered_immediately_not_deferred() {
    let mut s = logged_on();
    // We've sent nothing yet (next_out_seq == 1, so "sent so far" is empty) -- a ResendRequest
    // arriving at seq 5 (we expect 2) asking for range 1..=1 must still be answered right now,
    // not queued until our own gap (seq 2..4) is filled.
    let actions = s.handle(Event::Received(resend_request(5, 1, 1)));
    // Expect: our own ResendRequest for the gap (seq 2..4), AND a GapFill/resend answering their
    // ResendRequest for 1..1 (even though there's nothing to actually resend at seq 1, a GapFill
    // covering it should still be produced by build_resend).
    let out = sent(&actions);
    assert!(
        out.iter().any(|m| m.msg_type() == Some("2")),
        "expected our own ResendRequest for the gap toward the counterparty, got: {out:?}"
    );
    assert!(
        out.iter().any(|m| m.msg_type() == Some("4")),
        "expected an immediate GapFill/resend response to their too-high ResendRequest, not a \
         deferred one, got: {out:?}"
    );
}

#[test]
fn the_deadlock_scenario_resolves_instead_of_hanging() {
    // Simulates both sides holding an outstanding ResendRequest toward each other: we have
    // already sent messages 2..8 (something real to resend), then develop our own gap (expecting
    // seq 2, but a message at seq 10 arrives first) -- and *that* out-of-order message is itself a
    // ResendRequest asking us to resend part of what we already sent.
    let mut s = logged_on();
    for _ in 2..=8u32 {
        let mut m = Message::new();
        m.header.set(Field::string(35, "0")); // Heartbeat
        s.send_app(m);
    }
    let actions = s.handle(Event::Received(resend_request(10, 3, 6)));
    let out = sent(&actions);
    assert!(
        out.iter().any(|m| m.msg_type() == Some("2")),
        "we must still request our own gap"
    );
    assert!(
        out.iter()
            .any(|m| m.msg_type() == Some("4") || m.msg_type() == Some("0")),
        "their ResendRequest (for 3..6, which we already sent) must be answered now with a \
         resend/gap-fill, not only after our own gap (seq 2..9) fills -- otherwise both sides \
         wait on each other forever, got: {out:?}"
    );

    // Confirm the session is still healthy afterward (no disconnect from this exchange).
    assert!(!actions.iter().any(|a| matches!(a, Action::Disconnect)));

    // Fill our own gap normally afterward -- the queued ResendRequest is still reprocessed once
    // its turn comes (a deliberate, disclosed simplification: this means a too-high ResendRequest
    // may be answered twice total -- once immediately per BUG-29's fix, once more via the normal
    // gap-fill path -- rather than exactly once; harmless (PossDup-style redundant resend) but
    // worth noting).
    for seq in 2..=9 {
        s.handle(Event::Received(heartbeat(seq)));
    }
    assert_eq!(s.next_in_seq(), 11);
}
