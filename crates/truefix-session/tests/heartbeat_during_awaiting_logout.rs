//! T101/T102 (US3, feature 007): heartbeats continue to be sent while `AwaitingLogout` and its own
//! `logout_timeout` hasn't elapsed (BUG-69, FR-040) — previously `on_tick`'s `AwaitingLogout`
//! (not-yet-timed-out) arm fell through to the catch-all and sent nothing, so a long
//! `logout_timeout` risked the peer disconnecting us for heartbeat failure well before our own
//! timeout ever fired.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

fn logon(sender: &str, target: &str, seq: i64) -> Message {
    logon_with_hbi(sender, target, seq, 30)
}

/// The acceptor adopts the inbound Logon's own `HeartBtInt` (108) as its effective
/// `heartbeat_interval` (`on_logon`), overriding whatever `SessionConfig::heartbeat_interval` was
/// configured — so a test exercising heartbeat timing must pass the value it actually wants here.
fn logon_with_hbi(sender: &str, target: &str, seq: i64, hbi: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, sender));
    m.header.set(Field::string(56, target));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, hbi));
    m
}

fn sent_heartbeat(actions: &[Action]) -> bool {
    actions
        .iter()
        .any(|a| matches!(a, Action::Send(m) if m.msg_type() == Some("0")))
}

#[test]
fn heartbeats_are_sent_while_awaiting_logout_before_the_timeout() {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.logout_timeout = 30; // long enough that the timeout itself never fires in this test
    c.check_latency = false;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon_with_hbi("YOU", "ME", 1, 2)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    s.handle(Event::StartLogout);
    assert_eq!(s.state(), SessionState::AwaitingLogout);

    // The Logout itself was just sent (ticks_since_send reset to 0); after `heartbeat_interval`
    // (2, adopted from the peer's own Logon HeartBtInt) idle ticks, a Heartbeat should be sent,
    // same cadence as LoggedOn.
    let mut sent = false;
    for _ in 0..3 {
        let actions = s.handle(Event::Tick);
        assert!(
            !actions.iter().any(|a| matches!(a, Action::Disconnect)),
            "logout_timeout=30 must not fire this soon"
        );
        if sent_heartbeat(&actions) {
            sent = true;
            break;
        }
    }
    assert!(
        sent,
        "a heartbeat must be sent while AwaitingLogout, before logout_timeout elapses"
    );
}

#[test]
fn logout_timeout_still_disconnects_when_it_elapses() {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.logout_timeout = 2;
    c.check_latency = false;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon("YOU", "ME", 1)));
    s.handle(Event::StartLogout);

    let mut disconnected = false;
    for _ in 0..4 {
        if s.handle(Event::Tick)
            .iter()
            .any(|a| matches!(a, Action::Disconnect))
        {
            disconnected = true;
            break;
        }
    }
    assert!(disconnected, "logout timeout should still disconnect");
}
