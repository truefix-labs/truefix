//! T040/T041/T044 (US1, feature 007): acceptor schedule enforcement (BUG-86/BUG-87/FR-015) and
//! `ForceResendWhenCorruptedStore`'s gap-fill resend semantics (BUG-88/FR-016).

use time::{OffsetDateTime, Weekday};

use truefix_core::{Field, Message};
use truefix_session::{Action, Event, Role, Schedule, Session, SessionConfig, SessionState};

fn cfg(role: Role) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", role);
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

fn with(mut m: Message, tag: u32, value: &str) -> Message {
    m.body.set(Field::string(tag, value));
    m
}

fn logon(seq: i64) -> Message {
    let m = with(msg("A", seq), 108, "30");
    with(m, 98, "0")
}

/// The acceptor's `on_logon` adopts the inbound Logon's own `HeartBtInt(108)`, overriding
/// whatever the session's own config started with -- needed here so the config's
/// `heartbeat_interval` (used to drive fast, deterministic ticks in these tests) actually sticks.
fn logon_with_hb(seq: i64, hb_int: &str) -> Message {
    let m = with(msg("A", seq), 108, hb_int);
    with(m, 98, "0")
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

/// A schedule that is guaranteed NOT to be in session right now, regardless of time-of-day
/// (restricted to a weekday that isn't today) -- avoids any race against wall-clock time.
fn out_of_session_schedule() -> Schedule {
    let today = OffsetDateTime::now_utc().weekday();
    let other = if today == Weekday::Monday {
        Weekday::Tuesday
    } else {
        Weekday::Monday
    };
    Schedule {
        weekdays: Some(vec![other]),
        ..Schedule::default()
    }
}

/// A schedule that is guaranteed to BE in session right now (restricted to today's weekday, no
/// time-of-day window).
fn in_session_schedule() -> Schedule {
    let today = OffsetDateTime::now_utc().weekday();
    Schedule {
        weekdays: Some(vec![today]),
        ..Schedule::default()
    }
}

// --- T040: a Logon arriving outside the configured schedule window is rejected ---

#[test]
fn logon_outside_the_configured_schedule_is_rejected() {
    let mut c = cfg(Role::Acceptor);
    c.schedule = Some(out_of_session_schedule());
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    let actions = s.handle(Event::Received(logon(1)));

    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "a Logon outside the schedule window must be rejected, not logged on"
    );
    let out = sends(&actions);
    assert!(
        out.iter().any(|m| m.msg_type() == Some("5")),
        "expected a Logout among the rejection actions, got {out:?}"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

#[test]
fn logon_inside_the_configured_schedule_is_accepted() {
    let mut c = cfg(Role::Acceptor);
    c.schedule = Some(in_session_schedule());
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(s.state(), SessionState::LoggedOn);
}

// --- T041: an already-LoggedOn session is disconnected once schedule crosses out of window ---

#[test]
fn logged_on_session_is_disconnected_once_the_schedule_window_elapses() {
    let now = OffsetDateTime::now_utc();
    let mut c = cfg(Role::Acceptor);
    c.heartbeat_interval = 30; // long enough that heartbeat-timeout doesn't also fire
                               // A daily window that ends ~1 real second from now, so a short sleep genuinely crosses it.
    let end = (now + time::Duration::seconds(1)).time();
    let start = (now - time::Duration::minutes(5)).time();
    c.schedule = Some(Schedule::daily(start, end));
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1)));
    assert_eq!(
        s.state(),
        SessionState::LoggedOn,
        "should log on inside the window"
    );

    std::thread::sleep(std::time::Duration::from_millis(1500));

    let actions = s.handle(Event::Tick);
    assert_eq!(
        s.state(),
        SessionState::Disconnected,
        "the session must be disconnected once the current time crosses outside its schedule \
         window, not left logged on indefinitely"
    );
    assert!(actions.iter().any(|a| matches!(a, Action::Disconnect)));
}

// --- T044/T045: ForceResendWhenCorruptedStore also governs admin-vs-gap-fill during resend ---

fn logged_on_acceptor_with_force_resend() -> Session {
    let mut c = cfg(Role::Acceptor);
    c.heartbeat_interval = 1;
    c.force_resend_when_corrupted_store = true;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon_with_hb(1, "1")));
    assert_eq!(s.state(), SessionState::LoggedOn);
    s
}

#[test]
fn admin_messages_are_resent_not_gap_filled_when_force_resend_when_corrupted_store_is_set() {
    let mut s = logged_on_acceptor_with_force_resend();
    // Drive two ticks: with heartbeat_interval=1 (and the default TestRequestDelayMultiplier),
    // this sends a Heartbeat (seq 2) then a TestRequest (seq 3) -- both admin-typed, which is all
    // that matters for this test.
    s.handle(Event::Tick);
    s.handle(Event::Tick);
    assert_eq!(
        s.next_out_seq(),
        4,
        "two admin messages should have been sent (seq 2, 3)"
    );

    // The peer requests a resend covering exactly those two admin messages.
    let mut rr = msg("2", 2);
    rr.body.set(Field::int(7, 2)); // BeginSeqNo
    rr.body.set(Field::int(16, 3)); // EndSeqNo
    let actions = s.handle(Event::Received(rr));
    let resent: Vec<&Message> = actions
        .iter()
        .filter_map(|a| match a {
            Action::Resend(m, _) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(
        resent.len(),
        2,
        "both admin messages must be resent, not gap-filled, when \
         ForceResendWhenCorruptedStore is set -- got actions {actions:?}"
    );
    assert!(resent
        .iter()
        .all(|m| matches!(m.msg_type(), Some("0" | "1"))));
}

#[test]
fn admin_messages_are_gap_filled_not_resent_when_force_resend_when_corrupted_store_is_unset() {
    let mut c = cfg(Role::Acceptor);
    c.heartbeat_interval = 1;
    let mut s = Session::new(c);
    s.handle(Event::Connected);
    s.handle(Event::Received(logon_with_hb(1, "1")));
    s.handle(Event::Tick);
    s.handle(Event::Tick);
    assert_eq!(s.next_out_seq(), 4);

    let mut rr = msg("2", 2);
    rr.body.set(Field::int(7, 2));
    rr.body.set(Field::int(16, 3));
    let actions = s.handle(Event::Received(rr));
    let resent_count = actions
        .iter()
        .filter(|a| matches!(a, Action::Resend(..)))
        .count();
    assert_eq!(
        resent_count, 0,
        "admin messages must be gap-filled (not individually resent) by default -- got {actions:?}"
    );
    let gap_fill = actions.iter().find_map(|a| match a {
        Action::Send(m) if m.msg_type() == Some("4") => Some(m),
        _ => None,
    });
    assert!(
        gap_fill.is_some(),
        "expected a SequenceReset-GapFill covering the admin range"
    );
}
