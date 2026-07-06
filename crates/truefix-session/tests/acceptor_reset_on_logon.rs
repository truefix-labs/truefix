//! T010/T012 (US1, feature 009, `NEW-03`): an acceptor's `on_connected` only checks
//! `config.reset_on_logon` in the `Role::Initiator` branch; the `Role::Acceptor` branch does
//! nothing at connect time (waits for the inbound Logon, as it must). `on_logon`'s acceptor arm
//! then only resets when the inbound Logon itself carries `ResetSeqNumFlag=Y` -- it never
//! consults `config.reset_on_logon` to proactively reset regardless of what the peer requested.
//! In QuickFIX/J, `ResetOnLogon` is primarily an *acceptor* setting: an acceptor configured with
//! it resets both sequence numbers to 1 upon receiving a Logon, independent of the inbound
//! Logon's own `ResetSeqNumFlag`.
//!
//! The realistic trigger is a session that previously advanced sequences (e.g. 50/50), then a
//! peer reconnects and sends a fresh Logon at seq=1 without `ResetSeqNumFlag=Y` -- exactly the
//! "MsgSeqNum too low" shape the early Logon-rejection guard (BUG-05/FR-001) would otherwise
//! bounce, so the fix must exempt this case from that guard too (mirroring the existing
//! `PossDup`/`ResetSeqNumFlag=Y` exemptions), not merely add a reset call further down.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn acceptor_cfg(reset_on_logon: bool) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c.reset_on_logon = reset_on_logon;
    c
}

fn logon_no_reset_flag(seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "YOU"));
    m.header.set(Field::string(56, "ME"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    // Deliberately no ResetSeqNumFlag(141) -- the whole point is that the acceptor resets anyway.
    m
}

#[test]
fn acceptor_with_reset_on_logon_resets_even_without_inbound_reset_flag() {
    let mut s = Session::new(acceptor_cfg(true));
    s.seed_sequences(50, 50);
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(logon_no_reset_flag(1)));

    assert_eq!(
        s.state(),
        SessionState::LoggedOn,
        "the Logon must be accepted (reset, not rejected as too-low), got actions: {actions:?}"
    );
    assert!(
        actions.iter().any(|a| matches!(a, Action::ResetStore)),
        "an acceptor-driven ResetOnLogon reset must signal Action::ResetStore, got: {actions:?}"
    );
    assert_eq!(
        s.next_in_seq(),
        2,
        "next_in_seq must reset to 1 then advance past the consumed inbound Logon (seq 1)"
    );
    assert_eq!(
        s.next_out_seq(),
        2,
        "next_out_seq must reset to 1 then advance past the acceptor's own Logon response"
    );
}

#[test]
fn acceptor_without_reset_on_logon_still_rejects_a_too_low_seq_logon() {
    // Control: with `reset_on_logon=false`, the existing too-low-seq rejection must still apply
    // unchanged -- proving the fix's early-guard exemption is scoped to `reset_on_logon`, not a
    // blanket loosening of the too-low check.
    let mut s = Session::new(acceptor_cfg(false));
    s.seed_sequences(50, 50);
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(logon_no_reset_flag(1)));
    assert_ne!(
        s.state(),
        SessionState::LoggedOn,
        "without ResetOnLogon, a too-low-seq Logon must still be rejected, got: {actions:?}"
    );
}
