//! T099/T100 (US3, feature 007): `ResetOnError` is honored on the too-low-`MsgSeqNum`-without-
//! `PossDupFlag` rejection path, matching the latency-failure/identity-failure/validLogonState/
//! dictionary-validation disconnect paths, which already call `reset_on_error()` (BUG-68, FR-039)
//! — previously this was the one hard sequence-violation disconnect path that skipped it.

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

fn logon(seq: i64, hbi: i64) -> Message {
    let mut m = inbound("A", seq);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, hbi));
    m
}

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn has_disconnect(actions: &[Action]) -> bool {
    actions.iter().any(|a| matches!(a, Action::Disconnect))
}

fn has_reset_store(actions: &[Action]) -> bool {
    actions.iter().any(|a| matches!(a, Action::ResetStore))
}

fn logged_on(cfg: SessionConfig) -> Session {
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1, 30)));
    s
}

#[test]
fn reset_on_error_resets_on_a_too_low_seq_without_possdup() {
    let mut cfg = acc_cfg();
    cfg.reset_on_error = true;
    let mut s = logged_on(cfg);
    // next_in_seq is now 2; a Heartbeat at seq 1 (too low), no PossDupFlag, is a hard violation.
    let actions = s.handle(Event::Received(inbound("0", 1)));
    assert!(
        has_reset_store(&actions),
        "ResetOnError should emit Action::ResetStore on the too-low-seq-without-PossDup path, \
         matching every other hard-disconnect path"
    );
    assert!(has_disconnect(&actions));
}

#[test]
fn without_reset_on_error_a_too_low_seq_without_possdup_only_disconnects() {
    let mut s = logged_on(acc_cfg());
    let actions = s.handle(Event::Received(inbound("0", 1)));
    assert!(!has_reset_store(&actions));
    assert!(has_disconnect(&actions));
}
