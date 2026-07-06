//! T103/T104 (US3, feature 007): a `ResendRequest`'s `EndSeqNo=999999` on a `FIX.4.2`-or-earlier
//! session is treated as "resend through the highest sequence sent" -- the QFJ/QFGo-documented
//! `999999`-as-infinity convention (in addition to the existing `EndSeqNo=0` convention) --
//! (BUG-97, FR-041). Previously only `end_req == 0` was recognized; `end_req > last` already made
//! `999999` behave equivalently in the overwhelmingly common case, but a long-lived session that
//! has genuinely sent 999,999+ messages would otherwise treat `999999` as a literal (now
//! less-than-`last`) endpoint instead of the honored convention -- this test constructs exactly
//! that edge case via `seed_sequences` rather than actually sending a million messages.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig};

fn cfg(begin_string: &str) -> SessionConfig {
    let mut c = SessionConfig::new(begin_string, "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    // This test is about the EndSeqNo=999999 infinity convention with pre-existing sent
    // sequence state, not NEW-03's acceptor ResetOnLogon fix -- disable it explicitly so the
    // seeded sequences survive the Logon handshake below (SessionConfig::new's default is
    // `true`).
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

/// Set up a session that has (ostensibly) already sent past 999,999 messages: `next_out_seq` seeded
/// to 1_000_005 (so `last = 1_000_004`), logged on at seq 1, and asked to resend
/// `[1_000_000, 999999]` -- `999999` is now *less than* `last`, so only the version-aware
/// infinity-convention check (not the `end_req > last` fallback) can make this resolve correctly.
fn logged_on_with_high_seq(begin_string: &str) -> Session {
    let mut s = Session::new(cfg(begin_string));
    s.seed_sequences(1_000_005, 1);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(begin_string, 1)));
    s
}

#[test]
fn end_seq_no_999999_is_infinity_on_fix42() {
    let mut s = logged_on_with_high_seq("FIX.4.2");
    let actions = s.handle(Event::Received(resend_request(
        "FIX.4.2", 1_000_000, 999_999, 2,
    )));
    assert!(
        !actions.is_empty(),
        "EndSeqNo=999999 on a FIX.4.2 session must resolve as infinity (resend through the \
         highest sequence sent), not silently no-op because 999999 < last"
    );
    // The gap [1_000_000, 1_000_004] has nothing stored, so it should manifest as a
    // SequenceReset-GapFill covering (at least up to) the seeded next_out_seq.
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("4"))),
        "expected a SequenceReset-GapFill resolving the gap, got: {actions:?}"
    );
}

#[test]
fn end_seq_no_999999_is_infinity_on_fix40_and_fix41_too() {
    for begin in ["FIX.4.0", "FIX.4.1"] {
        let mut s = logged_on_with_high_seq(begin);
        let actions = s.handle(Event::Received(resend_request(
            begin, 1_000_000, 999_999, 2,
        )));
        assert!(!actions.is_empty(), "failed for {begin}");
    }
}

#[test]
fn end_seq_no_999999_is_a_literal_endpoint_on_fix44() {
    let mut s = logged_on_with_high_seq("FIX.4.4");
    let actions = s.handle(Event::Received(resend_request(
        "FIX.4.4", 1_000_000, 999_999, 2,
    )));
    assert!(
        actions.is_empty(),
        "on FIX.4.4 (not covered by the legacy infinity convention), begin(1_000_000) > \
         end(999_999) must still silently no-op, got: {actions:?}"
    );
}
