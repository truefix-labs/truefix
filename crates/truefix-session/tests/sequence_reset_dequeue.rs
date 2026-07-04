//! T065/T066 (US2, feature 007): a `SequenceReset` (gap-fill mode) discards now-superseded queued
//! messages rather than retaining them indefinitely (BUG-96/FR-027) — QFJ's `nextSequenceReset()`
//! calls `dequeueMessagesUpTo(newSequence)`; TrueFix's `drain_queue()` only ever removes an entry
//! matching the exact current `next_in_seq`, so a queued out-of-order message whose sequence falls
//! *below* a gap-fill's jump target (but wasn't itself drained by the contiguous-run walk) stayed
//! in the `BTreeMap` forever -- a memory leak.

use truefix_core::{Field, Message};
use truefix_session::{Event, Role, Session, SessionConfig, SessionState};

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

fn gap_fill(seq: i64, new_seq_no: i64) -> Message {
    let mut m = msg("4", seq);
    m.body.set(Field::string(123, "Y")); // GapFillFlag
    m.body.set(Field::int(36, new_seq_no)); // NewSeqNo
    m
}

#[test]
fn a_gap_fill_discards_queued_messages_it_jumps_over_not_just_ones_it_contiguously_drains() {
    let mut s = Session::new(cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    assert_eq!(s.next_in_seq(), 2);

    // A gap opens: seq 2 is expected, but 10 and 20 arrive first (both queued out-of-order,
    // neither contiguous with 2 nor with each other).
    s.handle(Event::Received(msg("0", 10)));
    s.handle(Event::Received(msg("0", 20)));

    // A gap-fill jumps straight to NewSeqNo=15 -- seq 10 (queued, now superseded) must be
    // discarded; seq 20 (still ahead of the new expectation) remains queued, not drained.
    s.handle(Event::Received(gap_fill(2, 15)));
    assert_eq!(s.next_in_seq(), 15);

    // Now the peer actually sends seq 15..19 in order, filling the remaining gap up to 20.
    for seq in 15..=19 {
        s.handle(Event::Received(msg("0", seq)));
    }
    // If seq 10 were still lingering in the queue (the BUG-96 leak), it would never be consumed by
    // anything (next_in_seq only counts up from 15), so this assertion alone can't observe it
    // directly -- but next_in_seq reaching 21 (draining the now-contiguous 15..=20 run, seq 20
    // included) confirms the queue's *live* entries drain correctly; the actual leak-freedom is
    // confirmed via the internal queue-length probe below.
    assert_eq!(
        s.next_in_seq(),
        21,
        "seq 15..=20 should all have drained once the gap to 20 closed"
    );
    assert_eq!(
        s.queued_len(),
        0,
        "no queued messages should remain -- seq 10 must have been discarded by the gap-fill \
         jump to 15, not leaked forever in the queue"
    );
}
