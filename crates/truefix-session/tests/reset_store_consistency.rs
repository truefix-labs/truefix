//! US2 (FR-004) — internally-triggered full resets (logon-time `ResetSeqNumFlag`, and
//! `ResetOnDisconnect`/`ResetOnLogout`) must signal the durable store to clear itself, not just the
//! in-memory state. `Session` is sans-IO, so it signals via `Action::ResetStore` rather than holding
//! an async store handle directly; the transport (already async) performs the actual `store.reset()`.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig};

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

fn logon(seq: i64, reset: bool) -> Message {
    let mut m = inbound("A", seq);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    if reset {
        m.body.set(Field::string(141, "Y"));
    }
    m
}

fn has_reset_store(actions: &[Action]) -> bool {
    actions.iter().any(|a| matches!(a, Action::ResetStore))
}

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

#[test]
fn logon_time_reset_seq_num_flag_signals_a_store_reset() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    // First Logon received (logon_sent=false at this point) carrying ResetSeqNumFlag=Y: a full
    // reset happens, so the store must be told to clear itself too.
    let actions = s.handle(Event::Received(logon(1, true)));
    assert!(
        has_reset_store(&actions),
        "a fresh logon-time reset should emit Action::ResetStore, got {actions:?}"
    );
}

#[test]
fn ordinary_logon_without_reset_flag_does_not_reset_the_store() {
    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(logon(1, false)));
    assert!(
        !has_reset_store(&actions),
        "a logon without ResetSeqNumFlag must not clear the durable store, got {actions:?}"
    );
}

#[test]
fn reset_on_logout_signals_a_store_reset_on_logout() {
    let mut cfg = acc_cfg();
    cfg.reset_on_logout = true;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, false)));
    // Peer-initiated Logout while logged on.
    let actions = s.handle(Event::Received(inbound("5", 2)));
    assert!(
        has_reset_store(&actions),
        "ResetOnLogout=Y should emit Action::ResetStore on logout, got {actions:?}"
    );
}

#[test]
fn without_reset_on_logout_a_logout_does_not_reset_the_store() {
    let mut cfg = acc_cfg();
    cfg.reset_on_logout = false;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, false)));
    let actions = s.handle(Event::Received(inbound("5", 2)));
    assert!(
        !has_reset_store(&actions),
        "without ResetOnLogout, a logout must not clear the durable store, got {actions:?}"
    );
}
