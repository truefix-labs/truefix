//! T041 — logon/logout timeouts and CheckLatency/MaxLatency.

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Session, SessionConfig, SessionState};

#[test]
fn logon_timeout_disconnects() {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Initiator);
    c.logon_timeout = 3;
    let mut s = Session::new(c);
    s.handle(Event::Connected); // AwaitingLogon (logon sent), no response

    let mut disconnected = false;
    for _ in 0..5 {
        if s.handle(Event::Tick)
            .iter()
            .any(|a| matches!(a, Action::Disconnect))
        {
            disconnected = true;
            break;
        }
    }
    assert!(disconnected, "logon timeout should disconnect");
    assert_eq!(s.state(), SessionState::Disconnected);
}

#[test]
fn logout_timeout_disconnects() {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.logout_timeout = 2;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon("YOU", "ME", 1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
    s.handle(Event::StartLogout);
    assert_eq!(s.state(), SessionState::AwaitingLogout);

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
    assert!(disconnected, "logout timeout should disconnect");
}

#[test]
fn stale_sending_time_aborts_session() {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.check_latency = true;
    c.max_latency = 120;
    let mut s = Session::new(c);
    s.handle(Event::Connected);

    // Logon with a SendingTime decades in the past.
    let mut m = logon("YOU", "ME", 1);
    m.header.set(Field::string(52, "20000101-00:00:00"));
    let actions = s.handle(Event::Received(m));

    assert_eq!(s.state(), SessionState::Disconnected);
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

// --- T011 (US1, feature 006): unparseable SendingTime bypasses the latency check (B7/FR-009) ---

#[test]
fn unparseable_sending_time_fails_the_latency_check() {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.check_latency = true;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    let mut m = logon("YOU", "ME", 1);
    m.header.set(Field::string(52, "not-a-timestamp"));
    let actions = s.handle(Event::Received(m));
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "an unparseable SendingTime must fail the latency check, not silently pass it"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn current_sending_time_is_accepted() {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Acceptor);
    c.check_latency = true;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    // logon() uses no SendingTime by default below; add a recent one.
    let mut m = logon("YOU", "ME", 1);
    m.header.set(Field::string(52, &now_timestamp()));
    s.handle(Event::Received(m));
    assert_eq!(s.state(), SessionState::LoggedOn);
}

fn logon(sender: &str, target: &str, seq: i64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, sender));
    m.header.set(Field::string(56, target));
    m.body.set(Field::int(108, 30));
    m.body.set(Field::string(141, "Y"));
    m
}

fn now_timestamp() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}{:02}{:02}-{:02}:{:02}:{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}
