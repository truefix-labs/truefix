//! T021/T023 (US1, feature 009, `NEW-56`): `enter_disconnected` used to check
//! `config.reset_on_disconnect || config.reset_on_logout` regardless of which of the two actually
//! applied to the current teardown -- so `reset_on_logout=true` alone incorrectly triggered a
//! reset on an ungraceful TCP drop, and `reset_on_disconnect=true` alone incorrectly triggered one
//! on a graceful Logout exchange. Fixed by threading a `DisconnectReason` through every call site.

use truefix_core::{Field, Message};
use truefix_session::{Action, DisconnectReason, Event, Role, Session, SessionConfig};

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c.reset_on_logon = false;
    c
}

fn logon(seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn logout(seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "5"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m
}

fn has_reset_store(actions: &[Action]) -> bool {
    actions.iter().any(|a| matches!(a, Action::ResetStore))
}

#[test]
fn reset_on_logout_alone_does_not_reset_on_a_raw_tcp_drop() {
    let mut cfg = acc_cfg();
    cfg.reset_on_logout = true;
    cfg.reset_on_disconnect = false;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));

    // A raw TCP drop -- not a graceful Logout exchange.
    let actions = s.handle(Event::Disconnected(DisconnectReason::Other));
    assert!(
        !has_reset_store(&actions),
        "reset_on_logout=true alone must not reset the store on an ungraceful TCP drop, got \
         {actions:?}"
    );
}

#[test]
fn reset_on_disconnect_alone_does_not_reset_on_a_graceful_logout() {
    let mut cfg = acc_cfg();
    cfg.reset_on_disconnect = true;
    cfg.reset_on_logout = false;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));

    // Peer-initiated Logout while logged on -- a graceful exchange, not a raw drop.
    let actions = s.handle(Event::Received(logout(2)));
    assert!(
        !has_reset_store(&actions),
        "reset_on_disconnect=true alone must not reset the store on a graceful Logout, got \
         {actions:?}"
    );
}

#[test]
fn reset_on_logout_alone_does_reset_on_a_graceful_logout() {
    // Control: the flag DOES apply to its own matching reason.
    let mut cfg = acc_cfg();
    cfg.reset_on_logout = true;
    cfg.reset_on_disconnect = false;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    let actions = s.handle(Event::Received(logout(2)));
    assert!(
        has_reset_store(&actions),
        "reset_on_logout=true must reset the store on its own matching reason (graceful \
         Logout), got {actions:?}"
    );
}

#[test]
fn reset_on_disconnect_alone_does_reset_on_a_raw_tcp_drop() {
    // Control: the flag DOES apply to its own matching reason.
    let mut cfg = acc_cfg();
    cfg.reset_on_disconnect = true;
    cfg.reset_on_logout = false;
    let mut s = Session::new(cfg);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    let actions = s.handle(Event::Disconnected(DisconnectReason::Other));
    assert!(
        has_reset_store(&actions),
        "reset_on_disconnect=true must reset the store on its own matching reason (raw TCP \
         drop), got {actions:?}"
    );
}
