//! T095/T096 (US3, feature 007): a resent message's refreshed `SendingTime` uses the session's
//! configured `TimeStampPrecision`, not hardcoded milliseconds (BUG-66, FR-037) — previously
//! `prepare_resend` called `now_utc_timestamp()` (always millisecond precision), unlike every
//! other outbound admin message in the same file (`logon`, `heartbeat`, etc., which all already
//! honor `config.timestamp_precision`).

use truefix_core::{Field, Message};
use truefix_session::{Event, Role, Session, SessionConfig, TimeStampPrecision};

fn cfg(precision: TimeStampPrecision) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "ME", "YOU", Role::Initiator);
    c.heartbeat_interval = 30;
    c.reset_on_logon = true;
    c.check_latency = false;
    c.timestamp_precision = precision;
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

fn with_header(mut m: Message, tag: u32, value: &str) -> Message {
    m.header.set(Field::string(tag, value));
    m
}

/// Drive a session to resend one stored app message, returning its refreshed SendingTime(52).
fn resent_sending_time(precision: TimeStampPrecision) -> String {
    let mut s = Session::new(cfg(precision));
    s.handle(Event::Connected); // logon, seq 1 (admin)
    s.send_app(with(msg("D", 0), 55, "AAPL")); // app seq 2

    let logon = with_header(with(msg("A", 1), 108, "30"), 141, "Y");
    s.handle(Event::Received(logon));

    // Peer asks us to resend 1..2
    let rr = {
        let mut m = msg("2", 2);
        m.body.set(Field::int(7, 1));
        m.body.set(Field::int(16, 2));
        m
    };
    let actions = s.handle(Event::Received(rr));
    let resent_app = actions
        .iter()
        .find_map(|a| match a {
            truefix_session::Action::Resend(m, _) if m.msg_type() == Some("D") => Some(m),
            truefix_session::Action::Send(m) if m.msg_type() == Some("D") => Some(m),
            _ => None,
        })
        .expect("app message should be resent");
    resent_app
        .header
        .get(52)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn a_resend_at_seconds_precision_has_no_fractional_part() {
    let sending_time = resent_sending_time(TimeStampPrecision::Seconds);
    // YYYYMMDD-HH:MM:SS => 17 chars, no '.'
    assert_eq!(sending_time.len(), 17, "{sending_time}");
    assert!(!sending_time.contains('.'), "{sending_time}");
}

#[test]
fn a_resend_at_nanosecond_precision_has_nine_fractional_digits() {
    let sending_time = resent_sending_time(TimeStampPrecision::Nanoseconds);
    // YYYYMMDD-HH:MM:SS.nnnnnnnnn => 27 chars
    assert_eq!(sending_time.len(), 27, "{sending_time}");
    let frac = sending_time.split('.').nth(1).expect("fractional part");
    assert_eq!(frac.len(), 9, "{sending_time}");
}

#[test]
fn a_resend_at_millisecond_precision_has_three_fractional_digits() {
    let sending_time = resent_sending_time(TimeStampPrecision::Milliseconds);
    // YYYYMMDD-HH:MM:SS.mmm => 21 chars
    assert_eq!(sending_time.len(), 21, "{sending_time}");
    let frac = sending_time.split('.').nth(1).expect("fractional part");
    assert_eq!(frac.len(), 3, "{sending_time}");
}
