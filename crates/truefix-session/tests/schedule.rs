//! T064 — session scheduling (daily window, weekdays, NonStop).

use time::macros::datetime;
use time::{Time, Weekday};
use truefix_session::Schedule;

#[test]
fn non_stop_is_always_in_session() {
    let s = Schedule::non_stop();
    assert!(s.is_in_session(datetime!(2024-01-01 03:00:00 UTC)));
    assert!(s.is_in_session(datetime!(2024-06-15 23:59:59 UTC)));
}

#[test]
fn daily_window_includes_and_excludes() {
    // 09:00:00 .. 17:00:00 UTC
    let s = Schedule::daily(
        Time::from_hms(9, 0, 0).unwrap(),
        Time::from_hms(17, 0, 0).unwrap(),
    );
    assert!(s.is_in_session(datetime!(2024-01-01 12:00:00 UTC)));
    assert!(!s.is_in_session(datetime!(2024-01-01 08:59:59 UTC)));
    assert!(!s.is_in_session(datetime!(2024-01-01 17:00:00 UTC)));
}

#[test]
fn window_wrapping_midnight() {
    // 22:00 .. 06:00 (overnight)
    let s = Schedule::daily(
        Time::from_hms(22, 0, 0).unwrap(),
        Time::from_hms(6, 0, 0).unwrap(),
    );
    assert!(s.is_in_session(datetime!(2024-01-01 23:00:00 UTC)));
    assert!(s.is_in_session(datetime!(2024-01-01 02:00:00 UTC)));
    assert!(!s.is_in_session(datetime!(2024-01-01 12:00:00 UTC)));
}

#[test]
fn utc_offset_shifts_local_time() {
    // 09:00..17:00 local, offset +5h. 05:00 UTC == 10:00 local => in session.
    let s = Schedule::daily(
        Time::from_hms(9, 0, 0).unwrap(),
        Time::from_hms(17, 0, 0).unwrap(),
    )
    .with_utc_offset_seconds(5 * 3600);
    assert!(s.is_in_session(datetime!(2024-01-01 05:00:00 UTC)));
    assert!(!s.is_in_session(datetime!(2024-01-01 03:00:00 UTC))); // 08:00 local
}

#[test]
fn weekday_filter_excludes_weekend() {
    // 2024-01-06 is a Saturday, 2024-01-08 a Monday.
    let s = Schedule::daily(
        Time::from_hms(0, 0, 0).unwrap(),
        Time::from_hms(23, 59, 59).unwrap(),
    )
    .with_weekdays(vec![
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
    ]);
    assert!(!s.is_in_session(datetime!(2024-01-06 12:00:00 UTC))); // Saturday
    assert!(s.is_in_session(datetime!(2024-01-08 12:00:00 UTC))); // Monday
}
