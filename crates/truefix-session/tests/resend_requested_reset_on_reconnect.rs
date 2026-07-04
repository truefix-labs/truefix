//! T047/T048 (US2, feature 007): a new connection resets `resend_requested` and the out-of-order
//! queue (BUG-35/FR-017) — previously a stale `resend_requested=true` (or leftover queued
//! messages) from a *prior* connection would silently suppress a `ResendRequest` for a genuinely
//! new gap on the new connection (since `send_redundant_resend_requests` defaults to `false`).

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

fn logon(seq: i64) -> Message {
    with(msg("A", seq), 108, "30")
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

#[test]
fn a_new_connection_resets_resend_requested_so_a_new_gap_triggers_a_fresh_resend_request() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.next_in_seq(), 2);

    // A gap on the first connection: seq 5 arrives while 2 is expected.
    let actions = s.handle(Event::Received(msg("0", 5)));
    let out = sends(&actions);
    assert!(
        out.iter().any(|m| m.msg_type() == Some("2")),
        "expected a ResendRequest for the first gap"
    );
    assert_eq!(
        s.next_in_seq(),
        2,
        "the high message is queued, not processed"
    );

    // Reconnect: a fresh TCP connection reusing the same in-memory Session (e.g. after a drop and
    // retry), without any sequence reset (ResetOnDisconnect/ResetOnLogon not configured here) --
    // the peer's new Logon continues its own sequence (its second-ever message is seq 2, matching
    // what we still expect from it).
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(2)));
    assert_eq!(
        s.state(),
        SessionState::LoggedOn,
        "the reconnect's continuing-sequence Logon should log back on cleanly"
    );

    // A new gap on the new connection: seq 5 arrives again while 2 is still expected. Before the
    // fix, `resend_requested` was still `true` from the prior connection, so this would be
    // silently suppressed (no new ResendRequest sent) since `send_redundant_resend_requests`
    // defaults to `false`.
    let actions2 = s.handle(Event::Received(msg("0", 5)));
    let out2 = sends(&actions2);
    assert!(
        out2.iter().any(|m| m.msg_type() == Some("2")),
        "a new gap on the new connection must trigger a fresh ResendRequest, not be suppressed by \
         stale `resend_requested` state carried over from the prior connection -- got {out2:?}"
    );
}
