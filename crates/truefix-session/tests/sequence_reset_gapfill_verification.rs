//! T013/T015 (US1, feature 009, `NEW-84`): `on_received` routed `MsgType=4` (SequenceReset)
//! directly to `on_sequence_reset` before the normal `seq.cmp(&next_in_seq)` (Equal/Greater/Less)
//! dispatch every other message type receives. The gap-fill branch then applied `NewSeqNo`
//! unconditionally, with no path for a gap-fill whose *own* `MsgSeqNum` is too high to be queued
//! for later re-verification. Combined with the (correct, separately-tracked) `queue.retain(|&seq,
//! _| seq >= ns)` cleanup, a spurious or reordered gap-fill message could cause silent, permanent
//! loss of legitimately-queued messages below the jump's target.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c.reset_on_logon = false;
    c
}

fn header(mut m: Message, msg_type: &str, seq: i64) -> Message {
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

fn logon(seq: i64) -> Message {
    let mut m = header(Message::new(), "A", seq);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn app_message(seq: i64) -> Message {
    header(Message::new(), "0", seq) // Heartbeat, standing in for a generic application message
}

fn gap_fill_sequence_reset(seq: i64, new_seq_no: i64) -> Message {
    let mut m = header(Message::new(), "4", seq);
    m.body.set(Field::string(123, "Y")); // GapFillFlag
    m.body.set(Field::int(36, new_seq_no)); // NewSeqNo
    m
}

fn logged_on() -> Session {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.next_in_seq(), 2, "next_in_seq after the Logon handshake");
    s
}

#[test]
fn too_high_gap_fill_is_queued_not_applied_immediately_and_does_not_discard_queued_messages() {
    let mut s = logged_on();

    // A legitimate too-high app message opens a real gap at seq 2 (expected), arrives as seq 3.
    let actions = s.handle(Event::Received(app_message(3)));
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("2"))),
        "expected a ResendRequest for the gap, got: {actions:?}"
    );
    assert_eq!(
        s.next_in_seq(),
        2,
        "the too-high message is queued, not applied"
    );
    assert_eq!(s.queued_len(), 1, "the too-high app message must be queued");

    // A gap-fill SequenceReset arrives with its OWN MsgSeqNum (10) also too high relative to
    // next_in_seq (2), claiming NewSeqNo=5. Before the fix, this would be applied
    // unconditionally: next_in_seq jumps to 5 and `queue.retain(|&seq, _| seq >= 5)` discards the
    // legitimately-queued seq=3 entry -- permanent, silent message loss.
    let actions2 = s.handle(Event::Received(gap_fill_sequence_reset(10, 5)));
    assert_eq!(
        s.next_in_seq(),
        2,
        "a too-high gap-fill's own MsgSeqNum must not be applied immediately -- next_in_seq must \
         stay at the real expected position (NEW-84)"
    );
    assert_eq!(
        s.queued_len(),
        2,
        "the too-high gap-fill must be queued alongside the earlier queued message, not discard \
         it (NEW-84's message-loss scenario) -- got {actions2:?}"
    );
}

#[test]
fn in_order_gap_fill_still_applies_immediately() {
    // Control: a gap-fill whose own MsgSeqNum matches next_in_seq exactly (the common,
    // non-reordered case) must continue to apply its NewSeqNo immediately, unchanged from before
    // this fix.
    let mut s = logged_on();
    let actions = s.handle(Event::Received(gap_fill_sequence_reset(2, 5)));
    assert_eq!(
        s.next_in_seq(),
        5,
        "an in-order gap-fill must still apply its NewSeqNo immediately, got: {actions:?}"
    );
}
