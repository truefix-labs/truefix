//! T075/T076 (US2, feature 009, `NEW-32`): an operational `reset()` mid-session (distinct from
//! the reconnect path already covered by `recovery.rs`'s
//! `reconnecting_clears_stale_chunked_resend_tracking...` test, which goes through
//! `on_connected()`) must also clear `resend_target`/`resend_chunk_end`/`test_request_outstanding`
//! — previously left stale, so a resend chunk from before the reset could be misapplied to the
//! freshly-reset sequence space afterward.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.resend_request_chunk_size = 3;
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

#[test]
fn operational_reset_clears_stale_chunked_resend_tracking() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    let logon = with(with(msg("A", 1), 108, "30"), 141, "Y");
    s.handle(Event::Received(logon));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.next_in_seq(), 2);

    // A big gap sets up chunked-resend tracking internally (resend_target=10, resend_chunk_end=4).
    s.handle(Event::Received(msg("0", 10)));

    // An operational reset (e.g. an administrative/manual reset, not a reconnect) must clear that
    // tracking -- this is deliberately *not* followed by a fresh `Event::Connected` (which already
    // clears the same fields via a separate, pre-existing mechanism this test must not rely on):
    // an operational reset happens mid-session, on the same live connection, per FR-L2's own doc
    // ("clear stored messages and set both sequence numbers back to 1").
    s.reset();
    assert_eq!(s.next_in_seq(), 1);

    // Feed enough consecutive in-order messages (starting fresh at seq 1, matching the reset) to
    // advance next_in_seq past where the *stale* resend_chunk_end (4) would have been -- if the
    // tracking wasn't cleared, `maybe_continue_chunked_resend` would spuriously fire a
    // continuation ResendRequest even though nothing is actually missing.
    for seq in 1..=6 {
        let actions = s.handle(Event::Received(msg("0", seq)));
        let spurious = sends(&actions)
            .into_iter()
            .find(|m| m.msg_type() == Some("2"));
        assert!(
            spurious.is_none(),
            "NEW-32: a gapless in-order message must not trigger a spurious ResendRequest from \
             stale pre-reset chunked-resend tracking, got: {spurious:?} at seq {seq}"
        );
    }
}
